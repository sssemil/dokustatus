Plan: Fix Stripe restricted key mode detection

Summary
- Update StripeMode/PaymentMode key prefix detection to treat `rk_live_` as live.
- Adjust unit tests to cover restricted live keys and validate error messages.

Step-by-step implementation approach
1. Review `StripeMode::from_key_prefix` and `PaymentMode::from_stripe_key_prefix` to confirm current restricted-key handling.
2. Update the prefix detection logic to classify `rk_live_` as live while keeping `rk_test_` as test.
3. Update unit tests in each module to assert the new `rk_live_` behavior and keep coverage for `rk_test_`.
4. Re-run tests or at minimum run the module tests to ensure behavior matches expectations.

Files to modify
- `apps/api/src/domain/entities/stripe_mode.rs`
- `apps/api/src/domain/entities/payment_mode.rs`

Testing approach
- Run targeted Rust tests for Stripe/Payment mode detection (e.g., `cargo test` for the two modules or full `./run api:test` if available).
- Ensure tests cover `rk_live_` and `rk_test_` prefixes alongside existing `sk_`/`pk_` coverage.

Edge cases to handle
- Unknown or malformed prefixes should continue to default to test mode.
- Case sensitivity: current logic is prefix-based and case-sensitive; do not alter unless required.
- Validation errors should report the detected mode accurately after the change.

Revision (2026-01-01 04:35 UTC)
- Feedback noted no changes needed; plan remains as-is.

Revision (2026-01-01 04:36 UTC)
- Reviewed iteration 2/3 feedback; no changes required.
