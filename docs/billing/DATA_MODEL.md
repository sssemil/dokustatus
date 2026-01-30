# reauth.dev Billing System - Data Model

Complete PostgreSQL schema. Implement exactly as specified.

---

## Schema Overview

```
┌─────────────┐     ┌─────────────┐
│    plan     │     │   bundle    │
└─────────────┘     └─────────────┘
       │                   │
       │                   │
       ▼                   │
┌──────────────────┐       │
│ billing_customer │       │
└──────────────────┘       │
       │                   │
       ├───────────────────┤
       │                   │
       ▼                   ▼
┌──────────────────────────────┐
│           invoice            │
└──────────────────────────────┘
       │
       ▼
┌──────────────────────────────┐
│           payment            │
└──────────────────────────────┘

┌──────────────────┐
│ billing_customer │
└──────────────────┘
       │
       ├──────────────────────────┐
       │                          │
       ▼                          ▼
┌──────────────────┐    ┌─────────────────────┐
│   subscription   │    │ provider_customer_ref│
└──────────────────┘    └─────────────────────┘
       │                          │
       ▼                          ▼
┌──────────────────┐    ┌─────────────────────┐
│subscription_period│   │   payment_method    │
└──────────────────┘    └─────────────────────┘

┌──────────────────┐
│ billing_customer │
└──────────────────┘
       │
       ├──────────────────────────┐
       │                          │
       ▼                          ▼
┌──────────────────┐    ┌─────────────────────┐
│   entitlement    │    │ credit_ledger_entry │
└──────────────────┘    └─────────────────────┘
```

---

## Catalog Tables

### plan

```sql
CREATE TABLE plan (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  app_id UUID NOT NULL REFERENCES app(id) ON DELETE CASCADE,
  
  -- Basic info
  name TEXT NOT NULL,
  description TEXT,
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
  
  -- Pricing
  billing_interval TEXT NOT NULL CHECK (billing_interval IN ('month', 'year')),
  price_amount INTEGER NOT NULL CHECK (price_amount >= 0),  -- in cents
  price_currency TEXT NOT NULL DEFAULT 'usd' CHECK (price_currency ~ '^[a-z]{3}$'),
  
  -- Yearly prepay option (for monthly plans)
  allow_yearly_prepay BOOLEAN NOT NULL DEFAULT false,
  yearly_prepay_price_amount INTEGER CHECK (yearly_prepay_price_amount >= 0),  -- null = 12x monthly
  
  -- Trial
  trial_days INTEGER CHECK (trial_days >= 0),
  
  -- Credits
  credits_grant_amount INTEGER CHECK (credits_grant_amount >= 0),
  credits_grant_cadence TEXT DEFAULT 'per_period' CHECK (credits_grant_cadence IN ('per_period', 'on_start')),
  credits_yearly_multiply BOOLEAN NOT NULL DEFAULT false,  -- if true, yearly gets 12x credits
  grant_credits_during_trial BOOLEAN NOT NULL DEFAULT false,
  
  -- Features (opaque to billing system, used by app)
  features JSONB DEFAULT '{}',
  
  -- Metadata
  display_order INTEGER DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  
  CONSTRAINT yearly_prepay_requires_monthly CHECK (
    NOT allow_yearly_prepay OR billing_interval = 'month'
  )
);

CREATE INDEX idx_plan_app_id ON plan(app_id);
CREATE INDEX idx_plan_app_status ON plan(app_id, status);
```

### bundle

```sql
CREATE TABLE bundle (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  app_id UUID NOT NULL REFERENCES app(id) ON DELETE CASCADE,
  
  -- Basic info
  name TEXT NOT NULL,
  description TEXT,
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
  
  -- Pricing
  price_amount INTEGER NOT NULL CHECK (price_amount >= 0),  -- in cents
  price_currency TEXT NOT NULL DEFAULT 'usd' CHECK (price_currency ~ '^[a-z]{3}$'),
  
  -- Credits
  credits_grant_amount INTEGER CHECK (credits_grant_amount >= 0),
  
  -- Features/unlocks (opaque to billing system)
  features JSONB DEFAULT '{}',
  
  -- Limits
  max_purchases_per_user INTEGER CHECK (max_purchases_per_user > 0),  -- null = unlimited
  
  -- Metadata
  display_order INTEGER DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_bundle_app_id ON bundle(app_id);
CREATE INDEX idx_bundle_app_status ON bundle(app_id, status);
```

