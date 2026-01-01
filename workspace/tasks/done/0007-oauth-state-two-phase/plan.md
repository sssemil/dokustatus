# Plan v3: Two-Phase Google OAuth State Usage

## Summary

The current OAuth flow consumes the state token **before** exchanging the code with Google, creating a vulnerability where network/API failures leave users unable to retry. If `exchange_google_code` fails (network timeout, Google outage, etc.), the state is already deleted and the user must restart the entire OAuth flow from scratch.

This plan introduces a **two-phase state lifecycle**:
1. **Phase 1 (Mark)**: Mark state as "in-use" when exchange begins
2. **Phase 2 (Complete)**: Delete state only after successful exchange

Failed exchanges allow retry within a short window (90 seconds) on the same state.

## Changes from Plan v2

Addressing feedback from review:

| Feedback Item | Resolution |
|---------------|------------|
| Error classification still uses string matching | Replace with typed `OAuthExchangeError` enum that captures error source and HTTP status |
| Retry window vs Redis TTL not specified | Refresh TTL on `mark_state_in_use` to always be >= retry window; document explicitly |
| `completed` status checked in Lua but never set | Remove `completed` status check; states are deleted, not marked completed |
| What to do when RetryWindowExpired | Call `abort_state` to delete stale state, then return 410 |
| Behavior when `complete_state` fails | Treat as best-effort: log warning and return success to user |
| How to detect retryable Google errors without string matching | Wrap Google exchange in typed error with HTTP status; classify by status code |

## Detailed Implementation

### Step 1: Extend OAuthStateData Structure

**File**: `apps/api/src/application/use_cases/domain_auth.rs`

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthStateData {
    pub domain: String,
    pub code_verifier: String,
    /// Status: "pending" (initial), "in_use" (being exchanged)
    #[serde(default = "default_pending")]
    pub status: String,
    /// Unix timestamp when state was marked in-use (for retry window)
    #[serde(default)]
    pub marked_at: Option<i64>,
}

fn default_pending() -> String {
    "pending".to_string()
}
```

**Backward Compatibility**: The `#[serde(default)]` attributes ensure old states without `status`/`marked_at` deserialize cleanly as `status="pending"`, `marked_at=None`. The existing `consume_state` method continues to work unchanged since it just reads and deletes.

### Step 2: Add Trait Methods with Structured Error Returns

**File**: `apps/api/src/application/use_cases/domain_auth.rs`

```rust
/// Result of attempting to mark state in-use
#[derive(Debug, Clone)]
pub enum MarkStateResult {
    /// State marked successfully, here's the data
    Success(OAuthStateData),
    /// State not found (doesn't exist or was deleted)
    NotFound,
    /// State is in-use and retry window has expired
    RetryWindowExpired,
}

#[async_trait]
pub trait OAuthStateStore: Send + Sync {
    // Existing methods (unchanged)
    async fn store_state(&self, state: &str, data: &OAuthStateData, ttl_minutes: i64) -> AppResult<()>;
    async fn consume_state(&self, state: &str) -> AppResult<Option<OAuthStateData>>;

    // New two-phase methods:

    /// Mark state as "in_use". Refreshes TTL to ensure retry window is available.
    /// Returns structured result instead of Option/Error.
    async fn mark_state_in_use(&self, state: &str, retry_window_secs: i64) -> AppResult<MarkStateResult>;

    /// Delete state unconditionally after successful completion.
    /// This is called only after user creation succeeds.
    async fn complete_state(&self, state: &str) -> AppResult<()>;

    /// Abort state for terminal errors (unconditional delete).
    /// Called when error is non-retryable (invalid_grant, validation failure).
    async fn abort_state(&self, state: &str) -> AppResult<()>;
}
```

### Step 3: Redis Implementation with TTL Refresh

**File**: `apps/api/src/infra/oauth_state.rs`

The Lua script now:
1. Returns integer status codes for unambiguous error handling
2. Refreshes TTL to `retry_window + 30s` buffer on mark to guarantee retries work

