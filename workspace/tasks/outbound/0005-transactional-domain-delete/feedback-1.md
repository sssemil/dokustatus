# Feedback: Plan v1 for 0005-transactional-domain-delete

## What’s good
- Clear summary of current behavior, risks, and the goal; shows awareness of schema constraints and how cascades behave.
- The plan evaluates alternatives (explicit deletes vs single DELETE) and lands on a simpler, maintainable approach.
- The codex review feedback is captured and integrated, including a cleaner `DELETE ... RETURNING` pattern.
- Edge cases and rollback strategy are explicitly documented.
- Testing section acknowledges current project constraints and outlines a practical manual verification flow.

## What’s missing or unclear
- The plan doesn’t confirm whether the use-case layer already treats “not found” as an error before calling the repo. If it does, the new `NotFound` in persistence may change behavior or duplicate checks.
- It doesn’t identify the exact adapter/use-case call chain for domain deletion (route → use case → repo). That makes it hard to know whether changing repo semantics affects API error mapping.
- There’s no mention of whether this change should also update SQLx offline data (if `sqlx::query_as` with `RETURNING` triggers it), or whether `cargo sqlx prepare` is needed.
- The plan doesn’t clarify whether observability/logging is actually desired or should be avoided (adding tracing may not be consistent with current logging patterns).

## Suggested improvements
- Verify the existing delete use-case behavior: if it already checks domain existence/ownership, decide whether repo-level `NotFound` should be added or kept consistent (and note any API response changes).
- Add a quick note on error mapping: confirm that `AppError::NotFound` maps to the correct HTTP status in the adapters layer and that this change doesn’t alter a previously “silent” no-op delete.
- If using `DELETE ... RETURNING`, confirm it won’t break SQLx offline expectations; explicitly state whether running `./run db:prepare` is required.
- Consider simplifying further: a transaction wrapper may not be needed unless you add an extra statement (audit/outbox); if you keep it, be explicit that the “transactional” requirement is satisfied only as a future‑proofing measure.
- If tracing is added, align with existing logging conventions (e.g., `instrument` macros already used in other repo methods) to keep consistency.

## Risks or concerns
- Potential behavior change: returning `NotFound` from the repo when a delete affects zero rows could turn idempotent deletes into errors. If callers previously treated “delete missing” as success, this would be a breaking change.
- The plan lists many explicit delete statements before recommending the simpler approach; if a reviewer implements the explicit deletes, it can diverge from the schema and increase maintenance risk (new tables may be missed).
- The “transactional” requirement is arguably already met by a single `DELETE ... CASCADE` statement; the change might be perceived as redundant unless the requirement is clarified in the task or acceptance criteria.
