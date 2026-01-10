# Plan v1: Split public_domain_auth.rs into focused modules

**Created**: 2026-01-01
**Task**: 0018-split-public-domain-auth-routes
**Parent ticket**: ./ticket.md

## Summary

The `public_domain_auth.rs` file has grown to 3,293 lines and contains multiple logical domains:
1. Core authentication (magic link, session, token refresh, logout)
2. Google OAuth integration
3. Billing (subscriptions, checkout, plans, payments)
4. Stripe webhooks
5. Dummy payment provider (test scenarios)

This plan splits the file into focused modules while maintaining the same public API and route structure.

## Current File Analysis

### Route Groups (from `router()` function at line 228-277)

**Config Route** (1 route):
- `GET /{domain}/config`

**Auth Routes** (9 routes):
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

### Shared Helper Functions

Located throughout the file:
- `append_cookie()` - lines 34-39
- `complete_login()` - lines 50-169
- `ensure_login_session()` - lines 768-788
- `get_current_user()` - lines 2462-2483
- Google OAuth helpers - lines 2537-2844

### Test Modules

- `oauth_exchange_tests` - lines 2846-2881
- `webhook_error_tests` - lines 3234-3293

## Proposed Module Structure

```
apps/api/src/adapters/http/routes/
├── mod.rs                       # (update)
├── public_domain_auth/          # (new directory)
│   ├── mod.rs                   # Re-exports router(), merges sub-routers
│   ├── common.rs                # Shared types and helpers
│   ├── config.rs                # GET /{domain}/config
│   ├── magic_link.rs            # Magic link auth routes
│   ├── session.rs               # Session, refresh, logout, delete account
│   ├── google_oauth.rs          # All Google OAuth routes + helpers
│   ├── billing.rs               # Core billing routes (plans, subscription, checkout, portal, cancel)
│   ├── billing_payments.rs      # Payment history and plan change routes
│   ├── billing_webhooks.rs      # Stripe webhook handlers
│   └── billing_dummy.rs         # Dummy payment provider routes
```

## Step-by-Step Implementation

### Phase 1: Create Module Directory and Common Module

1. Create `apps/api/src/adapters/http/routes/public_domain_auth/` directory
2. Create `common.rs` with shared code:
   - `append_cookie()` helper
   - `complete_login()` helper
   - `ensure_login_session()` helper
   - `get_current_user()` helper
   - `LoginCompletionResult` struct
   - Common response types: `PublicConfigResponse`, `AuthMethodsResponse`

### Phase 2: Extract Config Route

Create `config.rs` with:
- `get_config()` handler
- `router()` function returning the config sub-router

### Phase 3: Extract Magic Link Authentication

Create `magic_link.rs` with:
- `RequestMagicLinkPayload` struct
- `VerifyMagicLinkPayload` struct
- `VerifyMagicLinkResponse` struct
- `request_magic_link()` handler
- `verify_magic_link()` handler
- `router()` function

### Phase 4: Extract Session Management

Create `session.rs` with:
- `SessionSubscriptionInfo` struct
- `SessionResponse` struct
- `check_session()` handler
- `refresh_token()` handler
- `logout()` handler
- `delete_account()` handler
- `router()` function

### Phase 5: Extract Google OAuth

Create `google_oauth.rs` with:
- All Google OAuth types (GoogleStartResponse, GoogleExchangePayload, etc.)
- All Google OAuth handlers
- `OAuthExchangeError` enum and helpers
- Google token parsing/validation helpers
- `fetch_google_jwks()` helper
- `router()` function
- `oauth_exchange_tests` test module

### Phase 6: Extract Core Billing

Create `billing.rs` with:
- `PublicPlanResponse` struct
- `UserSubscriptionResponse` struct
- `CreateCheckoutPayload`, `CheckoutResponse` structs
- `CreatePortalPayload`, `PortalResponse` structs
- `get_public_plans()` handler
- `get_user_subscription()` handler
- `create_checkout()` handler
- `create_portal()` handler
- `cancel_subscription()` handler
- `router()` function

### Phase 7: Extract Payment and Plan Change Routes

