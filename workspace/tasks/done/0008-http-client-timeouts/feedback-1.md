# Feedback: Plan v1 (0008-http-client-timeouts)

## What's good
- Clear identification of the current risk (no timeouts) and its impact on server resources and cascading failures.
- Concrete inventory of known `reqwest::Client::new()` locations with file paths and line hints.
- Sensible default values (5s connect / 30s total) with a brief rationale.
- Explicit handling guidance for builder failures (panic for constructors, `map_err` for inline calls).
- Testing checklist includes the required `./run api:build` pre-deploy verification.

## What's missing or unclear
- No confirmation step to ensure there are no other HTTP clients elsewhere in the repo (e.g., other `reqwest::Client::new()` or `Client::builder()` usages in `apps/*` or `libs/*`).
- The plan doesn’t decide between Option A vs. Option B for Google OAuth; it leaves an implementation fork open.
- Timeout configurability is not addressed (hardcoded vs. config/env). It’s unclear how to adjust in prod if 30s is too short/long.
- Unclear whether the email client is Resend or another provider with specific timeout expectations; the plan assumes generic values.

## Suggested improvements
- Add a discovery step using `rg` to verify *all* HTTP client instantiations are covered, and document any exclusions.
- Create a small helper in `apps/api/src/infra/` (e.g., `http_client.rs`) to build a client with shared defaults and reuse it in all locations to avoid drift.
- Decide on a single approach for Google OAuth: either reuse a shared client from app state (preferred for connection pooling) or explicitly document why per-call clients are acceptable.
- Consider wiring the timeout values into configuration with defaults (env vars or settings struct) so they can be tuned without code changes; if not, note why hardcoding is acceptable.

## Risks or concerns
- Hardcoded timeouts may be too aggressive for slow upstreams (e.g., email provider or Stripe under load), potentially causing false failures; without configurability, tuning requires deploys.
- Inline client creation in OAuth handlers bypasses connection pooling and may add overhead; minor but avoidable if a shared client is easy to plumb.
- `expect` on client build will panic at startup if TLS or system certs are misconfigured; acceptable but worth calling out as an operational risk.
