Fix Stripe restricted key mode detection
Ensure `rk_live_` keys are treated as live mode in StripeMode/PaymentMode.

Checklist
- [x] Review current key prefix handling and tests
- [x] Add `rk_live_` handling in StripeMode/PaymentMode
- [x] Update or add unit tests for restricted keys

History
- 2026-01-01 06:52 Created from code review finding #1 Restricted Stripe keys misclassified as test mode.
- 2026-01-01 06:55 Renamed task directory to 0001-fix-stripe-restricted-key-mode/ to use 4-digit task numbering (ticket remains ticket.md).
- 2026-01-01 06:57 Updated Stripe/Payment mode detection to treat rk_live_ as live and added validation coverage.
- 2026-01-01 06:58 Added rk_test_/rk_live_ validation coverage for Stripe/Payment modes and clarified task history wording.
- 2026-01-01 06:59 Completed restricted key mode fix with updated validation coverage; ran `cargo test stripe_mode` and `cargo test payment_mode`.