```rust
async fn mark_state_in_use(&self, state: &str, retry_window_secs: i64) -> AppResult<MarkStateResult> {
    let mut conn = self.manager.clone();
    let key = Self::state_key(state);

    // TTL = retry_window + 30 seconds buffer for network delays
    let min_ttl = retry_window_secs + 30;

    // Lua script returns: [status_code, json_data_or_nil]
    // Status codes:
    //   0 = success (data is valid JSON)
    //   1 = not_found (state doesn't exist)
    //   2 = retry_window_expired
    let script = redis::Script::new(r#"
        local value = redis.call('GET', KEYS[1])
        if not value then
            return {1, nil}  -- not_found
        end

        local data = cjson.decode(value)
        local retry_window = tonumber(ARGV[1])
        local min_ttl = tonumber(ARGV[2])

        -- Get time from Redis to avoid clock skew
        local time_result = redis.call('TIME')
        local now = tonumber(time_result[1])

        if data.status == 'in_use' then
            local marked_at = data.marked_at or 0
            if (now - marked_at) > retry_window then
                return {2, nil}  -- retry_window_expired
            end
            -- Within retry window, allow retry (don't update marked_at)
            -- Refresh TTL to ensure retry window is available
            redis.call('EXPIRE', KEYS[1], min_ttl)
            return {0, value}  -- success
        end

        -- status == 'pending' or nil (backward compat), mark as in_use
        data.status = 'in_use'
        data.marked_at = now
        local new_value = cjson.encode(data)
        -- Set with refreshed TTL to guarantee retry window
        redis.call('SET', KEYS[1], new_value, 'EX', min_ttl)
        return {0, new_value}  -- success
    "#);

    let result: (i32, Option<String>) = script
        .key(&key)
        .arg(retry_window_secs)
        .arg(min_ttl)
        .invoke_async(&mut conn)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to mark OAuth state in-use: {e}")))?;

    match result.0 {
        0 => {
            let json = result.1.ok_or_else(|| AppError::Internal("Lua returned success but no data".into()))?;
            let data: OAuthStateData = serde_json::from_str(&json)
                .map_err(|e| AppError::Internal(format!("Failed to parse OAuth state: {e}")))?;
            Ok(MarkStateResult::Success(data))
        }
        1 => Ok(MarkStateResult::NotFound),
        2 => Ok(MarkStateResult::RetryWindowExpired),
        _ => Err(AppError::Internal(format!("Unknown status code from Lua: {}", result.0))),
    }
}

/// Delete state unconditionally. Called after successful user creation.
async fn complete_state(&self, state: &str) -> AppResult<()> {
    let mut conn = self.manager.clone();
    let key = Self::state_key(state);
    let _: () = conn.del(&key).await
        .map_err(|e| AppError::Internal(format!("Failed to complete OAuth state: {e}")))?;
    Ok(())
}

/// Alias for complete_state - both do unconditional delete.
/// Kept separate for semantic clarity in call sites.
async fn abort_state(&self, state: &str) -> AppResult<()> {
    self.complete_state(state).await
}
```

**Key Changes from v2**:
- TTL is refreshed on `mark_state_in_use` to `retry_window + 30s`, guaranteeing retries work regardless of original TTL
- Removed `completed` status check since we delete rather than mark completed

### Step 4: Typed Error Classification (No String Matching)

**File**: `apps/api/src/adapters/http/routes/public_domain_auth.rs`

Replace string-matching with typed errors:

