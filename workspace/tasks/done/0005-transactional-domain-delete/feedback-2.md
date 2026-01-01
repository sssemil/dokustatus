# Feedback on Plan v2: Transactional Domain Delete

## What's good
- Clear call-chain mapping with concrete file/line references makes scope and touch points obvious.
- Explicitly preserves existing semantics (use-case NotFound, repo idempotency) and explains why.
- Rationale for avoiding explicit deletes and `DELETE ... RETURNING` is well-argued and consistent with current patterns.
- Verification steps include build, lint, and tests, plus an optional manual validation query.
- Risks are acknowledged with mitigation; out-of-scope items are explicitly listed.

## What's missing or unclear
- No explicit validation that all `domain_id` FKs are `ON DELETE CASCADE` in the current schema; the plan assumes this remains true without listing evidence or a check.
- The plan doesn't mention whether the delete is expected to run in the same transaction context as the existence check (it doesn't), but doesn't clarify if this is acceptable for future audit/outbox work.
- The manual test SQL references `domain_auth_config` and `domain_api_keys` only; may omit other tables that should be verified (e.g., sessions, tokens, logs) if they exist.
- There is no explicit note about whether the transaction should be `READ COMMITTED` default (fine) or if row-level locks or `SELECT ... FOR UPDATE` are intentionally avoided.

## Suggested improvements
- Add a short “Schema verification” step listing the tables with `domain_id` FKs and a note that each has `ON DELETE CASCADE` (even if done via a quick query or prior audit). This avoids relying on an implicit assumption.
- In the risk section, explicitly note that future audit/outbox work would need to move existence check into the transaction or use `DELETE ... RETURNING` and adjust error semantics; that makes the future extension boundary concrete.
- Expand the manual verification query to include all known `domain_id` tables (or reference a single SQL query to enumerate them via `information_schema`), so the check stays accurate as schema grows.
- Add a brief note that no SQLx offline change is needed because only `sqlx::query` is used, and confirm that no tracing changes are planned (to prevent scope creep).

## Risks or concerns
- If any `domain_id` FK lacks `ON DELETE CASCADE`, the single DELETE will not be fully “transactional” for related data; cascade coverage should be verified or documented.
- The transaction wrapper adds minimal value now but can create a false sense of stronger atomicity (the existence check is outside the transaction). Consider a cautionary note in the plan.
- If delete semantics ever change (e.g., repo starts returning NotFound on 0 rows), the use-case behavior could inadvertently shift; keeping a regression test would help prevent that.
