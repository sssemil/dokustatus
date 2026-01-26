Integration Tests: Developer API

Add HTTP-level integration tests for developer API endpoints in `developer.rs`.
These endpoints require API key authentication.

Checklist
- [x] `POST /{domain}/auth/verify-token` - Verify JWT token
- [x] `GET /{domain}/users/{user_id}` - Get user details

Reference: `apps/api/src/adapters/http/routes/developer.rs`

History
- 2026-01-25 Created task for developer API integration tests.
- 2026-01-26 Implemented 10 tests covering both endpoints with API key middleware. Fixed InMemoryApiKeyRepo to use real SHA256 hashes for validate_api_key to work. All tests passing.
