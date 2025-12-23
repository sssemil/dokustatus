# Pricing

## Philosophy

1. **Free tier is real** — Not a trial. Enough for real side projects.
2. **Pricing grows with success** — Pay more when you're making money.
3. **No surprises** — Clear limits, predictable overages.
4. **Indie-friendly** — Pro at $20/mo is accessible.

---

## Phase 1 Pricing

Simple two-tier model for MVP launch.

```
┌────────────────────────────────────────────────────────────────┐
│                                                                │
│   FREE                              PRO                        │
│   $0/mo                             $20/mo                     │
│                                                                │
│   ✓ 1 project                       ✓ 3 projects               │
│   ✓ 100 users                       ✓ 5,000 users              │
│   ✓ 1,000 emails/mo                 ✓ 50,000 emails/mo         │
│   ✓ Google OAuth                    ✓ Google OAuth             │
│   ✓ Magic link                      ✓ Magic link               │
│   ✓ Stripe billing                  ✓ Stripe billing           │
│   ✓ Hosted UI                       ✓ Hosted UI                │
│   ✓ SDK access                      ✓ SDK access               │
│   • reauth branding                 ✓ Remove branding          │
│   • Community support               ✓ Email support            │
│                                                                │
│   [Get Started]                     [Start Free Trial]         │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

### Free Tier Limits

| Resource | Limit | What Happens |
|----------|-------|--------------|
| Projects | 1 | Can't create more |
| Users | 100 | New signups blocked |
| Emails | 1,000/mo | Magic links fail |

### Pro Tier Details

- **Trial:** 14 days free
- **Billing:** Monthly or annual (2 months free)
- **Payment:** Stripe only (we use reauth for ourselves)

---

## Phase 2 Pricing

Expanded tiers after feature growth.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│   FREE              PRO               SCALE             ENTERPRISE          │
│   $0/mo             $20/mo            $90/mo            Custom              │
│                                                                             │
│   1 project         Unlimited         Unlimited         Unlimited           │
│   100 users         5,000 users       10,000 users      Unlimited           │
│   1,000 emails      50,000 emails     100,000 emails    Unlimited           │
│   5,000 pageviews   500,000 pv        5,000,000 pv      Unlimited           │
│                                                                             │
│   ✓ Google OAuth    ✓ Everything      ✓ Everything      ✓ Everything        │
│   ✓ Magic link        in Free           in Pro            in Scale          │
│   ✓ Stripe billing  ✓ API keys        ✓ Teams           ✓ SSO/SAML          │
│   ✓ Hosted UI       ✓ Webhooks        ✓ Analytics       ✓ SLA               │
│   • Branding        ✓ No branding     ✓ Feature flags   ✓ Dedicated infra   │
│   • Community       ✓ Email support   ✓ Segments        ✓ On-premise        │
│                     ✓ GitHub OAuth    ✓ Priority        ✓ 24/7 support      │
│                     ✓ Password auth     support         ✓ Custom contract   │
│                     ✓ Multi-provider                                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Feature Breakdown

| Feature | Free | Pro | Scale | Enterprise |
|---------|:----:|:---:|:-----:|:----------:|
| Projects | 1 | ∞ | ∞ | ∞ |
| Users | 100 | 5K | 10K | ∞ |
| Emails/mo | 1K | 50K | 100K | ∞ |
| Pageviews/mo | 5K | 500K | 5M | ∞ |
| Google OAuth | ✓ | ✓ | ✓ | ✓ |
| Magic link | ✓ | ✓ | ✓ | ✓ |
| GitHub OAuth | | ✓ | ✓ | ✓ |
| Password auth | | ✓ | ✓ | ✓ |
| Stripe | ✓ | ✓ | ✓ | ✓ |
| LemonSqueezy | | ✓ | ✓ | ✓ |
| Paddle | | ✓ | ✓ | ✓ |
| API keys | | ✓ | ✓ | ✓ |
| Rate limiting | | ✓ | ✓ | ✓ |
| Credits | | ✓ | ✓ | ✓ |
| Webhooks | | ✓ | ✓ | ✓ |
| Teams | | | ✓ | ✓ |
| Feature flags | | | ✓ | ✓ |
| Analytics | | | ✓ | ✓ |
| Segments | | | ✓ | ✓ |
| Referrals | | | ✓ | ✓ |
| GDPR self-serve | | | ✓ | ✓ |
| Impersonation | | | ✓ | ✓ |
| Embeddables | | | ✓ | ✓ |
| SSO/SAML | | | | ✓ |
| SLA | | | | ✓ |
| Dedicated infra | | | | ✓ |
| On-premise | | | | ✓ |
| Remove branding | | ✓ | ✓ | ✓ |

---

## Overage Pricing

When you exceed plan limits, you're billed for overages.

| Resource | Overage Rate |
|----------|--------------|
| Users | $0.01/user/month |
| Emails | $0.50/1,000 emails |
| Pageviews | $0.10/10,000 pageviews |
| API key verifications | $0.05/10,000 calls |

### How Overages Work

1. **Soft limits** — You're not blocked immediately
2. **Grace period** — 10% buffer before billing
3. **Email warning** — At 80% and 100% of limit
4. **Monthly billing** — Overages added to next invoice
5. **Dashboard visibility** — Always see current usage

### Example

> Pro plan ($20/mo) with 6,500 users:
> - Included: 5,000 users
> - Overage: 1,500 users × $0.01 = $15
> - Total: $35/mo

---

## Annual Pricing

Pay annually, get 2 months free.

| Plan | Monthly | Annual | Savings |
|------|---------|--------|---------|
| Pro | $20/mo | $200/yr | $40 (17%) |
| Scale | $90/mo | $900/yr | $180 (17%) |

---

## Enterprise Pricing

Custom pricing based on:

- Number of users
- Number of projects
- Support requirements
- Infrastructure needs
- Contract length

**Typical range:** $500-5,000/mo

**Includes:**
- Dedicated account manager
- Custom onboarding
- SLA (99.9% uptime)
- Priority support (< 4h response)
- Custom integrations
- Security review

**Contact:** enterprise@reauth.dev

---

## Comparison to Alternatives

### Auth Providers

| Provider | Price at 5K MAU |
|----------|-----------------|
| Auth0 | $240/mo |
| Clerk | $125/mo |
| Supabase Auth | $25/mo |
| **reauth.dev** | **$20/mo** |

*Plus: reauth includes billing, email, analytics*

### Full Stack

| Stack | Price |
|-------|-------|
| Clerk + Stripe + Resend + Plausible | ~$100/mo |
| **reauth.dev Pro** | **$20/mo** |

---

## FAQ

**Can I switch plans anytime?**
Yes. Upgrades are immediate, prorated. Downgrades take effect at next billing cycle.

**What happens if I exceed limits?**
You're charged for overages on your next invoice. No service interruption.

**Is there a free trial?**
Pro and Scale have 14-day free trials. No credit card required.

**Do you offer discounts?**
- Startups: 50% off first year (apply)
- Non-profits: 50% off forever (verify)
- Education: Free Scale tier (verify)

**Can I pay with crypto?**
Not currently. USD via Stripe only.

**What's your refund policy?**
Full refund within 14 days if not satisfied. No questions asked.

---

## Pricing Changes

We commit to:

1. **90-day notice** for any price increases
2. **Grandfather existing customers** for 1 year
3. **Never reduce limits** on existing plans
4. **Transparent communication** about changes

---

## Internal Notes (Not Public)

### Unit Economics Target

| Metric | Target |
|--------|--------|
| Gross margin | >80% |
| CAC payback | <6 months |
| LTV:CAC | >3:1 |
| Net revenue retention | >100% |

### Cost Structure (per customer)

| Cost | Amount |
|------|--------|
| Infra (per 1K users) | ~$0.50/mo |
| Stripe fees | 2.9% + $0.30 |
| Support (avg) | ~$2/mo |
| Email (Resend) | Customer pays |

### Pricing Rationale

- **$20 Pro:** Accessible for indie devs, covers costs at 1K users
- **$90 Scale:** Margins improve, teams feature drives upgrades
- **Enterprise:** High-touch, custom, high margin
