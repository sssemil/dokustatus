Make domain deletion transactional
Ensure domain deletion removes related data atomically.

Checklist
- [ ] List related tables that must be removed
- [ ] Wrap deletes in transaction (or add cascade)
- [ ] Add test or validation for atomic delete

History
- 2026-01-01 06:52 Created from code review finding #5 Missing transaction in domain deletion.
- 2026-01-01 06:55 Renamed file to 0005-transactional-domain-delete.md to use 4-digit task numbering.
