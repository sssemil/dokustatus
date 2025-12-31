-- Stripe Test/Live Mode Support
-- Allows domains to have both test and live Stripe configurations,
-- with plans and subscriptions scoped to their respective modes.

-- ============================================================================
-- Create stripe_mode enum
-- ============================================================================

CREATE TYPE stripe_mode AS ENUM ('test', 'live');

-- ============================================================================
-- Add mode to domain_billing_stripe_config
-- ============================================================================

-- Add stripe_mode column (default to 'test' for existing rows)
ALTER TABLE domain_billing_stripe_config
  ADD COLUMN stripe_mode stripe_mode NOT NULL DEFAULT 'test';

-- Drop old unique constraint on domain_id alone
ALTER TABLE domain_billing_stripe_config
  DROP CONSTRAINT domain_billing_stripe_config_domain_id_key;

-- Add new unique constraint on (domain_id, stripe_mode)
-- This allows each domain to have both a test and live config
ALTER TABLE domain_billing_stripe_config
  ADD CONSTRAINT domain_billing_stripe_config_domain_mode_unique
  UNIQUE (domain_id, stripe_mode);

-- ============================================================================
-- Add active mode setting to domains
-- ============================================================================

ALTER TABLE domains
  ADD COLUMN billing_stripe_mode stripe_mode NOT NULL DEFAULT 'test';

-- ============================================================================
-- Add mode to subscription_plans
-- ============================================================================

ALTER TABLE subscription_plans
  ADD COLUMN stripe_mode stripe_mode NOT NULL DEFAULT 'test';

-- Drop old unique constraint on (domain_id, code)
ALTER TABLE subscription_plans
  DROP CONSTRAINT subscription_plans_domain_id_code_key;

-- Add new unique constraint scoped by mode
-- This allows the same plan code in both test and live modes
ALTER TABLE subscription_plans
  ADD CONSTRAINT subscription_plans_domain_mode_code_unique
  UNIQUE (domain_id, stripe_mode, code);

-- Index for efficient mode filtering
CREATE INDEX idx_subscription_plans_mode ON subscription_plans(domain_id, stripe_mode);

-- ============================================================================
-- Add mode to user_subscriptions
-- ============================================================================

ALTER TABLE user_subscriptions
  ADD COLUMN stripe_mode stripe_mode NOT NULL DEFAULT 'test';

-- Drop old unique constraint on (domain_id, end_user_id)
ALTER TABLE user_subscriptions
  DROP CONSTRAINT user_subscriptions_domain_id_end_user_id_key;

-- Add new unique constraint scoped by mode
-- This allows a user to have both test and live subscriptions
ALTER TABLE user_subscriptions
  ADD CONSTRAINT user_subscriptions_domain_mode_user_unique
  UNIQUE (domain_id, stripe_mode, end_user_id);

-- Index for efficient mode filtering
CREATE INDEX idx_user_subscriptions_mode ON user_subscriptions(domain_id, stripe_mode);
