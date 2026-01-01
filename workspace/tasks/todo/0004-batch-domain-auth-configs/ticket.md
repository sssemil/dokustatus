Remove N+1 in domain listing
Batch-load auth configs for domains to avoid per-domain queries.

Checklist
- [ ] Identify N+1 query path in domain list
- [ ] Add batch repository/use case method
- [ ] Update route to use batch results

History
- 2026-01-01 06:52 Created from code review finding #4 N+1 query in domain listing.
- 2026-01-01 06:55 Renamed file to 0004-batch-domain-auth-configs.md to use 4-digit task numbering.
