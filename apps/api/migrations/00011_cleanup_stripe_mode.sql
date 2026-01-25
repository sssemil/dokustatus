-- ============================================================================
-- Cleanup stripe_mode - Consolidate on payment_mode
-- ============================================================================
-- This migration removes the legacy stripe_mode columns and consolidates
-- on the payment_mode system introduced in migration 00010.

-- ============================================================================
-- Backfill any NULLs before making NOT NULL
-- ============================================================================

UPDATE subscription_plans SET payment_mode = COALESCE(payment_mode, stripe_mode::text::payment_mode, 'test');
UPDATE user_subscriptions SET payment_mode = COALESCE(payment_mode, stripe_mode::text::payment_mode, 'test');
UPDATE billing_payments SET payment_mode = COALESCE(payment_mode, stripe_mode::text::payment_mode, 'test');
UPDATE domain_billing_stripe_config SET payment_mode = COALESCE(payment_mode, stripe_mode::text::payment_mode, 'test');
UPDATE domains SET active_payment_mode = COALESCE(active_payment_mode, billing_stripe_mode::text::payment_mode, 'test');

-- ============================================================================
-- Drop indexes referencing stripe_mode
-- ============================================================================

DROP INDEX IF EXISTS idx_subscription_plans_mode;
DROP INDEX IF EXISTS idx_user_subscriptions_mode;
DROP INDEX IF EXISTS idx_billing_payments_domain_mode;
DROP INDEX IF EXISTS idx_billing_payments_domain_mode_date;

-- ============================================================================
-- Drop stripe_mode columns
-- ============================================================================

ALTER TABLE domains DROP COLUMN IF EXISTS billing_stripe_mode;
ALTER TABLE domain_billing_stripe_config DROP COLUMN IF EXISTS stripe_mode;
ALTER TABLE subscription_plans DROP COLUMN IF EXISTS stripe_mode;
ALTER TABLE user_subscriptions DROP COLUMN IF EXISTS stripe_mode;
ALTER TABLE billing_payments DROP COLUMN IF EXISTS stripe_mode;

-- ============================================================================
-- Make payment_mode NOT NULL
-- ============================================================================

ALTER TABLE subscription_plans ALTER COLUMN payment_mode SET NOT NULL;
ALTER TABLE user_subscriptions ALTER COLUMN payment_mode SET NOT NULL;
ALTER TABLE billing_payments ALTER COLUMN payment_mode SET NOT NULL;
ALTER TABLE domain_billing_stripe_config ALTER COLUMN payment_mode SET NOT NULL;
-- Set default for new domains (test mode is safe default for new domains)
ALTER TABLE domains ALTER COLUMN active_payment_mode SET DEFAULT 'test';
ALTER TABLE domains ALTER COLUMN active_payment_mode SET NOT NULL;

-- ============================================================================
-- Add unique constraints (replacing stripe_mode constraints)
-- ============================================================================

ALTER TABLE domain_billing_stripe_config
  ADD CONSTRAINT domain_billing_stripe_config_domain_payment_mode_key UNIQUE (domain_id, payment_mode);
ALTER TABLE subscription_plans
  ADD CONSTRAINT subscription_plans_domain_payment_mode_code_key UNIQUE (domain_id, payment_mode, code);
ALTER TABLE user_subscriptions
  ADD CONSTRAINT user_subscriptions_domain_payment_mode_end_user_id_key UNIQUE (domain_id, payment_mode, end_user_id);
ALTER TABLE billing_payments
  ADD CONSTRAINT billing_payments_domain_payment_mode_stripe_invoice_id_key UNIQUE (domain_id, payment_mode, stripe_invoice_id);

-- ============================================================================
-- Create new indexes on payment_mode
-- ============================================================================

CREATE INDEX idx_subscription_plans_payment_mode ON subscription_plans(domain_id, payment_mode);
CREATE INDEX idx_user_subscriptions_payment_mode ON user_subscriptions(domain_id, payment_mode);
CREATE INDEX idx_billing_payments_payment_mode ON billing_payments(domain_id, payment_mode);
CREATE INDEX idx_billing_payments_payment_mode_date ON billing_payments(domain_id, payment_mode, payment_date DESC NULLS LAST);

-- ============================================================================
-- Drop the stripe_mode enum type
-- ============================================================================

DROP TYPE IF EXISTS stripe_mode;
