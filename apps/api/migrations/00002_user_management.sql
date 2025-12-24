-- Add user management fields to domain_end_users
ALTER TABLE domain_end_users ADD COLUMN is_frozen BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE domain_end_users ADD COLUMN is_whitelisted BOOLEAN NOT NULL DEFAULT false;

-- Add whitelist mode setting to domain_auth_config
ALTER TABLE domain_auth_config ADD COLUMN whitelist_enabled BOOLEAN NOT NULL DEFAULT false;
