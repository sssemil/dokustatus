Make domain deletion transactional
Ensure domain deletion removes related data atomically.

Checklist
- [x] List related tables that must be removed
- [x] Wrap deletes in transaction (or add cascade)
- [x] Add test or validation for atomic delete

History
- 2026-01-01 06:52 Created from code review finding #5 Missing transaction in domain deletion.
- 2026-01-01 06:55 Renamed file to 0005-transactional-domain-delete.md to use 4-digit task numbering.
- 2026-01-01 07:30 Created plan-v1.md with detailed implementation approach. Key findings:
  - All domain_id foreign keys already have ON DELETE CASCADE
  - Single DELETE statement is atomic; transaction wrapper adds future extensibility
  - Reviewed by Codex: recommended explicit rollback and DELETE...RETURNING pattern
- 2026-01-01 08:15 Created plan-v2.md addressing feedback-1.md:
  - Clarified call chain: route → use case (checks existence/ownership) → repo (delete)
  - Confirmed no SQLx offline regeneration needed (using unchecked queries)
  - Documented race condition between check and delete as acceptable risk
  - Codex review: validated minimal approach, noted race is benign due to idempotency
- 2026-01-01 12:03 Added plan review feedback in feedback-2.md.
- 2026-01-01 12:06 Created plan-v3.md addressing feedback-2:
  - Added exhaustive schema verification: all 11 domain_id tables have ON DELETE CASCADE
  - Added explicit acknowledgment of race condition and idempotent behavior
  - Added transaction boundary trade-off note for future audit/outbox work
  - Added Schema Drift Check step with rg command for future verification
  - Expanded verification query to cover all domain-related tables
  - Codex review completed; all feedback integrated
- 2026-01-01 12:08 Added plan review notes in feedback-3.md.
- 2026-01-01 12:10 Wrapped domain delete in an explicit transaction in the persistence adapter.
- 2026-01-01 12:10 Marked checklist items complete after implementing transactional delete.
- 2026-01-01 12:11 Completed transactional delete update and task checklist; ready for outbound.
