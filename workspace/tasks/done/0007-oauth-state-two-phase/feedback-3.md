# Feedback on plan-v3

## What's good about the plan
- Clear two-phase lifecycle with mark/complete and explicit retry window; fixes the original failure mode.
- Typed error classification removes string matching and makes retry vs terminal behavior auditable.
- Redis Lua script uses TIME and structured return codes; concurrency behavior is reasoned through.
- Backward compatibility and TTL refresh are called out; tests cover happy path, retry, expiry, TTL refresh.
- UI/SDK handling for 410 is included and aligns with retry semantics.

## What's missing or unclear
- TTL refresh behavior when the existing TTL is longer than `retry_window + 30s` (current script always sets TTL to `min_ttl`, which can shorten the lifetime).
- Handling for states that might already have `status="in_use"` but `marked_at` missing (Lua treats this as expired because `marked_at` defaults to 0).
- Retryability classification for Google 4xx other than `invalid_grant` (e.g., 429 rate limit, 408 timeout, OAuth `temporarily_unavailable`).
- What logging/metrics will capture `RetryWindowExpired` and `complete_state`/`abort_state` failures (best-effort deletion is silent in some paths).
- Exact UI location and component to show the 410 modal (the plan shows pseudo-code but not the concrete file or hook).

## Suggested improvements
- In Redis Lua, set TTL to `max(current_ttl, min_ttl)` to avoid shrinking longer-lived states; use `TTL`/`PTTL` to compute current TTL before `EXPIRE` or `SET EX`.
- If `data.status == 'in_use'` and `marked_at` is missing, treat as pending and set `marked_at = now` instead of expiring immediately; this avoids accidental 410s from malformed data.
- Expand `OAuthExchangeError::is_retryable` to treat 429/408 and OAuth `temporarily_unavailable` as retryable even if they are 4xx.
- Add explicit logging (warn + counter tag) when `mark_state_in_use` returns `RetryWindowExpired` and when `abort_state` fails; this helps spot regression loops.
- Pin the UI change to the exact callback file/hook and ensure it uses the existing modal (no alerts).

## Risks or concerns
- Shortening TTL on mark could cause state to expire sooner than the original flow intended, leading to unexpected 410s.
- Overly strict terminal classification of all 4xx from Google could block legitimate retries during rate limits or temporary auth server issues.
- Best-effort deletion combined with Redis outages could leave stale `in_use` states around longer than expected; ensure TTL behavior still bounds this.
- If repeated retries keep resetting TTL, users can repeatedly hit the endpoint and extend storage use; ensure rate limiting or bounded retry window is enforced by `marked_at`.
