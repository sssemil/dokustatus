# Change Report: "automated extravaganza"

**Commit:** `96701d3` | **Files Changed:** 209 | **+38,946 / -3,659 lines**

This commit consolidates 43 individual commits spanning 20+ completed tasks. The changes fall into six major categories.

---

## 1. Agent Automation System (NEW)

A new 2,300-line Python orchestration system (`agent_loop.py`) that automates task execution using AI agents (Claude + Codex).

### Architecture
```
Task Lifecycle: todo/ → in-progress/ → outbound/ → done/

Each task runs in isolated git worktrees at ../worktrees/task-{slug}/
```

### Key Features
- **Parallel Execution**: Up to N concurrent tasks (default 3, configurable via `-j`)
- **Planning Phase**: 3-iteration loop with Claude planning and Codex reviewing
- **Merge Protocol**: First-to-finish wins; fcntl locks prevent race conditions
- **Crash Recovery**: State persisted to `.task-state`; crashed subprocesses auto-restart
- **Rate Limit Fallback**: If Codex hits limits, tasks switch to Claude

### Usage
```bash
./agent_loop.py           # 3 concurrent tasks
./agent_loop.py -j 5      # 5 concurrent tasks
./agent_loop.py -j 3 22 21 5  # priority queue
```

### Supporting Files
- `AGENTS.md` - Guidelines for workspace organization and Codex co-op patterns
- `workspace/` directory structure for plans/tasks/done tracking
- `.merge.lock`, `.task-state`, `.merge-requested` - coordination files

---

## 2. Security Fixes

### 2.1 Token Hash Collision Fix (0002)
**Problem:** Simple `hash(token + domain)` allowed collision attacks where `hash("abc" + "def") == hash("ab" + "cdef")`.

**Solution:** Length-prefixed hashing:
```rust
// Before: hasher.update(raw); hasher.update(domain);
// After:
hasher.update((raw_bytes.len() as u32).to_be_bytes());
hasher.update(raw_bytes);
hasher.update((domain_bytes.len() as u32).to_be_bytes());
hasher.update(domain_bytes);
```

### 2.2 Constant-Time Webhook Signature Verification (0003)
**Problem:** String comparison `==` leaked timing information for webhook signatures.

**Solution:** Use constant-time comparison to prevent timing attacks on HMAC verification.

### 2.3 CSV Formula Injection Prevention (0016)
**Problem:** Exported CSV data could contain cells starting with `=`, `+`, `-`, `@` that execute formulas in spreadsheets.

**Solution:** Prefix dangerous cells to neutralize injection vectors.

### 2.4 Stripe Error Log Redaction (0013)
**Problem:** Stripe API errors could contain sensitive data (customer IDs, payment info) in logs.

**Solution:** Redact sensitive fields before logging Stripe errors.

---

## 3. OAuth & Authentication Hardening

### 3.1 Two-Phase OAuth State (0007)
**Problem:** OAuth `state` parameter was consumed immediately, preventing retries if token exchange failed.

**Solution:** New two-phase state machine:
```
pending → in_use → completed
         ↓
    (retry window: 60s)
```

New error code `OAUTH_RETRY_EXPIRED` when retry window closes:
```rust
AppError::OAuthRetryExpired => error_resp(
    StatusCode::GONE,
    ErrorCode::OAuthRetryExpired,
    Some("OAuth session expired. Please restart the login process.".to_string()),
)
```

**New SDK exports:**
```typescript
export { ReauthErrorCode, requiresOAuthRestart } from './errors';
// ReauthErrorCode.OAUTH_RETRY_EXPIRED
// requiresOAuthRestart(error) → boolean
```

### 3.2 HTTP Client Timeouts (0008)
**Problem:** HTTP clients (Stripe, Resend, Google OAuth) had no timeouts, risking hung requests.

**Solution:** New `infra/http_client.rs` module:
```rust
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub fn build_client() -> Client {
    Client::builder()
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
        .timeout(DEFAULT_REQUEST_TIMEOUT)
        .build()
}
```

All HTTP clients (`StripeClient`, `DomainEmailSender`) now use this factory.

---

## 4. Performance Optimizations

### 4.1 Batch Domain Auth Configs (0004)
**Problem:** `list_domains` made N+1 queries to check auth methods for each domain.

