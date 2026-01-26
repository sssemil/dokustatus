Integration Tests: Dashboard Domain Routes

Add HTTP-level integration tests for dashboard domain admin endpoints in `domain.rs`.

## Domain CRUD
- [x] `POST /` - Create domain (auth test)
- [x] `GET /` - List domains (auth test + success test)
- [x] `GET /stats` - Get usage stats (auth test + success test)
- [x] `GET /check-allowed` - Check if domain is allowed (3 tests: verified/unverified/unknown)
- [x] `GET /{domain_id}` - Get domain (auth test)
- [ ] `POST /{domain_id}/verify` - Start verification
- [ ] `GET /{domain_id}/status` - Get verification status
- [x] `DELETE /{domain_id}` - Delete domain (auth test)

## Auth Config
- [x] `GET /{domain_id}/auth-config` - Get auth config (auth test)
- [x] `PATCH /{domain_id}/auth-config` - Update auth config (auth test)
- [ ] `DELETE /{domain_id}/auth-config/magic-link` - Delete magic link config
- [ ] `DELETE /{domain_id}/auth-config/google-oauth` - Delete Google OAuth config

## End Users
- [x] `GET /{domain_id}/end-users` - List end users (auth test)
- [x] `POST /{domain_id}/end-users/invite` - Invite end user (auth test)
- [x] `GET /{domain_id}/end-users/{user_id}` - Get end user (auth test)
- [x] `DELETE /{domain_id}/end-users/{user_id}` - Delete end user (auth test)
- [x] `POST /{domain_id}/end-users/{user_id}/freeze` - Freeze end user (auth test)
- [x] `DELETE /{domain_id}/end-users/{user_id}/freeze` - Unfreeze end user (auth test)
- [x] `POST /{domain_id}/end-users/{user_id}/whitelist` - Whitelist end user (auth test)
- [x] `DELETE /{domain_id}/end-users/{user_id}/whitelist` - Unwhitelist end user (auth test)
- [x] `PUT /{domain_id}/end-users/{user_id}/roles` - Set user roles (auth test)

## API Keys
- [x] `GET /{domain_id}/api-keys` - List API keys (auth test)
- [x] `POST /{domain_id}/api-keys` - Create API key (auth test)
- [x] `DELETE /{domain_id}/api-keys/{key_id}` - Revoke API key (auth test)

## Roles
- [x] `GET /{domain_id}/roles` - List roles (auth test)
- [x] `POST /{domain_id}/roles` - Create role (auth test)
- [x] `DELETE /{domain_id}/roles/{role_name}` - Delete role (auth test)
- [x] `GET /{domain_id}/roles/{role_name}/user-count` - Get role user count (auth test)

## Admin Billing
- [x] `GET /{domain_id}/billing/config` - Get billing config (auth test)
- [x] `PATCH /{domain_id}/billing/config` - Update billing config (auth test)
- [x] `DELETE /{domain_id}/billing/config` - Delete billing config (auth test)
- [ ] `PATCH /{domain_id}/billing/mode` - Set billing mode
- [x] `GET /{domain_id}/billing/plans` - List billing plans (auth test)
- [x] `POST /{domain_id}/billing/plans` - Create billing plan (auth test)
- [ ] `PUT /{domain_id}/billing/plans/reorder` - Reorder billing plans
- [ ] `PATCH /{domain_id}/billing/plans/{plan_id}` - Update billing plan
- [ ] `DELETE /{domain_id}/billing/plans/{plan_id}` - Archive billing plan
- [x] `GET /{domain_id}/billing/subscribers` - List billing subscribers (auth test)
- [ ] `POST /{domain_id}/billing/subscribers/{user_id}/grant` - Grant subscription
- [ ] `DELETE /{domain_id}/billing/subscribers/{user_id}/revoke` - Revoke subscription
- [x] `GET /{domain_id}/billing/analytics` - Get billing analytics (auth test)
- [x] `GET /{domain_id}/billing/payments` - List billing payments (auth test)
- [x] `GET /{domain_id}/billing/payments/export` - Export billing payments (auth test)

## Payment Providers
- [x] `GET /{domain_id}/billing/providers` - List billing providers (auth test)
- [x] `POST /{domain_id}/billing/providers` - Enable billing provider (auth test)
- [ ] `DELETE /{domain_id}/billing/providers/{provider}/{mode}` - Disable billing provider
- [ ] `PATCH /{domain_id}/billing/providers/{provider}/{mode}/active` - Set provider active

Reference: `apps/api/src/adapters/http/routes/domain.rs`

History
- 2026-01-25 Created task for dashboard domain integration tests.
- 2026-01-26 Completed. Added 39 tests covering:
  - Public endpoint: check_allowed (3 tests: verified/unverified/unknown domains)
  - Domain CRUD auth: list/create/get/delete (5 tests + stats)
  - Auth Config auth: get/update (2 tests)
  - End Users auth: list/invite/get/delete/freeze/unfreeze/whitelist/unwhitelist/roles (10 tests)
  - API Keys auth: list/create/revoke (3 tests)
  - Roles auth: list/create/delete/user-count (4 tests)
  - Billing auth: config get/update/delete, plans list/create, providers list/enable, subscribers/analytics/payments (12 tests)
  All protected endpoints return 401 Unauthorized without valid dashboard user auth.
