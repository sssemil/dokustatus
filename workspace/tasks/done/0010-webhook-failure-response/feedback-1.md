# Plan Review Feedback: 0010-webhook-failure-response

## What's good about the plan

- Clear identification of the exact warning-only branches and the specific line ranges in `public_domain_auth.rs`, which makes the refactor scope concrete.
- Thoughtful separation of **retryable vs non-retryable** errors and **must-succeed vs best-effort** events; this makes the response policy explicit.
- Idempotency verification is called out with evidence (event-level check + upserts), reducing the risk of duplicate side effects when Stripe retries.
- Includes both unit tests for the new helper and a manual verification path using the Stripe CLI.
- Notes out-of-scope items (DLQ, replay, alerting) so the plan is intentionally scoped.

## What's missing or unclear

- **HTTP status mapping for retryable errors**: the plan returns `Err(e)` for retryable cases, but it is unclear whether `AppError::RateLimited` (or other variants) maps to a 4xx status that would *prevent* Stripe retries. The plan should confirm/override the HTTP status for retryable errors.
- **`is_event_processed` placement**: the plan confirms the check exists, but does not verify *when* events are marked as processed. If the event is marked before all critical operations succeed, a 500 retry would still be skipped.
- **`AppError` completeness**: the plan shows a partial match list yet states “Others default to retryable.” The proposed helper does not include `_ => true`, so it’s unclear how new or currently unlisted variants (if any) should behave.
- **Plan lookup failure handling**: `Ok(None)` for `get_plan_by_stripe_price_id` is treated as non-retryable and returns 200. It might be a configuration issue that should at least be surfaced more aggressively (error log/alert) rather than silently skipped.

## Suggested improvements

- Add a short step that *confirms or enforces* that retryable errors yield a 5xx. If `AppError::RateLimited` maps to 429 or other 4xx, consider returning `StatusCode::INTERNAL_SERVER_ERROR` explicitly for retryable cases in this handler.
- Explicitly check where `mark_event_processed` (or equivalent) occurs in the handler. Ensure the event is only marked as processed **after** critical operations succeed; otherwise, retries will no-op.
- Decide and document whether `is_retryable_error` is **exhaustive** or includes a `_ => true` default. If you want “unknowns retryable,” add the default arm and update tests accordingly.
- Strengthen the logging plan: include `event_id`, `event_type`, and the affected object identifiers in all error logs to make retries and failures traceable.

## Risks or concerns

- **Accidental non-retry**: If retryable errors map to 4xx via `AppError` conversions, Stripe will not retry and the change won’t solve the core issue.
- **Silent skip on configuration errors**: Treating missing plan mappings as non-retryable may hide configuration regressions, leading to permanently missing subscriptions without forcing a retry.
- **Event ordering / missing records**: For payment status updates, treating `NotFound` as non-retryable could lead to permanent gaps when related records are created later by other events.
- **Retry storms on persistent DB issues**: While Stripe backoff helps, returning 5xx on repeated DB failures could still produce long retry tails. Ensure logs/alerts are sufficient to notice repeated failures quickly.
