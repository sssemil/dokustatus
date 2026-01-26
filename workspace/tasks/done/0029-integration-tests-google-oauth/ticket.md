Integration Tests: Google OAuth

Add HTTP-level integration tests for Google OAuth endpoints in `google_oauth.rs`.

Checklist
- [x] `POST /{domain}/auth/google/start` - Start OAuth flow
- [x] `POST /{domain}/auth/google/exchange` - Exchange OAuth code
- [x] `POST /{domain}/auth/google/confirm-link` - Confirm account linking
- [x] `POST /{domain}/auth/google/complete` - Complete OAuth flow
- [x] `POST /{domain}/auth/google/unlink` - Unlink account

Reference: `apps/api/src/adapters/http/routes/public_domain_auth/google_oauth.rs`

History
- 2026-01-25 Created task for Google OAuth integration tests.
- 2026-01-26 Implemented 10 tests covering all 5 endpoints:
  - google_start_unknown_domain_returns_404
  - google_start_unverified_domain_returns_404
  - google_start_oauth_disabled_returns_400
  - google_start_success_returns_state_and_auth_url
  - google_exchange_invalid_state_returns_400
  - google_confirm_link_invalid_token_returns_400
  - google_complete_invalid_token_returns_400
  - google_unlink_no_auth_returns_401
  - google_unlink_invalid_token_returns_401
  - google_unlink_with_valid_auth_succeeds
  Also added InMemoryOAuthStateStore to test_utils for OAuth state management.
