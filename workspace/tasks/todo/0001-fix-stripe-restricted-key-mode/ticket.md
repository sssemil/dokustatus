Fix Stripe restricted key mode detection
Ensure `rk_live_` keys are treated as live mode in StripeMode/PaymentMode.

Checklist
- [ ] Review current key prefix handling and tests
- [ ] Add `rk_live_` handling in StripeMode/PaymentMode
- [ ] Update or add unit tests for restricted keys

History
- 2026-01-01 06:52 Created from code review finding #1 Restricted Stripe keys misclassified as test mode.
- 2026-01-01 06:55 Renamed file to 0001-fix-stripe-restricted-key-mode.md to use 4-digit task numbering.
