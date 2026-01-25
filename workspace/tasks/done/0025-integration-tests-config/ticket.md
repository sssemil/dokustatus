Integration Tests: Domain Config

Add HTTP-level integration tests for domain config endpoint in `config.rs`.

Checklist
- [x] `GET /{domain}/config` - Get domain configuration

Reference: `apps/api/src/adapters/http/routes/public_domain_auth/config.rs`

History
- 2026-01-25 Created task for config route integration tests.
- 2026-01-25 Implemented 5 tests: unknown domain, unverified domain, default config, configured auth methods, hostname extraction. All tests passing.