---

## Billing Identity Tables

### billing_customer

```sql
CREATE TABLE billing_customer (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  app_id UUID NOT NULL REFERENCES app(id) ON DELETE CASCADE,
  user_id UUID NOT NULL,  -- references app's user table (external)
  
  email TEXT NOT NULL,
  name TEXT,
  
  -- Cached credits balance (updated by trigger or application)
  credits_balance INTEGER NOT NULL DEFAULT 0,
  
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  
  UNIQUE (app_id, user_id)
);

CREATE INDEX idx_billing_customer_app_id ON billing_customer(app_id);
CREATE INDEX idx_billing_customer_email ON billing_customer(app_id, email);
```

### provider_customer_ref

Links our billing customer to provider-side customer IDs (e.g., Stripe Customer).

```sql
CREATE TABLE provider_customer_ref (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  billing_customer_id UUID NOT NULL REFERENCES billing_customer(id) ON DELETE CASCADE,
  
  provider TEXT NOT NULL CHECK (provider IN ('stripe', 'coinbase')),
  provider_customer_id TEXT,  -- e.g., cus_xxx for Stripe (null for Coinbase which has no customer concept)
  
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  
  UNIQUE (provider, provider_customer_id) WHERE provider_customer_id IS NOT NULL
);

CREATE INDEX idx_provider_customer_ref_billing_customer ON provider_customer_ref(billing_customer_id);
CREATE INDEX idx_provider_customer_ref_lookup ON provider_customer_ref(provider, provider_customer_id);
```

### payment_method

Stored payment methods for off-session charging (cards only).

```sql
CREATE TABLE payment_method (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  provider_customer_ref_id UUID NOT NULL REFERENCES provider_customer_ref(id) ON DELETE CASCADE,
  
  provider TEXT NOT NULL CHECK (provider IN ('stripe')),  -- only card providers
  provider_payment_method_id TEXT NOT NULL,  -- e.g., pm_xxx
  
  type TEXT NOT NULL CHECK (type IN ('card', 'bank_account', 'other')),
  
  -- Card details (for display, not for charging)
  card_brand TEXT,  -- visa, mastercard, amex, etc.
  card_last4 TEXT CHECK (card_last4 ~ '^\d{4}$'),
  card_exp_month INTEGER CHECK (card_exp_month BETWEEN 1 AND 12),
  card_exp_year INTEGER CHECK (card_exp_year >= 2020),
  
  is_default BOOLEAN NOT NULL DEFAULT false,
  
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  
  UNIQUE (provider, provider_payment_method_id)
);

CREATE INDEX idx_payment_method_provider_customer ON payment_method(provider_customer_ref_id);

-- Ensure only one default per provider_customer_ref
CREATE UNIQUE INDEX idx_payment_method_default 
  ON payment_method(provider_customer_ref_id) 
  WHERE is_default = true;
```

---

## Money Objects

### invoice

Our record that money is due or has been paid.

