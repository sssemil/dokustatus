Remove N+1 in domain listing
Batch-load auth configs for domains to avoid per-domain queries.

Checklist
- [x] Identify N+1 query path in domain list
- [x] Add batch repository/use case method
- [x] Update route to use batch results

History
- 2026-01-01 06:52 Created from code review finding #4 N+1 query in domain listing.
- 2026-01-01 06:55 Renamed file to 0004-batch-domain-auth-configs.md to use 4-digit task numbering.
- 2026-01-01 10:40 Added plan review feedback (feedback-1.md).
- 2026-01-01 10:44 Added plan review feedback (feedback-2.md).
- 2026-01-01 10:53 Implemented batch auth config lookup scaffolding and tests per plan.
- 2026-01-01 11:03 Preserved list_domains error handling for auth config lookups.
- 2026-01-01 11:05 Completed batch auth config work and committed changes.
