# Architecture

## Tech Stack

| Component | Technology | Rationale |
|-----------|------------|-----------|
| Language | Rust | Performance, safety, long-lived infrastructure |
| Web Framework | Axum | Async, tower ecosystem, type-safe |
| Database | PostgreSQL | Reliable, feature-rich, familiar |
| Cache/Sessions | Redis | Fast, pub/sub, job queues |
| Email | Resend API | Customer brings their key |
| Payments | Stripe | Start with one, abstract for others |

---

## Infrastructure Overview

```
┌──────────────────────────────────────────────────────────────────────────┐
│                         reauth.dev Infrastructure                        │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │                          Edge Layer                                │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                │  │
│  │  │ Cloudflare  │  │   Nginx/    │  │    Rate     │                │  │
│  │  │  DNS + SSL  │  │   Traefik   │  │   Limiting  │                │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                      │                                   │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │                       Rust Application                             │  │
│  │                                                                    │  │
│  │  ┌────────────────────────────────────────────────────────────┐   │  │
│  │  │                      API Server (Axum)                     │   │  │
│  │  │                                                            │   │  │
│  │  │  • Auth routes (/login, /callback, /verify, /logout)       │   │  │
│  │  │  • Hosted UI serving                                       │   │  │
│  │  │  • SDK endpoints (/v1/user, /v1/session)                   │   │  │
│  │  │  • Dashboard API                                           │   │  │
│  │  │  • Stripe webhooks                                         │   │  │
│  │  └────────────────────────────────────────────────────────────┘   │  │
│  │                              │                                     │  │
│  │  ┌────────────────────────────────────────────────────────────┐   │  │
│  │  │                     Core Library                           │   │  │
│  │  │  • Domain models    • Session management                   │   │  │
│  │  │  • Auth logic       • Stripe integration                   │   │  │
│  │  │  • Email sending    • DNS verification                     │   │  │
│  │  └────────────────────────────────────────────────────────────┘   │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                      │                                   │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │                         Data Layer                                 │  │
│  │  ┌─────────────────────────┐  ┌─────────────────────────┐         │  │
│  │  │       PostgreSQL        │  │         Redis           │         │  │
│  │  │                         │  │                         │         │  │
│  │  │  • Organizations        │  │  • Sessions             │         │  │
│  │  │  • Projects             │  │  • Rate limits          │         │  │
│  │  │  • End users            │  │  • Cache                │         │  │
│  │  │  • Subscriptions        │  │                         │         │  │
│  │  │  • Email logs           │  │                         │         │  │
│  │  └─────────────────────────┘  └─────────────────────────┘         │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │                      External Services                             │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐                         │  │
│  │  │  Google  │  │  Stripe  │  │  Resend  │                         │  │
│  │  │  OAuth   │  │          │  │          │                         │  │
│  │  └──────────┘  └──────────┘  └──────────┘                         │  │
│  └────────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## Workspace Structure

```
reauth/
├── Cargo.toml                    # Workspace definition
├── Cargo.lock
│
├── crates/
│   ├── reauth-core/              # Shared logic, models, traits
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── models/           # Domain models
│   │   │   ├── auth/             # Auth logic (OAuth, magic link)
│   │   │   ├── billing/          # Stripe integration
│   │   │   ├── email/            # Resend client
│   │   │   ├── dns/              # DNS verification
│   │   │   └── error.rs          # Error types
│   │   └── Cargo.toml
│   │
│   └── reauth-api/               # Main HTTP server
│       ├── src/
│       │   ├── main.rs
│       │   ├── routes/           # Axum routes
│       │   │   ├── auth.rs       # /login, /callback, /verify
│       │   │   ├── hosted.rs     # Hosted UI pages
│       │   │   ├── api.rs        # SDK endpoints
│       │   │   ├── webhooks.rs   # Stripe webhooks
│       │   │   └── dashboard.rs  # Dashboard API
│       │   ├── middleware/       # Auth, rate limiting
│       │   ├── templates/        # HTML templates
│       │   └── config.rs
│       └── Cargo.toml
│
├── migrations/                   # SQL migrations (sqlx)
│   ├── 001_initial.sql
│   ├── 002_projects.sql
│   └── ...
│
├── sdk/
│   └── typescript/               # TypeScript SDK
│       ├── src/
│       │   ├── index.ts
│       │   ├── client.ts
│       │   └── middleware/
│       │       ├── next.ts
│       │       └── express.ts
│       ├── package.json
│       └── tsconfig.json
│
├── docs/                         # Documentation site
│
├── docker-compose.yml            # Local development
├── Dockerfile
└── .github/
    └── workflows/
        └── ci.yml
