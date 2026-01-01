# Remove StripeMode Enum

## Parent Task
[0014-unify-payment-mode-enums](../in-progress/0014-unify-payment-mode-enums/ticket.md)

## Description
Complete the migration from StripeMode to PaymentMode by removing the deprecated StripeMode enum and updating all remaining usages.

## Prerequisites
- Task 0014 must be completed (PaymentMode is now canonical, StripeMode is deprecated)
- Database migration to drop stripe_mode column must be run after data backfill completes
- All existing data should be migrated to payment_mode column

## Scope

### Backend Changes
1. Remove the StripeMode enum from `apps/api/src/domain/entities/stripe_mode.rs`
2. Update structs in `domain_billing.rs` that still have `stripe_mode` fields:
   - `BillingStripeConfigProfile.stripe_mode` -> remove or replace
   - `BillingPaymentProfile.stripe_mode` -> remove
   - `SubscriptionPlanProfile.stripe_mode` -> remove
   - `UserSubscriptionProfile.stripe_mode` -> remove
3. Update SQL queries to stop using stripe_mode column
4. Remove From/Into implementations between StripeMode and PaymentMode

### Database Migration
1. Create migration to drop `stripe_mode` columns from tables:
   - `billing_stripe_configs`
   - `billing_payments`
   - `subscription_plans`
   - `user_subscriptions`
   - `domains` (billing_stripe_mode column)
2. Drop the `stripe_mode` database enum type

### Frontend Changes
1. Remove StripeMode type alias from `apps/ui/types/billing.ts`
2. Remove deprecated `getModeLabel` and `getModeBadgeColor` functions
3. Update any remaining usages

## Checklist
- [ ] Verify all data has been migrated to payment_mode columns
- [ ] Remove StripeMode enum from Rust codebase
- [ ] Update domain_billing.rs structs
- [ ] Update SQL queries
- [ ] Create and run database migration
- [ ] Remove frontend StripeMode type
- [ ] Run all tests
- [ ] Deploy and verify

## Notes
- This is a breaking change - ensure backwards compatibility period has passed
- The dual-write strategy from task 0014 ensures data consistency during transition
- Consider keeping stripe_mode column as read-only for a grace period if needed

## History

### 2026-01-01
- Created task as follow-up from task 0014-unify-payment-mode-enums
- Scope defined based on remaining StripeMode usages identified during task 0014