**Solution:** New batch method:
```rust
// New repository method
async fn get_by_domain_ids(&self, domain_ids: &[Uuid])
    -> AppResult<Vec<DomainAuthConfigProfile>>;

// New use case method
pub async fn has_auth_methods_for_owner_domains(&self, domain_ids: &[Uuid])
    -> AppResult<HashMap<Uuid, bool>>;
```

**Impact:** Dashboard domain list loads in 1 query instead of N+1.

### 4.2 Database Index for Google ID (0015)
Added index on Google ID column for faster OAuth lookups.

### 4.3 SQL Parameter Builder (0019)
Improved dynamic SQL construction for better query performance and safety.

---

## 5. Payment System Improvements

### 5.1 Dummy Payment Provider Clarification (0006)
**Problem:** Dummy provider methods returned fake data, confusing callers about source of truth.

**Solution:** Explicit "not supported" behavior:
```rust
async fn get_subscription(&self, _: &SubscriptionId) -> AppResult<Option<SubscriptionInfo>> {
    tracing::trace!("Dummy provider: get_subscription not supported, use database");
    Ok(None)
}

async fn get_customer(&self, _: &CustomerId) -> AppResult<Option<CustomerInfo>> {
    tracing::trace!("Dummy provider: get_customer not supported, use database");
    Ok(None)
}
```

Updated docstrings to clarify: **For dummy provider, the database is the source of truth.**

### 5.2 MRR Precision Fix (0011)
Fixed floating-point precision issues in Monthly Recurring Revenue calculations.

### 5.3 Payment Provider Abstraction (0012)
New `PaymentProviderFactory` for cleaner provider instantiation:
```rust
let provider_factory = Arc::new(PaymentProviderFactory::new(
    billing_cipher.clone(),
    billing_stripe_config_repo.clone(),
));
```

---

## 6. Code Architecture

### 6.1 Route Module Split (0018)
**Before:** Single 2,694-line `public_domain_auth.rs` file.

**After:** Split into focused modules:
```
public_domain_auth/
├── mod.rs           # Router assembly
├── billing.rs       # Billing endpoints
├── billing_dummy.rs # Dummy billing
├── billing_webhooks.rs
├── common.rs        # Shared utilities
├── config.rs        # Config endpoints
├── google_oauth.rs  # OAuth flow
├── magic_link.rs    # Magic link auth
└── session.rs       # Session management
```

**Benefit:** Each module is now <400 lines, easier to navigate and test.

### 6.2 Legacy Function Removal (0021)
Cleaned up deprecated code paths:
```typescript
// Removed from apps/ui/types/billing.ts
interface StripeConfig {
  publishable_key: string | null;
  has_secret_key: boolean;
  is_connected: boolean;
}
```

### 6.3 Transactional Domain Delete (0005)
Domain deletion now uses database transactions to prevent partial deletes.

---

## 7. Frontend & DevOps

### 7.1 Umami Analytics (0022)
Added optional analytics tracking via build arg:
```bash
BUILD_ARGS="--build-arg NEXT_PUBLIC_UMAMI_WEBSITE_ID=xxx" ./infra/deploy.sh
```

If omitted, no analytics script is included (safe for local/staging).

### 7.2 Environment Examples
Added `.env.example` files for `apps/demo_ui` and `apps/ui`.

---

## Summary Table

| Category | Tasks | Risk Level |
|----------|-------|------------|
| Security | 0002, 0003, 0013, 0016 | **High** - Should be deployed promptly |
| Auth Hardening | 0007, 0008, 0009, 0010 | Medium - Improves reliability |
| Performance | 0004, 0015, 0019 | Medium - Better UX |
| Payment | 0006, 0011, 0012 | Low - Cleanup/clarity |
| Architecture | 0018, 0021, 0005 | Low - Maintainability |
| Automation | agent_loop.py | N/A - Dev tooling |

---

## Remaining TODOs

Files in `workspace/tasks/todo/`:
- `0014-unify-payment-mode-enums` - Consolidate payment mode enum definitions
- `0017-plan-code-validation` - Add validation for AI-generated plans
- `0020-redact-db-url-error` - Redact database URLs in error messages