```

---

## Key Design Decisions

### 1. Monolith First

Single Rust binary for MVP. No microservices. Simpler deployment, easier debugging, faster iteration.

Workers and queues come in Phase 2 when needed (webhook delivery, analytics aggregation).

### 2. Multi-Tenancy Model

```
reauth.dev (us)
└── Organization (our customer - the indie dev)
    └── Project (their SaaS app)
        └── End User (their customer)
```

- Organization → billing relationship with us
- Project → one SaaS app, one domain
- End User → their customers

### 3. Session Strategy

Sessions stored in Redis with user data denormalized:

```json
{
  "user_id": "usr_abc123",
  "email": "user@example.com",
  "plan": "pro",
  "subscription_status": "active",
  "expires_at": "2024-12-31T00:00:00Z"
}
```

Benefits:
- `getUser()` is one Redis call, not Postgres
- Session includes everything the SDK needs
- Refresh async when subscription changes

### 4. Email via Customer's Resend

Customer provides their Resend API key. We just call the API.

Benefits:
- No email infrastructure to manage
- No deliverability concerns
- Customer already set up their domain in Resend
- Simpler onboarding

### 5. Direct Stripe Integration (No Abstraction Yet)

Phase 1 is Stripe-only. No payment provider abstraction.

The abstraction comes in Phase 2 when we actually add LemonSqueezy/Paddle. Premature abstraction adds complexity.

---

## Request Flow

### SDK `getUser()` Call

```
1. Request hits reauth API with session cookie
2. Look up session in Redis
3. If valid, return user data from session
4. If expired, return null
5. ~5ms total
```

### Login Flow (Google OAuth)

```
1. User visits auth.customer.com/login
2. Clicks "Continue with Google"
3. Redirect to Google OAuth
4. Google redirects to /callback/google with code
5. Exchange code for tokens
6. Get user info from Google
7. Find or create end_user in Postgres
8. Create session in Redis
9. Set cookie, redirect to app
```

### Login Flow (Magic Link)

```
1. User visits auth.customer.com/login
2. Enters email, clicks "Send magic link"
3. Generate token, store hash in Postgres
4. Send email via Resend
5. User clicks link
6. Verify token
7. Find or create end_user
8. Create session in Redis
9. Set cookie, redirect to app
```

### Stripe Webhook

```
1. Stripe sends event to /webhooks/stripe
2. Verify signature
3. Parse event type
4. Update subscription status in Postgres
5. Invalidate/update session in Redis
6. Return 200
```

---

## Scalability Notes

### Phase 1 (MVP)

Single server is fine. Expect:
- <1000 projects
- <100k end users total
- <10 req/sec average

### Phase 2 (Growth)

When needed:
- Multiple API server instances behind load balancer
- Redis cluster for sessions
- Read replicas for Postgres
- Background workers for async tasks

---

## Security Considerations

| Concern | Approach |
|---------|----------|
| Session tokens | Cryptographically random, hashed in storage |
| OAuth state | CSRF protection via state parameter |
| Magic link tokens | Single-use, 15-minute expiry, hashed |
| API keys (Phase 2) | Argon2 hashed, prefix for identification |
| Stripe secrets | Encrypted at rest |
| Resend keys | Encrypted at rest |
| HTTPS | Required everywhere, HSTS |
