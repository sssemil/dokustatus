Integration Tests: Magic Link Auth

Add HTTP-level integration tests for magic link authentication in `magic_link.rs`.

Checklist
- [x] `POST /{domain}/auth/request-magic-link` - Request email magic link
- [x] `POST /{domain}/auth/verify-magic-link` - Verify magic link token

Reference: `apps/api/src/adapters/http/routes/public_domain_auth/magic_link.rs`

History
- 2026-01-25 Created task for magic link integration tests.
- 2026-01-26 Implemented 9 tests covering both endpoints:
  - request_magic_link_invalid_email_returns_400
  - request_magic_link_unknown_domain_returns_404
  - request_magic_link_unverified_domain_returns_404
  - request_magic_link_magic_link_disabled_returns_400
  - request_magic_link_success_sends_email_and_returns_202
  - request_magic_link_trims_email_whitespace
  - verify_magic_link_no_session_cookie_returns_session_mismatch
  - verify_magic_link_invalid_token_returns_401
  - verify_magic_link_session_mismatch_on_different_session
  Also added InMemoryMagicLinkStore and InMemoryEmailSender to test_utils.
