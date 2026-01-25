Integration Tests: Billing Webhooks

Add HTTP-level integration tests for Stripe webhook handlers in `billing_webhooks.rs`.

Checklist
- [ ] `POST /{domain}/billing/webhook/test` - Test mode webhook handler
- [ ] `POST /{domain}/billing/webhook/live` - Live mode webhook handler

Reference: `apps/api/src/adapters/http/routes/public_domain_auth/billing_webhooks.rs`

History
- 2026-01-25 Created task for billing webhook integration tests.
