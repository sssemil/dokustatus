# Feedback on plan v2: Harden Webhook Signature Compare

## What's good about the plan
- Clear decision to use `hmac::Mac::verify_slice` instead of a homegrown compare; removes the timing leak and avoids new deps.
- Concrete, actionable steps with specific file path and code snippets.
- Good coverage of edge cases (multiple signatures, malformed hex, empty signature) and preserves timestamp tolerance.
- Notes the single call site and clarifies no new dependencies needed.
- Security rationale and references are helpful for future audit/review.

## What's missing or unclear
- The plan recomputes the HMAC inside the loop for each signature, but does not address the potential performance hit or suggest a way to avoid repeated MAC initialization while still using `verify_slice` (which consumes `self`). If there are many signatures, this could be suboptimal.
- Tests use `chrono::Utc::now()` directly; this can still be flaky around tolerance boundaries or if clock skew is significant. The plan says “fixed timestamps” but uses `now()`.
- It is unclear how `verify_webhook_signature` handles whitespace or capitalization in headers (Stripe headers can contain spaces); the plan doesn’t mention normalization or trimming.
- The plan assumes `verify_slice` is constant-time for length mismatch; that is true in hmac, but the plan doesn’t verify the crate version or any feature flags that could change behavior.
- The plan does not mention whether any other webhook or signature compare code paths exist (e.g., in demo apps or SDK) that should be audited.

## Suggested improvements
- Consider computing the MAC once and then using `verify_slice` by recreating the MAC from a cloned key and signed payload only once per signature, but also document the rationale. If performance is a concern, note expected number of signatures (usually 1–2) to justify per-signature recompute.
- Use deterministic timestamps in tests by providing explicit `ts` values (e.g., `let ts = 1_700_000_000;`) and compute expired/valid timestamps relative to that; avoid `Utc::now()` in unit tests.
- Add a test for headers with spaces (e.g., `"t=..., v1=..."`) if the parser allows it, or explicitly trim in parsing and document behavior.
- Include a short check in the plan to confirm the `hmac` crate version in `Cargo.lock` uses the constant-time `verify_slice` (or link to the exact version’s docs). If a newer version is required, call it out.
- Note whether non-`v1` signatures should be ignored vs. rejected, and add a test if applicable.

## Risks or concerns
- Per-signature MAC recomputation could be a small performance regression if header contains many signatures (unlikely, but plan should acknowledge the tradeoff).
- Tests that depend on current time can be flaky, especially in CI if system time is skewed or slow.
- If `verify_webhook_signature` currently accepts non-trimmed headers, introducing strict parsing elsewhere could change behavior; the plan should ensure no behavior regression.
- If `hex::decode` failures are silently skipped, malformed signatures won’t raise a distinct error; this is acceptable but should be intentional and tested.
