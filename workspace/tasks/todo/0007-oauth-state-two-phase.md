Two-phase Google OAuth state usage
Allow retries by not consuming state before token exchange completes.

Checklist
- [ ] Review OAuth state lifecycle methods
- [ ] Implement mark/use/complete or retry window
- [ ] Update routes to handle failure paths

History
- 2026-01-01 06:52 Created from code review finding #7 OAuth state consumed before external call.
- 2026-01-01 06:55 Renamed file to 0007-oauth-state-two-phase.md to use 4-digit task numbering.
