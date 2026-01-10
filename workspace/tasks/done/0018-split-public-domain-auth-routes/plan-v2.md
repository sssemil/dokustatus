# Plan v2: Split public_domain_auth.rs into focused modules

**Created**: 2026-01-01
**Task**: 0018-split-public-domain-auth-routes
**Parent ticket**: ./ticket.md
**Previous version**: ./plan-v1.md

## Summary

The `public_domain_auth.rs` file has grown to 3,293 lines and contains multiple logical domains:
1. Core authentication (magic link, session, token refresh, logout)
2. Google OAuth integration
3. Billing (subscriptions, checkout, plans, payments)
4. Stripe webhooks
5. Dummy payment provider (test scenarios)

This plan splits the file into focused modules while maintaining the same public API and route structure.

---

## Changes from v1

This revision addresses feedback from plan-v1:

1. **Added Import Management Strategy** (Section: Import Distribution)
2. **Fixed route count**: Auth routes = 11 (was incorrectly listed as 9)
3. **Explicitly placed `unlink_google()`** in google_oauth.rs
4. **Added Cookie Helper Consolidation** (`clear_auth_cookies()` helper)
5. **Defined Visibility Strategy** (pub vs pub(crate) vs private)
6. **Clarified StripeMode vs PaymentMode** (confirmed PaymentMode is used)
7. **Added lint/fmt verification steps**
8. **Consolidated `billing.rs` and `billing_payments.rs`** (combined < 400 lines)

---

## Current File Analysis

### Route Groups (from `router()` function at lines 228-277)

**Config Route** (1 route):
- `GET /{domain}/config`

**Auth Routes** (11 routes - corrected from v1):
- `POST /{domain}/auth/request-magic-link`
- `POST /{domain}/auth/verify-magic-link`
- `POST /{domain}/auth/google/start`
- `POST /{domain}/auth/google/exchange`
- `POST /{domain}/auth/google/confirm-link`
- `POST /{domain}/auth/google/complete`
- `POST /{domain}/auth/google/unlink`
- `GET /{domain}/auth/session`
- `POST /{domain}/auth/refresh`
- `POST /{domain}/auth/logout`
- `DELETE /{domain}/auth/account`

**Billing Routes** (10 routes):
- `GET /{domain}/billing/plans`
- `GET /{domain}/billing/subscription`
- `POST /{domain}/billing/checkout`
- `POST /{domain}/billing/portal`
- `POST /{domain}/billing/cancel`
- `GET /{domain}/billing/payments`
- `GET /{domain}/billing/plan-change/preview`
- `POST /{domain}/billing/plan-change`
- `GET /{domain}/billing/providers`
- `POST /{domain}/billing/checkout/dummy`
- `POST /{domain}/billing/dummy/confirm`
- `GET /{domain}/billing/dummy/scenarios`

**Webhook Routes** (2 routes):
- `POST /{domain}/billing/webhook/test`
- `POST /{domain}/billing/webhook/live`

---

## Proposed Module Structure

```
apps/api/src/adapters/http/routes/
├── mod.rs                       # (update import)
├── public_domain_auth/          # (new directory)
│   ├── mod.rs                   # Declares sub-modules, exports router()
│   ├── common.rs                # Shared types, helpers, cookie utilities
│   ├── config.rs                # GET /{domain}/config
│   ├── magic_link.rs            # Magic link auth routes
│   ├── session.rs               # Session, refresh, logout, delete account
│   ├── google_oauth.rs          # All Google OAuth routes + unlink + helpers
│   ├── billing.rs               # All billing routes (plans, subscription, checkout, payments, plan-change)
│   ├── billing_webhooks.rs      # Stripe webhook handlers
│   └── billing_dummy.rs         # Dummy payment provider routes
```

**Changes from v1:**
- Merged `billing.rs` and `billing_payments.rs` into single `billing.rs` (~350 lines combined)
- Total: 8 modules instead of 9

---

## Import Distribution Strategy

### Shared imports (`common.rs`)

These imports are used across multiple modules and belong in `common.rs`:

```rust
// Core framework
pub use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
};
pub use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
pub use serde::{Deserialize, Serialize};
pub use time;
pub use uuid::Uuid;

// App-level
pub use crate::adapters::http::app_state::AppState;
pub use crate::app_error::{AppError, AppResult};
pub use crate::application::jwt;
pub use crate::application::use_cases::domain::extract_root_from_reauth_hostname;
```

### Module-specific imports

| Module | Additional Imports |
|--------|-------------------|
| `magic_link.rs` | `is_valid_email` |
| `session.rs` | `DomainEndUserProfile`, `SubscriptionClaims` |
| `google_oauth.rs` | `GoogleLoginResult`, `MarkStateResult`, `http_client`, `tracing::error` |
| `billing.rs` | `PaymentProvider`, `SubscriptionClaims` |
| `billing_webhooks.rs` | `PaymentMode`, `tracing::error` |
| `billing_dummy.rs` | `PaymentScenario`, `StripeMode`, `SubscriptionStatus`, chrono |

---

## Visibility Strategy

