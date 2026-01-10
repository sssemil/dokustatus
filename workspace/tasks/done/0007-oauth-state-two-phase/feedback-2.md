# Feedback 2: Plan v2 Review

## Whats good about the plan
- Clear rationale for two-phase state lifecycle and retry window.
- Redis Lua script uses `TIME` and structured return codes, which is safer than string matching.
- Backward compatibility callout with serde defaults is explicit.
- UI/SDK section acknowledges 410 handling and user restart flow.
- Tests cover happy path, retry window, expiration, and backward-compat deserialization.
- Concurrency section shows awareness of dual in-flight requests and idempotent user creation.

## Whats missing or unclear
- Error classification still relies on string matching in `classify_oauth_error`, which conflicts with the goal of structured handling and is brittle.
- Retry window vs Redis TTL is not specified; if TTL is shorter than 90s, retries can fail unexpectedly.
- The `completed` status is checked in Lua but never set in the plan; unclear if it is needed.
- The plan does not specify what to do when `mark_state_in_use` returns `RetryWindowExpired` beyond returning 410; should the state be deleted?
- Behavior when `complete_state` fails is not addressed; the current flow returns an error after a successful login.
- The plan does not state how to detect retryable Google errors (e.g., HTTP status codes) without string matching.

## Suggested improvements
- Replace string matching with typed error variants or explicit error codes from Google/HTTP; map these to retryable vs terminal.
- Ensure TTL is always >= retry window and document it, or refresh TTL on `mark_state_in_use` to guarantee retries.
- Either remove the `completed` status check in Lua or explicitly set `status = 'completed'` before delete if you want to preserve it for observability.
- On `RetryWindowExpired`, consider calling `abort_state` (delete) to clear stale in-use states and make restart clean.
- Treat `complete_state` failure as best-effort: log and continue returning success to avoid post-success login errors.
- Add tests for the retry-expired path returning 410 and for the error classification mapping.

## Risks or concerns
- String-matching errors can misclassify retryable failures as terminal (or vice versa), leading to confusing UX.
- If `complete_state` errors after a successful user creation, the response may fail even though the user exists, causing duplicate attempts.
- If TTL < retry window, users can see 410 even within the expected retry period.
- A fixed 90s window might be too short for some users; if this is a product decision, call it out explicitly.

