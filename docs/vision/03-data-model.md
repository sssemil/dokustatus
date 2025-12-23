# Data Model

## Overview

Two-level multi-tenancy:

```
Organization (our customer - indie dev)
└── Project (their SaaS app)
    └── End User (their customer)
```

---

## Phase 1: MVP Schema

### Organizations (Our Customers)

```sql
CREATE TABLE organizations (
    id              TEXT PRIMARY KEY DEFAULT generate_id('org'),
    name            TEXT NOT NULL,
    slug            TEXT NOT NULL UNIQUE,
    owner_email     TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Our own auth for the dashboard
CREATE TABLE org_users (
    id              TEXT PRIMARY KEY DEFAULT generate_id('ou'),
    org_id          TEXT NOT NULL REFERENCES organizations(id),
    email           TEXT NOT NULL,
    name            TEXT,
    google_id       TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    UNIQUE(org_id, email)
);

CREATE TABLE org_sessions (
    id              TEXT PRIMARY KEY DEFAULT generate_id('os'),
    user_id         TEXT NOT NULL REFERENCES org_users(id),
    token_hash      TEXT NOT NULL UNIQUE,
    expires_at      TIMESTAMPTZ NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### Projects (Their SaaS Apps)

```sql
CREATE TABLE projects (
    id                      TEXT PRIMARY KEY DEFAULT generate_id('proj'),
    org_id                  TEXT NOT NULL REFERENCES organizations(id),
    name                    TEXT NOT NULL,
    slug                    TEXT NOT NULL,
    
    -- Domain config
    domain                  TEXT NOT NULL,           -- e.g., "example.com"
    auth_subdomain          TEXT NOT NULL DEFAULT 'auth',  -- e.g., "auth"
    domain_verified         BOOLEAN NOT NULL DEFAULT false,
    
    -- Google OAuth
    google_client_id        TEXT,
    google_client_secret    TEXT,                    -- encrypted
    
    -- Stripe
    stripe_account_id       TEXT,                    -- from Connect
    stripe_webhook_secret   TEXT,                    -- encrypted
    stripe_test_mode        BOOLEAN NOT NULL DEFAULT true,
    
    -- Resend
    resend_api_key          TEXT,                    -- encrypted
    email_from_address      TEXT,                    -- e.g., "hello@example.com"
    email_from_name         TEXT,                    -- e.g., "Example App"
    
    -- Branding
    app_name                TEXT,
    logo_url                TEXT,
    primary_color           TEXT DEFAULT '#000000',
    
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    UNIQUE(org_id, slug)
);

CREATE INDEX idx_projects_domain ON projects(domain);
```

### Plans

```sql
CREATE TABLE plans (
    id                  TEXT PRIMARY KEY DEFAULT generate_id('plan'),
    project_id          TEXT NOT NULL REFERENCES projects(id),
    name                TEXT NOT NULL,              -- "Free", "Pro"
    slug                TEXT NOT NULL,              -- "free", "pro"
    stripe_price_id     TEXT,                       -- Stripe Price ID
    features            JSONB DEFAULT '{}',         -- { "maxProjects": 10 }
    display_order       INT NOT NULL DEFAULT 0,
    is_active           BOOLEAN NOT NULL DEFAULT true,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    UNIQUE(project_id, slug)
);
```

### End Users (Their Customers)

```sql
CREATE TABLE end_users (
    id                      TEXT PRIMARY KEY DEFAULT generate_id('usr'),
    project_id              TEXT NOT NULL REFERENCES projects(id),
    
    -- Identity
    email                   TEXT NOT NULL,
    email_verified          BOOLEAN NOT NULL DEFAULT false,
    name                    TEXT,
    avatar_url              TEXT,
    google_id               TEXT,                   -- from Google OAuth
    
    -- Billing (denormalized for fast access)
    stripe_customer_id      TEXT,
    plan_id                 TEXT REFERENCES plans(id),
    subscription_status     TEXT,                   -- active, past_due, cancelled, trialing, null
    subscription_id         TEXT,                   -- Stripe Subscription ID
    subscription_ends_at    TIMESTAMPTZ,            -- when current period ends (or cancellation date)
    
    -- Timestamps
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at            TIMESTAMPTZ,
    deleted_at              TIMESTAMPTZ,            -- soft delete
    
    UNIQUE(project_id, email)
);

CREATE INDEX idx_end_users_google ON end_users(project_id, google_id);
CREATE INDEX idx_end_users_stripe ON end_users(stripe_customer_id);
```

### Sessions

```sql
CREATE TABLE sessions (
    id              TEXT PRIMARY KEY DEFAULT generate_id('sess'),
    user_id         TEXT NOT NULL REFERENCES end_users(id),
    token_hash      TEXT NOT NULL UNIQUE,
    
    ip_address      INET,
    user_agent      TEXT,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at      TIMESTAMPTZ NOT NULL,
    revoked_at      TIMESTAMPTZ,
    
    -- Index for cleanup job
    CONSTRAINT sessions_not_expired CHECK (revoked_at IS NULL OR revoked_at <= now())
);

CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);
```

### Magic Links

```sql
CREATE TABLE magic_links (
    id              TEXT PRIMARY KEY DEFAULT generate_id('ml'),
    project_id      TEXT NOT NULL REFERENCES projects(id),
    email           TEXT NOT NULL,
    token_hash      TEXT NOT NULL UNIQUE,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at      TIMESTAMPTZ NOT NULL,
    used_at         TIMESTAMPTZ
);

CREATE INDEX idx_magic_links_expires ON magic_links(expires_at);
```

### Email Sends (Minimal Logging)

```sql
CREATE TABLE email_sends (
    id              TEXT PRIMARY KEY DEFAULT generate_id('em'),
    project_id      TEXT NOT NULL REFERENCES projects(id),
    user_id         TEXT REFERENCES end_users(id),
    
    to_address      TEXT NOT NULL,
    subject         TEXT NOT NULL,
    template        TEXT NOT NULL,              -- 'magic_link'
    
    resend_id       TEXT,                       -- from Resend API response
    status          TEXT NOT NULL DEFAULT 'sent',  -- sent, failed
    error_message   TEXT,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_email_sends_project ON email_sends(project_id, created_at DESC);
```

---

## ID Generation

Prefixed IDs for easy identification:

```sql
CREATE OR REPLACE FUNCTION generate_id(prefix TEXT)
RETURNS TEXT AS $$
BEGIN
    RETURN prefix || '_' || encode(gen_random_bytes(12), 'hex');
END;
$$ LANGUAGE plpgsql;
```

Examples:
- `org_a1b2c3d4e5f6...`
- `proj_a1b2c3d4e5f6...`
- `usr_a1b2c3d4e5f6...`
- `sess_a1b2c3d4e5f6...`

---

## Redis Schema

### Sessions (Fast Lookup)

```
Key:    session:{token_hash}
Value:  JSON blob
TTL:    matches session expiry

{
    "session_id": "sess_abc123",
    "user_id": "usr_xyz789",
    "project_id": "proj_def456",
    "email": "user@example.com",
    "name": "Jane Doe",
    "avatar_url": "https://...",
    "plan": "pro",
    "plan_features": { "maxProjects": 10 },
    "subscription_status": "active",
    "subscription_ends_at": "2025-01-15T00:00:00Z",
    "created_at": "2024-12-01T...",
    "expires_at": "2024-12-31T..."
}
```

### Rate Limiting

```
Key:    rate:{project_id}:{ip_address}
Value:  request count
TTL:    window duration (e.g., 60s)
```

### Magic Link Rate Limiting

```
Key:    magic_link_rate:{project_id}:{email}
Value:  count of links sent
TTL:    1 hour
```

---

## Phase 2 Additions

These tables are added in Phase 2 (not in MVP):

### Teams & Organizations

```sql
-- Teams (their users' organizations)
CREATE TABLE teams (
    id              TEXT PRIMARY KEY DEFAULT generate_id('team'),
    project_id      TEXT NOT NULL REFERENCES projects(id),
    name            TEXT NOT NULL,
    slug            TEXT NOT NULL,
    owner_user_id   TEXT NOT NULL REFERENCES end_users(id),
    billing_email   TEXT,
    
    -- Billing at team level
    stripe_customer_id      TEXT,
    plan_id                 TEXT REFERENCES plans(id),
    subscription_status     TEXT,
    subscription_id         TEXT,
    subscription_ends_at    TIMESTAMPTZ,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    UNIQUE(project_id, slug)
);

CREATE TABLE team_members (
    team_id         TEXT NOT NULL REFERENCES teams(id),
    user_id         TEXT NOT NULL REFERENCES end_users(id),
    role            TEXT NOT NULL DEFAULT 'member',  -- owner, admin, member
    invited_by      TEXT REFERENCES end_users(id),
    joined_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    PRIMARY KEY (team_id, user_id)
);

CREATE TABLE team_invitations (
    id              TEXT PRIMARY KEY DEFAULT generate_id('inv'),
    team_id         TEXT NOT NULL REFERENCES teams(id),
    email           TEXT NOT NULL,
    role            TEXT NOT NULL DEFAULT 'member',
    token_hash      TEXT NOT NULL UNIQUE,
    invited_by      TEXT NOT NULL REFERENCES end_users(id),
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at      TIMESTAMPTZ NOT NULL,
    accepted_at     TIMESTAMPTZ
);
```

### API Keys

```sql
CREATE TABLE api_keys (
    id                  TEXT PRIMARY KEY DEFAULT generate_id('key'),
    project_id          TEXT NOT NULL REFERENCES projects(id),
    user_id             TEXT REFERENCES end_users(id),
    team_id             TEXT REFERENCES teams(id),
    
    key_prefix          TEXT NOT NULL,              -- "sk_live_abc123"
    key_hash            TEXT NOT NULL,              -- Argon2 hash
    name                TEXT NOT NULL,
    permissions         JSONB DEFAULT '["read"]',
    
    rate_limit          INT,                        -- requests per minute
    last_used_at        TIMESTAMPTZ,
    expires_at          TIMESTAMPTZ,
    
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at          TIMESTAMPTZ,
    
    CONSTRAINT api_keys_owner CHECK (user_id IS NOT NULL OR team_id IS NOT NULL)
);

CREATE INDEX idx_api_keys_prefix ON api_keys(key_prefix);
```

### Credits

```sql
CREATE TABLE credits (
    id              TEXT PRIMARY KEY DEFAULT generate_id('cr'),
    project_id      TEXT NOT NULL REFERENCES projects(id),
    user_id         TEXT REFERENCES end_users(id),
    team_id         TEXT REFERENCES teams(id),
    
    balance         BIGINT NOT NULL DEFAULT 0,
    lifetime_added  BIGINT NOT NULL DEFAULT 0,
    lifetime_used   BIGINT NOT NULL DEFAULT 0,
    
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    CONSTRAINT credits_owner CHECK (user_id IS NOT NULL OR team_id IS NOT NULL),
    UNIQUE(project_id, user_id),
    UNIQUE(project_id, team_id)
);

CREATE TABLE credit_transactions (
    id              TEXT PRIMARY KEY DEFAULT generate_id('ctx'),
    credits_id      TEXT NOT NULL REFERENCES credits(id),
    
    amount          BIGINT NOT NULL,                -- positive or negative
    reason          TEXT NOT NULL,                  -- topup, usage, refund, bonus
    description     TEXT,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### Webhooks

```sql
CREATE TABLE webhook_endpoints (
    id              TEXT PRIMARY KEY DEFAULT generate_id('wh'),
    project_id      TEXT NOT NULL REFERENCES projects(id),
    
    url             TEXT NOT NULL,
    secret          TEXT NOT NULL,                  -- for signature verification
    events          JSONB NOT NULL,                 -- ["user.created", "subscription.updated"]
    
    is_active       BOOLEAN NOT NULL DEFAULT true,
    failure_count   INT NOT NULL DEFAULT 0,
    last_failure_at TIMESTAMPTZ,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE webhook_deliveries (
    id              TEXT PRIMARY KEY DEFAULT generate_id('whd'),
    endpoint_id     TEXT NOT NULL REFERENCES webhook_endpoints(id),
    
    event_type      TEXT NOT NULL,
    payload         JSONB NOT NULL,
    
    status          TEXT NOT NULL DEFAULT 'pending',  -- pending, delivered, failed
    attempts        INT NOT NULL DEFAULT 0,
    next_retry_at   TIMESTAMPTZ,
    
    response_status INT,
    response_body   TEXT,
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at    TIMESTAMPTZ
);
```

### Feature Flags

```sql
CREATE TABLE feature_flags (
    id              TEXT PRIMARY KEY DEFAULT generate_id('flag'),
    project_id      TEXT NOT NULL REFERENCES projects(id),
    
    key             TEXT NOT NULL,
    name            TEXT NOT NULL,
    description     TEXT,
    
    enabled         BOOLEAN NOT NULL DEFAULT false,
    rollout_pct     INT DEFAULT 0,                  -- 0-100
    targeting       JSONB DEFAULT '{}',             -- rules
    
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    UNIQUE(project_id, key)
);

CREATE TABLE user_flags (
    user_id         TEXT NOT NULL REFERENCES end_users(id),
    flag_id         TEXT NOT NULL REFERENCES feature_flags(id),
    enabled         BOOLEAN NOT NULL,
    
    set_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    PRIMARY KEY (user_id, flag_id)
);
```

### Analytics (Partitioned)

```sql
CREATE TABLE pageviews (
    id              TEXT NOT NULL DEFAULT generate_id('pv'),
    project_id      TEXT NOT NULL,
    
    timestamp       TIMESTAMPTZ NOT NULL DEFAULT now(),
    session_id      TEXT NOT NULL,                  -- anonymous session
    user_id         TEXT,                           -- if logged in
    
    path            TEXT NOT NULL,
    referrer        TEXT,
    
    country         TEXT,
    device_type     TEXT,                           -- desktop, mobile, tablet
    browser         TEXT,
    
    PRIMARY KEY (id, timestamp)
) PARTITION BY RANGE (timestamp);

-- Create monthly partitions
CREATE TABLE pageviews_2024_12 PARTITION OF pageviews
    FOR VALUES FROM ('2024-12-01') TO ('2025-01-01');
```

---

## Encryption

Sensitive fields encrypted at rest:

```rust
// Fields that are encrypted:
// - projects.google_client_secret
// - projects.stripe_webhook_secret
// - projects.resend_api_key

pub fn encrypt(plaintext: &str, key: &[u8]) -> String {
    // AES-256-GCM
}

pub fn decrypt(ciphertext: &str, key: &[u8]) -> String {
    // AES-256-GCM
}
```

Encryption key stored in environment variable, rotated periodically.
