Add HTTP client timeouts
Configure timeouts for Stripe and email clients.

Checklist
- [ ] Identify HTTP client constructors
- [ ] Add timeout/connect timeout to builders
- [ ] Confirm no callers rely on default client

History
- 2026-01-01 06:52 Created from code review finding #8 No request timeout on HTTP clients.
- 2026-01-01 06:55 Renamed file to 0008-http-client-timeouts.md to use 4-digit task numbering.
- 2026-01-01 07:00 Created plan-v1.md with detailed implementation approach. Identified 4 HTTP client locations: StripeClient, DomainEmailSender, and 2 inline clients in Google OAuth handlers.
- 2026-01-01 07:15 Created plan-v2.md addressing feedback: added discovery verification, shared http_client module, decided on OAuth approach, documented configurability decision.
- 2026-01-01 07:30 Created plan-v3.md addressing feedback: expanded discovery patterns, enhanced module documentation, added error logging, documented expected response times, clarified TLS/pooling concerns.
- 2026-01-01 12:32 Implemented shared HTTP client builder with timeouts and updated Stripe, Resend, and Google OAuth callers.
- 2026-01-01 12:46 Ran `./run api:build` and `./run api:test`; warnings only.
- 2026-01-01 12:47 Completed task; ready for outbound handoff.
