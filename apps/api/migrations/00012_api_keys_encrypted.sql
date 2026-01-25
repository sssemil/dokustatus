-- WARNING: This migration DELETES ALL EXISTING API KEYS
-- All environments must re-issue API keys after this migration
-- Development phase only - no real users affected

-- Delete all existing API keys (they don't have encrypted values)
DELETE FROM domain_api_keys;

-- Add required encrypted column for HKDF-based JWT derivation
-- This stores the raw API key encrypted with ProcessCipher (AES-256-GCM)
ALTER TABLE domain_api_keys
ADD COLUMN key_encrypted TEXT NOT NULL;
