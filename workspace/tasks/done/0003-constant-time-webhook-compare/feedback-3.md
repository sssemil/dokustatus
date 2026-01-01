# Feedback on Plan v3: Harden Webhook Signature Compare

## What's good about the plan
- Uses `hmac::Mac::verify_slice` for constant-time compare and removes the custom comparer, directly addressing the timing-leak finding.
- Clearly scoped to a single file and call site, with a solid audit that no other webhook verification paths exist.
- Covers multiple signature scenarios (multiple `v1`, non-`v1` ignored) and malformed input paths, which match Stripe’s header format expectations.
- Includes a performance rationale for per-signature MAC recomputation and documents header whitespace behavior.
- Calls out dependency version (`hmac = 0.12`) and links to upstream docs to justify constant-time behavior.

## What’s missing or unclear
- The plan says “deterministic timestamps” but the proposed tests still call `Utc::now()` via `now_ts()`. Those tests remain time-sensitive and can still flake.
- There’s no strategy for making `verify_webhook_signature` testable with a fixed “now” (e.g., injecting a clock or adding a `*_with_now` helper). Without that, truly deterministic timestamp tests aren’t possible.
- The claim that `verify_slice` is constant-time for length mismatches is asserted but not validated; `verify_slice` may short-circuit on length before the constant-time compare. Clarify the security impact or cite the exact behavior.
- Behavior for headers with mixed malformed + valid `v1` signatures is implied (skip malformed, accept valid) but not explicitly stated as desired or aligned with Stripe’s guidance.

## Suggested improvements
- Make the tests truly deterministic by introducing a small helper that accepts `now_ts: i64` (or a clock trait) and use a fixed timestamp in tests. Example: add `verify_webhook_signature_with_now(payload, header, secret, now_ts)` and keep the public function calling it with `Utc::now()`.
- Add a test for invalid (non-numeric) timestamps like `t=abc` to pin the current error path and ensure signature verification doesn’t accidentally accept malformed timestamps.
- Clarify the desired behavior when any malformed `v1` is present alongside valid signatures (e.g., “ignore malformed and accept if any valid exists”), and add a test if that behavior is important.
- If you want to be more robust to minor formatting differences, consider trimming whitespace around header parts and update the whitespace test accordingly. If not, explicitly document why strict parsing is acceptable (e.g., Stripe guarantees no spaces).

## Risks or concerns
- Current test plan can still flake because it relies on real time; future CI clock skew or slow test runs can push timestamps over the tolerance window.
- If `verify_slice` internally rejects length mismatches before constant-time compare, an attacker could still detect length differences. This isn’t a secret but it undermines the “constant-time for length mismatch” claim—worth verifying so the security rationale is accurate.
- Skipping malformed hex could allow attacker-controlled headers to consume extra CPU via repeated HMAC recomputation. Not a critical risk, but worth noting if header size isn’t bounded elsewhere.
