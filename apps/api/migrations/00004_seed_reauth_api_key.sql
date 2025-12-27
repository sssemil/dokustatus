-- Seed API key for reauth.dev domain
-- This enables server-to-server authentication for reauth.dev's own services
-- The raw key is stored in infra/secrets/reauth_dev_api_key

INSERT INTO domain_api_keys (
    id,
    domain_id,
    key_prefix,
    key_hash,
    name,
    created_at,
    created_by_end_user_id
) VALUES (
    '00000000-0000-0000-0000-000000000004',
    '00000000-0000-0000-0000-000000000001',  -- reauth.dev domain
    'sk_live_hUdeBwdr',                       -- First 16 chars of key
    'd62bee72911af5aaa94c3fc1644ffa350e30575147d710f588931b8897b89aa3',  -- SHA-256 hash
    'Platform API Key',
    NOW(),
    '00000000-0000-0000-0000-000000000003'   -- Admin user (emil@esnx.xyz)
);
