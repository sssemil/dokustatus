Unify StripeMode and PaymentMode
Reduce duplicated enums and conversion boilerplate.

Checklist
- [ ] Assess migration impact for enums
- [ ] Introduce unified enum or conversion layer
- [ ] Plan data migration if needed

History
- 2026-01-01 06:52 Created from code review finding #14 Redundant StripeMode and PaymentMode enums.
- 2026-01-01 06:55 Renamed file to 0014-unify-payment-mode-enums.md to use 4-digit task numbering.
- 2026-01-01 07:15 Created detailed implementation plan (plan-v1.md). Key findings:
  - Both enums have identical variants (Test/Live) and nearly identical methods
  - PaymentMode is the more generic/future-proof option (supports provider-agnostic architecture)
  - Migration 00010 already added payment_mode columns alongside stripe_mode columns with backfill
  - Approach: Keep PaymentMode as unified type, add From/Into conversions, deprecate StripeMode
  - Database schema changes deferred to separate task (migration 00011 was originally planned)
- 2026-01-01 08:45 Created plan-v2.md addressing feedback from plan-v1:
  - Added Phase 0 (verification/inventory) - confirmed 127 StripeMode occurrences in 10 Rust files
  - Confirmed SDK and demo apps have no StripeMode references (already clean)
  - Clarified API contract: preserve JSON field names using #[serde(rename = "stripe_mode")]
  - Added dual-write column strategy: read from payment_mode, write to both columns
  - Added Phase 5 to create follow-up task 0015 for database cleanup
  - Added verification checkpoints between phases
  - Minimized frontend changes (StripeMode becomes alias to PaymentMode)
- 2026-01-01 09:30 Created plan-v3.md addressing feedback from plan-v2:
  - Added SQLx cache regeneration step (./run db:prepare) after persistence layer changes
  - Added dual-write SQL code examples with exact syntax
  - Added test file inventory step in Phase 0
  - Added #[allow(deprecated)] suppression strategy for persistence layer files
  - Revised implementation order: persistence → application → adapters
  - Added commit checkpoint recommendations (one commit per phase)
  - Added pre-implementation checklist
  - Improved enum attribute verification step
