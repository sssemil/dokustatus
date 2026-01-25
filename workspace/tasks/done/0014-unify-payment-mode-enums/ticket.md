Unify StripeMode and PaymentMode
Reduce duplicated enums and conversion boilerplate.

Checklist
- [x] Assess migration impact for enums
- [x] Introduce unified enum or conversion layer
- [x] Plan data migration if needed

History
- 2026-01-01 06:52 Created from code review finding #14 Redundant StripeMode and PaymentMode enums.
- 2026-01-01 06:55 Renamed file to 0014-unify-payment-mode-enums.md to use 4-digit task numbering.
- 2026-01-24 Completed full unification:
  - Created migration 00011 to drop all stripe_mode columns and make payment_mode NOT NULL
  - Deleted stripe_mode.rs entity file
  - Updated domain_billing.rs use cases to use PaymentMode directly (major refactor)
  - Updated all persistence layer files (domain.rs, billing_stripe_config.rs, subscription_plan.rs, user_subscription.rs, billing_payment.rs)
  - Updated all HTTP routes (domain.rs, common.rs, billing.rs, billing_webhooks.rs, billing_dummy.rs)
  - Updated TypeScript types and UI to remove StripeMode references
  - All 91 tests pass, API and UI build successfully
- 2026-01-25 Removed additional backward compatibility patterns found by Codex:
  - Removed re-export comment in domain.rs (line 13)
  - Removed "legacy behavior preserved" comment in domain_billing.rs (line 50)
  - Removed "legacy" comment in domain_billing.rs (line 2295)
  - Deleted unused StripeConfigResponse struct in domain_billing.rs (lines 2195-2202)
  - Removed "backwards compatibility" comment in ThemeContext.tsx (line 50)
  - All 91 tests pass, API and UI build successfully
- 2026-01-25 Additional backward compat cleanup (round 2):
  - Removed test_backward_compat_old_state test in domain_auth.rs
  - Removed legacy PaymentMode aliases ("sandbox", "production", "prod") from FromStr
  - Updated test_from_str to verify legacy aliases are rejected
  - Removed unused OAuthExchangeError::Redis variant
  - Replaced #[allow(dead_code)] with underscore-prefixed fields for Google OAuth structs
  - Removed deprecated main() function from agent_loop.py
  - All 90 tests pass, API and UI build successfully
- 2026-01-25 Additional backward compat cleanup (round 3):
  - Removed BillingState legacy format aliases ("pendingswitch", "switchfailed", "failed") from FromStr
  - Fixed 'as any' type assertions in billing pages by using proper SubscriptionStatus type
  - Fixed 'as any' for error messages using typed cast { message?: string }
  - Tightened interval type from 'monthly' | 'yearly' | 'custom' | string to strict union
  - All 90 tests pass, API and UI build successfully
- 2026-01-25 Final backward compat cleanup (round 4):
  - Removed PaymentProvider "test" alias for "dummy" in FromStr
  - Removed PaymentScenario legacy aliases (declined, 3ds, threeds, insufficientfunds, etc.)
  - Fixed 'any' type in SDK client.ts with proper inline type
  - Tightened SDK interval type from 'monthly' | 'yearly' | string to strict union
  - Simplified error field checking in magic/page.tsx (removed code and message fallbacks)
  - Updated tests to verify legacy aliases are rejected
  - All 90 tests pass, API, UI, and SDK build successfully
- 2026-01-25 Codex review of all pending changes:
  - Codex reviewed 34 changed files (495 lines removed, 254 added)
  - Confirmed migration 00011_cleanup_stripe_mode.sql handles all schema changes correctly
  - Verified TypeScript imports are correct (PaymentMode properly imported in domain pages)
  - Note: Legacy alias removal is intentional per AGENTS.md (no backward compat needed)
  - All 90 tests pass, API, UI, and SDK build successfully
  - Ready for commit
