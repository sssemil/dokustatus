-- Unify auth: Make reauth.dev use domain end-user auth
-- This migration drops the old users/magic_links tables and enhances domain_end_users

-- First, drop the foreign key constraint on domains.user_id
ALTER TABLE domains DROP CONSTRAINT IF EXISTS domains_user_id_fkey;

-- Drop old auth tables
DROP TABLE IF EXISTS magic_links CASCADE;
DROP TABLE IF EXISTS waitlist CASCADE;
DROP TABLE IF EXISTS users CASCADE;

-- Add roles to domain_end_users (JSONB array for flexibility)
ALTER TABLE domain_end_users ADD COLUMN IF NOT EXISTS roles JSONB DEFAULT '[]'::jsonb;

-- Add TTL config to domain_auth_config
ALTER TABLE domain_auth_config
  ADD COLUMN IF NOT EXISTS access_token_ttl_secs INTEGER DEFAULT 86400,
  ADD COLUMN IF NOT EXISTS refresh_token_ttl_days INTEGER DEFAULT 30;

-- Change domains.user_id to owner_end_user_id (nullable for system domains)
ALTER TABLE domains DROP COLUMN IF EXISTS user_id;
ALTER TABLE domains ADD COLUMN IF NOT EXISTS owner_end_user_id UUID REFERENCES domain_end_users(id) ON DELETE SET NULL;

-- Create index for owner lookup
CREATE INDEX IF NOT EXISTS idx_domains_owner_end_user_id ON domains(owner_end_user_id);

-- Seed reauth.dev as a verified system domain (NULL owner = system domain)
INSERT INTO domains (id, owner_end_user_id, domain, status, verified_at, created_at, updated_at)
VALUES (
  '00000000-0000-0000-0000-000000000001',
  NULL,
  'reauth.dev',
  'verified',
  NOW(),
  NOW(),
  NOW()
) ON CONFLICT (domain) DO UPDATE SET
  status = 'verified',
  verified_at = NOW(),
  updated_at = NOW();

-- Configure reauth.dev auth (uses global Resend config via env vars)
INSERT INTO domain_auth_config (id, domain_id, magic_link_enabled, google_oauth_enabled, access_token_ttl_secs, refresh_token_ttl_days, redirect_url)
VALUES (
  '00000000-0000-0000-0000-000000000002',
  '00000000-0000-0000-0000-000000000001',
  true,
  false,
  86400,
  30,
  'https://reauth.dev/dashboard'
) ON CONFLICT (domain_id) DO UPDATE SET
  magic_link_enabled = true,
  access_token_ttl_secs = 86400,
  refresh_token_ttl_days = 30,
  redirect_url = 'https://reauth.dev/dashboard',
  updated_at = NOW();

-- Create admin user for reauth.dev (emil@esnx.xyz with admin role)
INSERT INTO domain_end_users (id, domain_id, email, email_verified_at, roles, created_at, updated_at)
VALUES (
  '00000000-0000-0000-0000-000000000003',
  '00000000-0000-0000-0000-000000000001',
  'emil@esnx.xyz',
  NOW(),
  '["admin"]'::jsonb,
  NOW(),
  NOW()
) ON CONFLICT (domain_id, email) DO UPDATE SET
  roles = '["admin"]'::jsonb,
  updated_at = NOW();
