# reauth.dev Billing System - Overview

## What This Is

A provider-agnostic billing system for reauth.dev that treats payment providers (Stripe, Coinbase Commerce, etc.) as dumb settlement pipes while **we own all subscription state, periods, entitlements, and credits**.

Developers add plans and bundles via dashboard. End users subscribe, purchase bundles, and consume credits. We handle renewals, upgrades, downgrades, refunds, and disputes internally.

---

## Core Architecture Principles

1. **We own subscription truth** - No reliance on Stripe Subscriptions or Coinbase recurring features. Providers only settle payments.

2. **Provider adapters are thin** - Each provider implements: `create_payment()`, `parse_webhook()`, `fetch_payment()`. Nothing else.

3. **Two provider classes**:
   - **Recurring-capable (cards)**: Can charge off-session (Stripe). We set `auto_renew=true`.
   - **Non-recurring (crypto)**: One-time charges only (Coinbase). We set `auto_renew=false`, user manually renews.

4. **Credits are ledger-based** - Balance = SUM(delta). Can go negative (refunds/disputes). Source tracking for audit.

5. **Entitlements are derived** - Computed from active SubscriptionPeriods. Cached for fast reads.

6. **All webhooks are idempotent** - Uniqueness on (provider, provider_payment_id). Safe upsert-then-transition.

---

## v1 Scope

### In Scope

| Feature | Details |
|---------|---------|
| Plans | Monthly/yearly, optional credits grant, optional trial |
| Bundles | One-time purchase, optional credits, purchase limits |
| Subscriptions | One per customer per app, status tracking, auto/manual renewal |
| Subscription Periods | Paid time windows, trial periods, grace periods |
| Invoices | Our record of money due/paid, maps to provider payments |
| Payments | Provider settlement records, status tracking |
| Credits | Ledger-based, grants from plans/bundles, negative balances allowed |
| Entitlements | Derived access windows for fast permission checks |
| Plan Changes | Upgrade immediate (price at renewal), downgrade at period end |
| Cancellation | Immediate or at period end, access until paid-through date |
| Refunds | Full refunds with credit reversal, partial = manual |
| Disputes | Immediate revocation, reinstate credits (not time) if won |
| Providers | Stripe (cards), Coinbase Commerce (crypto) |

### Explicitly Out of Scope (v2+)

| Feature | Reason |
|---------|--------|
| Proration | Complex, edge-case heavy, manual workarounds exist |
| Partial refund automation | Too many policy variations, support handles |
| Expiring credits | Significant complexity, not worth it yet |
| Multiple concurrent subscriptions | One plan per customer simplifies everything |
| Mid-period credit adjustments on upgrade | Prevents gaming, simpler accounting |
| Yearly → monthly switch mid-year | Block or forfeit; proration too complex |
| Complex credit cadences (monthly drip in yearly) | v1 = one grant per paid period |
| Coupons / discounts | Can add later, v1 uses price overrides |
| Usage-based billing | Different paradigm, v2 consideration |
| Tax calculation | Integrate with Stripe Tax or similar in v2 |

---

## Key Entities

```
┌─────────────────────────────────────────────────────────────────┐
│                           CATALOG                               │
│  ┌──────────┐  ┌──────────┐                                     │
│  │   Plan   │  │  Bundle  │                                     │
│  └──────────┘  └──────────┘                                     │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      BILLING IDENTITY                           │
│  ┌─────────────────┐  ┌─────────────────────┐                   │
│  │ BillingCustomer │──│ ProviderCustomerRef │                   │
│  └─────────────────┘  └─────────────────────┘                   │
│                              │                                   │
│                       ┌──────────────┐                          │
│                       │PaymentMethod │                          │
│                       └──────────────┘                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                       MONEY OBJECTS                             │
│  ┌──────────┐  ┌──────────┐                                     │
│  │ Invoice  │──│ Payment  │                                     │
│  └──────────┘  └──────────┘                                     │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    SUBSCRIPTION CORE                            │
│  ┌──────────────┐  ┌────────────────────┐                       │
│  │ Subscription │──│ SubscriptionPeriod │                       │
│  └──────────────┘  └────────────────────┘                       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     ACCESS & BALANCE                            │
│  ┌─────────────┐  ┌───────────────────┐                         │
│  │ Entitlement │  │ CreditLedgerEntry │                         │
│  └─────────────┘  └───────────────────┘                         │
└─────────────────────────────────────────────────────────────────┘
```

---

## Provider Flow Summary

### Card (Stripe) - Automatic Renewal

```
User subscribes → Invoice created → Stripe charges card → Webhook: paid
    → Period created → Credits granted → Entitlement active

Period ending → Renewal Invoice → Off-session charge → Webhook: paid
    → New period → Credits → Entitlement extended

Charge fails → past_due → Retry in 3 days → Grace period
    → Eventually: paused/canceled if not recovered
```

### Crypto (Coinbase) - Manual Renewal

```
User subscribes → Invoice created → Coinbase checkout URL shown → User pays
    → Webhook: paid → Period created → Credits → Entitlement active

Period ending → Renewal Invoice → "Renew" button shown → User pays
    → Same as above

User doesn't pay → Period ends → Subscription paused → "Renewal required"
    → NOT "payment failed" (this is expected behavior for prepaid)
```

---

## Document Index

| Document | Purpose |
|----------|---------|
| [DATA_MODEL.md](./DATA_MODEL.md) | Complete SQL schema with constraints and indexes |
| [STATE_MACHINES.md](./STATE_MACHINES.md) | Valid state transitions for all entities |
| [FLOWS.md](./FLOWS.md) | Step-by-step implementation flows with pseudocode |
| [DECISIONS.md](./DECISIONS.md) | All policy decisions, locked in, with rationale |
| [EDGE_CASES.md](./EDGE_CASES.md) | Acceptance criteria in Given/When/Then format |
| [API_SURFACE.md](./API_SURFACE.md) | Internal functions, REST endpoints, webhooks |

---

## Implementation Order

1. **Schema** - Implement DATA_MODEL.md exactly
2. **State machines** - Implement transition validators per STATE_MACHINES.md
3. **Provider adapters** - Stripe first, then Coinbase
4. **Core flows** - In order from FLOWS.md:
   - New subscription (no trial)
   - New subscription (with trial)
   - Renewal (card)
   - Renewal (crypto)
   - Cancellation
   - Bundle purchase
   - Plan change
   - Refunds
   - Disputes
5. **Edge cases** - Use EDGE_CASES.md as test suite
6. **API layer** - Expose per API_SURFACE.md
