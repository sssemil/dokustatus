# reauth.dev

> **Auth, billing, email. One DNS setup.**

reauth.dev is unified user infrastructure for indie SaaS developers. Instead of wiring together Clerk + Stripe + Resend, developers add a few DNS records and get everything working together out of the box.

---

## The Promise

```
Add 2 DNS records. Your SaaS now has login and billing. No code.
```

---

## Documentation

| Document | Description |
|----------|-------------|
| [Product Overview](./01-product-overview.md) | Problem, solution, target market |
| [Architecture](./02-architecture.md) | Tech stack, infrastructure, workspace |
| [Data Model](./03-data-model.md) | Database schema |
| [SDK & Integration](./04-sdk-integration.md) | SDK surface, middleware, examples |
| [Hosted UI](./05-hosted-ui.md) | Pages, flows, branding |
| [Phase 1: MVP](./06-phase-1-mvp.md) | Micro-SaaS scope, 12-week timeline |
| [Phase 2: Growth](./07-phase-2-growth.md) | Macro-SaaS expansion |
| [Pricing](./08-pricing.md) | Pricing model |

---

## Quick Start (The Vision)

```javascript
import { getUser } from 'reauth'

const user = await getUser(request)
// {
//   id: 'usr_abc123',
//   email: 'user@example.com',
//   plan: 'pro',
//   subscriptionStatus: 'active',
//   ...
// }
```

---

## Core Insight

Every indie SaaS developer rebuilds the same thing: a join between "who is this user" and "what have they paid for." 

reauth.dev collapses this into one primitive: **a user that knows who they are AND what they've paid for.**

---

## Development Phases

### Phase 1: Micro-SaaS (12 weeks)
- Google OAuth + magic link
- Stripe subscriptions
- Hosted UI (login, billing)
- Email via Resend
- `getUser()` SDK

### Phase 2: Macro-SaaS (36 weeks)
- Teams & organizations
- API keys + rate limiting
- Credits & usage-based billing
- Analytics & revenue metrics
- Webhooks, feature flags, segments
- Multi-provider billing (LemonSqueezy, Paddle)

---

## Tech Stack

- **Language:** Rust
- **Framework:** Axum
- **Database:** PostgreSQL
- **Cache:** Redis
- **Email:** Resend (customer's API key)

---

*Last updated: December 2024*
