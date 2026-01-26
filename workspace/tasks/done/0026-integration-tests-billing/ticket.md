Integration Tests: Public Billing Endpoints

Add HTTP-level integration tests for public billing endpoints in `billing.rs`.

Checklist
- [x] `GET /{domain}/billing/plans` - List public plans
- [x] `GET /{domain}/billing/subscription` - Get user subscription
- [x] `POST /{domain}/billing/checkout` - Create checkout session (auth validation)
- [x] `POST /{domain}/billing/portal` - Create billing portal session (auth validation)
- [x] `POST /{domain}/billing/cancel` - Cancel subscription (auth validation)
- [x] `GET /{domain}/billing/payments` - Get user payments
- [x] `GET /{domain}/billing/plan-change/preview` - Preview plan change (auth validation)
- [x] `POST /{domain}/billing/plan-change` - Change plan (auth validation)
- [x] `GET /{domain}/billing/providers` - Get available payment providers

Note: Endpoints requiring Stripe integration have auth validation tests only. Full Stripe integration tests would require mocking the Stripe client.

Reference: `apps/api/src/adapters/http/routes/public_domain_auth/billing.rs`

History
- 2026-01-25 Created task for public billing integration tests.
- 2026-01-26 Implemented 13 tests covering all 9 endpoints: auth validation for Stripe-dependent endpoints, full tests for mockable endpoints. All tests passing.
