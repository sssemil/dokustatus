# Phase 2: Growth (Macro-SaaS)

## Target Customer

- Growing indie SaaS, small team (2-10 people)
- B2B products with multi-seat needs
- API products needing key management
- Budget: $50-500/mo on infrastructure
- Need: "I've outgrown the basics, but I don't want to rebuild"

---

## Prerequisites

Phase 2 begins after Phase 1 is complete:
- ✓ 20+ paying customers
- ✓ $500+ MRR
- ✓ Core product is stable
- ✓ Customer feedback collected

---

## Features Added

| Feature | Description |
|---------|-------------|
| Teams & Orgs | Multi-user organizations with roles |
| API Keys | End-user API key management |
| Rate Limiting | Configurable per-key limits |
| Credits | Balance-based billing model |
| Usage Billing | Metered billing to Stripe |
| Webhooks | Event delivery to customer endpoints |
| Feature Flags | User/plan targeting |
| Analytics | Pageviews, events, revenue |
| Multi-Provider | LemonSqueezy, Paddle |
| Segments | Auto-group users |
| Referrals | Code tracking, rewards |
| Compliance | GDPR export/delete |
| Waitlist | Pre-launch signups |
| Impersonation | Admin "login as" |
| Embeddables | Drop-in components |
| More Auth | GitHub OAuth, passwords |

---

## Timeline (36 Weeks)

### Weeks 13-18: Teams + API Keys

```
Week 13-14: Team Data Model
├── Teams table
├── Team members table
├── Team invitations table
├── Membership CRUD
├── Role system (owner, admin, member)
└── Team ↔ billing relationship

Week 15-16: Team Flows
├── Create team
├── Invite member (email flow)
├── Accept invitation
├── Remove member
├── Transfer ownership
├── Leave team
├── Hosted UI (/team)
└── Session: current team context

Week 17-18: API Keys
├── Key generation (secure random)
├── Key hashing (Argon2)
├── Prefix system (sk_live_, sk_test_)
├── Verification endpoint (<10ms)
├── Permissions system
├── Rate limiting (Redis sliding window)
├── Hosted UI (/keys)
└── SDK: api.verify()
```

### Weeks 19-24: Advanced Billing

```
Week 19-20: Credits System
├── Credits table + ledger
├── Balance tracking
├── Add credits (purchase)
├── Deduct credits (usage)
├── Top-up checkout flow
├── Low balance warnings
├── Hosted UI (/credits)
└── SDK: credits.deduct()

Week 21-22: Usage-Based Billing
├── Usage records table
├── Metering API
├── Aggregation jobs
├── Stripe usage reporting
├── Hybrid billing support
├── Dashboard usage view
└── End-user usage display

Week 23-24: Dunning + Providers
├── Smart dunning
│   ├── Retry scheduling
│   ├── Email notifications
│   ├── In-app banners
│   └── Recovery tracking
├── LemonSqueezy adapter
│   ├── PaymentProvider trait impl
│   ├── Webhook handling
│   └── Checkout flow
├── Paddle adapter
│   ├── PaymentProvider trait impl
│   ├── Webhook handling
│   └── Checkout flow
└── Provider switching
```

### Weeks 25-30: Analytics + Webhooks

```
Week 25-26: Pageview Analytics
├── Tracking script (<3KB)
├── Collection endpoint
├── Postgres partitioning
├── Aggregation queries
├── Dashboard charts
│   ├── Visitors over time
│   ├── Top pages
│   ├── Top referrers
│   ├── Countries
│   └── Devices
└── Real-time visitors (Redis)

Week 27-28: Revenue Analytics
├── Revenue metrics calculation
│   ├── MRR
│   ├── ARR
│   ├── New/churned/expansion MRR
│   ├── Churn rate
│   └── ARPU
├── Daily aggregation job
├── Revenue dashboard
├── Trends and charts
└── Export API

Week 29-30: Webhooks
├── Event bus (internal)
├── Event types
│   ├── user.created, user.updated, user.deleted
│   ├── subscription.created, .updated, .cancelled
│   ├── payment.succeeded, .failed
│   ├── team.created, member.added
│   └── api_key.created, .revoked
├── Webhook endpoints table
├── Delivery engine
│   ├── Signature (HMAC)
│   ├── HTTP delivery
│   ├── Retry logic
│   └── Dead letter
├── Dashboard UI
└── SDK: webhooks.verify()
```

