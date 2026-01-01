Fix domain token hash collision risk
Prevent token+domain hash collisions by adding delimiting or HMAC.

Checklist
- [ ] Inspect current hash concatenation logic
- [ ] Implement delimiter or HMAC-based hashing
- [ ] Update tests or add regression coverage

History
- 2026-01-01 06:52 Created from code review finding #2 Token hash collision vulnerability.
- 2026-01-01 06:55 Renamed file to 0002-fix-token-hash-collision.md to use 4-digit task numbering.
