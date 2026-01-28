-- Add rate limiting for subscription plan changes
-- Users are limited to 5 plan changes per billing period to prevent abuse

ALTER TABLE user_subscriptions
ADD COLUMN changes_this_period INTEGER NOT NULL DEFAULT 0,
ADD COLUMN period_changes_reset_at TIMESTAMPTZ;

COMMENT ON COLUMN user_subscriptions.changes_this_period IS 'Number of plan changes made in the current billing period (max 5)';
COMMENT ON COLUMN user_subscriptions.period_changes_reset_at IS 'When the changes counter will reset (matches current_period_end)';
