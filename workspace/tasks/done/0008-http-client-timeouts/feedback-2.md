# Feedback on Plan v2 (0008-http-client-timeouts)

## What's good
- Clear discovery step with concrete file/line locations for all current `reqwest::Client` construction points.
- Centralizes timeout defaults in a single `http_client` module, reducing drift and making future tuning straightforward.
- Explicit decision + rationale for inline OAuth clients vs threading shared client through app state.
- Operational notes cover pooling and startup failure behavior, which helps future maintainers.
- Test/verification steps are realistic for a low-risk infra change.

## What's missing or unclear
- The discovery search pattern only covers `Client::new()` and `Client::builder()`; it does not catch `reqwest::get(...)`, `reqwest::blocking`, or `Client` re-exports/aliases. This leaves a gap if any other reqwest usage is present.
- Plan does not clarify whether there are any `reqwest` usages outside `apps/api` (e.g., helper crates in `libs/` or other Rust binaries) that should share the same timeout policy.
- Error handling choice for `try_build_client()` in request handlers is a bit ad hoc; it’s not stated whether this should be logged or include context (e.g., request id) consistently with other internal errors.

## Suggested improvements
- Expand the discovery verification to include additional patterns:
  - `rg 'reqwest::get\(|reqwest::blocking|ClientBuilder|use reqwest::Client'`
  - `rg 'reqwest' apps/ libs/` to confirm no other Rust crates use reqwest.
- Add a short note on whether `http_client` should be used by any future HTTP clients (document in `apps/api/src/infra/mod.rs` or a module-level comment) to keep new clients consistent.
- Consider a lightweight logging hook on `try_build_client()` failures (if logging is standard in request handlers) so the 500 has a traceable cause without exposing details to clients.
- Optional: note a follow-up to move OAuth clients to shared state if auth traffic grows and per-request client creation becomes a hotspot.

## Risks or concerns
- `build_client()` panics on TLS misconfiguration. This can break unit tests or local dev in environments missing root certs; call out a mitigation (e.g., ensure CI/dev images include certs) or consider using `try_build_client()` for test-only constructors if this has been an issue before.
- Hardcoded 30s request timeouts could be too tight for any long-running HTTP operations (if any exist now or are introduced later). At least flag which endpoints are expected to be fast to avoid surprises.
- If other reqwest clients exist outside `apps/api` (missed by the current search), they will still hang indefinitely; the plan should explicitly confirm they do not exist or include them in scope.