### Public (`pub`)
- `router()` in `mod.rs` - the only public API
- Types re-exported if used by other crates (none currently)

### Crate-public (`pub(crate)`)
- All handlers (needed for route registration)
- Response/request types (internal to the crate)
- Helper functions in `common.rs` (used across sibling modules)

### Private (no modifier)
- Module-internal helper functions
- Test modules

### Example `mod.rs` structure:
```rust
mod common;
mod config;
mod magic_link;
mod session;
mod google_oauth;
mod billing;
mod billing_webhooks;
mod billing_dummy;

use common::*;
use crate::adapters::http::app_state::AppState;
use axum::Router;

pub fn router() -> Router<AppState> {
    config::router()
        .merge(magic_link::router())
        .merge(session::router())
        .merge(google_oauth::router())
        .merge(billing::router())
        .merge(billing_webhooks::router())
        .merge(billing_dummy::router())
}
```

---

## Step-by-Step Implementation

### Phase 1: Create Module Directory and Common Module

1. Create `apps/api/src/adapters/http/routes/public_domain_auth/` directory
2. Create `common.rs` with shared code:

**Helpers to extract:**
- `append_cookie()` (lines 34-39)
- `complete_login()` (lines 50-169)
- `ensure_login_session()` (lines 768-788)
- `get_current_user()` (lines 2462-2483)

**New helper to create (consolidates duplicate code):**
```rust
/// Clears all auth cookies (access, refresh, email) for logout/delete
pub(crate) fn clear_auth_cookies(headers: &mut HeaderMap, root_domain: &str) -> Result<(), AppError> {
    let access_cookie = Cookie::build(("end_user_access_token", ""))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .domain(format!(".{}", root_domain))
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();

    let refresh_cookie = Cookie::build(("end_user_refresh_token", ""))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .domain(format!(".{}", root_domain))
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();

    let email_cookie = Cookie::build(("end_user_email", ""))
        .http_only(false)
        .secure(true)
        .same_site(SameSite::Lax)
        .domain(format!(".{}", root_domain))
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();

    append_cookie(headers, access_cookie)?;
    append_cookie(headers, refresh_cookie)?;
    append_cookie(headers, email_cookie)?;
    Ok(())
}
```

**Types to extract:**
- `LoginCompletionResult` struct (lines 42-46)
- `PublicConfigResponse` struct
- `AuthMethodsResponse` struct
- `SessionSubscriptionInfo` struct
- `SessionResponse` struct

### Phase 2: Extract Config Route

Create `config.rs` with:
- `get_config()` handler
- `pub(crate) fn router()` returning the config sub-router

### Phase 3: Extract Magic Link Authentication

Create `magic_link.rs` with:
- `RequestMagicLinkPayload` struct
- `VerifyMagicLinkPayload` struct
- `VerifyMagicLinkResponse` struct
- `request_magic_link()` handler
- `verify_magic_link()` handler
- `pub(crate) fn router()` function

### Phase 4: Extract Session Management

Create `session.rs` with:
- `check_session()` handler
- `refresh_token()` handler
- `logout()` handler - **use new `clear_auth_cookies()` helper**
- `delete_account()` handler - **use new `clear_auth_cookies()` helper**
- `pub(crate) fn router()` function

### Phase 5: Extract Google OAuth

Create `google_oauth.rs` with:

**Types:**
- `GoogleStartResponse` struct
- `GoogleExchangePayload` struct
- `GoogleExchangeResponse` struct
- `GoogleConfirmLinkPayload` struct
- `GoogleCompletePayload` struct
- `GoogleCompleteResponse` struct
- `OAuthExchangeError` enum + `impl` blocks

**Handlers:**
- `google_start()` handler
- `google_exchange()` handler
- `google_confirm_link()` handler
- `google_complete()` handler
- `unlink_google()` handler (explicitly included per feedback)

**Helpers:**
- `exchange_code_for_tokens()`
- `validate_id_token()`
- `fetch_google_jwks()`
- Internal token parsing functions

**Tests:**
- `oauth_exchange_tests` test module

**Implementation notes for Phase 5:**
This is the largest phase (~500 lines). Execute in sub-steps:
1. First: Move types and OAuthExchangeError enum
2. Then: Move helper functions
3. Then: Move handlers
4. Finally: Move tests
Run `cargo check` after each sub-step.

### Phase 6: Extract Billing (Combined)

