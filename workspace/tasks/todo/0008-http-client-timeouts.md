Add HTTP client timeouts
Configure timeouts for Stripe and email clients.

Checklist
- [ ] Identify HTTP client constructors
- [ ] Add timeout/connect timeout to builders
- [ ] Confirm no callers rely on default client

History
- 2026-01-01 06:52 Created from code review finding #8 No request timeout on HTTP clients.
- 2026-01-01 06:55 Renamed file to 0008-http-client-timeouts.md to use 4-digit task numbering.
