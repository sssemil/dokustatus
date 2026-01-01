# Feedback on Plan v3: HTTP Client Timeouts

## What's good

- Clear discovery verification with multiple rg patterns and explicit exclusions; good confidence in coverage.
- Centralized `http_client` module with default timeout constants and guidance to prevent drift is solid.
- Distinguishes between startup singletons and request-scoped clients with `build_client()` vs `try_build_client()`.
- Timeout values are justified with expected response times; aligns with external API usage.
- Operational notes anticipate TLS/pooling concerns and explain why panics are acceptable.
- Checklist is concrete and aligned with planned file changes and verification steps.

## What's missing or unclear

- No mention of per-request timeout override for Stripe/Resend specific operations (e.g., webhooks vs API calls) if they ever need different timeouts; the plan assumes one size fits all without stating why Stripe retry behavior is acceptable.
- `try_build_client()` error mapping uses `AppError::Internal("Failed to build HTTP client".into())`, but the plan does not confirm which `AppError` variant is appropriate or whether any existing error helpers should be used for consistency.
- The plan assumes `reqwest::Client::builder().timeout()` covers total request time, but does not mention read/write timeouts or how this interacts with streaming bodies (if any are used now or later).
- No explicit statement on whether to add a lint/CI check to prevent reintroducing `Client::new()` or direct builder usage outside the module.

## Suggested improvements

- Add a brief note about why the chosen timeouts are safe for Stripe retries and Resend queueing behavior, or explicitly note the risk of premature timeout and the rollback plan (e.g., adjust constants in one place).
- Consider adding a lightweight guardrail: a `rg` check in CI or a doc note in `CONTRIBUTING` to discourage direct `reqwest::Client::new()` usage outside `http_client.rs`.
- If `AppError` has a standard constructor for internal errors (e.g., `AppError::internal("...")`), use it for consistency; confirm in the plan which helper to call.
- Add a small note in the plan about `Client::builder().timeout()` covering total time including DNS/connection + response, so future readers understand the semantics.

## Risks or concerns

- Request-scoped clients in OAuth flows will not reuse connections; if those endpoints become hot, latency and socket churn may increase. The plan mentions this but does not define a trigger/metric to revisit the decision.
- Panicking on client build failure is fine, but if TLS configuration varies between dev/prod environments, this could cause startup failures without a recovery path. Consider documenting where TLS roots are managed (e.g., container base image) to reduce ambiguity.
- If any code elsewhere relies on the implicit default timeout of `reqwest` (infinite), changing to 30s could surface new timeouts; this is intended but might need monitoring after deployment.
