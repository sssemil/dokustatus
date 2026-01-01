# Feedback on Plan v3 - Redact Stripe Error Logs

**Reviewer:** Claude
**Date:** 2026-01-01

---

## What's Good

1. **Thorough verification of assumptions**: The plan correctly verified the `StripeErrorResponse` struct definition (lines 848-858) and confirmed field types. This prevents implementation surprises.

2. **Clear before/after code examples**: Each modification has explicit before and after code snippets with line numbers, making implementation straightforward.

3. **Comprehensive edge cases table**: Covers empty bodies, HTML error pages, non-Stripe JSON, and PII scenarios. The test cases align with these edge cases.

4. **Security-focused doc comment**: The `redact_stripe_body` function has a clear security rationale explaining why it exists.

5. **Test for PII non-leakage**: Test case `test_redact_stripe_body_non_error_json` explicitly verifies that PII (email) is NOT present in output - this is the right defensive test.

6. **Debugging sufficiency section**: Good acknowledgment that developers still get actionable info (type, code, message, status).

7. **Out-of-scope items documented**: Request-Id header enhancement is deferred with clear rationale rather than scope-creeping.

---

## What's Missing or Unclear

1. **No verification of actual line 606 context**: The plan states line 606 contains the log statement, but doesn't confirm what function it's inside or whether there might be other callers of `handle_response` that could bypass this change.

2. **No search for other `body` logging**: The plan focuses on `handle_response` but doesn't confirm there are no other places in the codebase that log Stripe response bodies (e.g., webhook handlers, other client methods).

3. **AppError::Internal propagation**: The plan changes the error message format but doesn't discuss whether this error ever surfaces to users via API responses. If so, even the redacted format could leak information (error type, message).

4. **Test location unspecified**: Plan says "after the existing webhook signature tests" but doesn't give a line number. Would be clearer with approximate line range.

5. **No mention of `./run db:prepare`**: If the change affects offline SQLx compilation, this should be noted. Likely not relevant here since it's just logging, but worth confirming.

---

## Suggested Improvements

1. **Verify no other Stripe body logging exists**:
   ```bash
   rg -n "body.*stripe" --ignore-case apps/api/src/
   rg -n "tracing::.*(body|response)" apps/api/src/infra/stripe_client.rs
   ```
   Add results to the plan to confirm scope is complete.

2. **Check AppError::Internal exposure**: Search for how `AppError::Internal` is rendered in HTTP responses. If the message is returned to clients, even redacted Stripe error details could be problematic. Consider:
   ```rust
   return Err(AppError::Internal("Stripe API error".to_string()));
   ```
   And log the details separately if they shouldn't reach clients.

3. **Add line number for test insertion point**: Specify where tests go (e.g., "after line 920" or similar).

4. **Consider `Request-Id` in this PR**: The plan defers it, but extracting the header before `response.text().await?` is a small change that would significantly improve debuggability. Worth reconsidering for minimal extra effort:
   ```rust
   let request_id = response.headers().get("request-id").map(|v| v.to_str().ok()).flatten();
   let body = response.text().await?;
   tracing::error!(status = %status, request_id = ?request_id, error_details = %redact_stripe_body(&body), "Stripe API error");
   ```

5. **Consider truncating message field**: The `message` field from Stripe sometimes includes customer-provided data (e.g., in certain validation errors). Consider truncating to first 100 chars:
   ```rust
   error.error.message.as_ref().map(|m| if m.len() > 100 { &m[..100] } else { m })
   ```

---

## Risks or Concerns

1. **False sense of security**: The plan focuses on `handle_response` but Stripe responses could be logged elsewhere (webhook handlers, debug statements added during development). A codebase-wide audit would be more thorough.

2. **Error message as API response**: If `AppError::Internal` messages surface in HTTP responses, the change from raw body to `type=X, code=Y, message=Z` could still leak information. Need to verify error handling middleware.

3. **Stripe message field PII risk**: Stripe error messages sometimes echo back user input (e.g., "The email address 'user@example.com' is invalid"). The plan logs this message unredacted. Low probability but worth noting.

4. **Unicode edge case**: `body.len()` returns byte count, not character count. For non-ASCII responses, this is technically correct but could be confusing in logs. Minor issue.

---

## Summary

Plan v3 is well-structured and ready for implementation with minor adjustments. The main actionable items before proceeding:

1. **Confirm scope**: Run a quick grep to verify no other Stripe body logging exists
2. **Check error propagation**: Verify `AppError::Internal` messages don't reach API clients
3. **Consider Request-Id**: Small effort, big debugging win

Overall: **Approved with suggestions**. The plan adequately addresses the security concern while maintaining debuggability.