```rust
/// Typed OAuth exchange error with source information
#[derive(Debug)]
pub enum OAuthExchangeError {
    /// Network error during Google API call
    Network { message: String },
    /// Google API returned an error response
    GoogleApi {
        status: u16,
        error_code: Option<String>,  // e.g., "invalid_grant"
        message: String,
    },
    /// Token parsing or validation failed
    TokenValidation { message: String },
    /// User data validation failed (e.g., email not verified)
    UserValidation { message: String },
    /// Database error during user creation
    Database { message: String },
    /// Redis error during state management
    Redis { message: String },
}

impl OAuthExchangeError {
    /// Classify error as retryable or terminal based on error type and codes
    pub fn is_retryable(&self) -> bool {
        match self {
            // Network errors are always retryable
            OAuthExchangeError::Network { .. } => true,

            // Google API errors: retryable if 5xx, terminal if 4xx
            OAuthExchangeError::GoogleApi { status, error_code, .. } => {
                // 5xx = Google's problem, retry
                if *status >= 500 {
                    return true;
                }
                // 4xx = our problem (bad code, expired, etc), terminal
                // Specifically, invalid_grant means code already used/expired
                false
            }

            // Token/user validation errors are terminal (data is bad)
            OAuthExchangeError::TokenValidation { .. } => false,
            OAuthExchangeError::UserValidation { .. } => false,

            // Infrastructure errors are retryable
            OAuthExchangeError::Database { .. } => true,
            OAuthExchangeError::Redis { .. } => true,
        }
    }
}

/// Wrap the existing exchange function to return typed errors
async fn exchange_google_code_typed(
    code: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<GoogleTokenResponse, OAuthExchangeError> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .map_err(|e| OAuthExchangeError::Network {
            message: e.to_string(),
        })?;

    let status = response.status().as_u16();

    if !response.status().is_success() {
        // Try to parse Google's error response
        let error_body = response.text().await.unwrap_or_default();
        let error_code = serde_json::from_str::<serde_json::Value>(&error_body)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from));

        return Err(OAuthExchangeError::GoogleApi {
            status,
            error_code,
            message: error_body,
        });
    }

    response
        .json::<GoogleTokenResponse>()
        .await
        .map_err(|e| OAuthExchangeError::TokenValidation {
            message: format!("Failed to parse token response: {e}"),
        })
}
```

### Step 5: Updated google_exchange Handler

**File**: `apps/api/src/adapters/http/routes/public_domain_auth.rs`

