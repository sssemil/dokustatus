Two-phase Google OAuth state usage
Allow retries by not consuming state before token exchange completes.

Checklist
- [x] Review OAuth state lifecycle methods
- [x] Implement mark/use/complete or retry window
- [x] Update routes to handle failure paths

History
- 2026-01-01 06:52 Created from code review finding #7 OAuth state consumed before external call.
- 2026-01-01 06:55 Renamed file to 0007-oauth-state-two-phase.md to use 4-digit task numbering.
- 2026-01-01 07:30 Created detailed implementation plan (plan-v1.md). Includes two-phase state lifecycle with mark/complete pattern, error classification (retryable vs terminal), and Redis Lua scripts using TIME for clock consistency. Reviewed by Codex and updated to address concurrency, atomicity, and error handling concerns.
- 2026-01-01 08:15 Created plan-v2.md addressing feedback-1.md. Key changes: replaced string-matching error detection with structured Lua return codes (integer status), changed complete_state to unconditional delete, added DB/infra failures as retryable, added UI/SDK handling section for 410 response, added time-controllable test clock for unit tests, documented concurrency behavior with dual in-flight requests.
- 2026-01-01 12:29 Added plan review feedback in feedback-2.md.
- 2026-01-01 12:45 Created plan-v3.md (revision 3/3) addressing all feedback-2.md items: replaced string-matching error classification with typed OAuthExchangeError enum, added TTL refresh to guarantee retry window, removed unused completed status, added abort_state call on RetryWindowExpired, made complete_state best-effort, added tests for error classification and TTL refresh.
- 2026-01-01 12:49 Implemented two-phase OAuth state handling, typed exchange errors, Redis TTL refresh, UI 410 handling, SDK error code helper, and added tests.
- 2026-01-01 13:05 Task complete. API builds successfully. Merge requested by orchestrator. Moving to outbound.
