-- Developer API keys for server-to-server authentication
CREATE TABLE domain_api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,

    -- Key identification (first 8 chars stored unhashed for UI display)
    key_prefix VARCHAR(16) NOT NULL,

    -- Hashed key (SHA-256, hex encoded - 64 chars)
    key_hash VARCHAR(64) NOT NULL UNIQUE,

    -- Metadata
    name VARCHAR(100) NOT NULL DEFAULT 'Default',

    -- Lifecycle
    last_used_at TIMESTAMP NULL,
    revoked_at TIMESTAMP NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    -- Track who created it (for audit)
    created_by_end_user_id UUID NOT NULL REFERENCES domain_end_users(id) ON DELETE SET NULL
);

CREATE INDEX idx_domain_api_keys_domain_id ON domain_api_keys(domain_id);
CREATE INDEX idx_domain_api_keys_key_hash ON domain_api_keys(key_hash);
