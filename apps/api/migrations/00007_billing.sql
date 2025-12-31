-- Billing support: Stripe integration, subscription plans, user subscriptions

-- ============================================================================
-- Domain Stripe Configuration (follows domain_auth_google_oauth pattern)
-- ============================================================================

CREATE TABLE domain_billing_stripe_config (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL UNIQUE REFERENCES domains(id) ON DELETE CASCADE,
    stripe_secret_key_encrypted TEXT NOT NULL,
    stripe_publishable_key TEXT NOT NULL,
    stripe_webhook_secret_encrypted TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE OR REPLACE FUNCTION set_domain_billing_stripe_config_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_domain_billing_stripe_config_set_updated_at
BEFORE UPDATE ON domain_billing_stripe_config
FOR EACH ROW
EXECUTE FUNCTION set_domain_billing_stripe_config_updated_at();

-- ============================================================================
-- Subscription Plans (per domain)
-- ============================================================================

CREATE TABLE subscription_plans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,

    -- Plan identification
    code VARCHAR(50) NOT NULL,
    name VARCHAR(100) NOT NULL,
    description TEXT,

    -- Pricing
    price_cents INTEGER NOT NULL,
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',

    -- Billing interval
    interval VARCHAR(20) NOT NULL,
    interval_count INTEGER NOT NULL DEFAULT 1,

    -- Trial
    trial_days INTEGER NOT NULL DEFAULT 0,

    -- Features metadata (JSON array for feature list)
    features JSONB DEFAULT '[]'::jsonb,

    -- Display settings
    is_public BOOLEAN NOT NULL DEFAULT true,
    display_order INTEGER NOT NULL DEFAULT 0,

    -- Stripe integration (populated after creating in Stripe)
    stripe_product_id TEXT,
    stripe_price_id TEXT,

    -- Lifecycle
    is_archived BOOLEAN NOT NULL DEFAULT false,
    archived_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    UNIQUE(domain_id, code)
);

CREATE INDEX idx_subscription_plans_domain_id ON subscription_plans(domain_id);
CREATE INDEX idx_subscription_plans_stripe_price_id ON subscription_plans(stripe_price_id) WHERE stripe_price_id IS NOT NULL;

CREATE OR REPLACE FUNCTION set_subscription_plans_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_subscription_plans_set_updated_at
BEFORE UPDATE ON subscription_plans
FOR EACH ROW
EXECUTE FUNCTION set_subscription_plans_updated_at();

-- ============================================================================
-- User Subscriptions
-- ============================================================================

CREATE TYPE subscription_status AS ENUM (
    'active',
    'past_due',
    'canceled',
    'trialing',
    'incomplete',
    'incomplete_expired',
    'unpaid',
    'paused'
);

CREATE TABLE user_subscriptions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    end_user_id UUID NOT NULL REFERENCES domain_end_users(id) ON DELETE CASCADE,
    plan_id UUID NOT NULL REFERENCES subscription_plans(id) ON DELETE RESTRICT,

    -- Status
    status subscription_status NOT NULL DEFAULT 'incomplete',

    -- Stripe references
    stripe_customer_id TEXT NOT NULL,
    stripe_subscription_id TEXT,

    -- Period info (from Stripe webhook)
    current_period_start TIMESTAMP,
    current_period_end TIMESTAMP,
    trial_start TIMESTAMP,
    trial_end TIMESTAMP,

    -- Cancellation
    cancel_at_period_end BOOLEAN NOT NULL DEFAULT false,
    canceled_at TIMESTAMP,

    -- Manual override (by domain owner)
    manually_granted BOOLEAN NOT NULL DEFAULT false,
    granted_by UUID REFERENCES domain_end_users(id),
    granted_at TIMESTAMP,

    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    -- One active subscription per user per domain
    UNIQUE(domain_id, end_user_id)
);

CREATE INDEX idx_user_subscriptions_end_user_id ON user_subscriptions(end_user_id);
-- Unique constraint on stripe_subscription_id to prevent duplicate subscriptions and ensure webhook updates target the correct row
CREATE UNIQUE INDEX idx_user_subscriptions_stripe_subscription_id ON user_subscriptions(stripe_subscription_id) WHERE stripe_subscription_id IS NOT NULL;
CREATE INDEX idx_user_subscriptions_stripe_customer_id ON user_subscriptions(stripe_customer_id);
CREATE INDEX idx_user_subscriptions_status ON user_subscriptions(status);
CREATE INDEX idx_user_subscriptions_domain_id ON user_subscriptions(domain_id);

CREATE OR REPLACE FUNCTION set_user_subscriptions_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_user_subscriptions_set_updated_at
BEFORE UPDATE ON user_subscriptions
FOR EACH ROW
EXECUTE FUNCTION set_user_subscriptions_updated_at();

-- ============================================================================
-- Subscription Events (audit log with idempotency)
-- ============================================================================

CREATE TABLE subscription_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subscription_id UUID NOT NULL REFERENCES user_subscriptions(id) ON DELETE CASCADE,
    event_type VARCHAR(50) NOT NULL,
    previous_status subscription_status,
    new_status subscription_status,
    stripe_event_id TEXT,
    metadata JSONB DEFAULT '{}'::jsonb,
    created_by UUID REFERENCES domain_end_users(id),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Unique constraint for Stripe webhook idempotency
CREATE UNIQUE INDEX idx_subscription_events_stripe_event_id ON subscription_events(stripe_event_id) WHERE stripe_event_id IS NOT NULL;
CREATE INDEX idx_subscription_events_subscription_id ON subscription_events(subscription_id);