### Weeks 31-36: Flags + Segments + Compliance

```
Week 31-32: Feature Flags
├── Flag definition table
├── Targeting rules
│   ├── By user ID
│   ├── By plan
│   ├── By percentage
│   ├── By segment
│   └── Custom rules
├── Fast evaluation engine
├── Dashboard flag builder
├── SDK: flags.isEnabled()
└── React hook: useFlag()

Week 33-34: User Segments
├── Segment definition
├── Rules engine
│   ├── login_count > N
│   ├── plan = 'pro'
│   ├── last_seen > 14 days ago
│   └── Custom conditions
├── Auto-assignment job
├── Enter/exit events
├── Dashboard segment builder
└── Segment → flag targeting

Week 35-36: Compliance
├── Data export flow
│   ├── User requests export
│   ├── Gather reauth data
│   ├── Webhook to customer
│   ├── Download available
│   └── Audit log
├── Data deletion flow
│   ├── User requests deletion
│   ├── Confirmation email
│   ├── Grace period
│   ├── Webhook to customer
│   ├── Cascade delete
│   └── Audit log
├── Consent management
├── Dashboard for requests
└── Hosted UI: request flows
```

### Weeks 37-42: Referrals + Waitlist + More

```
Week 37-38: Referrals
├── Referral program config
├── Code generation
├── Tracking
│   ├── Signup attribution
│   ├── Paid conversion
│   └── Attribution window
├── Reward distribution
│   ├── Credits
│   ├── Discount
│   └── Manual approval
├── Hosted UI (/referrals)
├── Dashboard analytics
└── Fraud detection basics

Week 39-40: Waitlist + Impersonation
├── Waitlist mode toggle
├── Waitlist entries table
├── Referral on waitlist
├── Position tracking
├── Hosted UI (/waitlist)
├── Batch invite
├── Admin impersonation
│   ├── Special session type
│   ├── Audit logging
│   ├── Visual indicator
│   └── Time limit
└── Dashboard controls

Week 41-42: More Auth
├── GitHub OAuth
├── Password auth (optional)
│   ├── Argon2 hashing
│   ├── Password reset flow
│   ├── Strength validation
│   └── Hosted UI updates
├── Additional OAuth providers
│   ├── Generic OAuth2
│   └── Easy to add more
└── Multi-environment
    ├── dev/staging/prod
    ├── Separate configs
    └── Test mode everywhere
```

### Weeks 43-48: Embeddables + Polish

```
Week 43-44: Embeddable Components
├── Iframe architecture
├── PostMessage API
├── Components
│   ├── UserButton (avatar + menu)
│   ├── BillingWidget
│   ├── CreditBalance
│   └── TeamSwitcher
├── Theming inheritance
├── React wrapper
├── Vue wrapper
└── Vanilla JS embed

Week 45-46: SDK Expansion
├── Python SDK
├── Go SDK
├── PHP SDK (if demand)
├── CLI tool
│   ├── reauth login
│   ├── reauth projects
│   ├── reauth dns check
│   └── reauth test
└── Framework guides
    ├── SvelteKit
    ├── Remix
    ├── Nuxt
    └── Rails

Week 47-48: Final Polish
├── Dashboard onboarding v2
├── Documentation complete
├── Example apps
│   ├── Next.js SaaS starter
│   ├── API with keys example
│   └── Team app example
├── Security audit
├── Penetration testing
├── Load testing
├── Status page
└── Scale tier launch
```

---

## Feature Details

### Teams & Organizations

```typescript
// SDK surface
const user = await getUser(request)

user.teams        // [{ id, name, role }]
user.currentTeam  // { id, name, role } or null

// Team-scoped operations
await billing.createCheckout(user.currentTeam.id, { plan: 'team_pro' })
await api.verify(request, { teamId: user.currentTeam.id })
```

**Roles:**
- `owner` — Full access, can delete team, transfer ownership
- `admin` — Manage members, billing, settings
- `member` — Use product, no admin access

### API Keys

