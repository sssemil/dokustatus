# Feedback-2: Plan Review for 0006-dummy-payment-state

Date: 2026-01-01 12:24

## What's good about the plan

- Clear problem statement and rationale; it explains why `None` is the right signal and ties it back to the checklist.
- Solid pre-implementation audit: enumerates where dummy IDs appear and calls out that there are no call sites for dummy `get_subscription()`.
- Step-by-step changes are concrete with before/after snippets, making implementation low-risk.
- Tests are scoped and aligned with the intended behavior change.
- Risk mitigation and out-of-scope sections show good boundary control.

## What's missing or unclear

- The test IDs (`SubscriptionId::new("sub_stripe_xyz")`, `CustomerId::new("dummy_cus_12345")`) assume constructors accept arbitrary strings. If these constructors validate formats, the proposed tests will fail. Clarify or use known-good IDs.
- Docstrings mention Coinbase, but the plan does not confirm a Coinbase provider exists in this repo. This could mislead readers if it's not implemented.
- The plan doesn't mention updating task artifacts required by the workspace rules (append history, move to done when complete).
- The plan assumes `DummyPaymentClient::new(Uuid::new_v4())` matches the actual constructor signature; confirm parameters in code.

## Suggested improvements

- Adjust test inputs to use whatever the codebase already treats as valid IDs (e.g., reuse a helper or fixture, or construct from real IDs seen in existing tests).
- Reword the trait docstrings to mention only providers that exist today; if Coinbase is planned, note that explicitly as future intent.
- Add a short implementation note to update the task `History` and checklist when work starts/finishes to stay compliant with repo guidelines.
- Consider adding a brief module-level comment in `dummy_payment_client.rs` explaining that dummy lookups return `None` so future contributors see it where they edit behavior.

## Risks or concerns

- If future code calls `get_subscription()` for dummy and treats `None` as an error path (vs. "unsupported"), it could introduce unexpected behavior. Consider documenting expected caller handling in the trait docs or adding a small helper comment near call sites if they are added later.
- If the ID constructors enforce provider-specific prefixes, the proposed tests will fail silently in CI until adjusted; validate early.
