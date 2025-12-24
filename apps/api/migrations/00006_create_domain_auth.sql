-- Domain authentication configuration (general settings)
CREATE TABLE domain_auth_config (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL UNIQUE REFERENCES domains(id) ON DELETE CASCADE,
    magic_link_enabled BOOLEAN NOT NULL DEFAULT false,
    google_oauth_enabled BOOLEAN NOT NULL DEFAULT false,
    redirect_url VARCHAR(500) NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Magic link specific configuration (separate table for method-specific settings)
CREATE TABLE domain_auth_magic_link (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL UNIQUE REFERENCES domains(id) ON DELETE CASCADE,
    resend_api_key_encrypted TEXT NOT NULL,
    from_email VARCHAR(255) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- End-users who login via custom domains (separate from workspace users)
CREATE TABLE domain_end_users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    email VARCHAR(255) NOT NULL,
    email_verified_at TIMESTAMP NULL,
    last_login_at TIMESTAMP NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(domain_id, email)
);

CREATE INDEX idx_domain_end_users_domain_id ON domain_end_users(domain_id);
CREATE INDEX idx_domain_end_users_email ON domain_end_users(email);
