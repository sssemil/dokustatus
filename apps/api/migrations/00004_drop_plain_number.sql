-- Remove legacy plaintext column now that ciphertext + hash are authoritative.
ALTER TABLE document_tracks
  DROP COLUMN IF EXISTS number;
