Integration Tests: Developer API

Add HTTP-level integration tests for developer API endpoints in `developer.rs`.
These endpoints require API key authentication.

Checklist
- [ ] `POST /{domain}/auth/verify-token` - Verify JWT token
- [ ] `GET /{domain}/users/{user_id}` - Get user details

Reference: `apps/api/src/adapters/http/routes/developer.rs`

History
- 2026-01-25 Created task for developer API integration tests.
