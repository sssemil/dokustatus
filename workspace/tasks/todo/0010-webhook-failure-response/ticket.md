Return failure for webhook sync errors
Ensure Stripe retries when webhook processing fails.

Checklist
- [ ] Review webhook handlers that warn-only
- [ ] Return error response on sync failure
- [ ] Consider retry/queue strategy

History
- 2026-01-01 06:52 Created from code review finding #10 Webhook failures silently swallowed.
- 2026-01-01 06:55 Renamed file to 0010-webhook-failure-response.md to use 4-digit task numbering.
