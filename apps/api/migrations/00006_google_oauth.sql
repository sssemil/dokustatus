-- Google OAuth support: per-domain custom credentials and account linking

-- Per-domain Google OAuth config (optional, falls back to global)
CREATE TABLE domain_auth_google_oauth (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL UNIQUE REFERENCES domains(id) ON DELETE CASCADE,
    client_id TEXT NOT NULL,
    client_secret_encrypted TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Trigger to auto-update updated_at
CREATE OR REPLACE FUNCTION set_domain_auth_google_oauth_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_domain_auth_google_oauth_set_updated_at
BEFORE UPDATE ON domain_auth_google_oauth
FOR EACH ROW
EXECUTE FUNCTION set_domain_auth_google_oauth_updated_at();

-- Add google_id to domain_end_users for account linking
ALTER TABLE domain_end_users ADD COLUMN google_id TEXT NULL;

-- Unique index ensures one Google account can only link to one user per domain
CREATE UNIQUE INDEX idx_domain_end_users_google_id ON domain_end_users(domain_id, google_id)
  WHERE google_id IS NOT NULL;
