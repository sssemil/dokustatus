# Feedback: plan-v1 for 0007-oauth-state-two-phase

## What's good
- Clear statement of the bug and how the two-phase lifecycle addresses it.
- Concrete step-by-step edits with file paths and rough line anchors.
- Lua script design covers atomicity, retry window, and clock consistency; good note on TTL safety margin.
- Error classification and abort path show awareness of terminal vs retryable failures.
- Edge cases and rollback plan are explicit.

## What's missing or unclear
- **AppError mapping**: `OAuthRetryExpired` is introduced, but the plan does not specify where the error is created from `mark_state_in_use` (the Lua script returns a JSON error string). That mapping is implicit and brittle (string match). It needs a defined error path and response behavior for callers.
- **Retry-window behavior on the frontend**: The plan adds a 410 response, but doesn't mention updating UI/SDK to surface this as a “restart login” flow; otherwise users will see a generic error.
- **Status storage compatibility**: The Redis JSON payload format change adds `status` and `marked_at`. The plan doesn't confirm that all code paths that read state tolerate these fields or that `consume_state` (legacy) still works with the new payload.
- **Failure path after `parse_google_id_token`**: The plan aborts on parse errors, but doesn't specify whether downstream errors (e.g., DB failures during `find_or_create_user`) should allow retry or abort (likely retryable). This is a key behavioral decision.
- **Idempotency/cleanup**: `complete_state` only deletes when status is `in_use`. If a state is still `pending` due to a bug or a parallel path, it will remain. Plan should clarify if that's intended or if `complete_state` should delete regardless after success.
- **Testing details**: Unit test plan mentions an in-memory store, but doesn’t explain how to simulate Redis `TIME`/retry window or how to assert the error mapping for `OAuthRetryExpired`.

## Suggested improvements
- Replace the string sentinel (`value.contains("retry_window_expired")`) with a structured Lua return (e.g., `{ok=false, err="retry_window_expired"}`) and explicit error mapping in Rust.
- Define an explicit error for “state already in use but retry window expired” and ensure it maps to `AppError::OAuthRetryExpired` without string-matching.
- Clarify error classification for downstream failures after Google token parse (DB/network). Consider retryable for infra failures and abort for validation/user-state conflicts.
- Confirm and document how `consume_state` behaves with the new `status` field; if it deserializes, ensure serde defaults match and there is no schema mismatch.
- Add a UI/SDK note: when API returns `OAUTH_RETRY_EXPIRED`, restart the OAuth flow and discard any cached state.
- In tests, include a case that ensures `complete_state` is called only after successful user creation and that terminal errors call `abort_state`.

## Risks or concerns
- **String-matching error detection** could misclassify errors if OAuth state data contains similar text or Lua output changes; explicit error codes would be safer.
- **Retry window semantics** might allow two in-flight requests to proceed and both attempt user creation, risking duplicate/partial state if idempotency isn't guaranteed in downstream logic.
- **State lifecycle mismatch** if `complete_state` is too strict (only `in_use`) could leave orphaned states, which might be acceptable but should be intentional.
- **User experience**: Without UI handling for 410, users may be blocked without guidance.
