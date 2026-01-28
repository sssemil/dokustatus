ALTER TABLE user_subscriptions
DROP COLUMN IF EXISTS changes_this_period,
DROP COLUMN IF EXISTS period_changes_reset_at;
