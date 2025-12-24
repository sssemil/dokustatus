-- Consolidated init migration for reauth
-- Creates all tables and seeds reauth.dev with admin user

---------------------------------------------------
-- TABLES
---------------------------------------------------

-- Domains registered with the platform
CREATE TABLE domains (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_end_user_id UUID NULL,  -- FK added after domain_end_users exists
    domain VARCHAR(255) UNIQUE NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending_dns',
    verification_started_at TIMESTAMP NULL,
    verified_at TIMESTAMP NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_domains_status ON domains(status);

-- Trigger to auto-update updated_at
CREATE OR REPLACE FUNCTION set_domains_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_domains_set_updated_at
BEFORE UPDATE ON domains
FOR EACH ROW
EXECUTE FUNCTION set_domains_updated_at();

-- End-users who login via custom domains
CREATE TABLE domain_end_users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    email VARCHAR(255) NOT NULL,
    email_verified_at TIMESTAMP NULL,
    last_login_at TIMESTAMP NULL,
    roles JSONB DEFAULT '[]'::jsonb,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(domain_id, email)
);

CREATE INDEX idx_domain_end_users_domain_id ON domain_end_users(domain_id);
CREATE INDEX idx_domain_end_users_email ON domain_end_users(email);

-- Now add the FK from domains to domain_end_users
ALTER TABLE domains
ADD CONSTRAINT fk_domains_owner_end_user
FOREIGN KEY (owner_end_user_id) REFERENCES domain_end_users(id) ON DELETE SET NULL;

CREATE INDEX idx_domains_owner_end_user_id ON domains(owner_end_user_id);

-- Domain authentication configuration
CREATE TABLE domain_auth_config (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL UNIQUE REFERENCES domains(id) ON DELETE CASCADE,
    magic_link_enabled BOOLEAN NOT NULL DEFAULT false,
    google_oauth_enabled BOOLEAN NOT NULL DEFAULT false,
    redirect_url VARCHAR(500) NULL,
    access_token_ttl_secs INTEGER DEFAULT 86400,
    refresh_token_ttl_days INTEGER DEFAULT 30,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Per-domain magic link config (optional, falls back to global)
CREATE TABLE domain_auth_magic_link (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL UNIQUE REFERENCES domains(id) ON DELETE CASCADE,
    resend_api_key_encrypted TEXT NOT NULL,
    from_email VARCHAR(255) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

---------------------------------------------------
-- SEED DATA: reauth.dev
---------------------------------------------------

-- 1. Insert reauth.dev domain (owner will be set after end_user is created)
INSERT INTO domains (id, owner_end_user_id, domain, status, verified_at, created_at, updated_at)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    NULL,
    'reauth.dev',
    'verified',
    NOW(),
    NOW(),
    NOW()
);

-- 2. Create admin end-user for reauth.dev
INSERT INTO domain_end_users (id, domain_id, email, email_verified_at, roles, created_at, updated_at)
VALUES (
    '00000000-0000-0000-0000-000000000003',
    '00000000-0000-0000-0000-000000000001',
    'emil@esnx.xyz',
    NOW(),
    '["admin"]'::jsonb,
    NOW(),
    NOW()
);

-- 3. Set the admin user as owner of reauth.dev
UPDATE domains
SET owner_end_user_id = '00000000-0000-0000-0000-000000000003'
WHERE id = '00000000-0000-0000-0000-000000000001';

-- 4. Configure auth for reauth.dev
INSERT INTO domain_auth_config (id, domain_id, magic_link_enabled, google_oauth_enabled, access_token_ttl_secs, refresh_token_ttl_days, redirect_url)
VALUES (
    '00000000-0000-0000-0000-000000000002',
    '00000000-0000-0000-0000-000000000001',
    true,
    false,
    86400,
    30,
    'https://reauth.dev/dashboard'
);
