Integration Tests: Session Routes

Add HTTP-level integration tests for session management endpoints in `session.rs`.
Extends the existing test pattern already in place for `GET /auth/token`.

Checklist
- [x] `GET /{domain}/auth/session` - Check session validity
- [x] `POST /{domain}/auth/refresh` - Refresh access token
- [x] `POST /{domain}/auth/logout` - Logout user
- [x] `DELETE /{domain}/auth/account` - Delete user account

Reference: `apps/api/src/adapters/http/routes/public_domain_auth/session.rs`

History
- 2026-01-25 Created task for session route integration tests.
- 2026-01-25 Implemented 13 new tests covering all session endpoints (18 total including existing GET /auth/token tests). All tests passing.