```sql
CREATE TYPE invoice_purpose AS ENUM (
  'subscription_period',
  'bundle_purchase',
  'plan_change_settlement'  -- future use
);

CREATE TYPE invoice_status AS ENUM (
  'draft',
  'open',
  'paid',
  'void',
  'uncollectible',
  'refunded',
  'disputed'
);

CREATE TABLE invoice (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  app_id UUID NOT NULL REFERENCES app(id) ON DELETE CASCADE,
  billing_customer_id UUID NOT NULL REFERENCES billing_customer(id) ON DELETE CASCADE,
  
  -- What this invoice is for
  purpose invoice_purpose NOT NULL,
  
  -- Amount
  amount_due INTEGER NOT NULL CHECK (amount_due >= 0),  -- in cents
  currency TEXT NOT NULL DEFAULT 'usd' CHECK (currency ~ '^[a-z]{3}$'),
  
  -- Status
  status invoice_status NOT NULL DEFAULT 'draft',
  
  -- Timing
  due_at TIMESTAMPTZ,
  paid_at TIMESTAMPTZ,
  voided_at TIMESTAMPTZ,
  
  -- Refund tracking
  refund_amount INTEGER CHECK (refund_amount >= 0),
  refunded_at TIMESTAMPTZ,
  
  -- Links to what this invoice funds (denormalized for queries)
  -- For subscription_period: subscription_id, plan_id, period_start, period_end
  -- For bundle_purchase: bundle_id
  metadata JSONB NOT NULL DEFAULT '{}',
  
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_invoice_app_id ON invoice(app_id);
CREATE INDEX idx_invoice_billing_customer ON invoice(billing_customer_id);
CREATE INDEX idx_invoice_status ON invoice(app_id, status);
CREATE INDEX idx_invoice_purpose ON invoice(app_id, purpose);
CREATE INDEX idx_invoice_due_at ON invoice(due_at) WHERE status = 'open';
```

### payment

Provider settlement record mapped to our invoice.

```sql
CREATE TYPE payment_status AS ENUM (
  'pending',
  'authorized',
  'paid',
  'failed',
  'refunded',
  'disputed',
  'canceled',
  'expired'
);

CREATE TABLE payment (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  invoice_id UUID NOT NULL REFERENCES invoice(id) ON DELETE CASCADE,
  
  -- Provider info
  provider TEXT NOT NULL CHECK (provider IN ('stripe', 'coinbase')),
  provider_payment_id TEXT NOT NULL,  -- e.g., pi_xxx or charge_xxx
  
  -- Amount (may differ from invoice if partial or currency conversion)
  amount INTEGER NOT NULL CHECK (amount >= 0),  -- in cents
  currency TEXT NOT NULL CHECK (currency ~ '^[a-z]{3}$'),
  
  -- For crypto: locked exchange rate at invoice creation
  exchange_rate NUMERIC(20, 10),  -- e.g., BTC/USD rate
  crypto_amount NUMERIC(30, 18),  -- amount in crypto (high precision)
  crypto_currency TEXT,  -- e.g., 'BTC', 'ETH', 'USDC'
  
  -- Status
  status payment_status NOT NULL DEFAULT 'pending',
  
  -- Timestamps
  confirmed_at TIMESTAMPTZ,
  failed_at TIMESTAMPTZ,
  refunded_at TIMESTAMPTZ,
  
  -- Raw provider response for debugging
  raw_provider_payload JSONB,
  
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  
  UNIQUE (provider, provider_payment_id)
);

CREATE INDEX idx_payment_invoice ON payment(invoice_id);
CREATE INDEX idx_payment_provider_lookup ON payment(provider, provider_payment_id);
CREATE INDEX idx_payment_status ON payment(status);
```

---

## Subscription Core

### subscription

```sql
CREATE TYPE subscription_status AS ENUM (
  'trialing',
  'active',
  'past_due',
  'paused',
  'canceled'
);

CREATE TABLE subscription (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  app_id UUID NOT NULL REFERENCES app(id) ON DELETE CASCADE,
  billing_customer_id UUID NOT NULL REFERENCES billing_customer(id) ON DELETE CASCADE,
  plan_id UUID NOT NULL REFERENCES plan(id),
  
  -- Status
  status subscription_status NOT NULL,
  
  -- Renewal behavior
  auto_renew BOOLEAN NOT NULL DEFAULT true,  -- false for crypto/prepaid
  
  -- Current state
  current_period_id UUID,  -- set after first period created
  
  -- Plan change scheduling
  pending_plan_id UUID REFERENCES plan(id),  -- for "downgrade at period end"
  
  -- Cancellation
  cancel_at_period_end BOOLEAN NOT NULL DEFAULT false,
  canceled_at TIMESTAMPTZ,
  
  -- Price locking (optional, for grandfathering)
  locked_price_amount INTEGER,  -- if set, overrides plan price
  
  -- Timestamps
  trial_ends_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Only one active-ish subscription per customer per app
CREATE UNIQUE INDEX idx_subscription_one_active 
  ON subscription(app_id, billing_customer_id) 
  WHERE status IN ('trialing', 'active', 'past_due', 'paused');

CREATE INDEX idx_subscription_app ON subscription(app_id);
CREATE INDEX idx_subscription_customer ON subscription(billing_customer_id);
CREATE INDEX idx_subscription_status ON subscription(app_id, status);
CREATE INDEX idx_subscription_plan ON subscription(plan_id);

-- Add FK after subscription_period exists
-- ALTER TABLE subscription ADD FOREIGN KEY (current_period_id) REFERENCES subscription_period(id);
```

