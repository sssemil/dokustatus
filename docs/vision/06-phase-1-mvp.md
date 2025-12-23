# Phase 1: MVP (Micro-SaaS)

## Target Customer

- Solo dev, side project, early-stage indie hacker
- Simple SaaS with individual users
- Straightforward subscription pricing
- Budget: $0-50/mo on infrastructure
- Need: "Let me stop building auth and start building my thing"

---

## Scope

### What's In

| Feature | Scope |
|---------|-------|
| **Auth** | Google OAuth + magic link |
| **Billing** | Stripe subscriptions only |
| **Hosted UI** | Login, billing portal, settings |
| **Email** | Magic link via Resend (customer's API key) |
| **SDK** | `getUser()` + Next.js/Express middleware |
| **DNS** | Auth subdomain setup |
| **Dashboard** | Project setup, user list, basic config |

### What's Explicitly Out

| Feature | Why Not Yet |
|---------|-------------|
| GitHub OAuth | One OAuth provider is enough for MVP |
| Password auth | Adds complexity (reset flow, security) |
| Teams/orgs | Micro-SaaS is usually single-user |
| API keys | Their users don't need API access yet |
| Credits | Simple subscriptions are enough |
| Usage-based billing | Complexity they don't need |
| Webhooks | They're not building integrations yet |
| Feature flags | Can hardcode `if (user.plan === 'pro')` |
| Analytics | Use Plausible/Umami separately |
| LemonSqueezy/Paddle | Stripe is enough |
| Referrals | Premature optimization |
| Welcome email | Nice to have, not MVP |
| Receipt emails | Stripe sends these |

---

## Timeline (12 Weeks)

### Weeks 1-4: Foundation + Auth

```
Week 1: Project Setup
├── Rust workspace (reauth-core, reauth-api)
├── Cargo.toml, dependencies
├── Docker Compose (Postgres, Redis)
├── SQLx setup + initial migrations
├── Configuration (config-rs)
├── Logging (tracing)
└── CI/CD (GitHub Actions)

Week 2: Multi-Tenancy + Dashboard Auth
├── Organizations table + CRUD
├── Projects table + CRUD
├── Dashboard login (Google OAuth for yourself)
├── Dashboard sessions
├── Basic dashboard UI (project list)
└── API key generation for SDK

Week 3: End-User Auth - Google OAuth
├── Google OAuth flow
│   ├── Redirect to Google
│   ├── Handle callback
│   ├── Exchange code for tokens
│   ├── Fetch user info
│   └── Create/update end_user
├── Session management
│   ├── Token generation
│   ├── Redis storage
│   ├── Cookie handling
│   └── Session validation endpoint
└── Logout flow

Week 4: End-User Auth - Magic Link
├── Magic link generation
├── Token storage (hashed)
├── Resend integration
│   ├── API client
│   ├── Magic link template
│   └── Send email
├── Verify endpoint
├── Rate limiting (5/hour/email)
└── Link expiry (15 min)
```

### Weeks 5-8: DNS + Stripe

```
Week 5: Domain Setup
├── DNS verification system
│   ├── TXT record check
│   ├── CNAME record check
│   └── Verification status
├── Project domain configuration
├── Verification UI in dashboard
└── Clear error messages

Week 6: SSL + Routing
├── Subdomain routing (auth.customer.com)
├── SSL certificate provisioning
├── Wildcard cert handling
├── Request routing to correct project
└── Host header validation

Week 7: Stripe Integration
├── Stripe Connect (Standard)
├── Connect OAuth flow
├── Plan configuration in dashboard
├── Stripe Price creation/linking
└── Store Stripe credentials

Week 8: Billing Flow
├── Checkout session creation
├── Success/cancel handling
├── Webhook endpoint
│   ├── Signature verification
│   ├── checkout.session.completed
│   ├── customer.subscription.created
│   ├── customer.subscription.updated
│   ├── customer.subscription.deleted
│   └── invoice.payment_failed
├── Subscription status sync
└── Session refresh on change
```

### Weeks 9-12: SDK + Polish

```
Week 9: Hosted UI
├── Login page (Google + magic link)
├── Magic link sent page
├── Verify page (loading state)
├── Settings page (profile, logout)
├── Branding (logo, colors)
└── Mobile responsiveness

Week 10: Billing Portal + Email
├── Stripe Customer Portal integration
├── Portal redirect from /billing
├── Email template (magic link)
├── Resend error handling
├── Email logging (minimal)
└── Test email flow

Week 11: TypeScript SDK
├── Package setup (tsup, npm)
├── getUser() implementation
├── Type definitions
├── Next.js middleware
├── Express middleware
├── Error handling
└── npm publish

Week 12: Launch Prep
├── Documentation site
│   ├── Quickstart guide
│   ├── DNS setup guide (with screenshots)
│   ├── SDK reference
│   └── Example app
├── Dashboard polish
│   ├── Onboarding wizard
│   ├── User list view
│   ├── Basic metrics
│   └── Error states
├── Load testing
├── Security review
├── Marketing site
└── Beta launch
```

---

## Technical Specifications

### Auth Flows

**Google OAuth:**
```
1. GET /login
   → Render login page

2. GET /auth/google
   → Generate state, store in Redis
   → Redirect to Google OAuth URL

3. GET /callback/google?code=...&state=...
   → Verify state
   → Exchange code for tokens
   → Get user info from Google
   → Find or create end_user (by email)
   → Update google_id, name, avatar
   → Create session
   → Set cookie
   → Redirect to app (or redirect param)
```

**Magic Link:**
```
1. POST /auth/magic-link
   Body: { email }
   → Check rate limit
   → Generate token (32 bytes, base64url)
   → Store hash in magic_links
   → Send email via Resend
   → Return success

2. GET /verify?token=...
   → Hash token
   → Find magic_link by hash
   → Check not expired, not used
   → Mark as used
   → Find or create end_user (by email)
   → Create session
   → Set cookie
   → Redirect to app
```

### Session Management

**Storage (Redis):**
```
Key:   session:{token_hash}
Value: {
    "session_id": "sess_...",
    "user_id": "usr_...",
    "project_id": "proj_...",
    "email": "user@example.com",
    "name": "Jane",
    "avatar_url": "https://...",
    "plan": "pro",
    "plan_features": { "maxProjects": 10 },
    "subscription_status": "active",
    "created_at": "...",
    "expires_at": "..."
}
TTL:   30 days
```

**Cookie:**
```
Name:     reauth_session
Value:    {token}  (not hashed, hashed for storage lookup)
Domain:   .customer.com
HttpOnly: true
Secure:   true
SameSite: Lax
MaxAge:   30 days
```

### SDK Endpoint

**GET /v1/session**
```
Headers:
  Cookie: reauth_session=...
  OR
  Authorization: Bearer {session_token}

Response 200:
{
    "user": {
        "id": "usr_...",
        "email": "user@example.com",
        "name": "Jane",
        "avatarUrl": "https://...",
        "plan": "pro",
        "planFeatures": { "maxProjects": 10 },
        "subscriptionStatus": "active",
        "subscriptionEndsAt": null,
        "createdAt": "...",
        "lastSeenAt": "..."
    }
}

Response 401:
{
    "error": "invalid_session",
    "message": "Session is invalid or expired"
}
```

### Stripe Webhooks

**Endpoint:** `POST /webhooks/stripe`

**Events handled:**
```rust
match event.type_ {
    "checkout.session.completed" => {
        // Create subscription record
        // Update user's plan_id, subscription_status
        // Refresh session in Redis
    }
    "customer.subscription.updated" => {
        // Update subscription status
        // Handle plan changes
        // Refresh session
    }
    "customer.subscription.deleted" => {
        // Mark subscription as cancelled
        // Refresh session
    }
    "invoice.payment_failed" => {
        // Update status to past_due
        // Refresh session
        // (Stripe sends email automatically)
    }
    _ => {
        // Log unknown event, return 200
    }
}
```

---

## Dashboard Pages

### Onboarding Wizard

```
Step 1: Create Project
├── Project name
├── Domain (e.g., myapp.com)
└── [Create]

Step 2: Add DNS Records
├── Show required records:
│   ├── CNAME auth → ingress.reauth.dev
│   └── TXT _reauth → project=proj_...
├── [Verify DNS] button
├── Status indicators (✓ or ✗)
└── Registrar-specific help links

Step 3: Connect Google OAuth
├── Instructions to create OAuth app
├── Client ID input
├── Client Secret input
├── [Test Login] button
└── Success indicator

Step 4: Connect Stripe
├── [Connect with Stripe] button
├── Stripe Connect OAuth flow
├── Success indicator
└── Create first plan

Step 5: Connect Resend
├── API key input
├── From address input
├── [Send Test Email] button
└── Success indicator

✓ Ready to go!
├── Your login URL: https://auth.myapp.com/login
├── SDK code snippet
└── [View Documentation]
```

### Project Dashboard

```
┌─────────────────────────────────────────────────────┐
│  myapp.com                              [Settings]  │
├─────────────────────────────────────────────────────┤
│                                                     │
│  Quick Stats                                        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐         │
│  │ 47       │  │ 12       │  │ $240     │         │
│  │ Users    │  │ This week│  │ MRR      │         │
│  └──────────┘  └──────────┘  └──────────┘         │
│                                                     │
│  Recent Users                                       │
│  ┌─────────────────────────────────────────────┐   │
│  │ jane@example.com    Pro    Active   2h ago  │   │
│  │ bob@test.com        Free   -        1d ago  │   │
│  │ alice@company.com   Pro    Active   3d ago  │   │
│  └─────────────────────────────────────────────┘   │
│  [View All Users]                                   │
│                                                     │
│  Setup Status                                       │
│  ✓ DNS verified                                    │
│  ✓ Google OAuth connected                          │
│  ✓ Stripe connected                                │
│  ✓ Resend connected                                │
│                                                     │
└─────────────────────────────────────────────────────┘
```

---

## Pricing (Phase 1)

```
FREE        $0/mo
├── 1 project
├── 100 users
├── 1,000 emails/mo
└── reauth branding on hosted UI

PRO         $20/mo
├── 3 projects
├── 5,000 users
├── 50,000 emails/mo
└── Remove branding
```

No Scale tier. No Enterprise. Not yet.

---

## Success Criteria

| Metric | Target |
|--------|--------|
| Time to first working login | <15 minutes |
| DNS setup success rate | >80% |
| Docs-to-working-app | <30 minutes |
| Paying customers | 20+ |
| MRR | $500+ |

**Phase 1 is done when you have paying customers who are not your friends.**

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| DNS setup is confusing | Registrar-specific screenshots, video guide |
| Stripe Connect is complex | Wizard with clear steps, test mode first |
| Session sync gets stale | Webhook-driven refresh, short cache TTL |
| Email deliverability issues | Customer owns Resend setup, not our problem |
| Google OAuth app approval | Guide for development vs production |

---

## What's Not Built (Intentionally)

These are deferred to Phase 2, not forgotten:

- [ ] GitHub OAuth
- [ ] Password authentication
- [ ] Teams/organizations
- [ ] API key management
- [ ] Credits system
- [ ] Usage-based billing
- [ ] Webhooks (customer-facing)
- [ ] Feature flags
- [ ] Analytics
- [ ] LemonSqueezy/Paddle
- [ ] Referrals
- [ ] Welcome emails
- [ ] Admin impersonation
- [ ] User segments
- [ ] GDPR self-serve
- [ ] Embeddable components
