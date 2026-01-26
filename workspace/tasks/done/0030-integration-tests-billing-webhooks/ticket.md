Integration Tests: Billing Webhooks

Add HTTP-level integration tests for Stripe webhook handlers in `billing_webhooks.rs`.

Checklist
- [x] `POST /{domain}/billing/webhook/test` - Test mode webhook handler
- [x] `POST /{domain}/billing/webhook/live` - Live mode webhook handler

Reference: `apps/api/src/adapters/http/routes/public_domain_auth/billing_webhooks.rs`

History
- 2026-01-25 Created task for billing webhook integration tests.
- 2026-01-26 Implemented 6 tests covering both webhook endpoints:
  - webhook_test_unknown_domain_returns_404
  - webhook_test_unverified_domain_returns_400
  - webhook_test_no_stripe_config_returns_400
  - webhook_test_missing_signature_returns_400
  - webhook_live_unknown_domain_returns_404
  - webhook_live_no_stripe_config_returns_400
  Note: Actual webhook processing requires Stripe signature verification
  and Stripe API calls, which can't be tested without external dependencies.