Create `billing_payments.rs` with:
- `PaymentListQuery`, `PaymentListResponse`, `PaymentResponse` structs
- `PlanChangePreviewQuery`, `PlanChangePreviewResponse` structs
- `PlanChangeRequest`, `PlanChangeResponse`, `PlanChangeNewPlanResponse` structs
- `get_user_payments()` handler
- `preview_plan_change()` handler
- `change_plan()` handler
- `get_available_providers()` handler (provider listing)
- `router()` function

### Phase 8: Extract Stripe Webhooks

Create `billing_webhooks.rs` with:
- `is_retryable_error()` helper
- `webhook_retryable_error()` helper
- `handle_webhook_test()` handler
- `handle_webhook_live()` handler
- `handle_webhook_for_mode()` internal handler
- `router()` function
- `webhook_error_tests` test module

### Phase 9: Extract Dummy Payment Provider

Create `billing_dummy.rs` with:
- `DummyScenarioInfo` struct
- `DummyCheckoutPayload`, `DummyCheckoutResponse` structs
- `DummyConfirmPayload` struct
- `get_dummy_scenarios()` handler
- `create_dummy_checkout()` handler
- `confirm_dummy_checkout()` handler
- `router()` function

### Phase 10: Create Main Module File

Create `mod.rs` that:
- Declares all sub-modules
- Imports all sub-routers
- Exposes single `router()` function that merges all sub-routers

### Phase 11: Update Parent mod.rs

Update `apps/api/src/adapters/http/routes/mod.rs`:
- Change `pub mod public_domain_auth;` to reference the directory module

## Files to Modify

| File | Action |
|------|--------|
| `apps/api/src/adapters/http/routes/mod.rs` | Update module declaration |
| `apps/api/src/adapters/http/routes/public_domain_auth.rs` | Delete (replaced by directory) |
| `apps/api/src/adapters/http/routes/public_domain_auth/mod.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/common.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/config.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/magic_link.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/session.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/google_oauth.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/billing.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/billing_payments.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/billing_webhooks.rs` | Create |
| `apps/api/src/adapters/http/routes/public_domain_auth/billing_dummy.rs` | Create |

## Testing Approach

1. **Compilation Check**: Run `cargo check` after each phase
2. **Unit Tests**: Ensure existing tests in `oauth_exchange_tests` and `webhook_error_tests` still pass
3. **Full Test Suite**: Run `./run api:test` after all phases complete
4. **Build Verification**: Run `./run api:build` (SQLX_OFFLINE=true cargo build --release)

### Test Commands

```bash
# After each phase
cargo check --package reauth-api

# After all phases
./run api:test
./run api:build
```

## Edge Cases to Handle

1. **Circular Dependencies**: The `complete_login()` function uses billing use cases - ensure imports are correct
2. **Re-exports**: Keep `pub use` statements minimal; only export what's needed publicly
3. **Test Module Visibility**: Test modules need access to private functions - keep tests in same file as the code they test
4. **Cookie Domain Logic**: Ensure `append_cookie()` is accessible from all modules that need it
5. **AppError Dependency**: All modules depend on AppError - ensure consistent imports
6. **StripeMode Import**: Webhook module needs StripeMode entity - ensure import path is correct

## Dependencies Between Modules

```
common.rs
  └── Used by: all other modules

config.rs
  └── Uses: common.rs

magic_link.rs
  └── Uses: common.rs

session.rs
  └── Uses: common.rs

google_oauth.rs
  └── Uses: common.rs

billing.rs
  └── Uses: common.rs

billing_payments.rs
  └── Uses: common.rs

billing_webhooks.rs
  └── Uses: common.rs (get_current_user not needed, but AppState access)

billing_dummy.rs
  └── Uses: common.rs
```

## Rollback Plan

If issues are discovered:
1. Keep the original `public_domain_auth.rs` in a backup branch
2. Git revert to before the split if needed
3. The refactoring is purely structural - no logic changes

## Estimated Complexity

- **Low Risk**: Pure refactoring, no logic changes
- **Lines of Code**: ~3,300 lines → ~10 files of 100-500 lines each
- **Phases**: 11 implementation phases

---

## Revision History

- 2026-01-01: Initial plan created