```rust
async fn google_exchange(
    State(app_state): State<AppState>,
    Path(_hostname): Path<String>,
    Json(payload): Json<GoogleExchangePayload>,
) -> AppResult<impl IntoResponse> {
    const RETRY_WINDOW_SECS: i64 = 90;

    // Phase 1: Mark state as in-use
    let state_data = match app_state
        .domain_auth_use_cases
        .mark_google_oauth_state_in_use(&payload.state, RETRY_WINDOW_SECS)
        .await?
    {
        MarkStateResult::Success(data) => data,
        MarkStateResult::NotFound => {
            return Err(AppError::InvalidInput("Invalid or expired OAuth state".into()));
        }
        MarkStateResult::RetryWindowExpired => {
            // Delete stale in-use state before returning 410
            let _ = app_state
                .domain_auth_use_cases
                .abort_google_oauth_state(&payload.state)
                .await;
            return Err(AppError::OAuthRetryExpired);
        }
    };

    // ... existing domain lookup, credential fetch ...

    // Exchange code with Google (typed errors)
    let token_response = match exchange_google_code_typed(
        &payload.code,
        &client_id,
        &client_secret,
        &redirect_uri,
        &state_data.code_verifier,
    ).await {
        Ok(response) => response,
        Err(e) => {
            handle_oauth_exchange_error(&app_state, &payload.state, &e).await;
            return Err(e.into());
        }
    };

    // Parse id_token
    let (google_id, email, email_verified) = match parse_google_id_token(
        &token_response.id_token,
        &client_id
    ).await {
        Ok(result) => result,
        Err(e) => {
            // Token parse failure is always terminal
            let _ = app_state.domain_auth_use_cases
                .abort_google_oauth_state(&payload.state)
                .await;
            return Err(e);
        }
    };

    // Validate email
    if !email_verified {
        let _ = app_state.domain_auth_use_cases
            .abort_google_oauth_state(&payload.state)
            .await;
        return Err(AppError::InvalidInput("Email not verified with Google".into()));
    }

    // Create/find user - wrap in typed error for classification
    let result = match app_state.domain_auth_use_cases
        .find_or_create_google_user(&domain, &google_id, &email)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            // Database errors are retryable
            let exchange_err = OAuthExchangeError::Database {
                message: e.to_string(),
            };
            handle_oauth_exchange_error(&app_state, &payload.state, &exchange_err).await;
            return Err(e);
        }
    };

    // Phase 2: Complete state (delete) after full success
    // Best-effort: log and continue if delete fails (user is already created)
    if let Err(e) = app_state
        .domain_auth_use_cases
        .complete_google_oauth_state(&payload.state)
        .await
    {
        tracing::warn!(
            state = %payload.state,
            error = %e,
            "Failed to delete OAuth state after successful login (best-effort)"
        );
        // Continue - user login succeeded, state will expire via TTL
    }

    // Return success response
    // ... existing response logic ...
}

/// Handle OAuth exchange error by either leaving state for retry or aborting.
async fn handle_oauth_exchange_error(
    app_state: &AppState,
    state: &str,
    error: &OAuthExchangeError,
) {
    if error.is_retryable() {
        tracing::warn!(
            state = %state,
            error = ?error,
            "OAuth exchange failed (retryable), state preserved for retry"
        );
        // State stays in "in_use", user can retry within window
    } else {
        tracing::warn!(
            state = %state,
            error = ?error,
            "OAuth exchange failed (terminal), aborting state"
        );
        let _ = app_state.domain_auth_use_cases
            .abort_google_oauth_state(state)
            .await;
    }
}

impl From<OAuthExchangeError> for AppError {
    fn from(e: OAuthExchangeError) -> Self {
        match e {
            OAuthExchangeError::Network { message } => {
                AppError::Internal(format!("Network error during OAuth: {message}"))
            }
            OAuthExchangeError::GoogleApi { status, error_code, message } => {
                if let Some(code) = error_code {
                    if code == "invalid_grant" {
                        return AppError::InvalidInput("Authorization code expired or already used".into());
                    }
                }
                AppError::Internal(format!("Google API error ({}): {}", status, message))
            }
            OAuthExchangeError::TokenValidation { message } => {
                AppError::Internal(format!("Token validation failed: {message}"))
            }
            OAuthExchangeError::UserValidation { message } => {
                AppError::InvalidInput(message)
            }
            OAuthExchangeError::Database { message } => {
                AppError::Internal(format!("Database error: {message}"))
            }
            OAuthExchangeError::Redis { message } => {
                AppError::Internal(format!("Redis error: {message}"))
            }
        }
    }
}
```

### Step 6: Error Type and HTTP Response

**File**: `apps/api/src/app_error.rs`

```rust
pub enum AppError {
    // ... existing variants ...

    /// OAuth retry window has expired. User must restart the OAuth flow.
    OAuthRetryExpired,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, body) = match &self {
            // ... existing matches ...

            AppError::OAuthRetryExpired => (
                StatusCode::GONE,  // 410 Gone
                json!({
                    "error": "OAUTH_RETRY_EXPIRED",
                    "message": "OAuth session expired. Please restart the login process.",
                    "action": "restart_oauth"
                }),
            ),
        };
        // ...
    }
}
```

### Step 7: UI/SDK Handling for 410 Response

**Frontend** (`apps/ui/`): Update the OAuth callback handler:

```typescript
// In the OAuth callback page or hook
async function handleOAuthCallback(code: string, state: string) {
  try {
    const result = await exchangeGoogleCode({ code, state });
    // ... handle success
  } catch (error) {
    if (error.response?.status === 410 || error.code === 'OAUTH_RETRY_EXPIRED') {
      // Clear any cached OAuth state
      sessionStorage.removeItem('oauth_state');

      // Show user-friendly message and restart button
      showOAuthExpiredModal({
        message: "Your login session expired. Please try again.",
        action: () => initiateGoogleOAuth()  // Restart the flow
      });
      return;
    }
    // ... handle other errors
  }
}
```

**SDK** (`libs/reauth-sdk-ts/`): Add error type documentation:

