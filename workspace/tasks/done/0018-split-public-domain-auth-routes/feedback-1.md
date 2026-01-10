# Feedback on Plan v1: Split public_domain_auth.rs

**Reviewer**: Claude (Opus 4.5)
**Date**: 2026-01-01
**Plan Version**: v1

---

## What's Good About the Plan

1. **Thorough File Analysis**: The plan accurately maps the 3,293-line file structure, identifying route groups, helper functions, and test modules with correct line numbers.

2. **Logical Module Grouping**: The proposed 9-module structure (`common`, `config`, `magic_link`, `session`, `google_oauth`, `billing`, `billing_payments`, `billing_webhooks`, `billing_dummy`) aligns well with the file's natural boundaries marked by comment headers.

3. **Dependency Tracking**: The plan correctly identifies that `common.rs` will be the shared dependency for all other modules, avoiding circular dependencies.

4. **Incremental Approach**: The 11-phase approach with `cargo check` after each phase is sensible for a refactoring of this size.

5. **Test Preservation**: The plan explicitly mentions keeping tests (`oauth_exchange_tests`, `webhook_error_tests`) with the code they test.

6. **Rollback Plan**: Having a clear rollback strategy (git revert) is appropriate for a pure structural refactoring.

---

## What's Missing or Unclear

### 1. Import Management Not Detailed

The plan lists what goes into each module but doesn't address:
- Which imports need to be moved to `common.rs` vs module-specific
- The large import block at lines 1-31 needs to be distributed
- Re-export strategy for types that other crates might import from `public_domain_auth`

**Recommendation**: Add a section listing which crate imports belong in `common.rs` (e.g., `axum`, `axum_extra::extract::cookie`, `serde`, `time`, `uuid`, `AppState`, `AppError`, `jwt`).

### 2. Route Count Mismatch

The plan lists "Auth Routes (9 routes)" but the list shows 11 routes:
- `request-magic-link`
- `verify-magic-link`
- `google/start`
- `google/exchange`
- `google/confirm-link`
- `google/complete`
- `google/unlink`
- `session`
- `refresh`
- `logout`
- `account` (DELETE)

This is a minor documentation error but could cause confusion during implementation.

### 3. Missing `unlink_google` Handler Placement

The `unlink_google()` handler (lines 2487-2535) is listed under Google OAuth routes in the router, but the plan doesn't explicitly mention where it belongs. It should go in `google_oauth.rs`.

**Recommendation**: Explicitly list `unlink_google()` in Phase 5 (Google OAuth extraction).

### 4. `get_current_user` Location Ambiguity

The plan places `get_current_user()` in `common.rs`, but this helper:
- Uses JWT verification
- Is only used by billing routes and `unlink_google`

Consider whether it should stay in `common.rs` or go in a billing-specific location.

### 5. No Visibility (pub/pub(crate)) Strategy

The plan doesn't specify:
- What functions/types should be `pub` vs `pub(crate)` vs private
- How the public API should be exposed from `mod.rs`

**Recommendation**: Add a section specifying that only `router()` needs to be `pub` from the main `mod.rs`, and all internal helpers can be `pub(crate)` or private.

### 6. Duplicate Code Patterns

The file contains repeated cookie-clearing logic in multiple places:
- `logout()` (lines 634-664)
- `delete_account()` (lines 730-763)

These should be consolidated into a helper in `common.rs`.

**Recommendation**: Add a `clear_auth_cookies()` helper to `common.rs` during Phase 1.

### 7. `StripeMode` Entity Import Not Addressed

The plan mentions "StripeMode Import" as an edge case but doesn't explain where it comes from or how it will be handled. Looking at the code, webhooks use `PaymentMode` which is already imported.

**Recommendation**: Verify if `StripeMode` is actually used or if this is referring to `PaymentMode`.

---

## Suggested Improvements

### 1. Consolidate Cookie Logic

Create shared helpers in `common.rs`:
```rust
pub fn build_access_cookie(root_domain: &str, token: &str, ttl_secs: i64) -> Cookie<'static>
pub fn build_refresh_cookie(root_domain: &str, token: &str, ttl_days: i64) -> Cookie<'static>
pub fn build_email_cookie(root_domain: &str, email: &str, ttl_days: i64) -> Cookie<'static>
pub fn clear_auth_cookies(headers: &mut HeaderMap, root_domain: &str) -> Result<(), AppError>
```

This reduces duplication across `session.rs`, `magic_link.rs`, and `google_oauth.rs`.

### 2. Consider Merging `billing.rs` and `billing_payments.rs`

The split between "core billing" and "payments/plan changes" may be unnecessary complexity:
- `billing.rs`: 5 handlers (~200 lines)
- `billing_payments.rs`: 4 handlers (~150 lines)

Combined they'd still be under 400 lines, which is manageable. However, if the intent is to enable future expansion, keeping them separate is fine.

### 3. Add Pre-Commit Verification

Before committing, the plan should specify running:
```bash
./run api:lint
./run api:fmt
```

Not just `cargo check` and tests.

### 4. Consider `StripeWebhookMode` Enum Placement

The webhook handlers need the `PaymentMode` enum. Verify this import path is correct when extracted to `billing_webhooks.rs`.

### 5. Document Re-export Strategy

In the main `public_domain_auth/mod.rs`, specify what gets re-exported:
```rust
mod common;
mod config;
// ... other modules

pub use common::LoginCompletionResult; // if needed externally
pub fn router() -> Router<AppState> {
    config::router()
        .merge(magic_link::router())
        // ...
}
```

---

## Risks and Concerns

### 1. **Medium Risk**: Accidental Behavior Change

Even though this is "pure refactoring," subtle issues can arise:
- Different import paths might resolve to different types
- Visibility changes could break downstream code
- Test module visibility (`#[cfg(test)]`) needs careful handling

**Mitigation**: Run the full test suite (`./run api:test`) after each phase, not just `cargo check`.

### 2. **Low Risk**: Missing Use Case Imports

Several handlers import from `application::use_cases::domain_auth` inline (e.g., line 1056: `use crate::application::use_cases::domain_auth::GoogleLoginResult;`). These inline imports need to be moved to module-level imports or kept inline consistently.

**Mitigation**: Review all inline `use` statements and document their handling.

### 3. **Low Risk**: Test Discovery

The tests `oauth_exchange_tests` and `webhook_error_tests` use `super::*` imports. When moved:
- `oauth_exchange_tests` → `google_oauth.rs`: Should still work
- `webhook_error_tests` → `billing_webhooks.rs`: Needs `is_retryable_error` function

Verify the `is_retryable_error` helper (used by webhook tests) is in scope after the split.

### 4. **Low Risk**: Large Phase 5 (Google OAuth)

Phase 5 extracts ~500 lines including:
- 6 route handlers
- `OAuthExchangeError` enum with `impl` blocks
- Multiple helper functions
- 1 test module

This is the largest single phase. Consider verifying compilation at sub-steps:
1. First: Move types and helpers
2. Then: Move handlers
3. Finally: Move tests

---

## Summary

The plan is solid and well-researched. The main gaps are:

1. **Import management details** - needs explicit strategy
2. **Minor route count error** - 11 routes, not 9
3. **Cookie helper consolidation opportunity** - reduce duplication
4. **Visibility strategy** - what's `pub` vs `pub(crate)`

With these additions, the plan should execute smoothly. The 11-phase approach is appropriately cautious for a 3,300-line refactoring.

---

**Recommendation**: Address feedback items 1-5 from "What's Missing or Unclear" before implementation begins. The rest can be handled during implementation.
