Return failure for webhook sync errors
Ensure Stripe retries when webhook processing fails.

Checklist
- [x] Review webhook handlers that warn-only
- [x] Return error response on sync failure
- [x] Consider retry/queue strategy (using Stripe's built-in retry mechanism)

History
- 2026-01-01 06:52 Created from code review finding #10 Webhook failures silently swallowed.
- 2026-01-01 06:55 Renamed file to 0010-webhook-failure-response.md to use 4-digit task numbering.
- 2026-01-01 07:15 Created plan-v1.md with detailed implementation approach. Identified 7 webhook event handlers that swallow errors; plan categorizes errors as retryable (DB/internal) vs non-retryable (not found/validation) to enable Stripe's built-in retry mechanism.
- 2026-01-01 07:30 Revised plan-v1.md after Codex review. Addressed feedback: verified typed AppError enum (not anyhow), confirmed idempotency protection via is_event_processed and upsert patterns, added event classification table (must-succeed vs best-effort), added unit test requirement for is_retryable_error helper.
- 2026-01-01 12:56 Added plan review feedback in feedback-1.md.
- 2026-01-01 13:15 Created plan-v2.md addressing feedback. Key changes: explicit 500 return for retryable errors (RateLimited maps to 429), verified idempotency timing is correct, added `_ => true` default arm, strengthened logging with required fields, configuration error handling.
- 2026-01-01 Created plan-v3.md (final revision) addressing feedback-2.md. Verified return type compatibility ((StatusCode, &str) implements IntoResponse), confirmed event_id/event_type are in scope at lines 1763-1764, fixed log level inconsistency (error! for critical, debug! for expected), added retryable:bool field to structured logs, added Stripe retry schedule reference.
- 2026-01-01 14:XX Implementation complete. Changes:
  - Added `is_retryable_error()` helper to categorize AppError variants as retryable (Database, Internal, RateLimited) vs non-retryable (NotFound, InvalidInput, etc.)
  - Added `_ => true` catch-all for unknown/new error variants (safer to retry)
  - Added `webhook_retryable_error()` helper that logs with structured fields (event_id, event_type, context, retryable=true) and returns 500
  - Refactored 5 payment status update handlers (invoice.payment_failed, invoice.voided, invoice.marked_uncollectible, charge.refunded, charge.failed) to use retryable error handling
  - Added 6 unit tests for `is_retryable_error()` covering all known AppError variants
  - Build, lint, and tests pass (61 tests total)
- 2026-01-01 15:00 Task complete. Verified implementation: 10 webhook handlers now return 500 for retryable errors (fetch subscription, lookup plan, create subscription, update subscription, sync invoice, and 5 payment status updates). All tests pass. Moving to outbound for merge.
