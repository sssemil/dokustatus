Integration Tests: Dummy Billing Provider

Add HTTP-level integration tests for dummy/test payment provider in `billing_dummy.rs`.

Checklist
- [ ] `POST /{domain}/billing/checkout/dummy` - Create dummy checkout
- [ ] `POST /{domain}/billing/dummy/confirm` - Confirm dummy checkout
- [ ] `GET /{domain}/billing/dummy/scenarios` - Get test scenarios

Reference: `apps/api/src/adapters/http/routes/public_domain_auth/billing_dummy.rs`

History
- 2026-01-25 Created task for dummy billing integration tests.
