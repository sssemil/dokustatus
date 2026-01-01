# Plan Feedback: 0006-dummy-payment-state

## What's good
- Clear identification of the real issue (dummy provider returns fabricated "active" state) and confirmation that DB state already exists.
- Code audit is explicit about current call sites, reducing risk of unintended breakage.
- Proposed change is minimal and defensive (returning None), aligned with task checklist.
- Step-by-step edits and concrete file list make implementation straightforward.
- Tests are scoped to the new behavior and tied to intent.

## What's missing or unclear
- No confirmation that returning None won't break any downstream expectations in interfaces or tests (e.g., any code that assumes Some for dummy IDs in tests or fixtures).
- Documentation update scope is limited to docstrings; unclear if developer-facing docs or README should note the dummy provider limitation.
- Logging impact is not considered (debug log on every call might be noisy if called in loops).
- The plan doesn't specify how to handle or update existing tests that may assert on placeholder fields like "dummy_cus_unknown".

## Suggested improvements
- Add a quick search/grep step for "dummy_sub_" or "dummy_cus_" in tests/fixtures to confirm no expectations of fabricated data; document findings in history.
- Consider using trace-level logging or a single comment without logging if this could be called frequently.
- If project has developer docs or a dummy provider note elsewhere, add a short note there in addition to docstrings (or explicitly state no other docs exist).
- Add an explicit rationale in the plan for removing placeholder data (prevent footgun) and link it to the checklist item about documenting limitations.

## Risks or concerns
- If any tests or consumers currently rely on the placeholder data, they will start failing; pre-checks are needed to avoid surprise.
- Returning None could cause silent behavior changes if future callers assume Some and do not handle None correctly; docstrings help, but consider a future lint/check or explicit error type if this expands.
- The tests proposed are fine, but they won't protect against accidental reintroduction of placeholder data in other code paths (e.g., create flow still generates data); ensure unit scope is appropriate.