```typescript
// Their users create keys at /keys
// Key format: sk_live_abc123def456...

// SDK verification
const key = await api.verify(request)
// {
//   valid: true,
//   keyId: 'key_...',
//   userId: 'usr_...',
//   teamId: 'team_...',
//   permissions: ['read', 'write'],
//   rateLimit: { limit: 1000, remaining: 847, reset: 1702847200 }
// }

// With requirements
const key = await api.verify(request, {
    require: ['write'],
    cost: 10,  // deduct credits
})
```

### Feature Flags

```typescript
// SDK
const enabled = await flags.isEnabled('new-dashboard', user)
const variant = await flags.getVariant('checkout-flow', user)

// React
function Component() {
    const showNewFeature = useFlag('new-feature')
    return showNewFeature ? <NewFeature /> : <OldFeature />
}
```

**Targeting:**
```json
{
    "rules": [
        { "type": "user_id", "values": ["usr_123"], "enabled": true },
        { "type": "plan", "values": ["pro", "enterprise"], "enabled": true },
        { "type": "segment", "values": ["beta_users"], "enabled": true },
        { "type": "percentage", "value": 25, "enabled": true }
    ],
    "default": false
}
```

### Webhooks

**Event types:**
```
user.created
user.updated
user.deleted
subscription.created
subscription.updated
subscription.cancelled
payment.succeeded
payment.failed
team.created
team.member_added
team.member_removed
api_key.created
api_key.revoked
credits.added
credits.depleted
compliance.export_requested
compliance.deletion_requested
```

**Payload:**
```json
{
    "id": "evt_...",
    "type": "subscription.updated",
    "created": "2024-12-15T...",
    "data": {
        "user_id": "usr_...",
        "subscription_id": "sub_...",
        "old_plan": "free",
        "new_plan": "pro"
    }
}
```

### Analytics

**SDK:**
```typescript
// Track pageview (usually automatic via script)
analytics.pageview()

// Track event
analytics.track('feature_used', {
    feature: 'export',
    format: 'csv'
})

// Identify (link anonymous to user)
analytics.identify(user.id)
```

**Tracking script:**
```html
<script
    defer
    data-project="proj_..."
    src="https://js.reauth.dev/analytics.js"
></script>
```

### Compliance (GDPR)

**Export flow:**
```
1. User clicks "Export my data" in /settings
2. Request logged, event emitted
3. Webhook sent to customer: "User X requested export"
4. Customer gathers their app data
5. reauth gathers its data (profile, activity)
6. Download link available (expires in 7 days)
7. User downloads, audit logged
```

**Deletion flow:**
```
1. User clicks "Delete my account" in /settings
2. Confirmation email sent
3. User confirms via link
4. Grace period starts (7 days configurable)
5. Webhook sent to customer: "User X requested deletion"
6. Customer deletes their app data
7. reauth deletes after grace period
8. Sessions revoked, data purged
9. Audit log retained (anonymized)
```

---

## Phase 2 Pricing

```
FREE        $0/mo
├── 1 project
├── 100 users
├── 1,000 emails/mo
├── reauth branding

PRO         $20/mo
├── Unlimited projects
├── 5,000 users
├── 50,000 emails/mo
├── API keys
├── Webhooks
├── Remove branding

SCALE       $90/mo
├── 10,000 users
├── 100,000 emails/mo
├── Teams
├── Analytics
├── Feature flags
├── Priority support

ENTERPRISE  Custom
├── Unlimited everything
├── SSO/SAML
├── SLA
├── Dedicated infra
├── On-premise option
```

**Overages:**
| Resource | Rate |
|----------|------|
| Users | $0.01/user/mo |
| Emails | $0.50/1,000 |
| Pageviews | $0.10/10,000 |
| API verifications | $0.05/10,000 |

---

## Success Criteria

| Metric | Target |
|--------|--------|
| Paying customers | 200+ |
| MRR | $10k+ |
| Scale tier adoption | 20%+ of paid |
| Enterprise inquiries | 5+/month |
| Churn rate | <5% monthly |
| NPS | 40+ |

---

## Migration Path

Phase 1 customers upgrading to Phase 2 features:

1. **Adding Teams** — Existing user becomes team owner, can invite members
2. **Adding API Keys** — Enable in dashboard, hosted UI appears
3. **Adding Credits** — Configure alongside or instead of subscriptions
4. **Adding Analytics** — Add tracking script, data starts flowing
5. **Adding Webhooks** — Configure endpoints in dashboard

No breaking changes. All features are additive.
