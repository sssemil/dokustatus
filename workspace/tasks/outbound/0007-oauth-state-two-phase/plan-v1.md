# Plan: Two-Phase Google OAuth State Usage

## Summary

The current OAuth flow consumes the state token **before** exchanging the code with Google, creating a vulnerability where network/API failures leave users unable to retry. If `exchange_google_code` fails (network timeout, Google outage, etc.), the state is already deleted and the user must restart the entire OAuth flow from scratch.

This plan introduces a **two-phase state lifecycle**:
1. **Phase 1 (Mark)**: Mark state as "in-use" when exchange begins
2. **Phase 2 (Complete)**: Delete state only after successful exchange

Failed exchanges allow retry within a short window (e.g., 90 seconds) on the same state.

## Codex Review Feedback (Incorporated)

Per Codex review, this plan addresses:
- **Concurrency risk**: Using atomic Lua scripts with Redis TIME for clock consistency
- **Error classification**: Terminal errors (invalid_grant, parse errors) delete state immediately; only network/5xx errors allow retry
- **Atomicity**: complete_state uses compare-and-delete to only remove in_use states
- **Time source**: Using Redis `TIME` command in Lua scripts to avoid clock skew
- **TTL interaction**: 10-minute TTL >> 90s retry window + worst-case exchange time

## Current Flow (Problem)

```
google_exchange():
  1. consume_state(state)      # ← State DELETED immediately
  2. exchange_google_code()    # ← If this fails, state is gone
  3. parse_google_id_token()   # ← User cannot retry
  4. find_or_create_user()
  5. return completion_token
```

**File**: `apps/api/src/adapters/http/routes/public_domain_auth.rs:900-1022`

The `consume_google_oauth_state` at line 908 atomically deletes the state before the external call to Google at line 950.

## Proposed Flow (Solution)

```
google_exchange():
  1. mark_state_in_use(state)     # ← Mark state as "in-use", set retry window
  2. exchange_google_code()       # ← External call to Google
  3. parse_google_id_token()
  4. find_or_create_user()
  5. complete_state(state)        # ← Only NOW delete the state
  6. return completion_token
```

If step 2-4 fails, the state remains in Redis with status "in_use" and can be retried within the retry window.

## Step-by-Step Implementation

### Step 1: Extend OAuthStateData Structure

**File**: `apps/api/src/application/use_cases/domain_auth.rs:128-134`

Add status and timestamp fields:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthStateData {
    pub domain: String,
    pub code_verifier: String,
    /// Status: "pending" (initial), "in_use" (being exchanged), "completed" (done)
    #[serde(default = "default_pending")]
    pub status: String,
    /// Timestamp when state was marked in-use (for retry window)
    #[serde(default)]
    pub marked_at: Option<i64>,
}

fn default_pending() -> String {
    "pending".to_string()
}
```

### Step 2: Add New Trait Methods

**File**: `apps/api/src/application/use_cases/domain_auth.rs:156-191`

Extend the `OAuthStateStore` trait:

```rust
#[async_trait]
pub trait OAuthStateStore: Send + Sync {
    // Existing methods...
    async fn store_state(&self, state: &str, data: &OAuthStateData, ttl_minutes: i64) -> AppResult<()>;
    async fn consume_state(&self, state: &str) -> AppResult<Option<OAuthStateData>>;  // Keep for backward compat

    // New two-phase methods:

    /// Mark state as "in_use" - returns data if state is valid and not already completed.
    /// Sets a retry window (e.g., 60 seconds) from this point.
    /// Returns None if state doesn't exist, is expired, or already completed.
    /// Returns Err(AlreadyInUse) if state is in_use but outside retry window.
    async fn mark_state_in_use(&self, state: &str, retry_window_secs: i64) -> AppResult<Option<OAuthStateData>>;

    /// Complete the state - deletes it permanently.
    /// Called after successful exchange.
    async fn complete_state(&self, state: &str) -> AppResult<()>;

