# reauth.dev Billing System - Flows

Step-by-step implementation flows with pseudocode. Implement in this order.

---

## Table of Contents

1. [New Subscription (No Trial, Card)](#1-new-subscription-no-trial-card)
2. [New Subscription (No Trial, Crypto)](#2-new-subscription-no-trial-crypto)
3. [New Subscription (With Trial, Card)](#3-new-subscription-with-trial-card)
4. [New Subscription (With Trial, Crypto)](#4-new-subscription-with-trial-crypto)
5. [Trial Conversion](#5-trial-conversion)
6. [Renewal (Card - Automatic)](#6-renewal-card---automatic)
7. [Renewal (Crypto - Manual)](#7-renewal-crypto---manual)
8. [Payment Failure & Dunning (Card)](#8-payment-failure--dunning-card)
9. [Cancellation](#9-cancellation)
10. [Reactivation](#10-reactivation)
11. [Plan Upgrade](#11-plan-upgrade)
12. [Plan Downgrade](#12-plan-downgrade)
13. [Bundle Purchase](#13-bundle-purchase)
14. [Refund (Full)](#14-refund-full)
15. [Refund (Partial)](#15-refund-partial)
16. [Dispute / Chargeback](#16-dispute--chargeback)
17. [Webhook Handlers](#17-webhook-handlers)
18. [Credit Operations](#18-credit-operations)
19. [Entitlement Checks](#19-entitlement-checks)
20. [Background Jobs](#20-background-jobs)

---

## 1. New Subscription (No Trial, Card)

**Trigger:** User selects plan, provides card, clicks subscribe.

### Steps

```
1. INPUT: billing_customer_id, plan_id, payment_method_id

2. VALIDATE:
   - Plan exists and status = 'active'
   - Plan.trial_days = 0 or null
   - No existing active subscription for this customer
   - Payment method exists and belongs to customer

3. BEGIN TRANSACTION

4. CREATE subscription:
   - app_id: plan.app_id
   - billing_customer_id: billing_customer_id
   - plan_id: plan_id
   - status: 'active'  // set to active in same transaction as payment creation;
                        // if payment fails via webhook, subscription transitions to paused
   - auto_renew: true
   - current_period_id: null  // set after period created via webhook
   - cancel_at_period_end: false

5. CREATE invoice:
   - app_id: plan.app_id
   - billing_customer_id: billing_customer_id
   - purpose: 'subscription_period'
   - amount_due: plan.price_amount
   - currency: plan.price_currency
   - status: 'open'
   - due_at: now()
   - metadata: {
       subscription_id: subscription.id,
       plan_id: plan_id,
       period_start: now(),
       period_end: now() + plan.billing_interval
     }

6. IF invoice.amount_due = 0:
   - SKIP payment creation (free tier or 100% discount)
   - UPDATE invoice.status = 'paid', paid_at = now()
   - Proceed directly to period/entitlement/credit grant (same as webhook success)
   - GOTO step 8 (COMMIT), then run "On Payment Success" logic inline

   ELSE:
   CALL stripe_adapter.create_payment_intent(
     amount: invoice.amount_due,
     currency: invoice.currency,
     customer: provider_customer_ref.provider_customer_id,
     payment_method: payment_method.provider_payment_method_id,
     confirm: true,
     off_session: false
   )

7. CREATE payment:
   - invoice_id: invoice.id
   - provider: 'stripe'
   - provider_payment_id: payment_intent.id
   - amount: invoice.amount_due
   - currency: invoice.currency
   - status: 'pending'

8. COMMIT TRANSACTION

9. RETURN { subscription_id, invoice_id, payment_status: 'pending' }

// Payment confirmation happens via webhook (see section 17)
```

### On Payment Success (via webhook)

```
1. FIND payment by provider_payment_id
2. IF payment.status = 'paid': RETURN (idempotent)

3. BEGIN TRANSACTION

4. UPDATE payment:
   - status: 'paid'
   - confirmed_at: now()

5. UPDATE invoice:
   - status: 'paid'
   - paid_at: now()

6. CREATE subscription_period:
   - subscription_id: invoice.metadata.subscription_id
   - IF subscription.status = 'past_due' (recovery per D02):
     - start_at: now()
     - end_at: now() + billing_interval
   - ELSE (initial or normal renewal):
     - start_at: invoice.metadata.period_start
     - end_at: invoice.metadata.period_end
   - status: 'active'
   - invoice_id: invoice.id
   - is_trial: false

7. UPDATE subscription:
   - current_period_id: subscription_period.id
   - status: 'active'

8. CREDIT GRANT (if applicable):
   - SKIP if plan.credits_grant_amount = 0
   - CHECK cadence:
     - IF plan.credits_grant_cadence = 'on_start':
       - QUERY: does customer have any previous paid period for this subscription?
       - IF yes: SKIP credit grant (on_start = first period only)
     - IF plan.credits_grant_cadence = 'per_period': always grant
   - CALCULATE amount:
     - base = plan.credits_grant_amount
     - IF plan.billing_interval = 'year' AND plan.credits_yearly_multiply = true:
       - amount = base * 12  (per D13)
     - ELSE:
       - amount = base
   - CREATE credit_ledger_entry:
     - source_type: 'subscription_period'
     - source_id: subscription_period.id
     - delta: amount
   - UPDATE subscription_period.credits_granted = amount
   - UPDATE billing_customer.credits_balance += amount

9. UPSERT entitlement:
   - kind: 'plan_access'
   - ref_type: 'subscription'
   - ref_id: subscription.id
   - active_from: subscription_period.start_at
   - active_to: subscription_period.end_at
   - status: 'active'

10. EMIT event: 'subscription.activated'

11. COMMIT TRANSACTION
```

---

## 2. New Subscription (No Trial, Crypto)

**Trigger:** User selects plan, chooses crypto payment.

### Steps

```
1. INPUT: billing_customer_id, plan_id

2. VALIDATE:
   - Plan exists and status = 'active'
   - No existing active subscription for this customer

3. BEGIN TRANSACTION

4. CREATE subscription:
   - status: 'active'  // set active; if payment never arrives, period expiry job pauses it
   - auto_renew: false  // crypto = manual renewal
   - (rest same as card flow)

5. CREATE invoice:
   - (same as card flow)

6. CALL coinbase_adapter.create_charge(
     name: plan.name,
     description: "Subscription to {plan.name}",
     pricing_type: 'fixed_price',
     local_price: { amount: invoice.amount_due / 100, currency: invoice.currency.upper() },
     metadata: { invoice_id: invoice.id }
   )

7. CREATE payment:
   - provider: 'coinbase'
   - provider_payment_id: charge.id
   - status: 'pending'
   - (amount, currency same)

8. COMMIT TRANSACTION

9. RETURN {
     subscription_id,
     invoice_id,
     checkout_url: charge.hosted_url,
     expires_at: charge.expires_at
   }

// User completes payment on Coinbase hosted page
// Confirmation via webhook
```

---

## 3. New Subscription (With Trial, Card)

**Trigger:** User selects plan with trial, provides card.

### Steps

```
1. INPUT: billing_customer_id, plan_id, payment_method_id

2. VALIDATE:
   - Plan.trial_days > 0
   - No existing active subscription
   - Payment method exists (required to charge after trial)

3. BEGIN TRANSACTION

4. CREATE subscription:
   - status: 'trialing'
   - auto_renew: true
   - trial_ends_at: now() + plan.trial_days

5. CREATE subscription_period (trial):
   - start_at: now()
   - end_at: now() + plan.trial_days
   - status: 'active'
   - invoice_id: null  // no payment for trial
   - is_trial: true

6. UPDATE subscription.current_period_id = subscription_period.id

7. IF plan.grant_credits_during_trial AND plan.credits_grant_amount > 0:
   - CREATE credit_ledger_entry (same as paid period)
   - UPDATE subscription_period.credits_granted
   - UPDATE billing_customer.credits_balance

8. UPSERT entitlement:
   - active_from: now()
   - active_to: trial_end
   - status: 'active'

9. COMMIT TRANSACTION

10. SCHEDULE job: trial_conversion at (trial_ends_at - lead_time)

11. RETURN { subscription_id, trial_ends_at }
```

---

## 4. New Subscription (With Trial, Crypto)

**Trigger:** User selects plan with trial, no card needed.

### Steps

```
Same as (3) but:
- auto_renew: false
- No payment_method_id required
- Schedule job shows "Renew" UI instead of auto-charging
```

---

## 5. Trial Conversion

**Trigger:** Background job runs at `trial_ends_at - lead_time` (e.g., 3 days before).

### Card Provider

```
1. QUERY subscriptions WHERE status = 'trialing' AND trial_ends_at <= now() + lead_time

2. FOR EACH subscription:

   a. CHECK: Invoice for conversion already exists?
      - QUERY invoice WHERE metadata.subscription_id = subscription.id
        AND purpose = 'subscription_period'
        AND metadata.period_start = subscription.trial_ends_at
      - IF exists AND status = 'paid': SKIP (already converted)
      - IF exists AND status = 'open': SKIP (payment pending)

   b. GET payment_method (default) for billing_customer

   c. IF no payment_method:
      - UPDATE subscription.status = 'paused'
      - EMIT event: 'subscription.trial_ended_no_payment_method'
      - CONTINUE

   d. CREATE invoice:
      - purpose: 'subscription_period'
      - amount_due: plan.price_amount
      - status: 'open'
      - due_at: subscription.trial_ends_at
      - metadata: {
          subscription_id, plan_id,
          period_start: trial_ends_at,
          period_end: trial_ends_at + billing_interval
        }

   e. ATTEMPT off-session charge via adapter

   f. CREATE payment (pending)

   g. // Success/failure handled by webhook
```

### Crypto Provider

```
Same flow but:
- Don't attempt charge
- EMIT event: 'subscription.trial_ending_renewal_required'
- Send email: "Your trial ends on X. Click here to subscribe."
- If not paid by trial_ends_at:
  - UPDATE subscription.status = 'paused'
  - entitlement.status = 'inactive'
```

---

## 6. Renewal (Card - Automatic)

**Trigger:** Background job runs daily, checks periods ending soon.

```
1. QUERY subscription_periods WHERE:
   - status = 'active'
   - is_trial = false
   - end_at <= now() + lead_time (e.g., 3 days)
   - subscription.status = 'active'
   - subscription.auto_renew = true
   - subscription.cancel_at_period_end = false

2. FOR EACH period:

   a. CHECK: Renewal invoice already exists?
      - QUERY invoice WHERE metadata.subscription_id = subscription.id
        AND metadata.period_start = period.end_at
      - IF exists: SKIP

   b. GET plan (check for pending_plan_id, use that price if downgrade scheduled)
      - effective_plan = subscription.pending_plan_id ?? subscription.plan_id
      - price = subscription.locked_price_amount ?? plan.price_amount

   c. CREATE invoice:
      - amount_due: price
      - due_at: period.end_at
      - metadata: {
          subscription_id, plan_id: effective_plan,
          period_start: period.end_at,
          period_end: period.end_at + billing_interval
        }

   d. GET default payment_method

   e. ATTEMPT off-session charge:
      - stripe_adapter.create_payment_intent(..., off_session: true)

   f. CREATE payment (pending)

   g. // Webhook handles success/failure
```

### On Renewal Success (webhook)

```
1. Same as "New Subscription - On Payment Success" but also:

2. IF subscription.pending_plan_id:
   - UPDATE subscription.plan_id = pending_plan_id
   - UPDATE subscription.pending_plan_id = null
   - UPDATE subscription.locked_price_amount = null  // reset to new plan's current price
   - EMIT event: 'subscription.plan_changed'

3. UPDATE old period.status = 'ended'
```

---

## 7. Renewal (Crypto - Manual)

**Trigger:** Background job, same timing as card.

```
1. QUERY same as card, but auto_renew = false

2. FOR EACH period:

   a. CHECK: Renewal invoice exists?

   b. IF NOT exists:
      - CREATE invoice (status: 'open')
      - CREATE coinbase charge
      - CREATE payment (pending)
      - EMIT event: 'subscription.renewal_required'
      - Send email with checkout_url

   c. IF exists AND status = 'open':
      - CHECK: charge expired?
      - IF expired:
        - UPDATE payment.status = 'expired'
        - UPDATE invoice.status = 'void'
        - CREATE new invoice + charge
        - EMIT event: 'subscription.renewal_invoice_refreshed'

3. AT period.end_at (separate job):
   - IF no paid invoice for next period:
     - UPDATE subscription.status = 'paused'
     - UPDATE period.status = 'ended'
     - UPDATE entitlement.status = 'inactive'
     - EMIT event: 'subscription.paused_renewal_required'
```

---

## 8. Payment Failure & Dunning (Card)

**Trigger:** Webhook receives `payment_intent.payment_failed`.

```
1. FIND payment by provider_payment_id
2. IF payment.status IN ['failed', 'canceled']: RETURN

3. BEGIN TRANSACTION

4. UPDATE payment.status = 'failed'

5. GET invoice, subscription

6. IF subscription.status = 'active':
   - UPDATE subscription.status = 'past_due'

7. SET grace period:
   - UPDATE current_period.grace_end_at = now() + grace_days (e.g., 7 days)
   - // Entitlement remains active until grace_end_at

8. SCHEDULE retries per D21 (RETRY_SCHEDULE = [0, 3, 7] days from initial failure):
   - Day 0: initial attempt (this failure)
   - Day 3: retry 1
   - Day 7: retry 2 (final)

9. EMIT event: 'invoice.payment_failed'
10. Send email: "Payment failed, please update your card"

11. COMMIT TRANSACTION
```

### Retry Logic

```
1. QUERY invoices WHERE status = 'open' AND has failed payment AND retry_count < 3

2. FOR EACH invoice:
   - CHECK retry_schedule: [0, 3, 7] days from initial failure (per D21)
   - IF not yet time for next retry: SKIP
   - ATTEMPT off-session charge
   - IF success: webhook handles (subscription -> active, grant credits, etc.)
   - IF failure:
     - INCREMENT retry_count
     - IF final retry (attempt 3) failed:
       - UPDATE invoice.status = 'uncollectible'
       - UPDATE subscription.status = 'paused'
       - UPDATE entitlement.status = 'inactive'
       - EMIT event: 'subscription.dunning_exhausted'
```

---

## 9. Cancellation

### Cancel at Period End

```
1. INPUT: subscription_id

2. VALIDATE: subscription.status IN ['trialing', 'active', 'past_due']

3. BEGIN TRANSACTION

4. UPDATE subscription:
   - cancel_at_period_end: true
   - pending_plan_id: null  // clear any scheduled downgrade

5. EMIT event: 'subscription.cancel_scheduled'

6. COMMIT TRANSACTION

// At period end, background job:
7. IF cancel_at_period_end = true AND now() >= current_period.end_at:
   - UPDATE subscription.status = 'canceled'
   - UPDATE subscription.canceled_at = now()
   - UPDATE period.status = 'ended'
   - UPDATE entitlement.status = 'inactive'
   - EMIT event: 'subscription.canceled'
```

### Cancel Immediately

```
1. INPUT: subscription_id

2. VALIDATE: subscription.status IN ['trialing', 'active', 'past_due', 'paused']

3. BEGIN TRANSACTION

4. UPDATE subscription:
   - status: 'canceled'
   - canceled_at: now()
   - cancel_at_period_end: false

5. UPDATE current_period.status = 'ended'  // or keep 'active' until end_at for access

6. // Decision: Do they keep access until period.end_at?
   // Recommendation: YES (they paid for it)
   // So entitlement.active_to remains at period.end_at

7. EMIT event: 'subscription.canceled'

8. COMMIT TRANSACTION
```

### Cancel During Trial

```
Same as immediate cancel, but:
- entitlement ends immediately (they didn't pay)
- UPDATE entitlement.active_to = now()
- UPDATE entitlement.status = 'inactive'
```

### Undo Cancel (before period ends)

```
1. VALIDATE: subscription.cancel_at_period_end = true AND now() < current_period.end_at

2. UPDATE subscription.cancel_at_period_end = false

3. EMIT event: 'subscription.cancel_undone'
```

---

## 10. Reactivation

**Trigger:** User clicks "Resubscribe" while paused/canceled.

### From Paused

```
1. INPUT: subscription_id, payment_method_id (or crypto)

2. VALIDATE: subscription.status = 'paused'

3. CREATE invoice for new period:
   - period_start: now()
   - period_end: now() + billing_interval

4. COLLECT payment (same as new subscription)

5. ON success:
   - UPDATE subscription.status = 'active'
   - CREATE new subscription_period
   - Grant credits
   - Update entitlement
```

### From Canceled

```
1. Treat as NEW subscription (canceled = terminal)
2. Could offer same plan at current price
3. New subscription_period starts now
```

---

## 11. Plan Upgrade

**Trigger:** User changes to higher-tier plan (same billing interval).

```
1. INPUT: subscription_id, new_plan_id

2. VALIDATE:
   - subscription.status IN ['active', 'trialing']
   - new_plan.billing_interval = current_plan.billing_interval
   - new_plan.price_amount > current_plan.price_amount (upgrade)

3. BEGIN TRANSACTION

4. UPDATE subscription.plan_id = new_plan_id

5. // Features: IMMEDIATE
   // User can now access new plan features

6. // Price: AT NEXT RENEWAL
   // Next invoice will use new_plan.price_amount

7. // Credits: NO MID-PERIOD ADJUSTMENT
   // Prevents gaming

8. EMIT event: 'subscription.upgraded'

9. COMMIT TRANSACTION
```

---

## 12. Plan Downgrade

**Trigger:** User changes to lower-tier plan (same billing interval).

```
1. INPUT: subscription_id, new_plan_id

2. VALIDATE:
   - subscription.status IN ['active', 'trialing']
   - new_plan.billing_interval = current_plan.billing_interval
   - new_plan.price_amount < current_plan.price_amount (downgrade)

3. BEGIN TRANSACTION

4. // Schedule for period end (they paid for current tier)
   UPDATE subscription.pending_plan_id = new_plan_id

5. EMIT event: 'subscription.downgrade_scheduled'

6. COMMIT TRANSACTION

// At renewal (see flow 6):
7. subscription.plan_id = pending_plan_id
8. subscription.pending_plan_id = null
9. New period uses new plan price and credits
```

### Immediate Downgrade (alternative)

```
If developer configures immediate downgrades:
1. UPDATE subscription.plan_id = new_plan_id
2. No refund for remaining time on higher tier
3. EMIT event: 'subscription.downgraded'
```

---

## 13. Bundle Purchase

**Trigger:** User buys a bundle (one-time).

```
1. INPUT: billing_customer_id, bundle_id, payment_provider

2. VALIDATE:
   - Bundle exists and status = 'active'
   - IF bundle.max_purchases_per_user:
     - COUNT paid invoices for this bundle + customer
     - IF count >= max: REJECT "Purchase limit reached"

3. BEGIN TRANSACTION

4. CREATE invoice:
   - purpose: 'bundle_purchase'
   - amount_due: bundle.price_amount
   - metadata: { bundle_id }

5. CREATE payment via adapter (Stripe or Coinbase)

6. COMMIT TRANSACTION

7. RETURN { invoice_id, checkout_url? }
```

### On Payment Success

```
1. UPDATE payment.status = 'paid'
2. UPDATE invoice.status = 'paid'

3. IF bundle.credits_grant_amount > 0:
   - CREATE credit_ledger_entry:
     - source_type: 'bundle'
     - source_id: invoice.id
     - delta: bundle.credits_grant_amount
   - UPDATE billing_customer.credits_balance

4. IF bundle.features (non-credit perks):
   - CREATE entitlement:
     - kind: 'bundle_unlock'
     - ref_type: 'invoice'
     - ref_id: invoice.id
     - active_to: null  // perpetual, or set expiry

5. EMIT event: 'bundle.purchased'
```

---

## 14. Refund (Full)

**Trigger:** Admin initiates refund for invoice.

### Subscription Period Refund

```
1. INPUT: invoice_id, reason (required)

2. VALIDATE:
   - invoice.status = 'paid'
   - invoice.purpose = 'subscription_period'

3. BEGIN TRANSACTION

4. CALL adapter.refund(payment.provider_payment_id, amount: payment.amount)

5. UPDATE payment.status = 'refunded'
6. UPDATE invoice:
   - status: 'refunded'
   - refund_amount: invoice.amount_due
   - refunded_at: now()
   - refund_reason: reason

7. GET subscription_period by invoice_id

8. UPDATE subscription_period.status = 'revoked'

9. IF subscription_period.credits_granted > 0:
   - CREATE credit_ledger_entry:
     - source_type: 'refund_reversal'
     - source_id: invoice.id
     - delta: -subscription_period.credits_granted
   - UPDATE billing_customer.credits_balance

10. UPDATE entitlement:
    - active_to: now()
    - status: 'inactive'

11. UPDATE subscription.status = 'canceled'

12. EMIT event: 'subscription.refunded'

13. COMMIT TRANSACTION
```

### Bundle Refund

```
Same pattern:
1. Refund via adapter
2. Update payment, invoice
3. Reverse credits
4. Revoke entitlement (if bundle unlock)
```

---

## 15. Refund (Partial)

**v1: Manual handling only.**

```
1. INPUT: invoice_id, refund_amount, reason (required)

2. VALIDATE: refund_amount < invoice.amount_due

3. CALL adapter.refund(payment.provider_payment_id, amount: refund_amount)

4. UPDATE payment, invoice (no status change, just record partial)
   - invoice.refund_amount = refund_amount

5. // DO NOT automatically reverse credits or revoke access
   // This is a support/goodwill refund

6. LOG for support review

7. EMIT event: 'invoice.partially_refunded'
```

---

## 16. Dispute / Chargeback

### Dispute Opened

```
1. WEBHOOK: dispute.created (Stripe) or charge:dispute:created (Coinbase)

2. FIND payment by provider_payment_id

3. BEGIN TRANSACTION

4. UPDATE payment.status = 'disputed'
5. UPDATE invoice.status = 'disputed'

6. // REVOKE ACCESS IMMEDIATELY
   IF invoice.purpose = 'subscription_period':
     - GET subscription_period
     - UPDATE period.status = 'revoked'
     - IF period.credits_granted > 0:
       - CREATE credit_ledger_entry (delta: -credits_granted, source_type: 'dispute_reversal')
       - UPDATE billing_customer.credits_balance
     - UPDATE entitlement.status = 'inactive'
     - UPDATE subscription.status = 'paused'

   IF invoice.purpose = 'bundle_purchase':
     - Reverse credits
     - Revoke bundle entitlement

7. EMIT event: 'invoice.disputed'

8. COMMIT TRANSACTION
```

### Dispute Won (Merchant Wins)

```
1. WEBHOOK: dispute.won

2. FIND payment

3. BEGIN TRANSACTION

4. UPDATE payment.status = 'paid'  // restored
5. UPDATE invoice.status = 'paid'  // restored

6. // RESTORE CREDITS ONLY, NOT TIME
   - CREATE credit_ledger_entry:
     - source_type: 'dispute_won_restoration'
     - delta: +original_credits_granted

7. // DO NOT reinstate subscription period
   // User must renew if they want access

8. EMIT event: 'invoice.dispute_won'

9. COMMIT TRANSACTION
```

### Dispute Lost (Customer Wins)

```
1. WEBHOOK: dispute.lost

2. FIND payment

3. UPDATE payment.status = 'refunded'
4. UPDATE invoice.status = 'refunded'

5. // Already revoked, no further action

6. EMIT event: 'invoice.dispute_lost'
```

---

## 17. Webhook Handlers

### Stripe Webhook Handler

```
1. Verify stripe-signature header against webhook secret
2. Parse event from request body

3. Check idempotency:
   - Query billing_event for matching stripe_event_id
   - If already processed: return 200 OK immediately

4. Route by event type:
   - payment_intent.succeeded    -> handlePaymentSucceeded
   - payment_intent.payment_failed -> handlePaymentFailed
   - charge.refunded             -> handleRefund
   - charge.dispute.created      -> handleDisputeCreated
   - charge.dispute.closed       -> handleDisputeClosed

5. Log event to billing_event table for idempotency tracking:
   - event_type: "stripe.{event.type}"
   - metadata: { stripe_event_id: event.id }
   - payload: event data

6. Return 200 OK
```

### Coinbase Webhook Handler

```
1. Verify x-cc-webhook-signature header against webhook secret
2. Parse event from request body

3. Route by event type:
   - charge:confirmed -> handleCoinbasePaymentConfirmed
   - charge:failed    -> handleCoinbasePaymentFailed
   - charge:delayed   -> handleCoinbaseDelayedPayment (payment after expiry)
   - charge:pending   -> (awaiting blockchain confirmation, no action)
   - charge:resolved  -> (manual resolution, log only)

4. Return 200 OK
```

### Generic Webhook Handler Pattern

```
1. Verify signature (provider-specific)
2. Check idempotency (has this event been processed?)
3. Route to handler based on event type
4. Process in database transaction:
   - Execute handler logic
   - Mark event as processed
5. Emit internal billing event
6. Return 200 OK (always acknowledge to prevent retries)
```

---

## 18. Credit Operations

### Deduct Credits

```
1. INPUT: billing_customer_id, amount, reason

2. GET current balance (from cache or SUM)

3. IF balance < amount:
   - RETURN error: 'insufficient_credits'

4. BEGIN TRANSACTION

5. CREATE credit_ledger_entry:
   - source_type: 'adjustment'
   - delta: -amount
   - note: reason

6. UPDATE billing_customer.credits_balance -= amount

7. COMMIT TRANSACTION

8. RETURN { new_balance }
```

### Grant Credits (Manual)

```
1. INPUT: billing_customer_id, amount, reason, admin_user_id

2. BEGIN TRANSACTION

3. CREATE credit_ledger_entry:
   - source_type: 'manual'
   - delta: +amount
   - note: reason
   - admin_user_id: admin_user_id

4. UPDATE billing_customer.credits_balance += amount

5. EMIT event: 'credits.manual_grant'

6. COMMIT TRANSACTION
```

---

## 19. Entitlement Checks

### Check Plan Access

```
Query entitlement WHERE:
  - billing_customer_id = target customer
  - kind = 'plan_access'
  - status = 'active'
  - active_from <= now()
  - active_to IS NULL OR active_to > now()

If row found: customer has active plan access.
```

### Check Feature Access

```
1. Check plan features via active entitlement (not subscription status):
   Query entitlement WHERE:
     - billing_customer_id = target customer
     - kind = 'plan_access'
     - status = 'active'
     - active_from <= now()
     - active_to IS NULL OR active_to > now()
   If found, get subscription -> plan.features for the requested feature key
   - If feature present: return true

   NOTE: This uses entitlements, not subscription.status. A canceled subscription
   with D04 access (paid period not yet ended) still has an active entitlement,
   so feature checks work correctly.

2. Check bundle entitlements:
   Query entitlement WHERE:
     - billing_customer_id = target customer
     - kind = 'bundle_unlock'
     - status = 'active'
     - active_from <= now()
     - active_to IS NULL OR active_to > now()
   Join to invoice and bundle to check bundle.features for the feature key

3. If found in any source: return true
   Otherwise: return false
```

---

## 20. Background Jobs

### Job: Check Trial Conversions

```
SCHEDULE: Every hour

1. QUERY subscriptions WHERE status = 'trialing' AND trial_ends_at <= now() + 3 days
2. FOR EACH: Run trial conversion flow
```

### Job: Check Renewals

```
SCHEDULE: Every hour

1. QUERY active subscription_periods ending within lead_time
2. FOR EACH: Run renewal flow (card or crypto)
```

### Job: Check Expired Periods

```
SCHEDULE: Every 15 minutes

1. QUERY subscription_periods WHERE status = 'active' AND end_at <= now()
2. FOR EACH:
   - IF subscription.cancel_at_period_end: Cancel subscription
   - ELIF subscription.auto_renew = false AND no paid renewal: Pause subscription
   - ELSE: Mark period as 'ended' (renewal already processed)
```

### Job: Check Grace Periods

```
SCHEDULE: Every hour

1. QUERY subscription_periods WHERE grace_end_at <= now() AND subscription.status = 'past_due'
2. FOR EACH:
   - UPDATE subscription.status = 'paused'
   - UPDATE entitlement.status = 'inactive'
   - EMIT event: 'subscription.grace_period_expired'
```

### Job: Expire Stale Crypto Invoices

```
SCHEDULE: Every hour

1. QUERY invoices WHERE status = 'open' AND purpose = 'subscription_period'
   AND due_at < now() - 24 hours
   AND payment.provider = 'coinbase'
2. FOR EACH:
   - UPDATE invoice.status = 'void'
   - UPDATE payment.status = 'expired'
```
