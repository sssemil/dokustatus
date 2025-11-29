-- Add encrypted storage for document numbers and hashed index for uniqueness.
ALTER TABLE document_tracks
  ADD COLUMN IF NOT EXISTS number_ciphertext TEXT NOT NULL,
  ADD COLUMN IF NOT EXISTS number_hash TEXT NOT NULL;

-- Replace unique index to use hashed value only (no plaintext).
DROP INDEX IF EXISTS document_tracks_user_number_idx;
CREATE UNIQUE INDEX IF NOT EXISTS document_tracks_user_hash_idx
  ON document_tracks(user_id, number_hash);
