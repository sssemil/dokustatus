Redact sensitive Stripe error logs
Avoid logging full Stripe response bodies.

Checklist
- [x] Find Stripe API error logging
- [x] Redact or truncate body fields
- [x] Verify logs still useful for debugging

History
- 2026-01-01 06:52 Created from code review finding #13 Sensitive data in error logs.
- 2026-01-01 06:55 Renamed file to 0013-redact-stripe-error-logs.md to use 4-digit task numbering.
- 2026-01-01 07:15 Created plan-v1.md: identified `handle_response` in `stripe_client.rs` as the source (lines 606, 619-622, 626). Plan adds a `redact_stripe_body` helper to extract only error type/code/message while omitting raw body content.
- 2026-01-01 07:45 Created plan-v2.md addressing feedback: verified no other Stripe logging locations exist (searched codebase), decided Request-Id header is out of scope (requires refactoring), added doc comment, expanded test cases to 5, added tables for edge cases and risks.
- 2026-01-01 08:30 Created plan-v3.md (final revision): verified StripeErrorResponse struct at lines 848-858 matches expected field names/types, added fmt step to testing, confirmed line numbers against current file state, added inline comment explaining byte count fallback, enhanced non-error JSON test to verify PII is not leaked.
- 2026-01-01 09:15 Implementation started. Added `redact_stripe_body` helper function in `stripe_client.rs` that extracts only safe debugging info (error type, code, message) from Stripe error responses, or logs byte count for non-error/invalid JSON.
- 2026-01-01 09:20 Updated `handle_response` method: replaced raw body logging with `error_details` field using redaction helper; replaced `body` in parse failure log with `body_len`; sanitized `AppError::Internal` message to use redacted body.
- 2026-01-01 09:25 Added 5 unit tests for redact_stripe_body: valid error, minimal error, invalid JSON, empty body, and non-error JSON (PII protection).
- 2026-01-01 09:30 Verification complete: `./run api:fmt`, `./run api:lint`, `./run api:test` (68 tests pass), `./run api:build` all succeeded.
- 2026-01-01 09:35 Task complete. Committed as 7ab1f5a. Moving to outbound for merge.
