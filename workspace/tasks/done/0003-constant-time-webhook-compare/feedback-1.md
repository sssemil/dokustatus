# Feedback: 0003-constant-time-webhook-compare plan-v1

## What's good about the plan
- Identifies the specific vulnerable helper and explains the timing leak clearly.
- Proposes using a well-known constant-time primitive (`subtle`) rather than bespoke logic.
- Includes concrete test cases for valid/invalid signatures and timestamp tolerance.
- Calls out relevant edge cases (empty strings, multi-signature headers, expired timestamps).
- Lists affected files and a minimal verification command set.

## What's missing or unclear
- The recommended “simpler” approach still short-circuits on length mismatch, which conflicts with the task goal to eliminate timing leaks.
- It’s unclear whether `constant_time_compare` is used anywhere besides Stripe webhook verification; the plan doesn’t confirm all call sites.
- The plan notes multiple `v1` signatures but the tests don’t cover that path.
- The plan doesn’t specify how to handle malformed hex in the signature header (non-hex chars, odd length), which affects how comparisons are performed.
- Line references to `stripe_client.rs` are likely stale; the plan should avoid hard-coded line numbers.

## Suggested improvements
- Decide on a strict policy: either truly constant-time regardless of length mismatch or explicitly justify why length checks are acceptable for this endpoint.
- Prefer comparing fixed-size bytes: parse the `v1` signature into `[u8; 32]` and use `hmac::Mac::verify_slice` (already constant-time) to avoid custom compare logic.
- If keeping `subtle::ct_eq`, avoid early returns by comparing fixed-length buffers (e.g., zero-pad to 64 hex chars or compare decoded bytes with a default value on parse failure).
- Add tests for multiple `v1` entries in the Stripe header, malformed header segments, and invalid hex signatures.
- Use the actual tolerance constant (if defined) in tests rather than hard-coded `600`, or set fixed timestamps to avoid time-based flakiness.

## Risks or concerns
- The current recommendation could still leak timing via length checks, undermining the task’s stated security goal.
- Adding `subtle` just for this helper may be unnecessary if `hmac::Mac::verify_slice` can replace the compare entirely.
- Tests relying on `Utc::now()` can become brittle if tolerance logic changes, leading to false negatives in CI.