```typescript
/**
 * Error codes returned by the API
 */
export enum ReAuthErrorCode {
  // ... existing codes ...

  /**
   * OAuth retry window expired. The OAuth flow must be restarted.
   * This occurs when a user tries to retry after the retry window (90s) has passed.
   */
  OAUTH_RETRY_EXPIRED = 'OAUTH_RETRY_EXPIRED',
}

/**
 * Check if an error requires restarting the OAuth flow
 */
export function requiresOAuthRestart(error: ReAuthError): boolean {
  return error.code === ReAuthErrorCode.OAUTH_RETRY_EXPIRED;
}
```

### Step 8: Testing Strategy

#### 8.1 In-Memory Store with Controllable Clock

```rust
#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::collections::HashMap;

    /// Controllable clock for testing
    #[derive(Clone)]
    struct TestClock {
        now: Arc<Mutex<i64>>,
    }

    impl TestClock {
        fn new(initial: i64) -> Self {
            Self { now: Arc::new(Mutex::new(initial)) }
        }

        fn now(&self) -> i64 {
            *self.now.lock().unwrap()
        }

        fn advance(&self, seconds: i64) {
            let mut now = self.now.lock().unwrap();
            *now += seconds;
        }
    }

    struct InMemoryOAuthStateStore {
        states: Mutex<HashMap<String, (OAuthStateData, i64)>>,  // (data, expires_at)
        clock: TestClock,
    }

    #[async_trait]
    impl OAuthStateStore for InMemoryOAuthStateStore {
        async fn mark_state_in_use(&self, state: &str, retry_window_secs: i64) -> AppResult<MarkStateResult> {
            let mut states = self.states.lock().unwrap();
            let now = self.clock.now();

            let Some((data, expires_at)) = states.get_mut(state) else {
                return Ok(MarkStateResult::NotFound);
            };

            // Check if expired
            if now > *expires_at {
                states.remove(state);
                return Ok(MarkStateResult::NotFound);
            }

            if data.status == "in_use" {
                let marked_at = data.marked_at.unwrap_or(0);
                if (now - marked_at) > retry_window_secs {
                    return Ok(MarkStateResult::RetryWindowExpired);
                }
                // Refresh TTL
                *expires_at = now + retry_window_secs + 30;
                return Ok(MarkStateResult::Success(data.clone()));
            }

            data.status = "in_use".to_string();
            data.marked_at = Some(now);
            // Refresh TTL
            *expires_at = now + retry_window_secs + 30;
            Ok(MarkStateResult::Success(data.clone()))
        }

        async fn complete_state(&self, state: &str) -> AppResult<()> {
            self.states.lock().unwrap().remove(state);
            Ok(())
        }

        async fn abort_state(&self, state: &str) -> AppResult<()> {
            self.states.lock().unwrap().remove(state);
            Ok(())
        }
    }
}
```

#### 8.2 Test Cases

