Integration Tests: Public Billing Endpoints

Add HTTP-level integration tests for public billing endpoints in `billing.rs`.

Checklist
- [ ] `GET /{domain}/billing/plans` - List public plans
- [ ] `GET /{domain}/billing/subscription` - Get user subscription
- [ ] `POST /{domain}/billing/checkout` - Create checkout session
- [ ] `POST /{domain}/billing/portal` - Create billing portal session
- [ ] `POST /{domain}/billing/cancel` - Cancel subscription
- [ ] `GET /{domain}/billing/payments` - Get user payments
- [ ] `GET /{domain}/billing/plan-change/preview` - Preview plan change
- [ ] `POST /{domain}/billing/plan-change` - Change plan
- [ ] `GET /{domain}/billing/providers` - Get available payment providers

Reference: `apps/api/src/adapters/http/routes/public_domain_auth/billing.rs`

History
- 2026-01-25 Created task for public billing integration tests.
