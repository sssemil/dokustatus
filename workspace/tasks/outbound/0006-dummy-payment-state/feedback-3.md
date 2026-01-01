# Plan Review Feedback: 0006-dummy-payment-state (plan-v3)

## What's good about the plan
- Clear scope and rationale: the plan explicitly ties the "return None" approach to the ticket checklist and the existing DB source of truth.
- Solid pre-implementation audit: call-site verification and test/fixture scan reduce regression risk.
- Actionable steps: concrete before/after code blocks, file list, and a realistic testing plan.
- Risk table and out-of-scope notes help keep the work bounded and visible.

## What's missing or unclear
- The semantics of `None` are still a bit overloaded ("not found" vs "not supported"). The docstrings mention this, but the plan does not spell out how callers should interpret `None` in generic code paths if they are added later.
- The plan does not mention cleaning up any now-unused imports (e.g., `Utc`, `Duration`, `HashMap`) after removing the fabricated data; this may cause warnings or make the file noisier than needed.
- Module doc placement could be ambiguous if a module-level doc already exists. The plan says "after any existing module doc" but doesn't specify how to merge or update if one is present.

## Suggested improvements
- Add a short, explicit sentence in the trait docstrings that `None` can mean "provider does not support lookup" to guide any future generic code paths.
- Include a quick "check and remove unused imports" step in the implementation checklist to keep the file clean after removing dummy data construction.
- If a module doc already exists, update it rather than adding a second top-level doc block; note this in the plan to avoid duplicate module docs.

## Risks or concerns
- Returning `None` for dummy lookups could be interpreted as "missing subscription" by future generic code and lead to unintended flows (e.g., re-creating subscriptions). The doc updates mitigate this, but consider adding a trace message that includes the provider name and ID to make debugging easier.
- If any downstream code (outside current call sites) relied on the dummy "always active" response for manual testing, behavior will change. Consider a short note in the plan about expected impact on local testing.