Create `billing.rs` with (merged from v1's billing.rs + billing_payments.rs):

**Types:**
- `PublicPlanResponse` struct
- `UserSubscriptionResponse` struct
- `CreateCheckoutPayload`, `CheckoutResponse` structs
- `CreatePortalPayload`, `PortalResponse` structs
- `PaymentListQuery`, `PaymentListResponse`, `PaymentResponse` structs
- `PlanChangePreviewQuery`, `PlanChangePreviewResponse` structs
- `PlanChangeRequest`, `PlanChangeResponse`, `PlanChangeNewPlanResponse` structs

**Handlers:**
- `get_public_plans()` handler
- `get_user_subscription()` handler
- `create_checkout()` handler
- `create_portal()` handler
- `cancel_subscription()` handler
- `get_user_payments()` handler
- `preview_plan_change()` handler
- `change_plan()` handler
- `get_available_providers()` handler

**Router:**
- `pub(crate) fn router()` function

### Phase 7: Extract Stripe Webhooks

Create `billing_webhooks.rs` with:
- `is_retryable_error()` helper
- `webhook_retryable_error()` helper
- `handle_webhook_test()` handler
- `handle_webhook_live()` handler
- `handle_webhook_for_mode()` internal handler
- `pub(crate) fn router()` function
- `webhook_error_tests` test module

**Import note:** Uses `PaymentMode` from `domain::entities::payment_mode` (not StripeMode - v1 was incorrect).

### Phase 8: Extract Dummy Payment Provider

Create `billing_dummy.rs` with:
- `DummyScenarioInfo` struct
- `DummyCheckoutPayload`, `DummyCheckoutResponse` structs
- `DummyConfirmPayload` struct
- `get_dummy_scenarios()` handler
- `create_dummy_checkout()` handler
- `confirm_dummy_checkout()` handler
- `pub(crate) fn router()` function

**Import note:** Uses `StripeMode` entity for dummy subscription creation (line 3182).

### Phase 9: Create Main Module File

Create `mod.rs` that:
- Declares all sub-modules with `mod`
- Imports `AppState` and `Router`
- Exposes single `pub fn router()` that merges all sub-routers

### Phase 10: Update Parent mod.rs and Remove Old File

Update `apps/api/src/adapters/http/routes/mod.rs`:
- The `pub mod public_domain_auth;` declaration automatically picks up the directory module
- Delete `public_domain_auth.rs` file

---

## Files to Modify

| File | Action |
|------|--------|
| `apps/api/src/adapters/http/routes/mod.rs` | Verify module declaration (no change needed) |
| `apps/api/src/adapters/http/routes/public_domain_auth.rs` | Delete (replaced by directory) |
| `apps/api/src/adapters/http/routes/public_domain_auth/mod.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/common.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/config.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/magic_link.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/session.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/google_oauth.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/billing.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/billing_webhooks.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/billing_dummy.rs` | Create |

**Total: 9 new files, 1 deleted file**

---

## Testing Approach

### After Each Phase
```bash
# Compilation check
cargo check --package reauth-api

# Optionally run full tests (recommended for complex phases)
cargo test --package reauth-api
```

### After All Phases
```bash
# Full verification
./run api:fmt      # Format code
./run api:lint     # Lint check
./run api:test     # Run all tests
./run api:build    # Production build (SQLX_OFFLINE=true)
```

---

## Dependencies Between Modules

```
common.rs
  └── Used by: all other modules (via `use super::common::*`)

config.rs
  └── Uses: common

magic_link.rs
  └── Uses: common

session.rs
  └── Uses: common

google_oauth.rs
  └── Uses: common, http_client

billing.rs
  └── Uses: common

billing_webhooks.rs
  └── Uses: common (not get_current_user, but AppState access)

billing_dummy.rs
  └── Uses: common
```

---

## Edge Cases to Handle

1. **Circular Dependencies**: None expected - `common.rs` is a shared leaf module
2. **Re-exports**: Only `router()` needs to be public from `mod.rs`
3. **Test Module Visibility**: Tests stay in same file as code they test; use `super::*` for imports
4. **Cookie Domain Logic**: `append_cookie()` and `clear_auth_cookies()` in common.rs available to all
5. **Inline Use Statements**: Several handlers have inline `use` (e.g., `GoogleLoginResult` in google_exchange). Move these to module-level imports.
6. **PaymentMode vs StripeMode**: Webhooks use `PaymentMode`; dummy billing uses `StripeMode`. Both are correct for their contexts.

---

## Rollback Plan

If issues are discovered:
1. Git revert to before the split
2. The original `public_domain_auth.rs` is preserved in git history
3. The refactoring is purely structural with one improvement (cookie helper consolidation)

---

## Complexity Assessment

- **Low Risk**: Pure refactoring with one small improvement (cookie helper)
- **Lines of Code**: ~3,300 lines → ~8 files of 100-500 lines each
- **Phases**: 10 implementation phases (reduced from 11)
- **Estimated file sizes**:
  - `common.rs`: ~250 lines (helpers + shared types)
  - `config.rs`: ~50 lines
  - `magic_link.rs`: ~150 lines
  - `session.rs`: ~200 lines
  - `google_oauth.rs`: ~500 lines (largest)
  - `billing.rs`: ~350 lines (combined)
  - `billing_webhooks.rs`: ~150 lines + tests
  - `billing_dummy.rs`: ~300 lines
  - `mod.rs`: ~30 lines

---

## Revision History

- 2026-01-01: Initial plan created (v1)
- 2026-01-01: Revised plan addressing feedback (v2)
  - Added import management strategy
  - Fixed route count (11, not 9)
  - Added `clear_auth_cookies()` helper
  - Defined visibility strategy
  - Merged billing modules (9 → 8 modules)
  - Added lint/fmt verification
  - Clarified StripeMode vs PaymentMode usage
