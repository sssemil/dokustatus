Integration Tests: Dashboard User Routes

Add HTTP-level integration tests for dashboard user endpoints in `user.rs`.

Checklist
- [x] `GET /api/user/me` - Get current user
- [x] `DELETE /api/user/delete` - Delete account

Reference: `apps/api/src/adapters/http/routes/user.rs`

History
- 2026-01-25 Created task for dashboard user integration tests.
- 2026-01-26 Completed. Added 6 tests:
  - `get_me_no_auth_returns_401` - auth required
  - `get_me_returns_user_info` - returns email and roles
  - `get_me_non_main_domain_returns_401` - rejects non-dashboard users
  - `delete_account_no_auth_returns_401` - auth required
  - `delete_account_clears_cookies` - verifies all 4 cookies cleared
  - `delete_account_non_main_domain_returns_401` - rejects non-dashboard users
- Added `with_main_domain()` method to TestAppStateBuilder for testing main domain access checks.
