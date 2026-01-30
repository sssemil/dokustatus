# reauth.dev Billing System - Edge Cases

Acceptance criteria in Given/When/Then format. Use as test cases.

---

## Table of Contents

1. [Webhook Idempotency](#1-webhook-idempotency)
2. [Duplicate Prevention](#2-duplicate-prevention)
3. [State Consistency](#3-state-consistency)
4. [Trial Scenarios](#4-trial-scenarios)
5. [Renewal Scenarios](#5-renewal-scenarios)
6. [Payment Failure Scenarios](#6-payment-failure-scenarios)
7. [Cancellation Scenarios](#7-cancellation-scenarios)
8. [Plan Change Scenarios](#8-plan-change-scenarios)
9. [Bundle Scenarios](#9-bundle-scenarios)
10. [Credit Scenarios](#10-credit-scenarios)
11. [Refund Scenarios](#11-refund-scenarios)
12. [Dispute Scenarios](#12-dispute-scenarios)
13. [Crypto-Specific Scenarios](#13-crypto-specific-scenarios)
14. [Payment Method Scenarios](#14-payment-method-scenarios)
15. [Concurrent Operation Scenarios](#15-concurrent-operation-scenarios)
16. [Boundary Conditions](#16-boundary-conditions)

---

## 1. Webhook Idempotency

### EC-101: Duplicate payment success webhook
```
GIVEN: Payment exists with status='paid' for (stripe, pi_123)
WHEN: Webhook received for payment_intent.succeeded with pi_123
THEN:
  - No state changes
  - No duplicate credit grant
  - No duplicate entitlement
  - Return 200 OK
```

### EC-102: Duplicate payment failed webhook
```
GIVEN: Payment exists with status='failed' for (stripe, pi_456)
WHEN: Webhook received for payment_intent.payment_failed with pi_456
THEN:
  - No state changes
  - No duplicate dunning emails
  - Return 200 OK
```

### EC-103: Out-of-order webhooks (success before pending)
```
GIVEN: Invoice exists, no Payment record yet
WHEN: Webhook received for payment_intent.succeeded with pi_789
THEN:
  - Create Payment with status='paid' directly
  - Process success flow (credits, entitlement)
  - Handle subsequent 'pending' webhook as no-op
```

### EC-104: Webhook for unknown payment
```
GIVEN: No invoice or payment matching provider_payment_id
WHEN: Webhook received for payment_intent.succeeded with pi_unknown
THEN:
  - Log warning: "Unknown payment received"
  - DO NOT create any records blindly
  - Return 200 OK (acknowledge to prevent retries)
  - Alert for manual review
```

---

## 2. Duplicate Prevention

### EC-201: Double-subscribe attempt
```
GIVEN: User has active subscription (status='active')
WHEN: User attempts to subscribe to same or different plan
THEN:
  - Reject with error: "Already subscribed"
  - No new subscription created
```

### EC-202: Double-subscribe during trial
```
GIVEN: User has trialing subscription
WHEN: User attempts to subscribe again
THEN:
  - Reject with error: "Already subscribed"
  - Suggest: "Upgrade your plan" or "Wait for trial to end"
```

### EC-203: Renewal invoice already exists
```
GIVEN: Invoice exists for subscription renewal (period_start = current_period.end_at)
WHEN: Renewal job runs
THEN:
  - Skip creating duplicate invoice
  - Log: "Renewal invoice already exists"
```

### EC-204: Bundle purchase at limit
```
GIVEN: User has 3 paid invoices for bundle X
  AND: bundle.max_purchases_per_user = 3
WHEN: User attempts to purchase bundle X
THEN:
  - Reject with error: "Purchase limit reached for this bundle"
```

---

## 3. State Consistency

### EC-301: Active subscription with ended period
```
GIVEN: subscription.status = 'active'
  AND: current_period.status = 'ended'
WHEN: System health check runs
THEN:
  - Flag inconsistency
  - Either: create renewal period, OR transition subscription to paused/canceled
```

### EC-302: Paid invoice with pending payment
```
GIVEN: invoice.status = 'paid'
  AND: payment.status = 'pending'
WHEN: System health check runs
THEN:
  - Flag inconsistency
  - Reconcile by checking provider status
```

### EC-303: Active entitlement past active_to
```
GIVEN: entitlement.status = 'active'
  AND: entitlement.active_to < now()
WHEN: Entitlement check or background job runs
THEN:
  - Update entitlement.status = 'inactive'
```

---

## 4. Trial Scenarios

### EC-401: Trial ends with valid card
```
GIVEN: User on trial with valid payment_method
  AND: now() = trial_ends_at
WHEN: Trial conversion job runs
THEN:
  - Create invoice for first paid period
  - Attempt charge
  - On success: subscription.status = 'active', grant credits
```

### EC-402: Trial ends without payment method
```
GIVEN: User on trial, no payment_method on file (crypto flow)
  AND: now() = trial_ends_at
WHEN: Trial expiry job runs
THEN:
  - subscription.status = 'paused'
  - entitlement.status = 'inactive'
  - Emit event: 'subscription.trial_expired_no_payment'
```

### EC-403: Trial conversion payment fails
```
GIVEN: User on trial with card that declines
  AND: now() = trial_ends_at
WHEN: Trial conversion attempted
THEN:
  - subscription.status = 'paused' (NOT 'past_due' - no grace for trial)
  - entitlement.status = 'inactive'
  - NO dunning retries (differs from paid customer)
```

### EC-404: User cancels during trial
```
GIVEN: subscription.status = 'trialing'
WHEN: User clicks cancel
THEN:
  - subscription.status = 'canceled'
  - entitlement.active_to = now() (immediate, not trial_ends_at)
  - entitlement.status = 'inactive'
```

### EC-405: Trial with credits, user consumes and cancels
```
GIVEN: Plan with grant_credits_during_trial = true
  AND: User on trial, granted 1000 credits
  AND: User spent 800 credits
WHEN: User cancels trial
THEN:
  - subscription.status = 'canceled'
  - credits_balance = 200 (no clawback)
  - User keeps unused credits
```

---

## 5. Renewal Scenarios

### EC-501: Successful automatic renewal (card)
```
GIVEN: Active subscription, auto_renew = true
  AND: current_period.end_at = now() + 2 days
WHEN: Renewal job runs
THEN:
  - Invoice created for next period
  - Charge attempted
  - On success: new period created, credits granted, entitlement extended
```

### EC-502: Renewal with pending downgrade
```
GIVEN: Active subscription, pending_plan_id = Basic (from Pro)
WHEN: Renewal succeeds
THEN:
  - subscription.plan_id = Basic
  - subscription.pending_plan_id = null
  - Invoice uses Basic plan price
  - Credits grant = Basic plan amount
```

### EC-503: Renewal with cancel_at_period_end
```
GIVEN: Active subscription, cancel_at_period_end = true
  AND: current_period.end_at = now()
WHEN: Period end job runs
THEN:
  - NO renewal invoice created
  - subscription.status = 'canceled'
  - period.status = 'ended'
  - entitlement.status = 'inactive'
```

### EC-504: Crypto renewal not paid by due date
```
GIVEN: subscription with auto_renew = false
  AND: Renewal invoice open, due_at passed
WHEN: Period end job runs
THEN:
  - subscription.status = 'paused' (NOT 'past_due')
  - Message: "Renewal required" (NOT "Payment failed")
  - User can renew by paying new invoice
```

---

## 6. Payment Failure Scenarios

### EC-601: First payment failure
```
GIVEN: Renewal invoice created, charge attempted
WHEN: Payment fails (card declined)
THEN:
  - payment.status = 'failed'
  - subscription.status = 'past_due'
  - current_period.grace_end_at = now() + 7 days
  - entitlement remains active until grace_end_at
  - Schedule retry in 3 days
```

### EC-602: Payment recovered during grace
```
GIVEN: subscription.status = 'past_due', within grace period
WHEN: Retry payment succeeds
THEN:
  - payment.status = 'paid'
  - subscription.status = 'active'
  - New period starts now (NOT backdated)
  - Credits granted for new period
```

### EC-603: Grace period expires
```
GIVEN: subscription.status = 'past_due'
  AND: current_period.grace_end_at = now()
  AND: Invoice still unpaid
WHEN: Grace expiry job runs
THEN:
  - subscription.status = 'paused'
  - entitlement.status = 'inactive'
  - Emit event: 'subscription.grace_period_expired'
```

### EC-604: All retries exhausted
```
GIVEN: Invoice with 3 failed payment attempts
WHEN: Final retry fails
THEN:
  - invoice.status = 'uncollectible'
  - subscription.status = 'paused'
  - entitlement.status = 'inactive'
```

---

## 7. Cancellation Scenarios

### EC-701: Cancel at period end, then undo
```
GIVEN: subscription.cancel_at_period_end = true
  AND: Period has not ended yet
WHEN: User clicks "Undo cancel" / "Stay subscribed"
THEN:
  - subscription.cancel_at_period_end = false
  - Renewal will proceed normally
```

### EC-702: Cancel at period end with pending downgrade
```
GIVEN: subscription.pending_plan_id = Basic
  AND: User sets cancel_at_period_end = true
WHEN: Cancel processed
THEN:
  - subscription.pending_plan_id = null (cleared)
  - subscription.cancel_at_period_end = true
  - At period end: canceled (not downgraded)
```

### EC-703: Cancel during past_due
```
GIVEN: subscription.status = 'past_due'
WHEN: User cancels immediately
THEN:
  - subscription.status = 'canceled'
  - Outstanding invoice: void or leave for collections (config)
  - entitlement ends at current grace_end_at or now (config)
```

### EC-704: Cancel paused subscription
```
GIVEN: subscription.status = 'paused'
WHEN: User cancels
THEN:
  - subscription.status = 'canceled'
  - Already no entitlement, no change
```

---

## 8. Plan Change Scenarios

### EC-801: Upgrade same interval
```
GIVEN: User on Basic Monthly ($10/mo)
WHEN: User upgrades to Pro Monthly ($20/mo)
THEN:
  - subscription.plan_id = Pro immediately
  - Features: Pro level immediately
  - Price: $20 at next renewal (not now)
  - Credits: no mid-period grant
```

### EC-802: Downgrade same interval
```
GIVEN: User on Pro Monthly ($20/mo)
WHEN: User downgrades to Basic Monthly ($10/mo)
THEN:
  - subscription.pending_plan_id = Basic
  - subscription.plan_id remains Pro
  - Features: Pro until period end
  - At renewal: plan_id = Basic, $10 charged
```

### EC-803: Upgrade then downgrade same period
```
GIVEN: User upgrades Basic -> Pro mid-period
  AND: Later same period, user downgrades Pro -> Basic
WHEN: Processing downgrade
THEN:
  - subscription.plan_id = Pro (current)
  - subscription.pending_plan_id = Basic
  - At renewal: reverts to Basic
```

### EC-804: Monthly to yearly switch
```
GIVEN: User on Pro Monthly, 15 days left in period
WHEN: User switches to Pro Yearly
THEN:
  - Yearly invoice created
  - On payment: current monthly period ends (forfeit 15 days)
  - New yearly period starts now
  - Yearly credits granted
```

### EC-805: Yearly to monthly switch (blocked)
```
GIVEN: User on Pro Yearly, 6 months remaining
WHEN: User attempts Pro Monthly switch
THEN:
  - Reject: "Cannot switch to monthly until yearly period ends"
  - Suggest: Schedule for renewal
```

### EC-806: Plan archived while subscribed
```
GIVEN: User subscribed to plan X
WHEN: Admin archives plan X
THEN:
  - User's subscription continues
  - Renewals continue at current price
  - New users cannot subscribe to plan X
```

---

## 9. Bundle Scenarios

### EC-901: Bundle purchase without subscription
```
GIVEN: User has no active subscription
  AND: Bundle grants 5000 credits
WHEN: User purchases bundle
THEN:
  - Invoice created and paid
  - credits_balance += 5000
  - No subscription created
```

### EC-902: Bundle purchase with subscription
```
GIVEN: User has active subscription
  AND: Plan grants 1000 credits/period
  AND: Bundle grants 5000 credits
WHEN: User purchases bundle
THEN:
  - credits_balance += 5000 (same pool)
  - Total balance = previous + 5000
```

### EC-903: Bundle with feature unlock
```
GIVEN: Bundle includes feature "premium_export" (no credits)
WHEN: User purchases bundle
THEN:
  - entitlement created: kind='bundle_unlock', ref_id=invoice.id
  - hasFeature('premium_export') returns true
```

### EC-904: Bundle refund
```
GIVEN: User purchased bundle (5000 credits), used 2000
WHEN: Full refund processed
THEN:
  - credit_ledger_entry: delta = -5000
  - credits_balance = previous - 5000 (may go negative)
  - bundle entitlement revoked (if any)
```

---

## 10. Credit Scenarios

### EC-1001: Credit grant on period start
```
GIVEN: Plan with credits_grant_amount = 1000, cadence = 'per_period'
WHEN: Paid period starts
THEN:
  - credit_ledger_entry created: delta = +1000
  - credits_balance += 1000
  - subscription_period.credits_granted = 1000
```

### EC-1002: Credit grant on_start cadence
```
GIVEN: Plan with cadence = 'on_start'
  AND: User has previous periods
WHEN: New period starts
THEN:
  - NO credit grant (on_start = first period only)
```

### EC-1003: Yearly credits (default 1x)
```
GIVEN: Plan with credits_grant_amount = 1000, credits_yearly_multiply = false
WHEN: Yearly period paid
THEN:
  - credit_ledger_entry: delta = +1000 (not 12000)
```

### EC-1004: Yearly credits (12x opt-in)
```
GIVEN: Plan with credits_grant_amount = 1000, credits_yearly_multiply = true
WHEN: Yearly period paid
THEN:
  - credit_ledger_entry: delta = +12000
```

### EC-1005: Credit deduction success
```
GIVEN: credits_balance = 500
WHEN: Deduct 300 credits
THEN:
  - credit_ledger_entry: delta = -300
  - credits_balance = 200
```

### EC-1006: Credit deduction insufficient
```
GIVEN: credits_balance = 100
WHEN: Attempt to deduct 300 credits
THEN:
  - Reject: "Insufficient credits"
  - No ledger entry created
```

### EC-1007: Negative balance after refund
```
GIVEN: credits_balance = 200
  AND: User received 1000 credits from bundle
  AND: User spent 800 credits
WHEN: Bundle refunded (reverses 1000)
THEN:
  - credit_ledger_entry: delta = -1000
  - credits_balance = 200 - 1000 = -800
  - Negative balance allowed
```

### EC-1008: Grant to negative balance
```
GIVEN: credits_balance = -500
WHEN: New period grants 1000 credits
THEN:
  - credit_ledger_entry: delta = +1000
  - credits_balance = -500 + 1000 = 500
```

---

## 11. Refund Scenarios

### EC-1101: Full refund current period
```
GIVEN: Active subscription, current period paid
WHEN: Full refund initiated
THEN:
  - payment.status = 'refunded'
  - invoice.status = 'refunded'
  - subscription_period.status = 'revoked'
  - credits reversed (negative entry)
  - entitlement.active_to = now()
  - subscription.status = 'canceled'
```

### EC-1102: Full refund past period
```
GIVEN: Period already ended (status = 'ended')
WHEN: Full refund initiated
THEN:
  - payment/invoice refunded
  - period.status = 'revoked' (retroactive)
  - credits reversed (may go negative)
  - No entitlement change (already inactive)
```

### EC-1103: Partial refund
```
GIVEN: Invoice for $100, user requests $30 refund
WHEN: Partial refund processed
THEN:
  - Provider refund for $30
  - invoice.refund_amount = 3000 (cents)
  - invoice.status remains 'paid'
  - NO automatic credit reversal
  - Log for support review
```

### EC-1104: Yearly mid-year refund attempt
```
GIVEN: User on yearly plan, 6 months in
WHEN: User requests refund
THEN:
  - NOT auto-processed
  - Route to support queue
  - Support decides prorated amount manually
```

---

## 12. Dispute Scenarios

### EC-1201: Dispute opened
```
GIVEN: Paid invoice for subscription period
WHEN: Chargeback dispute created
THEN:
  - payment.status = 'disputed'
  - invoice.status = 'disputed'
  - subscription_period.status = 'revoked'
  - credits reversed
  - entitlement.status = 'inactive'
  - subscription.status = 'paused'
```

### EC-1202: Dispute won (merchant wins)
```
GIVEN: Disputed invoice/payment
WHEN: Dispute resolved in our favor
THEN:
  - payment.status = 'paid'
  - invoice.status = 'paid'
  - credit_ledger_entry: +original_credits (restore)
  - NO new subscription period (user must renew)
  - subscription remains 'paused' until user action
```

### EC-1203: Dispute lost (customer wins)
```
GIVEN: Disputed invoice/payment
WHEN: Dispute resolved in customer favor
THEN:
  - payment.status = 'refunded'
  - invoice.status = 'refunded'
  - No additional changes (already revoked)
```

### EC-1204: Dispute on bundle purchase
```
GIVEN: Paid bundle purchase
WHEN: Dispute opened
THEN:
  - credits reversed
  - bundle entitlement revoked (if any)
  - No subscription impact
```

---

## 13. Crypto-Specific Scenarios

### EC-1301: Crypto invoice expires without payment
```
GIVEN: Coinbase charge created, expires_at passed
  AND: No payment received
WHEN: Expiry check runs
THEN:
  - payment.status = 'expired'
  - invoice.status = 'void'
  - User can retry (creates new invoice/charge)
```

### EC-1302: Crypto underpayment
```
GIVEN: Invoice for $100, user sends $80 in BTC
WHEN: Coinbase reports underpaid
THEN:
  - payment.status remains 'pending'
  - Wait for remaining payment or expiry
  - If expires with partial: void invoice, Coinbase handles refund
```

### EC-1303: Crypto overpayment
```
GIVEN: Invoice for $100, user sends $120 in BTC
WHEN: Coinbase reports paid
THEN:
  - payment.status = 'paid' for $100
  - Log overpayment for records
  - Coinbase handles overage (refund or customer support)
```

### EC-1304: Late crypto payment (after invoice void)
```
GIVEN: Invoice voided due to expiry
  AND: Later, blockchain confirms payment
WHEN: Coinbase webhook arrives for old charge
THEN:
  - Log: "Payment received for voided invoice"
  - Alert support for manual reconciliation
  - DO NOT auto-process
```

### EC-1305: Crypto renewal flow
```
GIVEN: Subscription with auto_renew = false (crypto)
  AND: Period ending in 3 days
WHEN: Renewal job runs
THEN:
  - Create invoice (open)
  - Create Coinbase charge
  - Send email: "Renew your subscription" with checkout link
  - User must pay before period ends
```

---

## 14. Payment Method Scenarios

### EC-1401: Add first payment method
```
GIVEN: User has no payment methods
WHEN: User adds a card
THEN:
  - Create PaymentMethod record
  - Set is_default = true automatically
```

### EC-1402: Add second payment method
```
GIVEN: User has one payment method (default)
WHEN: User adds another card
THEN:
  - Create PaymentMethod record
  - is_default = false (original stays default)
```

### EC-1403: Remove only payment method with active sub
```
GIVEN: User has one payment method
  AND: Active subscription with auto_renew = true
WHEN: User attempts to remove payment method
THEN:
  - Warn: "Your subscription will fail to renew without a payment method"
  - Require confirmation or block removal
```

### EC-1404: Change default payment method
```
GIVEN: User has Card A (default) and Card B
WHEN: User sets Card B as default
THEN:
  - Card B: is_default = true
  - Card A: is_default = false
  - Next charge uses Card B
```

### EC-1405: Payment method expires
```
GIVEN: Card expires end of this month
  AND: Renewal due next month
WHEN: Stripe webhook: payment_method.card_automatically_updated
THEN:
  - Update our PaymentMethod record with new expiry
  - If no update from Stripe: notify user to update card
```

---

## 15. Concurrent Operation Scenarios

### EC-1501: Simultaneous cancel and renewal
```
GIVEN: Period ending now
  AND: User clicks cancel at same moment renewal job runs
WHEN: Both operations attempt to process
THEN:
  - Use database locking on subscription
  - One wins: either cancel completes and renewal skipped,
    or renewal completes then cancel applies to next period
  - No inconsistent state
```

### EC-1502: Double webhook delivery
```
GIVEN: Stripe sends webhook twice (network retry)
WHEN: Both arrive within milliseconds
THEN:
  - First processes normally
  - Second is idempotent no-op (same provider_payment_id)
```

### EC-1503: Upgrade during renewal processing
```
GIVEN: Renewal invoice created, payment pending
WHEN: User upgrades plan before payment confirms
THEN:
  - Upgrade modifies subscription.plan_id
  - Payment confirms for old price (this period)
  - Next renewal uses new plan price
```

---

## 16. Boundary Conditions

### EC-1601: Zero-amount invoice (100% discount)
```
GIVEN: Plan price = $0 (free tier or 100% coupon)
WHEN: Subscription created
THEN:
  - Invoice created with amount_due = 0
  - invoice.status = 'paid' immediately (no payment needed)
  - Period, credits, entitlement granted normally
```

### EC-1602: Subscription to archived plan (migration)
```
GIVEN: Plan archived
WHEN: Admin migrates user to archived plan (support case)
THEN:
  - Allowed (admin override)
  - User continues on archived plan
  - Should typically migrate to active plan instead
```

### EC-1603: Credits exactly zero
```
GIVEN: credits_balance = 0
WHEN: Attempt to deduct any credits
THEN:
  - Reject: "Insufficient credits"
```

### EC-1604: Very long trial (edge of int range)
```
GIVEN: Plan with trial_days = 365
WHEN: Trial subscription created
THEN:
  - trial_ends_at = now() + 365 days
  - System handles correctly (no overflow)
```

### EC-1605: Invoice due_at in past
```
GIVEN: Invoice created with due_at = yesterday (clock skew or bug)
WHEN: Processing
THEN:
  - Still process normally
  - Log warning about past due_at
  - Don't immediately mark uncollectible
```

### EC-1606: Entitlement active_to exactly now
```
GIVEN: entitlement.active_to = now() (exact millisecond)
WHEN: hasActivePlan() check
THEN:
  - Return false (active_to must be > now, not >=)
  - Consistent: expired at the second it hits active_to
```

---

## Test Data Requirements

For comprehensive testing, create fixtures for:

1. **Plans:** Free, Basic ($10/mo), Pro ($20/mo), Enterprise ($100/mo), Pro Yearly ($200/yr)
2. **Bundles:** Small (1000 credits, $10), Large (10000 credits, $50, max 3 per user)
3. **Users:** New user, Trial user, Active subscriber, Past-due subscriber, Paused subscriber, Canceled subscriber
4. **Payment methods:** Valid card, Expiring card, Decline-always card (test mode)
5. **Invoices:** Draft, Open, Paid, Refunded, Disputed
6. **Credit states:** Positive balance, Zero balance, Negative balance

---

## Running Edge Case Tests

These edge cases serve as acceptance criteria. Implement as integration tests in the Rust test suite using `#[cfg(test)]` modules. Each EC-XXXX identifier maps to a test case.

```
Run all billing edge case tests:
  cargo test --package api billing::edge_cases

Run specific category (e.g., credit scenarios):
  cargo test --package api billing::edge_cases::credits

Run single test:
  cargo test --package api billing::edge_cases::ec_1007
```
