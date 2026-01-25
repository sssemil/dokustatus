Integration Tests: Dashboard Domain Routes

Add HTTP-level integration tests for dashboard domain admin endpoints in `domain.rs`.

## Domain CRUD
- [ ] `POST /` - Create domain
- [ ] `GET /` - List domains
- [ ] `GET /stats` - Get usage stats
- [ ] `GET /check-allowed` - Check if domain is allowed (Caddy on-demand TLS)
- [ ] `GET /{domain_id}` - Get domain
- [ ] `POST /{domain_id}/verify` - Start verification
- [ ] `GET /{domain_id}/status` - Get verification status
- [ ] `DELETE /{domain_id}` - Delete domain

## Auth Config
- [ ] `GET /{domain_id}/auth-config` - Get auth config
- [ ] `PATCH /{domain_id}/auth-config` - Update auth config
- [ ] `DELETE /{domain_id}/auth-config/magic-link` - Delete magic link config
- [ ] `DELETE /{domain_id}/auth-config/google-oauth` - Delete Google OAuth config

## End Users
- [ ] `GET /{domain_id}/end-users` - List end users
- [ ] `POST /{domain_id}/end-users/invite` - Invite end user
- [ ] `GET /{domain_id}/end-users/{user_id}` - Get end user
- [ ] `DELETE /{domain_id}/end-users/{user_id}` - Delete end user
- [ ] `POST /{domain_id}/end-users/{user_id}/freeze` - Freeze end user
- [ ] `DELETE /{domain_id}/end-users/{user_id}/freeze` - Unfreeze end user
- [ ] `POST /{domain_id}/end-users/{user_id}/whitelist` - Whitelist end user
- [ ] `DELETE /{domain_id}/end-users/{user_id}/whitelist` - Unwhitelist end user
- [ ] `PUT /{domain_id}/end-users/{user_id}/roles` - Set user roles

## API Keys
- [ ] `GET /{domain_id}/api-keys` - List API keys
- [ ] `POST /{domain_id}/api-keys` - Create API key
- [ ] `DELETE /{domain_id}/api-keys/{key_id}` - Revoke API key

## Roles
- [ ] `GET /{domain_id}/roles` - List roles
- [ ] `POST /{domain_id}/roles` - Create role
- [ ] `DELETE /{domain_id}/roles/{role_name}` - Delete role
- [ ] `GET /{domain_id}/roles/{role_name}/user-count` - Get role user count

## Admin Billing
- [ ] `GET /{domain_id}/billing/config` - Get billing config
- [ ] `PATCH /{domain_id}/billing/config` - Update billing config
- [ ] `DELETE /{domain_id}/billing/config` - Delete billing config
- [ ] `PATCH /{domain_id}/billing/mode` - Set billing mode
- [ ] `GET /{domain_id}/billing/plans` - List billing plans
- [ ] `POST /{domain_id}/billing/plans` - Create billing plan
- [ ] `PUT /{domain_id}/billing/plans/reorder` - Reorder billing plans
- [ ] `PATCH /{domain_id}/billing/plans/{plan_id}` - Update billing plan
- [ ] `DELETE /{domain_id}/billing/plans/{plan_id}` - Archive billing plan
- [ ] `GET /{domain_id}/billing/subscribers` - List billing subscribers
- [ ] `POST /{domain_id}/billing/subscribers/{user_id}/grant` - Grant subscription
- [ ] `DELETE /{domain_id}/billing/subscribers/{user_id}/revoke` - Revoke subscription
- [ ] `GET /{domain_id}/billing/analytics` - Get billing analytics
- [ ] `GET /{domain_id}/billing/payments` - List billing payments
- [ ] `GET /{domain_id}/billing/payments/export` - Export billing payments

## Payment Providers
- [ ] `GET /{domain_id}/billing/providers` - List billing providers
- [ ] `POST /{domain_id}/billing/providers` - Enable billing provider
- [ ] `DELETE /{domain_id}/billing/providers/{provider}/{mode}` - Disable billing provider
- [ ] `PATCH /{domain_id}/billing/providers/{provider}/{mode}/active` - Set provider active

Reference: `apps/api/src/adapters/http/routes/domain.rs`

History
- 2026-01-25 Created task for dashboard domain integration tests.