### subscription_period

```sql
CREATE TYPE period_status AS ENUM (
  'scheduled',
  'active',
  'ended',
  'revoked'
);

CREATE TABLE subscription_period (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  subscription_id UUID NOT NULL REFERENCES subscription(id) ON DELETE CASCADE,
  
  -- Time window
  start_at TIMESTAMPTZ NOT NULL,
  end_at TIMESTAMPTZ NOT NULL,
  
  -- Status
  status period_status NOT NULL DEFAULT 'scheduled',
  
  -- Payment link (null for trials)
  invoice_id UUID REFERENCES invoice(id),
  
  -- Grace period for failed payments
  grace_end_at TIMESTAMPTZ,
  
  -- Tracking
  is_trial BOOLEAN NOT NULL DEFAULT false,
  credits_granted INTEGER,  -- how many credits were granted for this period (for reversal)
  
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  
  CHECK (end_at > start_at)
);

CREATE INDEX idx_subscription_period_subscription ON subscription_period(subscription_id);
CREATE INDEX idx_subscription_period_invoice ON subscription_period(invoice_id);
CREATE INDEX idx_subscription_period_active ON subscription_period(subscription_id, status) WHERE status = 'active';
CREATE INDEX idx_subscription_period_end_at ON subscription_period(end_at) WHERE status = 'active';

-- Now add the FK from subscription
ALTER TABLE subscription ADD FOREIGN KEY (current_period_id) REFERENCES subscription_period(id);
```

---

## Access & Balance

### entitlement

Derived/cached access windows for fast permission checks.

```sql
CREATE TYPE entitlement_kind AS ENUM (
  'plan_access',
  'bundle_unlock'
);

CREATE TYPE entitlement_status AS ENUM (
  'active',
  'inactive'
);

CREATE TABLE entitlement (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  app_id UUID NOT NULL REFERENCES app(id) ON DELETE CASCADE,
  billing_customer_id UUID NOT NULL REFERENCES billing_customer(id) ON DELETE CASCADE,
  
  kind entitlement_kind NOT NULL,
  
  -- What grants this entitlement
  ref_type TEXT NOT NULL,  -- 'subscription', 'invoice' (for bundle)
  ref_id UUID NOT NULL,
  
  -- Time window
  active_from TIMESTAMPTZ NOT NULL,
  active_to TIMESTAMPTZ,  -- null = perpetual (e.g., lifetime bundle unlock)
  
  -- Status
  status entitlement_status NOT NULL DEFAULT 'active',
  
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_entitlement_customer ON entitlement(billing_customer_id);
CREATE INDEX idx_entitlement_active ON entitlement(billing_customer_id, status) WHERE status = 'active';
CREATE INDEX idx_entitlement_ref ON entitlement(ref_type, ref_id);
CREATE INDEX idx_entitlement_app_kind ON entitlement(app_id, kind);
```

### credit_ledger_entry

Immutable ledger of all credit changes.

