-- Domain roles table for managing custom roles per domain
CREATE TABLE domain_roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(domain_id, name)
);

CREATE INDEX idx_domain_roles_domain_id ON domain_roles(domain_id);
