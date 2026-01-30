# reauth.dev Billing System - Decisions

All policy decisions, locked in. Follow these exactly. Do not deviate.

---

## Decision Index

| ID | Topic | Decision |
|----|-------|----------|
| D01 | [Trial conversion grace](#d01-trial-conversion-failure-grace-period) | No grace period |
| D02 | [Period start after recovery](#d02-period-start-after-past_due-recovery) | Starts now, not backdated |
| D03 | [Expired crypto invoice](#d03-expired-crypto-invoice-handling) | Void and create new |
| D04 | [Cancel immediate access](#d04-cancel-immediately-access) | Keep until period end |
| D05 | [Downgrade timing](#d05-downgrade-timing) | At period end |
| D06 | [Yearly → monthly](#d06-yearly--monthly-switch) | Blocked until year ends |
| D07 | [Monthly → yearly](#d07-monthly--yearly-switch) | Immediate, old period forfeit |
| D08 | [Price changes](#d08-price-change-for-existing-subscribers) | Grandfather until plan change |
| D09 | [Chargeback access](#d09-chargeback-access-revocation) | Revoke immediately |
| D10 | [Chargeback won](#d10-chargeback-won-restoration) | Restore credits only |
| D11 | [Partial refund credits](#d11-partial-refund-credits) | Manual only |
| D12 | [Credit expiration](#d12-credit-expiration) | Never expire |
| D13 | [Yearly credits](#d13-yearly-credits-grant) | 1x default, 12x opt-in |
| D14 | [Upgrade credits](#d14-mid-period-upgrade-credits) | No adjustment |
| D15 | [Negative balance](#d15-negative-credit-balance) | Allowed |
| D16 | [Trial credits](#d16-credits-during-trial) | Not granted by default |
| D17 | [Downgrade credits](#d17-downgrade-credit-clawback) | No clawback |
| D18 | [Reactivation](#d18-reactivation-pricing) | Current plan price |
| D19 | [Renewal lead time](#d19-renewal-lead-time) | 3 days |
| D20 | [Grace period duration](#d20-grace-period-duration) | 7 days |
| D21 | [Max retry attempts](#d21-max-payment-retry-attempts) | 3 attempts |
| D22 | [Yearly refund](#d22-yearly-subscription-refund) | Manual/support only |
| D23 | [Bundle credits pool](#d23-bundle-credits-pool) | Shared with plan credits |
| D24 | [Multiple subscriptions](#d24-multiple-subscriptions-per-customer) | One per customer |
| D25 | [Trial cancellation access](#d25-trial-cancellation-access) | Ends immediately |

---

## D01: Trial Conversion Failure Grace Period

**Decision:** No grace period for trial conversion failure. Access ends at trial end.

**Rationale:** Grace periods are for existing paying customers who have demonstrated value. Trial users haven't paid yet. If conversion fails, they can fix their payment method and start a new subscription.

**Implementation:**
```
IF subscription.status = 'trialing' AND payment_failed:
  - entitlement ends at trial_ends_at
  - subscription.status = 'paused' (not 'past_due')
  - no dunning emails (just "subscribe to continue")
```

---

## D02: Period Start After past_due Recovery

**Decision:** New period starts from recovery date, not backdated to original renewal date.

**Rationale:** Don't gift free days during the time they were in past_due. They had access during grace period; that's enough. Backdating would effectively extend their subscription for free.

**Implementation:**
```
ON payment_recovered:
  - subscription_period.start_at = now()
  - subscription_period.end_at = now() + billing_interval
  - NOT: start_at = original_due_date
```

---

## D03: Expired Crypto Invoice Handling

**Decision:** Void expired invoice, create new invoice on retry.

**Rationale:** Clean audit trail. Each invoice is a distinct attempt. Coinbase charges expire; reusing the same invoice with a new charge is confusing. Voided invoices clearly show "user didn't pay this one."

**Implementation:**
```
ON crypto_invoice_expired:
  - UPDATE invoice.status = 'void'
  - UPDATE payment.status = 'expired'

ON user_clicks_renew:
  - CREATE new invoice
  - CREATE new coinbase charge
  - CREATE new payment
```

---

## D04: Cancel Immediately Access

**Decision:** User keeps access until the end of the current paid period.

**Rationale:** They paid for this period. Revoking immediately feels punitive and may cause support issues. Standard SaaS practice.

**Implementation:**
```
ON cancel_immediate:
  - subscription.status = 'canceled'
  - subscription.canceled_at = now()
  - entitlement.active_to = current_period.end_at  // unchanged
  - entitlement.status remains 'active' until period ends
```

**Exception:** If developer explicitly configures immediate revocation (not recommended).

---

## D05: Downgrade Timing

**Decision:** Downgrades take effect at period end.

**Rationale:** User paid for the higher tier this period. Immediate downgrade would give them less than they paid for. Scheduling for period end is fair and standard practice.

**Implementation:**
```
ON downgrade_request:
  - subscription.pending_plan_id = new_plan_id
  - subscription.plan_id unchanged
  - features stay at current plan until renewal

ON renewal:
  - subscription.plan_id = pending_plan_id
  - subscription.pending_plan_id = null
  - new period uses new plan price/credits
```

---

## D06: Yearly → Monthly Switch

**Decision:** Blocked until year ends. User must wait for yearly period to complete.

**Rationale:** Too complex to prorate in v1. User paid for a year upfront; switching mid-year requires calculating refund, which varies by when they switch. Support can handle exceptions manually.

**Implementation:**
```
ON switch_yearly_to_monthly:
  IF subscription.current_period is yearly:
    - REJECT "Cannot switch to monthly until current yearly period ends"
    - OR: Allow scheduling pending_plan_id for renewal
```

**Alternative (if needed):** Allow with forfeiture of remaining time, explicit user consent required.

---

## D07: Monthly → Yearly Switch

**Decision:** Allowed immediately. User pays yearly price now, gets 12 months starting today. Remaining monthly time is forfeit.

**Rationale:** User is prepaying for a longer commitment. Simple to implement. If they want to switch, they accept losing remaining days on monthly. Yearly discount compensates.

**Implementation:**
```
ON switch_monthly_to_yearly:
  - CREATE invoice for yearly price
  - ON paid:
    - End current monthly period (status = 'ended')
    - CREATE new yearly period (start_at = now, end_at = now + 1 year)
    - Grant yearly credits
```

---

## D08: Price Change for Existing Subscribers

**Decision:** Grandfather existing subscribers at their current price until they change plans.

**Rationale:** Surprise price increases cause churn and support load. Users who subscribed at a price should keep it. If they upgrade/downgrade, they get current pricing.

**Implementation:**
```
ON plan.price_amount changes:
  - Existing subscriptions unaffected
  - New subscriptions use new price
  - Plan change (any direction) uses new plan's current price

OPTIONAL: subscription.locked_price_amount
  - If set, overrides plan.price_amount for renewals
  - Used for special deals, grandfathering
```

---

## D09: Chargeback Access Revocation

**Decision:** Revoke access immediately when dispute opened.

**Rationale:** User is contesting the charge, claiming they didn't authorize it or didn't receive value. Continuing to provide access while they're disputing is risky. If they win the dispute, they got free access; if we win, they've been using the service while fighting the charge.

**Implementation:**
```
ON dispute.created:
  - payment.status = 'disputed'
  - invoice.status = 'disputed'
  - subscription_period.status = 'revoked'
  - entitlement.status = 'inactive'
  - subscription.status = 'paused'
  - reverse credits
```

---

## D10: Chargeback Won Restoration

**Decision:** Restore credits only, not access time.

**Rationale:** The dispute process takes weeks/months. Restoring the original period would either (a) give them a period that already ended, or (b) require creating a new period starting now, which is effectively free time. Credits are fungible and fair to restore.

**Implementation:**
```
ON dispute.won:
  - payment.status = 'paid'
  - invoice.status = 'paid'
  - CREATE credit_ledger_entry(+original_credits)
  - DO NOT create new subscription_period
  - User must renew to regain access
```

---

## D11: Partial Refund Credits

**Decision:** Manual handling only in v1. No automatic credit reversal for partial refunds.

**Rationale:** Partial refunds are typically support gestures (goodwill, service issues). Automatically reversing a proportional amount of credits would be confusing ("I got a 20% refund but lost 20% of my credits?"). Support can manually adjust if needed.

**Implementation:**
```
ON partial_refund:
  - Record refund_amount on invoice
  - DO NOT automatically adjust credits
  - Log for support review
  - Support can manually create credit_ledger_entry if needed
```

---

## D12: Credit Expiration

**Decision:** Credits never expire in v1.

**Rationale:** Expiring credits add significant complexity: tracking expiry per grant, FIFO consumption, handling partial expirations. Not worth it for v1. Can add later if needed.

**Implementation:**
```
credit_ledger_entry has no expires_at field
Balance = SUM(delta) with no time filter
```

---

## D13: Yearly Credits Grant

**Decision:** Default is 1x grant (same as monthly). Opt-in flag for 12x.

**Rationale:** If monthly plan grants 1000 credits/month, yearly should not automatically grant 12,000 upfront unless developer explicitly wants that. Accidental 12x grants could be expensive. Developer must opt in per plan.

**Implementation:**
```sql
plan.credits_yearly_multiply BOOLEAN DEFAULT false

ON yearly_period_paid:
  IF plan.credits_yearly_multiply:
    credits = plan.credits_grant_amount * 12
  ELSE:
    credits = plan.credits_grant_amount
```

---

## D14: Mid-Period Upgrade Credits

**Decision:** No credit adjustment on upgrade. New credit amount applies at next renewal.

**Rationale:** Prevents gaming. User could upgrade, get bonus credits, downgrade, repeat. Mid-period prorated credits are complex to calculate fairly. Simplest: you get what your plan grants at period start.

**Implementation:**
```
ON upgrade:
  - subscription.plan_id = new_plan_id (features change)
  - NO credit_ledger_entry
  - Next renewal grants new plan's credits
```

---

## D15: Negative Credit Balance

**Decision:** Allowed. Users can have negative credit balance.

**Rationale:** Required for refund/dispute reversals. If user has 100 credits, spends 80, then gets refunded for a 500-credit grant, balance becomes -400. This is correct accounting. Next grant or bundle purchase adds to this (e.g., -400 + 500 = 100).

**Implementation:**
```
credit_ledger_entry.delta can be any integer
billing_customer.credits_balance can be negative

ON credit_usage_request:
  IF credits_balance <= 0:
    REJECT "Insufficient credits"
```

---

## D16: Credits During Trial

**Decision:** Not granted by default. Developer must opt in, and we show a warning.

**Rationale:** Users could start trial, use all credits, cancel before paying. Developer should consciously accept this risk. Warning in dashboard: "Users may consume credits during trial and cancel without paying."

**Implementation:**
```sql
plan.grant_credits_during_trial BOOLEAN DEFAULT false

ON trial_period_created:
  IF plan.grant_credits_during_trial AND plan.credits_grant_amount > 0:
    CREATE credit_ledger_entry
  ELSE:
    no credits
```

---

## D17: Downgrade Credit Clawback

**Decision:** No clawback on downgrade. User keeps what they have.

**Rationale:** Credits were legitimately earned when they were on the higher plan. Clawing back feels punitive and creates support issues. Next period simply grants fewer credits.

**Implementation:**
```
ON downgrade:
  - subscription.pending_plan_id = new_plan_id
  - NO negative credit_ledger_entry
  - Next renewal: grants new plan's (lower) credit amount
```

---

## D18: Reactivation Pricing

**Decision:** Reactivation uses current plan price, not original signup price.

**Rationale:** Consistent with D08. Grandfathering applies while actively subscribed. Once canceled/paused for a period, they're effectively a new customer for that plan. If prices increased, they pay the new price.

**Implementation:**
```
ON reactivation:
  - price = plan.price_amount (current)
  - NOT subscription.locked_price_amount (cleared on cancel)
```

---

## D19: Renewal Lead Time

**Decision:** 3 days before period end.

**Rationale:** Enough time for: initial charge attempt, at least one retry if failed, user notification to update payment method. Not so early that user is confused ("I just got charged but my period isn't over yet").

**Implementation:**
```
RENEWAL_LEAD_TIME_DAYS = 3

Background job queries:
  WHERE period.end_at <= now() + INTERVAL '3 days'
```

---

## D20: Grace Period Duration

**Decision:** 7 days from payment failure before access revocation.

**Rationale:** Gives user time to update payment method. Common industry practice. Short enough that we're not giving excessive free access.

**Implementation:**
```
GRACE_PERIOD_DAYS = 7

ON payment_failed:
  - subscription_period.grace_end_at = now() + 7 days
  - entitlement active until grace_end_at
```

---

## D21: Max Payment Retry Attempts

**Decision:** 3 attempts over 7 days (initial + 2 retries).

**Rationale:** Standard dunning schedule. More retries rarely succeed and annoy the user. Schedule: day 0 (initial), day 3 (retry 1), day 7 (retry 2 / final).

**Implementation:**
```
RETRY_SCHEDULE = [0, 3, 7]  // days from initial failure

ON final_retry_failed:
  - invoice.status = 'uncollectible'
  - subscription.status = 'paused'
  - entitlement.status = 'inactive'
```

---

## D22: Yearly Subscription Refund

**Decision:** Manual/support only for mid-year refunds.

**Rationale:** Too many edge cases for automation. How much to refund? Prorate by days? Months? What about credits used? What if they used more credits than remaining months would grant? Support evaluates case-by-case.

**Implementation:**
```
ON yearly_refund_request:
  - IF full refund (all 12 months): automated flow allowed
  - IF partial/mid-year: route to support queue
  - Support uses admin tools to:
    - Process partial refund via provider
    - Manually adjust credits if appropriate
    - Cancel subscription
```

---

## D23: Bundle Credits Pool

**Decision:** Bundle credits go into the same pool as plan credits. Single balance.

**Rationale:** Simpler for developers to check balance. One number. Separate pools would require tracking which credits to consume first (FIFO?), complicate refunds, and confuse users.

**Implementation:**
```
credit_ledger_entry tracks source_type for audit
billing_customer.credits_balance is single integer
All credits fungible for usage
```

---

## D24: Multiple Subscriptions Per Customer

**Decision:** One subscription per customer per app. Enforced by unique constraint.

**Rationale:** Simplifies everything: entitlement checks, billing logic, UI. "What plan am I on?" has one answer. Multiple concurrent plans is a v2+ feature if ever needed.

**Implementation:**
```sql
CREATE UNIQUE INDEX idx_subscription_one_active 
  ON subscription(app_id, billing_customer_id) 
  WHERE status IN ('trialing', 'active', 'past_due', 'paused');
```

---

## D25: Trial Cancellation Access

**Decision:** Access ends immediately on trial cancellation.

**Rationale:** They didn't pay. Unlike paid periods (D04), there's no "they paid for this time" argument. Letting them use the rest of trial after canceling has no benefit and may be abused.

**Implementation:**
```
ON cancel_during_trial:
  - subscription.status = 'canceled'
  - entitlement.active_to = now()
  - entitlement.status = 'inactive'
```

---

## Decision Changelog

| Date | ID | Change | Reason |
|------|-----|--------|--------|
| Initial | All | Initial decisions | v1 design |

---

## How to Add New Decisions

1. Assign next ID (D26, D27, ...)
2. Document: Decision, Rationale, Implementation
3. Add to index table at top
4. Add to changelog
5. Update relevant FLOWS.md and EDGE_CASES.md
