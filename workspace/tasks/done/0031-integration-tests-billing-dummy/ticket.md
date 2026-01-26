Integration Tests: Dummy Billing Provider

Add HTTP-level integration tests for dummy/test payment provider in `billing_dummy.rs`.

Checklist
- [x] `POST /{domain}/billing/checkout/dummy` - Create dummy checkout
- [x] `POST /{domain}/billing/dummy/confirm` - Confirm dummy checkout
- [x] `GET /{domain}/billing/dummy/scenarios` - Get test scenarios

Reference: `apps/api/src/adapters/http/routes/public_domain_auth/billing_dummy.rs`

History
- 2026-01-25 Created task for dummy billing integration tests.
- 2026-01-26 Completed. Added 4 tests:
  - `get_scenarios_returns_all_scenarios` - verifies all 6 payment scenarios returned
  - `checkout_dummy_no_auth_returns_401` - auth required for checkout
  - `confirm_dummy_no_auth_returns_401` - auth required for confirm
  - `confirm_dummy_invalid_token_returns_400` - validates token format
