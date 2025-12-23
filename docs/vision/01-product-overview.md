# Product Overview

## The Problem

Every indie SaaS developer does this dance:

1. Set up Clerk/Auth0/Supabase Auth
2. Set up Stripe
3. Write glue code to sync user IDs ↔ Stripe customer IDs
4. Build webhooks to track subscription status
5. Store subscription state somewhere
6. Build middleware to check "can this user do X?"
7. Set up transactional email (Resend/Postmark)
8. Configure DNS for email deliverability

That's 2-4 weeks of work before writing a single line of product code.

---

## The Solution

reauth.dev collapses all of this into one primitive: **a user that knows who they are AND what they've paid for.**

```javascript
const user = await getUser(request)
// {
//   id: 'usr_abc123',
//   email: 'user@example.com',
//   plan: 'pro',
//   subscriptionStatus: 'active',
//   ...
// }
```

One DNS setup. One SDK. One dashboard.

---

## The Setup

```
1. Add DNS records:
   CNAME  auth     →  ingress.reauth.dev
   TXT    _reauth  →  "project=proj_abc123"

2. Configure in dashboard:
   - Google OAuth credentials
   - Stripe account (Connect)
   - Resend API key

3. Done. You now have:
   - auth.example.com/login     (hosted UI)
   - auth.example.com/billing   (customer portal)
   - Magic link emails from your domain
   - Everything wired together
```

---

## Target Market

### Phase 1: Micro-SaaS
- Solo devs and indie hackers
- Side projects going to production
- Simple SaaS with individual users
- Straightforward subscription pricing
- Budget: $0-50/mo on infrastructure

### Phase 2: Macro-SaaS
- Growing indie SaaS
- Small teams (2-10 people)
- B2B with multi-seat needs
- API products needing key management
- Budget: $50-500/mo on infrastructure

---

## Core Value Proposition

| For | Without reauth.dev | With reauth.dev |
|-----|-------------------|-----------------|
| Auth | 1 week setup | 10 minutes |
| Billing integration | 1 week + ongoing maintenance | Already wired |
| User ↔ subscription sync | Custom glue code | Automatic |
| Hosted UI | Build it yourself | Included |
| Email setup | DNS + provider + templates | Paste Resend key |

---

## What reauth.dev Is Not

- **Not an auth provider** — It's auth + billing + email unified
- **Not just Stripe glue** — It's the whole user layer
- **Not enterprise-first** — Built for indie devs, enterprise comes later
- **Not a template** — It's infrastructure, not boilerplate

---

## Competitive Positioning

| Category | Competitors | reauth.dev Edge |
|----------|-------------|-----------------|
| Auth | Clerk, Auth0 | Unified with billing |
| Payments | Stripe direct | Pre-wired to users |
| Email | Resend, Postmark | Already knows your users |

**The moat:** Integration. Each piece alone is replaceable. Together, with one setup? That's sticky.

---

## Success Metrics

### Product Metrics
- Time to first working login: <15 minutes
- DNS setup success rate: >80%
- Docs-to-working-app: <30 minutes

### Business Metrics (Phase 1)
- Paying customers: 20+
- MRR: $500+

### Business Metrics (Phase 2)
- Paying customers: 200+
- MRR: $10k+
- Churn rate: <5% monthly
