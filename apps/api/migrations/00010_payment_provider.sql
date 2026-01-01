-- ============================================================================
-- Payment Provider System - Generic Provider Support
-- ============================================================================
-- Introduces a generic payment provider architecture to support multiple
-- payment providers (Stripe, Dummy/Test, Coinbase) alongside each other.
-- This migration is ADDITIVE - new columns are added alongside existing
-- stripe_mode columns for safe rollback. Cleanup happens in migration 00011.

-- ============================================================================
-- Create payment_provider enum
-- ============================================================================

CREATE TYPE payment_provider AS ENUM ('stripe', 'dummy', 'coinbase');

-- ============================================================================
-- Create payment_mode enum (replaces stripe_mode concept)
-- ============================================================================

CREATE TYPE payment_mode AS ENUM ('test', 'live');

-- ============================================================================
-- Create billing_state enum for provider switching state machine
-- ============================================================================

CREATE TYPE billing_state AS ENUM ('active', 'pending_switch', 'switch_failed');

-- ============================================================================
-- Domain Enabled Payment Providers (like auth methods)
-- ============================================================================
-- Each domain can enable multiple provider+mode combinations.
-- For example: stripe+test, stripe+live, dummy+test

CREATE TABLE domain_enabled_payment_providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    provider payment_provider NOT NULL,
    mode payment_mode NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    display_order INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(domain_id, provider, mode)
);

CREATE INDEX idx_domain_enabled_providers_domain ON domain_enabled_payment_providers(domain_id);
CREATE INDEX idx_domain_enabled_providers_active ON domain_enabled_payment_providers(domain_id, is_active) WHERE is_active = true;

CREATE OR REPLACE FUNCTION set_domain_enabled_payment_providers_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_domain_enabled_payment_providers_set_updated_at
BEFORE UPDATE ON domain_enabled_payment_providers
FOR EACH ROW
EXECUTE FUNCTION set_domain_enabled_payment_providers_updated_at();

-- ============================================================================
-- Add payment_provider and payment_mode columns to domains
-- ============================================================================

ALTER TABLE domains
    ADD COLUMN active_payment_provider payment_provider,
    ADD COLUMN active_payment_mode payment_mode;

-- ============================================================================
-- Add payment_provider and payment_mode columns to domain_billing_stripe_config
-- ============================================================================

ALTER TABLE domain_billing_stripe_config
    ADD COLUMN payment_provider payment_provider,
    ADD COLUMN payment_mode payment_mode;

-- ============================================================================
-- Add payment_provider and payment_mode columns to subscription_plans
-- ============================================================================

ALTER TABLE subscription_plans
    ADD COLUMN payment_provider payment_provider,
    ADD COLUMN payment_mode payment_mode;

-- ============================================================================
-- Add payment_provider, payment_mode, and billing_state columns to user_subscriptions
-- ============================================================================

ALTER TABLE user_subscriptions
    ADD COLUMN payment_provider payment_provider,
    ADD COLUMN payment_mode payment_mode,
    ADD COLUMN billing_state billing_state NOT NULL DEFAULT 'active';

-- ============================================================================
-- Add payment_provider and payment_mode columns to billing_payments
-- ============================================================================

ALTER TABLE billing_payments
    ADD COLUMN payment_provider payment_provider,
    ADD COLUMN payment_mode payment_mode;

-- ============================================================================
-- Backfill data from existing stripe_mode columns
-- ============================================================================

-- Backfill domains
UPDATE domains SET
    active_payment_provider = 'stripe',
    active_payment_mode = billing_stripe_mode::text::payment_mode
WHERE billing_stripe_mode IS NOT NULL;

-- Backfill domain_billing_stripe_config
UPDATE domain_billing_stripe_config SET
    payment_provider = 'stripe',
    payment_mode = stripe_mode::text::payment_mode;

-- Backfill subscription_plans
UPDATE subscription_plans SET
    payment_provider = 'stripe',
    payment_mode = stripe_mode::text::payment_mode;

-- Backfill user_subscriptions
UPDATE user_subscriptions SET
    payment_provider = 'stripe',
    payment_mode = stripe_mode::text::payment_mode;

-- Backfill billing_payments
UPDATE billing_payments SET
    payment_provider = 'stripe',
    payment_mode = stripe_mode::text::payment_mode;

-- ============================================================================
-- Auto-enable providers for existing domains based on their billing config
-- ============================================================================

INSERT INTO domain_enabled_payment_providers (domain_id, provider, mode, is_active, display_order)
SELECT DISTINCT d.id, 'stripe'::payment_provider, d.active_payment_mode, true, 0
FROM domains d
WHERE d.active_payment_mode IS NOT NULL
ON CONFLICT (domain_id, provider, mode) DO NOTHING;

-- Also enable any additional Stripe configs that exist (e.g., if domain has both test and live)
INSERT INTO domain_enabled_payment_providers (domain_id, provider, mode, is_active, display_order)
SELECT DISTINCT dbsc.domain_id, 'stripe'::payment_provider, dbsc.payment_mode, true, 0
FROM domain_billing_stripe_config dbsc
WHERE dbsc.payment_mode IS NOT NULL
ON CONFLICT (domain_id, provider, mode) DO NOTHING;

-- ============================================================================
-- Add indexes for new columns (to be used after cleanup migration)
-- ============================================================================

-- Composite indexes for common queries
CREATE INDEX idx_subscription_plans_provider_mode ON subscription_plans(domain_id, payment_provider, payment_mode);
CREATE INDEX idx_user_subscriptions_provider_mode ON user_subscriptions(domain_id, payment_provider, payment_mode);
CREATE INDEX idx_billing_payments_provider_mode ON billing_payments(domain_id, payment_provider, payment_mode);
CREATE INDEX idx_user_subscriptions_billing_state ON user_subscriptions(billing_state) WHERE billing_state != 'active';