    // ... other existing methods
}
```

### Step 3: Implement Redis Two-Phase Logic

**File**: `apps/api/src/infra/oauth_state.rs`

Add new methods with atomic Lua scripts. **Key improvements per Codex review**:
- Use Redis `TIME` command instead of app-provided timestamp to avoid clock skew
- `complete_state` uses compare-and-delete to only remove states in "in_use" status

```rust
async fn mark_state_in_use(&self, state: &str, retry_window_secs: i64) -> AppResult<Option<OAuthStateData>> {
    let mut conn = self.manager.clone();
    let key = Self::state_key(state);

    // Lua script: atomically check status and mark in_use
    // Uses Redis TIME for clock consistency across app instances
    // - If status == "pending": mark as "in_use", set marked_at from Redis time
    // - If status == "in_use" AND within retry window: return data (allow retry)
    // - If status == "in_use" AND outside retry window: return error
    // - If status == "completed" or doesn't exist: return nil
    let script = redis::Script::new(r#"
        local value = redis.call('GET', KEYS[1])
        if not value then return nil end

        local data = cjson.decode(value)
        local retry_window = tonumber(ARGV[1])

        -- Get time from Redis to avoid clock skew
        local time_result = redis.call('TIME')
        local now = tonumber(time_result[1])

        if data.status == 'completed' then
            return nil
        end

        if data.status == 'in_use' then
            local marked_at = data.marked_at or 0
            if (now - marked_at) > retry_window then
                return cjson.encode({error = 'retry_window_expired'})
            end
            -- Within retry window, allow retry
            return value
        end

        -- status == 'pending' or nil (backward compat), mark as in_use
        data.status = 'in_use'
        data.marked_at = now
        local new_value = cjson.encode(data)
        redis.call('SET', KEYS[1], new_value, 'KEEPTTL')
        return new_value
    "#);

    let raw: Option<String> = script
        .key(&key)
        .arg(retry_window_secs)
        .invoke_async(&mut conn)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to mark OAuth state in-use: {e}")))?;

    match raw {
        Some(value) if value.contains("retry_window_expired") => {
            Err(AppError::OAuthRetryExpired)
        }
        Some(value) => {
            let data: OAuthStateData = serde_json::from_str(&value)
                .map_err(|e| AppError::Internal(format!("Failed to parse OAuth state: {e}")))?;
            Ok(Some(data))
        }
        None => Ok(None),
    }
}

async fn complete_state(&self, state: &str) -> AppResult<()> {
    let mut conn = self.manager.clone();
    let key = Self::state_key(state);

    // Compare-and-delete: only delete if state is in "in_use" status
    // This prevents a late completion from deleting a newly re-created state
    let script = redis::Script::new(r#"
        local value = redis.call('GET', KEYS[1])
        if not value then return 1 end  -- Already gone, that's fine

        local data = cjson.decode(value)
        if data.status == 'in_use' then
            redis.call('DEL', KEYS[1])
        end
        return 1
    "#);

    let _: i32 = script
        .key(&key)
        .invoke_async(&mut conn)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to complete OAuth state: {e}")))?;

    Ok(())
}

/// Force-delete state for terminal errors (invalid_grant, parse failures).
/// Unlike complete_state, this deletes unconditionally.
async fn abort_state(&self, state: &str) -> AppResult<()> {
    let mut conn = self.manager.clone();
    let key = Self::state_key(state);

    let _: () = conn.del(&key).await
        .map_err(|e| AppError::Internal(format!("Failed to abort OAuth state: {e}")))?;

    Ok(())
}
```

### Step 4: Add Use Case Methods

**File**: `apps/api/src/application/use_cases/domain_auth.rs`

Add wrapper methods in `DomainAuthUseCases`:

```rust
/// Mark OAuth state as in-use (phase 1 of two-phase).
/// Returns the state data if valid, None if not found/expired/completed.
/// Allows retry within retry_window_secs if state is already in-use.
#[instrument(skip(self))]
pub async fn mark_google_oauth_state_in_use(
    &self,
    state: &str,
    retry_window_secs: i64,
) -> AppResult<Option<OAuthStateData>> {
    self.oauth_state_store.mark_state_in_use(state, retry_window_secs).await
}

/// Complete OAuth state (phase 2 of two-phase).
/// Deletes the state permanently after successful exchange.
#[instrument(skip(self))]
pub async fn complete_google_oauth_state(&self, state: &str) -> AppResult<()> {
    self.oauth_state_store.complete_state(state).await
}

/// Abort OAuth state for terminal errors.
/// Forces state deletion to prevent retry loops on unrecoverable errors.
#[instrument(skip(self))]
pub async fn abort_google_oauth_state(&self, state: &str) -> AppResult<()> {
    self.oauth_state_store.abort_state(state).await
}
```

Also update the trait in `OAuthStateStore`:

```rust
#[async_trait]
pub trait OAuthStateStore: Send + Sync {
    // ... existing methods ...

    /// Mark state as "in_use" - returns data if valid and retryable.
    async fn mark_state_in_use(&self, state: &str, retry_window_secs: i64) -> AppResult<Option<OAuthStateData>>;

    /// Complete state after successful exchange (compare-and-delete).
    async fn complete_state(&self, state: &str) -> AppResult<()>;

    /// Abort state for terminal errors (unconditional delete).
    async fn abort_state(&self, state: &str) -> AppResult<()>;
}
```

### Step 5: Update google_exchange Handler

**File**: `apps/api/src/adapters/http/routes/public_domain_auth.rs:900-1022`

Modify the exchange flow with **error classification** per Codex review:
- Network/5xx errors: Keep state in "in_use", allow retry
- Terminal errors (invalid_grant, parse failures): Abort state, force restart

```rust
/// Classifies errors as retryable (network issues) or terminal (invalid data)
fn is_retryable_oauth_error(e: &AppError) -> bool {
    match e {
        // Network timeouts, connection failures - retryable
        AppError::Internal(msg) if msg.contains("Failed to exchange") => true,
        AppError::Internal(msg) if msg.contains("timeout") => true,
        // Google returned invalid_grant, bad token - terminal
        AppError::InvalidInput(_) => false,
        // Default to terminal to be safe
        _ => false,
    }
}

async fn google_exchange(
    State(app_state): State<AppState>,
    Path(_hostname): Path<String>,
    Json(payload): Json<GoogleExchangePayload>,
) -> AppResult<impl IntoResponse> {
    const RETRY_WINDOW_SECS: i64 = 90;  // Increased from 60s per Codex feedback

    // Phase 1: Mark state as in-use (instead of consuming)
    let state_data = app_state
        .domain_auth_use_cases
        .mark_google_oauth_state_in_use(&payload.state, RETRY_WINDOW_SECS)
        .await?
        .ok_or_else(|| AppError::InvalidInput("Invalid or expired OAuth state".into()))?;

    // ... rest of the code (get domain, check oauth enabled, get credentials)

    // External call to Google - classify failures
    let token_response = match exchange_google_code(
        &payload.code,
        &client_id,
        &client_secret,
        &redirect_uri,
        &state_data.code_verifier,
    ).await {
        Ok(response) => response,
        Err(e) => {
            if is_retryable_oauth_error(&e) {
                // Network failure - state remains in "in_use", user can retry
                tracing::warn!("Google code exchange failed (retryable), state {} can be retried", payload.state);
            } else {
                // Terminal error (invalid_grant, etc.) - abort state to prevent retry loops
                tracing::warn!("Google code exchange failed (terminal), aborting state {}", payload.state);
                let _ = app_state.domain_auth_use_cases
                    .abort_google_oauth_state(&payload.state)
                    .await;
            }
            return Err(e);
        }
    };

    // Parse and validate id_token - terminal errors abort state
    let (google_id, email, email_verified) = match parse_google_id_token(
        &token_response.id_token,
        &client_id
    ).await {
        Ok(result) => result,
        Err(e) => {
            // Invalid token is terminal - abort state
            tracing::warn!("Google id_token parse failed (terminal), aborting state {}", payload.state);
            let _ = app_state.domain_auth_use_cases
                .abort_google_oauth_state(&payload.state)
                .await;
            return Err(e);
        }
    };

    // ... rest of validation and user creation

    // Phase 2: Complete state (delete) only after everything succeeded
    app_state
        .domain_auth_use_cases
        .complete_google_oauth_state(&payload.state)
        .await?;

    // Return success response
    match result {
        GoogleLoginResult::LoggedIn(user) => {
            // ... existing code
        }
        GoogleLoginResult::NeedsLinkConfirmation { ... } => {
            // ... existing code
        }
    }
}
```

### Step 6: Handle Stale In-Use States

States stuck in "in_use" status (e.g., server crashed mid-exchange) will naturally expire via Redis TTL. The original 10-minute TTL remains in effect:

- State created with 10-minute TTL
- Marked in-use with retry window of 90 seconds
- After 90 seconds, retry is blocked (returns `OAuthRetryExpired`)
- After 10 minutes, Redis auto-deletes

**TTL Safety Margin**: 10 minutes >> 90s retry window + ~30s worst-case exchange time. This ensures states never expire mid-exchange.

No additional cleanup needed.

### Step 7: Update Error Responses

Add a specific error for retry-window-expired to help the frontend:

**File**: `apps/api/src/app_error.rs`

```rust
pub enum AppError {
    // ... existing variants
    OAuthRetryExpired,  // Retry window has passed, must restart OAuth flow
}

// In IntoResponse impl:
AppError::OAuthRetryExpired => (
    StatusCode::GONE, // 410 Gone
    json!({ "error": "OAUTH_RETRY_EXPIRED", "message": "OAuth retry window expired. Please restart the login process." }),
),
```

## Files to Modify

| File | Changes |
|------|---------|
| `apps/api/src/application/use_cases/domain_auth.rs` | Extend `OAuthStateData` struct, add trait methods, add use case wrappers |
| `apps/api/src/infra/oauth_state.rs` | Implement `mark_state_in_use` and `complete_state` with Lua scripts |
| `apps/api/src/adapters/http/routes/public_domain_auth.rs` | Update `google_exchange` to use two-phase flow |
| `apps/api/src/app_error.rs` | (Optional) Add `OAuthRetryExpired` error variant |

## Testing Approach

### Unit Tests (domain_auth.rs)

Add tests in the existing `mod tests` section:

```rust
#[tokio::test]
async fn test_oauth_state_two_phase_happy_path() {
    // 1. Store state
    // 2. Mark in-use -> should succeed
    // 3. Complete -> should succeed
    // 4. Mark in-use again -> should return None
}

#[tokio::test]
async fn test_oauth_state_retry_within_window() {
    // 1. Store state
    // 2. Mark in-use -> should succeed
    // 3. Mark in-use again (within 60s) -> should succeed (same data)
}

#[tokio::test]
async fn test_oauth_state_retry_after_window() {
    // 1. Store state
    // 2. Mark in-use
    // 3. Wait/mock time past retry window
    // 4. Mark in-use again -> should error
}

#[tokio::test]
async fn test_oauth_state_complete_before_mark() {
    // 1. Store state
    // 2. Complete without marking -> should be ok (idempotent)
}
```

### Integration Testing

Manual testing via the demo app:
1. Start OAuth flow normally
2. Inject a failure in `exchange_google_code` (e.g., wrong client_secret temporarily)
3. Verify error is returned but state is not consumed
4. Fix the failure and retry with same state
5. Verify successful login

### In-Memory Mock for Tests

Create an `InMemoryOAuthStateStore` for unit testing (similar to existing `InMemoryMagicLinkStore`):

```rust
#[derive(Default)]
struct InMemoryOAuthStateStore {
    states: Mutex<HashMap<String, (OAuthStateData, Instant)>>,
}

#[async_trait]
impl OAuthStateStore for InMemoryOAuthStateStore {
    async fn mark_state_in_use(&self, state: &str, retry_window_secs: i64) -> AppResult<Option<OAuthStateData>> {
        let mut states = self.states.lock().unwrap();
        // ... implement two-phase logic
    }
    // ... other methods
}
```

## Edge Cases to Handle

1. **Double-click/Rapid Retry**: Two simultaneous exchange requests with same state
   - First request marks in-use, second sees "in_use" status
   - If within retry window, second request gets the data and can proceed
   - Only one will successfully complete (code is single-use at Google's end)
   - **Note**: This is acceptable since Google enforces code single-use; the second request will fail with `invalid_grant` and abort the state

2. **State Already Completed**: If somehow complete_state is called twice
   - Compare-and-delete only removes if status is "in_use"
   - Subsequent calls are no-ops (idempotent)

3. **Server Crash Mid-Exchange**: State stuck in "in_use"
   - User can retry within 90 seconds
   - After 90 seconds, returns `OAuthRetryExpired` - must restart OAuth flow
   - After 10 minutes, state auto-expires via Redis TTL

4. **Code Reuse Attack**: Attacker tries to reuse same code with stolen state
   - Code is single-use at Google's end
   - Even if state allows retry, Google will reject the code with `invalid_grant`
   - Terminal error triggers `abort_state`, preventing further retries

5. **Backward Compatibility**: Old states without `status` field
   - Default `status` to "pending" via serde default (Lua handles nil as pending)
   - Works seamlessly with existing stored states

6. **Terminal Error After Retry Window**: User retries with same state after terminal error
   - State was already aborted, returns "Invalid or expired OAuth state"
   - User must restart the full OAuth flow

7. **Clock Skew Between App Instances**: Different servers have different clocks
   - Using Redis `TIME` command in Lua scripts ensures consistent time across all instances
   - All time comparisons happen in Redis, not app code

## Rollback Plan

If issues arise:
1. Revert `google_exchange` to use `consume_google_oauth_state` (single-phase)
2. Keep new trait methods but ignore them
3. No migration needed - existing states continue to work

## Success Criteria

- [ ] Exchange failures (network) allow retry within 90-second window
- [ ] Terminal errors (invalid_grant) abort state immediately
- [ ] Successful exchange still prevents reuse (state deleted)
- [ ] No regression in happy-path OAuth flow
- [ ] Tests cover all edge cases
- [ ] API builds successfully (`./run api:build`)

## History

- 2026-01-01: Initial plan created
- 2026-01-01: Updated per Codex review feedback:
  - Added error classification (retryable vs terminal)
  - Added `abort_state` method for terminal errors
  - Changed to Redis `TIME` for clock consistency
  - Updated `complete_state` to use compare-and-delete
  - Increased retry window from 60s to 90s
  - Added edge cases for clock skew and terminal error handling
