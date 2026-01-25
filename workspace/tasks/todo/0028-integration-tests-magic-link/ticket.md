Integration Tests: Magic Link Auth

Add HTTP-level integration tests for magic link authentication in `magic_link.rs`.

Checklist
- [ ] `POST /{domain}/auth/request-magic-link` - Request email magic link
- [ ] `POST /{domain}/auth/verify-magic-link` - Verify magic link token

Reference: `apps/api/src/adapters/http/routes/public_domain_auth/magic_link.rs`

History
- 2026-01-25 Created task for magic link integration tests.
