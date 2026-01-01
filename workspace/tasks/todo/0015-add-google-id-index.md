Add index for domain_end_users google_id lookup
Speed up domain+google_id queries with a composite index.

Checklist
- [ ] Confirm query path uses domain_id + google_id
- [ ] Create migration for composite index
- [ ] Document index in migration notes

History
- 2026-01-01 06:52 Created from code review finding #15 Missing index on google_id lookup.
- 2026-01-01 06:55 Renamed file to 0015-add-google-id-index.md to use 4-digit task numbering.