```rust
#[tokio::test]
async fn test_two_phase_happy_path() {
    let clock = TestClock::new(1000);
    let store = InMemoryOAuthStateStore::new(clock.clone());

    // Store state
    store.store_state("abc", &OAuthStateData::new("example.com", "verifier"), 10).await.unwrap();

    // Mark in-use
    let result = store.mark_state_in_use("abc", 90).await.unwrap();
    assert!(matches!(result, MarkStateResult::Success(_)));

    // Complete
    store.complete_state("abc").await.unwrap();

    // Should be gone
    let result = store.mark_state_in_use("abc", 90).await.unwrap();
    assert!(matches!(result, MarkStateResult::NotFound));
}

#[tokio::test]
async fn test_retry_within_window() {
    let clock = TestClock::new(1000);
    let store = InMemoryOAuthStateStore::new(clock.clone());

    store.store_state("abc", &OAuthStateData::new("example.com", "verifier"), 10).await.unwrap();

    // First mark
    store.mark_state_in_use("abc", 90).await.unwrap();

    // Advance 30 seconds (within window)
    clock.advance(30);

    // Retry should succeed
    let result = store.mark_state_in_use("abc", 90).await.unwrap();
    assert!(matches!(result, MarkStateResult::Success(_)));
}

#[tokio::test]
async fn test_retry_after_window_expires() {
    let clock = TestClock::new(1000);
    let store = InMemoryOAuthStateStore::new(clock.clone());

    store.store_state("abc", &OAuthStateData::new("example.com", "verifier"), 10).await.unwrap();

    // First mark
    store.mark_state_in_use("abc", 90).await.unwrap();

    // Advance 100 seconds (past window)
    clock.advance(100);

    // Retry should fail with RetryWindowExpired
    let result = store.mark_state_in_use("abc", 90).await.unwrap();
    assert!(matches!(result, MarkStateResult::RetryWindowExpired));
}

#[tokio::test]
async fn test_retry_expired_returns_410() {
    // Integration test: verify handler returns 410 on RetryWindowExpired
    // and deletes the stale state
    let clock = TestClock::new(1000);
    let store = InMemoryOAuthStateStore::new(clock.clone());

    store.store_state("abc", &OAuthStateData::new("example.com", "verifier"), 10).await.unwrap();
    store.mark_state_in_use("abc", 90).await.unwrap();

    clock.advance(100);

    // Simulate handler behavior
    let result = store.mark_state_in_use("abc", 90).await.unwrap();
    assert!(matches!(result, MarkStateResult::RetryWindowExpired));

    // Handler should abort state
    store.abort_state("abc").await.unwrap();

    // State should be gone
    let result = store.mark_state_in_use("abc", 90).await.unwrap();
    assert!(matches!(result, MarkStateResult::NotFound));
}

#[tokio::test]
async fn test_abort_removes_state() {
    let clock = TestClock::new(1000);
    let store = InMemoryOAuthStateStore::new(clock.clone());

    store.store_state("abc", &OAuthStateData::new("example.com", "verifier"), 10).await.unwrap();
    store.mark_state_in_use("abc", 90).await.unwrap();

    // Abort (terminal error)
    store.abort_state("abc").await.unwrap();

    // Should be gone
    let result = store.mark_state_in_use("abc", 90).await.unwrap();
    assert!(matches!(result, MarkStateResult::NotFound));
}

#[tokio::test]
async fn test_backward_compat_old_state() {
    // Simulate old state without status/marked_at fields
    let json = r#"{"domain":"example.com","code_verifier":"verifier"}"#;
    let data: OAuthStateData = serde_json::from_str(json).unwrap();

    assert_eq!(data.status, "pending");
    assert_eq!(data.marked_at, None);
}

#[tokio::test]
async fn test_error_classification() {
    // Network errors are retryable
    let err = OAuthExchangeError::Network { message: "timeout".into() };
    assert!(err.is_retryable());

    // Google 5xx is retryable
    let err = OAuthExchangeError::GoogleApi {
        status: 503,
        error_code: None,
        message: "Service unavailable".into(),
    };
    assert!(err.is_retryable());

    // Google 4xx is terminal
    let err = OAuthExchangeError::GoogleApi {
        status: 400,
        error_code: Some("invalid_grant".into()),
        message: "Code already used".into(),
    };
    assert!(!err.is_retryable());

    // Token validation is terminal
    let err = OAuthExchangeError::TokenValidation { message: "bad token".into() };
    assert!(!err.is_retryable());

    // Database errors are retryable
    let err = OAuthExchangeError::Database { message: "connection reset".into() };
    assert!(err.is_retryable());
}

#[tokio::test]
async fn test_ttl_refresh_on_mark() {
    let clock = TestClock::new(1000);
    let store = InMemoryOAuthStateStore::new(clock.clone());

    // Store with short TTL (would expire before retry window)
    store.store_state("abc", &OAuthStateData::new("example.com", "verifier"), 1).await.unwrap();  // 1 minute = 60s

    // Mark in-use should refresh TTL to retry_window + 30s = 120s
    store.mark_state_in_use("abc", 90).await.unwrap();

    // Advance 80 seconds (past original TTL but within refreshed TTL)
    clock.advance(80);

    // Should still be retryable (within window, TTL was refreshed)
    let result = store.mark_state_in_use("abc", 90).await.unwrap();
    assert!(matches!(result, MarkStateResult::Success(_)));
}
```

## Concurrency Considerations