```sql
CREATE TYPE credit_source_type AS ENUM (
  'subscription_period',
  'bundle',
  'manual',
  'refund_reversal',
  'dispute_reversal',
  'dispute_won_restoration',
  'adjustment'
);

CREATE TABLE credit_ledger_entry (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  app_id UUID NOT NULL REFERENCES app(id) ON DELETE CASCADE,
  billing_customer_id UUID NOT NULL REFERENCES billing_customer(id) ON DELETE CASCADE,
  
  -- What caused this entry
  source_type credit_source_type NOT NULL,
  source_id UUID,  -- subscription_period.id, invoice.id, etc.
  
  -- The change
  delta INTEGER NOT NULL,  -- positive = grant, negative = reversal/deduction
  
  -- Running balance (optional, for fast queries)
  balance_after INTEGER,
  
  -- Audit
  note TEXT,
  admin_user_id UUID,  -- if manual adjustment
  
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_credit_ledger_customer ON credit_ledger_entry(billing_customer_id);
CREATE INDEX idx_credit_ledger_source ON credit_ledger_entry(source_type, source_id);
CREATE INDEX idx_credit_ledger_app_customer ON credit_ledger_entry(app_id, billing_customer_id);

-- Function to get current balance
CREATE OR REPLACE FUNCTION get_credits_balance(p_billing_customer_id UUID)
RETURNS INTEGER AS $$
  SELECT COALESCE(SUM(delta), 0)::INTEGER
  FROM credit_ledger_entry
  WHERE billing_customer_id = p_billing_customer_id;
$$ LANGUAGE SQL STABLE;
```

---

## Audit & Events (Optional but Recommended)

### billing_event

Audit log of all billing actions.

```sql
CREATE TABLE billing_event (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  app_id UUID NOT NULL,
  billing_customer_id UUID,
  
  event_type TEXT NOT NULL,  -- e.g., 'subscription.created', 'invoice.paid', 'refund.processed'
  
  -- Related entities
  subscription_id UUID,
  invoice_id UUID,
  payment_id UUID,
  
  -- Event data
  payload JSONB NOT NULL DEFAULT '{}',
  
  -- Source
  source TEXT NOT NULL DEFAULT 'system',  -- 'system', 'webhook', 'api', 'admin'
  source_ip TEXT,
  admin_user_id UUID,
  
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_billing_event_app ON billing_event(app_id);
CREATE INDEX idx_billing_event_customer ON billing_event(billing_customer_id);
CREATE INDEX idx_billing_event_type ON billing_event(app_id, event_type);
CREATE INDEX idx_billing_event_created ON billing_event(created_at);
```

---

## Summary of Constraints

| Table | Constraint | Purpose |
|-------|------------|---------|
| plan | `yearly_prepay_requires_monthly` | Can only prepay yearly on monthly plans |
| billing_customer | `UNIQUE(app_id, user_id)` | One billing customer per app user |
| provider_customer_ref | `UNIQUE(provider, provider_customer_id)` | No duplicate provider mappings |
| payment_method | `UNIQUE INDEX ... WHERE is_default` | Only one default per customer |
| payment | `UNIQUE(provider, provider_payment_id)` | Idempotency key for webhooks |
| subscription | `UNIQUE INDEX ... WHERE status IN (...)` | One active sub per customer |
| subscription_period | `CHECK(end_at > start_at)` | Valid time ranges |

---

## Indexes for Common Queries

| Query | Index |
|-------|-------|
| Get customer's active subscription | `idx_subscription_one_active` |
| Get invoices due for renewal | `idx_invoice_due_at WHERE status = 'open'` |
| Find payment by provider ID (webhooks) | `idx_payment_provider_lookup` |
| Get active periods ending soon | `idx_subscription_period_end_at WHERE status = 'active'` |
| Get customer's credit balance | `idx_credit_ledger_customer` + `get_credits_balance()` |
| Check entitlements | `idx_entitlement_active WHERE status = 'active'` |

---

## Notes

1. **All money in cents** - No decimals, avoids floating point issues. $10.00 = 1000.

2. **All timestamps in UTC** - Use `TIMESTAMPTZ`, never `TIMESTAMP`.

3. **Soft delete via status** - Plans/bundles use `status = 'archived'`, not deletion.

4. **JSONB metadata** - Flexible storage for invoice details, features, etc.

5. **Credits can go negative** - Required for refund/dispute reversals after spend.

6. **No cascading deletes on business entities** - Only on identity mappings.
