# reauth.dev Billing System - API Surface

Internal functions, REST endpoints, and webhook handlers.

---

## Table of Contents

1. [Internal Functions](#internal-functions)
2. [REST API Endpoints](#rest-api-endpoints)
3. [Webhook Handlers](#webhook-handlers)
4. [Background Jobs](#background-jobs)
5. [Admin Functions](#admin-functions)
6. [Event Types](#event-types)
7. [Response Shapes](#response-shapes)

---

## Internal Functions

These are the core billing module functions. Called by REST handlers, webhooks, and jobs.

### Subscription Management

**create_subscription**
- Inputs: billing_customer_id, plan_id, payment_method_id (required for card, optional for crypto), payment_provider
- Outputs: subscription, invoice, checkout_url (for crypto)
- Creates subscription, invoice, and initiates payment. See FLOWS.md sections 1-4.

**cancel_subscription**
- Inputs: subscription_id, immediate (bool)
- Outputs: updated subscription
- If immediate=false: sets cancel_at_period_end. If immediate=true: cancels now with access until period end (paid) or now (trial).

**undo_cancel_subscription**
- Inputs: subscription_id
- Outputs: updated subscription
- Clears cancel_at_period_end flag. Only valid before period ends.

**change_plan**
- Inputs: subscription_id, new_plan_id
- Outputs: updated subscription
- Determines upgrade vs downgrade by price comparison. Upgrade = immediate plan_id change, downgrade = sets pending_plan_id for renewal.

**reactivate_subscription**
- Inputs: subscription_id, payment_method_id (optional), payment_provider
- Outputs: subscription, invoice, checkout_url (for crypto)
- For paused subscriptions. Creates new period starting now at current plan price.

**get_subscription**
- Inputs: subscription_id
- Outputs: subscription with current_period and plan details, or null

**get_subscription_by_customer**
- Inputs: billing_customer_id
- Outputs: subscription with current_period and plan details, or null

### Bundle Management

**purchase_bundle**
- Inputs: billing_customer_id, bundle_id, payment_provider, payment_method_id (for card)
- Outputs: invoice, checkout_url (for crypto)
- Validates purchase limits. Creates invoice and payment.

**get_bundle_purchases**
- Inputs: billing_customer_id, bundle_id (optional filter)
- Outputs: list of bundle purchases

### Credit Operations

**get_credits_balance**
- Inputs: billing_customer_id
- Outputs: integer balance (can be negative)

**deduct_credits**
- Inputs: billing_customer_id, amount, reason
- Outputs: success/failure, new_balance, error code if insufficient
- Rejects if balance < amount.

**grant_credits**
- Inputs: billing_customer_id, amount, reason, admin_user_id
- Outputs: ledger entry, new_balance

**get_credit_history**
- Inputs: billing_customer_id, limit, offset
- Outputs: list of ledger entries, total count

### Entitlement Checks

**has_active_plan**
- Inputs: billing_customer_id
- Outputs: boolean
- Checks entitlement table: kind='plan_access', status='active', active_from <= now, active_to is null or > now.

**has_feature**
- Inputs: billing_customer_id, feature_key
- Outputs: boolean
- Checks plan features (via active subscription) and bundle entitlements.

**get_entitlements**
- Inputs: billing_customer_id
- Outputs: list of entitlements

**get_plan_features**
- Inputs: billing_customer_id
- Outputs: features map from active plan, or null

### Invoice & Payment

**get_invoice**
- Inputs: invoice_id
- Outputs: invoice with payments list, or null

**get_invoices**
- Inputs: billing_customer_id, status filter (optional), limit, offset
- Outputs: list of invoices, total count

**refund_invoice**
- Inputs: invoice_id, amount (optional, omit for full refund), reason
- Outputs: updated invoice, refunded amount
- Full refund: reverses credits, revokes period/entitlement. Partial: records amount only, no automatic reversal.

**retry_payment**
- Inputs: invoice_id, payment_method_id (optional, to use different card)
- Outputs: payment record, success boolean

### Payment Method Management

**add_payment_method**
- Inputs: billing_customer_id, provider ('stripe'), provider_payment_method_id, set_as_default (optional)
- Outputs: payment method record
- First card auto-set as default.

**remove_payment_method**
- Inputs: payment_method_id
- Outputs: success, warning if no methods remaining with active subscription

**set_default_payment_method**
- Inputs: payment_method_id
- Outputs: updated payment method record

**get_payment_methods**
- Inputs: billing_customer_id
- Outputs: list of payment methods

### Customer Management

**get_or_create_billing_customer**
- Inputs: app_id, user_id, email, name (optional)
- Outputs: billing customer record

**update_billing_customer**
- Inputs: billing_customer_id, email (optional), name (optional)
- Outputs: updated billing customer record

---

## REST API Endpoints

### Public Endpoints (Developer's App -> reauth.dev)

These are called by the developer's application to manage their users' billing.

#### Subscriptions

```
POST /v1/subscriptions
  Create new subscription
  Body: { billing_customer_id, plan_id, payment_provider, payment_method_id? }
  Response: { subscription, invoice, checkout_url? }

GET /v1/subscriptions/:id
  Get subscription details
  Response: { subscription, current_period, plan }

GET /v1/customers/:billing_customer_id/subscription
  Get subscription by customer
  Response: { subscription, current_period, plan } | null

POST /v1/subscriptions/:id/cancel
  Cancel subscription
  Body: { immediate: boolean }
  Response: { subscription }

POST /v1/subscriptions/:id/undo-cancel
  Undo cancel at period end
  Response: { subscription }

POST /v1/subscriptions/:id/change-plan
  Change plan
  Body: { plan_id }
  Response: { subscription }

POST /v1/subscriptions/:id/reactivate
  Reactivate paused subscription
  Body: { payment_provider, payment_method_id? }
  Response: { subscription, invoice, checkout_url? }
```

#### Bundles

```
POST /v1/bundles/purchase
  Purchase a bundle
  Body: { billing_customer_id, bundle_id, payment_provider, payment_method_id? }
  Response: { invoice, checkout_url? }

GET /v1/customers/:billing_customer_id/bundle-purchases
  Get bundle purchase history
  Query: ?bundle_id=xxx
  Response: { purchases: [...] }
```

#### Credits

```
GET /v1/customers/:billing_customer_id/credits
  Get credit balance
  Response: { balance }

POST /v1/customers/:billing_customer_id/credits/deduct
  Deduct credits
  Body: { amount, reason }
  Response: { success, new_balance, error? }

GET /v1/customers/:billing_customer_id/credits/history
  Get credit history
  Query: ?limit=50&offset=0
  Response: { entries: [...], total }
```

#### Entitlements

```
GET /v1/customers/:billing_customer_id/entitlements
  Get all entitlements
  Response: { entitlements: [...] }

GET /v1/customers/:billing_customer_id/has-plan
  Check plan access
  Response: { has_active_plan: boolean }

GET /v1/customers/:billing_customer_id/has-feature/:feature_key
  Check feature access
  Response: { has_feature: boolean }
```

#### Invoices

```
GET /v1/invoices/:id
  Get invoice details
  Response: { invoice, payments: [...] }

GET /v1/customers/:billing_customer_id/invoices
  Get customer invoices
  Query: ?status=paid,open&limit=20&offset=0
  Response: { invoices: [...], total }
```

#### Payment Methods

```
GET /v1/customers/:billing_customer_id/payment-methods
  List payment methods
  Response: { payment_methods: [...] }

POST /v1/customers/:billing_customer_id/payment-methods
  Add payment method
  Body: { provider, provider_payment_method_id, set_as_default? }
  Response: { payment_method }

DELETE /v1/payment-methods/:id
  Remove payment method
  Response: { success, warning? }

POST /v1/payment-methods/:id/set-default
  Set as default
  Response: { payment_method }
```

#### Customers

```
POST /v1/customers
  Create or get billing customer
  Body: { app_id, user_id, email, name? }
  Response: { billing_customer, created: boolean }

PATCH /v1/customers/:billing_customer_id
  Update customer
  Body: { email?, name? }
  Response: { billing_customer }
```

### Checkout Endpoints

```
POST /v1/checkout/create-session
  Create Stripe checkout session (alternative to direct payment method)
  Body: { billing_customer_id, type: 'subscription' | 'bundle', plan_id | bundle_id }
  Response: { session_id, checkout_url }

POST /v1/checkout/create-setup-intent
  Create Stripe setup intent for adding payment method
  Body: { billing_customer_id }
  Response: { client_secret }
```

---

## Webhook Handlers

### Stripe Webhooks

```
POST /webhooks/stripe
  Handles all Stripe events
  Headers: stripe-signature

  Events handled:
  - payment_intent.succeeded
  - payment_intent.payment_failed
  - payment_intent.canceled
  - charge.refunded
  - charge.dispute.created
  - charge.dispute.updated
  - charge.dispute.closed
  - customer.subscription.* (ignored - we own subscription state)
  - payment_method.attached
  - payment_method.detached
  - payment_method.automatically_updated
```

### Coinbase Commerce Webhooks

```
POST /webhooks/coinbase
  Handles all Coinbase Commerce events
  Headers: x-cc-webhook-signature

  Events handled:
  - charge:created
  - charge:confirmed
  - charge:failed
  - charge:delayed
  - charge:pending
  - charge:resolved
```

### Webhook Handler Pattern

All webhook handlers follow this pattern:

1. **Verify signature** - Provider-specific header verification
2. **Check idempotency** - Has this event been processed? (check billing_event table)
3. **Route to handler** - Match event type to handler function
4. **Process in transaction** - Execute handler logic + mark event as processed atomically
5. **Emit internal event** - Write to billing_event table for audit
6. **Return 200 OK** - Always acknowledge to prevent provider retries

---

## Background Jobs

### Job: Trial Conversion Check

- **Schedule:** Every hour
- **Purpose:** Convert trials to paid subscriptions or pause
- **Query:** subscriptions WHERE status = 'trialing' AND trial_ends_at <= now() + 3 days
- **Action:** For each, run trial conversion flow (attempt charge for card, send renewal email for crypto)

### Job: Renewal Processing

- **Schedule:** Every hour
- **Purpose:** Create renewal invoices and charge
- **Query:** active subscription_periods ending within lead_time (3 days), where subscription is active and not canceling
- **Action:** For each, create renewal invoice and attempt charge (card) or send checkout link (crypto)

### Job: Period Expiry Check

- **Schedule:** Every 15 minutes
- **Purpose:** End expired periods, transition subscriptions
- **Query:** subscription_periods WHERE status = 'active' AND end_at <= now()
- **Action:** For each:
  - If cancel_at_period_end: cancel subscription
  - If auto_renew = false and no paid renewal: pause subscription
  - Otherwise: mark period as 'ended' (renewal already processed)

### Job: Grace Period Expiry

- **Schedule:** Every hour
- **Purpose:** Pause subscriptions after grace period
- **Query:** subscription_periods WHERE grace_end_at <= now() AND subscription.status = 'past_due'
- **Action:** Pause subscription, deactivate entitlement, emit grace_period_expired event

### Job: Payment Retry

- **Schedule:** Every 4 hours
- **Purpose:** Retry failed payments per dunning schedule
- **Query:** Open invoices with failed payments, within retry window
- **Action:** Attempt off-session charge. On final failure: mark uncollectible, pause subscription

### Job: Crypto Invoice Expiry

- **Schedule:** Every hour
- **Purpose:** Void expired crypto invoices
- **Query:** Open invoices overdue by 24+ hours with coinbase payment
- **Action:** Void invoice, mark payment as expired

### Job: Entitlement Sync

- **Schedule:** Every hour
- **Purpose:** Ensure entitlements match subscription state (catch any missed transitions)
- **Query:** Active entitlements where active_to <= now()
- **Action:** Set status to 'inactive'

---

## Admin Functions

These are internal-only, for support/ops dashboard.

**admin_grant_credits**
- Inputs: billing_customer_id, amount, reason, admin_user_id
- Creates manual credit_ledger_entry with audit trail

**admin_extend_subscription**
- Inputs: subscription_id, days, reason, admin_user_id
- Extends current period end_at and entitlement active_to

**admin_process_refund**
- Inputs: invoice_id, amount (optional for partial), reason, admin_user_id
- Processes refund via provider, updates records

**admin_create_comp_subscription**
- Inputs: billing_customer_id, plan_id, duration_days, reason, admin_user_id
- Creates subscription without payment (comp/gift)

**admin_force_subscription_status**
- Inputs: subscription_id, new_status, reason, admin_user_id
- Force status transition (bypasses normal validation, use with caution)

**admin_get_billing_events**
- Inputs: billing_customer_id, subscription_id, invoice_id, event_type, date range, limit, offset
- Returns paginated audit log from billing_event table

---

## Event Types

Events emitted to `billing_event` table and optionally to external webhooks.

### Subscription Events

```
subscription.created
subscription.activated
subscription.trial_started
subscription.trial_ending          (X days before end)
subscription.trial_converted
subscription.trial_expired
subscription.renewed
subscription.renewal_failed
subscription.past_due
subscription.grace_period_started
subscription.grace_period_expired
subscription.paused
subscription.reactivated
subscription.canceled
subscription.cancel_scheduled
subscription.cancel_undone
subscription.upgraded
subscription.downgrade_scheduled
subscription.downgraded
subscription.plan_changed
subscription.refunded
```

### Invoice Events

```
invoice.created
invoice.finalized
invoice.paid
invoice.payment_failed
invoice.voided
invoice.refunded
invoice.partially_refunded
invoice.disputed
invoice.dispute_won
invoice.dispute_lost
invoice.uncollectible
```

### Payment Events

```
payment.created
payment.succeeded
payment.failed
payment.refunded
payment.disputed
```

### Credit Events

```
credits.granted
credits.deducted
credits.reversed
credits.manual_adjustment
```

### Bundle Events

```
bundle.purchased
bundle.refunded
```

### Customer Events

```
customer.created
customer.updated
payment_method.added
payment_method.removed
payment_method.updated
payment_method.expiring_soon
```

---

## Response Shapes

### SubscriptionDetails

| Field | Type | Description |
|-------|------|-------------|
| id | string | Subscription ID |
| status | enum | trialing, active, past_due, paused, canceled |
| plan.id | string | Current plan ID |
| plan.name | string | Plan display name |
| plan.price_amount | integer | Price in cents |
| plan.price_currency | string | 3-letter currency code |
| plan.billing_interval | enum | 'month' or 'year' |
| plan.features | object | Opaque features map |
| pending_plan | object or null | Scheduled plan change (id, name) |
| current_period | object or null | id, start_at, end_at, is_trial |
| auto_renew | boolean | Whether renewal is automatic |
| cancel_at_period_end | boolean | Whether cancellation is scheduled |
| trial_ends_at | string or null | ISO timestamp |
| created_at | string | ISO timestamp |

### InvoiceDetails

| Field | Type | Description |
|-------|------|-------------|
| id | string | Invoice ID |
| purpose | enum | subscription_period, bundle_purchase, plan_change_settlement |
| amount_due | integer | Amount in cents |
| currency | string | 3-letter currency code |
| status | enum | draft, open, paid, void, uncollectible, refunded, disputed |
| due_at | string or null | ISO timestamp |
| paid_at | string or null | ISO timestamp |
| refund_amount | integer or null | Refunded amount in cents |
| payments | array | List of payment records (id, provider, status, amount, confirmed_at) |
| metadata | object | Purpose-specific data |
| created_at | string | ISO timestamp |

### Error Responses

All errors follow this shape:

| Field | Type | Description |
|-------|------|-------------|
| error.code | string | Machine-readable error code |
| error.message | string | Human-readable message |
| error.details | object or null | Additional context |

Error codes:
- `subscription_exists` - Already subscribed
- `invalid_plan` - Plan not found or archived
- `insufficient_credits` - Not enough credits
- `payment_required` - No payment method
- `payment_failed` - Charge declined
- `not_found` - Resource not found
- `invalid_transition` - Invalid status change
- `purchase_limit_reached` - Bundle max purchases hit

---

## Authentication

All endpoints require:

```
Authorization: Bearer <api_key>
X-App-ID: <app_id>
```

Validate:
1. API key is valid and active
2. API key belongs to the app_id
3. Rate limits not exceeded
4. Requested resources belong to the app