### Dual In-Flight Requests

When two requests arrive with the same state (e.g., user double-clicks):

1. Both call `mark_state_in_use` nearly simultaneously
2. Due to Redis atomicity (Lua script), one will mark it first
3. The second will see status="in_use" and succeed (within retry window)
4. Both proceed to call Google with the same code
5. **Google's code is single-use**: first request succeeds, second gets `invalid_grant`
6. Second request classifies `invalid_grant` as terminal and calls `abort_state`
7. First request's `complete_state` may race with `abort_state`

This is acceptable because:
- Only one request actually succeeds with Google
- The other fails cleanly with a terminal error
- State cleanup happens either way (complete or abort)
- User gets logged in once, as expected

### Idempotency of User Creation

The `find_or_create_google_user` function should be idempotent:
- If user exists with this Google ID, return existing user
- If email exists but no Google link, prompt for link confirmation
- Creates user only if neither condition is met

This ensures concurrent requests don't create duplicate users.

## Configuration

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Retry window | 90 seconds | Enough time for user to notice failure and retry, short enough to prevent stale state accumulation |
| TTL buffer | 30 seconds | Extra time on top of retry window to handle network delays |
| Minimum TTL after mark | 120 seconds | retry_window + buffer = 90 + 30; ensures retries work regardless of original TTL |

The 90-second retry window is a product decision based on typical user behavior. It can be adjusted via constant if needed.

## Files to Modify

| File | Changes |
|------|---------|
| `apps/api/src/application/use_cases/domain_auth.rs` | Add `MarkStateResult` enum, extend `OAuthStateData`, add trait methods |
| `apps/api/src/infra/oauth_state.rs` | Implement Redis methods with TTL refresh and structured Lua returns |
| `apps/api/src/adapters/http/routes/public_domain_auth.rs` | Add `OAuthExchangeError` enum, update handler with typed error handling |
| `apps/api/src/app_error.rs` | Add `OAuthRetryExpired` variant |
| `apps/ui/app/.../oauth-callback/` | Handle 410 response with restart flow |
| `libs/reauth-sdk-ts/src/errors.ts` | Add `OAUTH_RETRY_EXPIRED` error code |

## Success Criteria

- [ ] Typed error classification (no string matching)
- [ ] TTL refresh guarantees retry window works regardless of original TTL
- [ ] Network/DB failures allow retry within 90-second window
- [ ] Terminal errors (invalid_grant, validation) abort state immediately
- [ ] RetryWindowExpired deletes stale state before returning 410
- [ ] `complete_state` failure is best-effort (log and continue)
- [ ] Successful exchange deletes state
- [ ] UI shows clear "restart" prompt on 410
- [ ] Tests cover happy path, retry, expiry, TTL refresh, error classification, and backward compatibility
- [ ] API builds successfully (`./run api:build`)
- [ ] No regression in happy-path OAuth flow

## Rollback Plan

If issues arise:
1. Revert `google_exchange` to use `consume_google_oauth_state` (single-phase)
2. Keep new trait methods as no-ops
3. No data migration needed - serde defaults handle old states

## History

- 2026-01-01: v1 created with basic two-phase design
- 2026-01-01: v2 revision addressing feedback:
  - Replaced string-matching with structured Lua return codes
  - Changed `complete_state` to unconditional delete
  - Added error classification for DB/infra failures
  - Added UI/SDK handling section for 410 response
  - Documented backward compatibility with serde defaults
  - Added time-controllable test clock for unit tests
  - Documented concurrency behavior with dual requests
- 2026-01-01: v3 revision addressing feedback:
  - Replaced string-matching error classification with typed `OAuthExchangeError` enum
  - TTL refresh on `mark_state_in_use` to guarantee retry window (retry_window + 30s buffer)
  - Removed unused `completed` status check from Lua script
  - `RetryWindowExpired` now calls `abort_state` to clean up stale state before returning 410
  - `complete_state` failure is now best-effort (log warning, continue returning success)
  - Added test for error classification and TTL refresh behavior
  - Added configuration table documenting retry window and TTL buffer values
