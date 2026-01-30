# Billing System Specification

Comprehensive spec for reauth.dev's billing subsystem: provider-agnostic subscriptions, bundles, credits, and entitlements.

---

## Reading Order

| # | Document | Description |
|---|----------|-------------|
| 1 | [OVERVIEW.md](./OVERVIEW.md) | Architecture principles, v1 scope, entity map, provider flows |
| 2 | [DATA_MODEL.md](./DATA_MODEL.md) | PostgreSQL schema — tables, enums, constraints, indexes |
| 3 | [STATE_MACHINES.md](./STATE_MACHINES.md) | Subscription, invoice, payment, period lifecycle transitions |
| 4 | [FLOWS.md](./FLOWS.md) | Step-by-step flows: checkout, renewal, cancellation, upgrade, refund |
| 5 | [DECISIONS.md](./DECISIONS.md) | 25 policy decisions with rationale (locked in for v1) |
| 6 | [EDGE_CASES.md](./EDGE_CASES.md) | 60+ acceptance criteria in Given/When/Then format |
| 7 | [API_SURFACE.md](./API_SURFACE.md) | Internal functions, REST endpoints, webhooks, background jobs, events |

---

## Origin and Status

These specs were designed as a greenfield billing system reference. The reauth codebase already has substantial billing implementation that predates this spec. The existing code is the starting point; this spec is the **canonical target** for where the billing system is headed.

Where existing code conflicts with these specs, the spec is authoritative (pre-production project, no real users).

---

## Current Implementation Status

### Already Built (in Rust codebase)

| Concept | Existing Code |
|---------|--------------|
| Stripe integration | `infra/stripe_client.rs`, `infra/stripe_payment_adapter.rs` |
| Subscription plans (CRUD, pricing) | `domain/entities/subscription_plan.rs`, `persistence/subscription_plan.rs` |
| User subscriptions (status tracking) | `domain/entities/user_subscription.rs`, `persistence/user_subscription.rs` |
| Payment records (history) | `persistence/billing_payment.rs` |
| Webhook delivery (signing, retries) | `infra/webhook_signer.rs`, `infra/webhook_delivery_worker.rs` |
| Stripe webhooks (ingestion) | `adapters/http/routes/public_domain_auth/billing_webhooks.rs` |
| Payment provider abstraction | `application/ports/payment_provider.rs`, `application/use_cases/payment_provider_factory.rs` |
| Billing state machine (provider switching) | `domain/entities/billing_state.rs` |
| Payment modes (test/live) | `domain/entities/payment_mode.rs` |
| Dummy payment provider (testing) | `infra/dummy_payment_client.rs` |
| Billing use cases (proration, MRR) | `application/use_cases/domain_billing.rs` |
| Subscription events (audit) | `persistence/subscription_event.rs` |

All paths relative to `apps/api/src/`.

### Not Yet Built (spec-only, needs implementation)

| Concept | Spec Document |
|---------|--------------|
| Bundles (one-time purchases) | DATA_MODEL, FLOWS (section 13) |
| Billing customer (separate identity) | DATA_MODEL |
| Provider customer ref | DATA_MODEL |
| Payment methods (stored cards) | DATA_MODEL |
| Invoices (as first-class entity) | DATA_MODEL, FLOWS |
| Subscription periods (as separate entity) | DATA_MODEL, STATE_MACHINES |
| Entitlements (derived access) | DATA_MODEL, FLOWS (section 19) |
| Credits ledger (ledger-based balance) | DATA_MODEL, FLOWS (section 18) |
| Coinbase Commerce adapter | OVERVIEW, FLOWS (sections 2, 7) |
| Background cron jobs (renewal, trial, grace) | FLOWS (section 20), API_SURFACE |
| Trial management | FLOWS (sections 3-5), EDGE_CASES (section 4) |
| Dispute/chargeback handling | FLOWS (section 16), EDGE_CASES (section 12) |
| Plan upgrade/downgrade logic | FLOWS (sections 11-12), EDGE_CASES (section 8) |
| Grace period and dunning | FLOWS (section 8), DECISIONS (D20, D21) |

---

## Divergences to Note

**ID generation**: The spec uses `gen_random_uuid()` (plain UUIDs). The existing codebase uses `generate_id(prefix)` which produces prefixed IDs like `sub_a1b2c3...`. When implementing the spec schema, use the existing `generate_id()` pattern with appropriate prefixes.

**Table naming**: The spec uses singular table names (`plan`, `invoice`, `subscription`). The existing codebase uses `subscription_plans`, `user_subscriptions`, `billing_payments`. Decide on convention when implementing migrations — recommend aligning new tables with the existing naming style.

**Domain scoping**: The spec references `app_id` as the tenant key. The existing codebase uses `domain_id`. These are equivalent concepts; use `domain_id` in implementation.

**Money and credits storage**: All monetary amounts and credit balances are **purely integer**. Money is stored in cents (e.g., $10.00 = 1000), credits as raw integers. The billing system never formats or converts values for display — developers present them however they like.

**Timestamps**: Both agree: `TIMESTAMPTZ` (UTC) everywhere.
