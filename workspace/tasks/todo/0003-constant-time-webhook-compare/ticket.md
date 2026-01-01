Harden webhook signature compare
Eliminate timing leaks in Stripe webhook signature verification.

Checklist
- [ ] Review compare helper and call sites
- [ ] Switch to constant-time compare (e.g., subtle crate)
- [ ] Add/adjust tests for signature verification

History
- 2026-01-01 06:52 Created from code review finding #3 Timing attack in webhook signature verification.
- 2026-01-01 06:55 Renamed file to 0003-constant-time-webhook-compare.md to use 4-digit task numbering.
