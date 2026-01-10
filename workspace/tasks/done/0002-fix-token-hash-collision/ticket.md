Fix domain token hash collision risk
Prevent token+domain hash collisions by adding delimiting or HMAC.

Checklist
- [ ] Inspect current hash concatenation logic
- [ ] Implement delimiter or HMAC-based hashing
- [ ] Update tests or add regression coverage

History
- 2026-01-01 06:52 Created from code review finding #2 Token hash collision vulnerability.
- 2026-01-01 06:55 Renamed file to 0002-fix-token-hash-collision.md to use 4-digit task numbering.
- 2026-01-01 08:56 Added plan-v1.md with implementation plan and testing notes.
- 2026-01-01 08:58 Added plan-v2.md after feedback review; refined steps, tests, and edge cases.
- 2026-01-01 08:59 Added plan-v3.md after feedback review; retained plan and tightened steps.
- 2026-01-01 09:05 Implemented length-prefixed token hashing with legacy fallback and added regression tests.
- 2026-01-01 09:05 Ran `cargo test domain_auth::tests` in `apps/api`.
- 2026-01-01 09:07 Completed implementation, tests, and ready for outbound handoff.
