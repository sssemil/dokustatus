Redact sensitive Stripe error logs
Avoid logging full Stripe response bodies.

Checklist
- [ ] Find Stripe API error logging
- [ ] Redact or truncate body fields
- [ ] Verify logs still useful for debugging

History
- 2026-01-01 06:52 Created from code review finding #13 Sensitive data in error logs.
- 2026-01-01 06:55 Renamed file to 0013-redact-stripe-error-logs.md to use 4-digit task numbering.
