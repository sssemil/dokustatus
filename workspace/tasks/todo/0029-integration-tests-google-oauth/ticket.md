Integration Tests: Google OAuth

Add HTTP-level integration tests for Google OAuth endpoints in `google_oauth.rs`.

Checklist
- [ ] `POST /{domain}/auth/google/start` - Start OAuth flow
- [ ] `POST /{domain}/auth/google/exchange` - Exchange OAuth code
- [ ] `POST /{domain}/auth/google/confirm-link` - Confirm account linking
- [ ] `POST /{domain}/auth/google/complete` - Complete OAuth flow
- [ ] `POST /{domain}/auth/google/unlink` - Unlink account

Reference: `apps/api/src/adapters/http/routes/public_domain_auth/google_oauth.rs`

History
- 2026-01-25 Created task for Google OAuth integration tests.
