-- Tracks document (passport/ID) status per user.
CREATE TABLE IF NOT EXISTS document_tracks (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    number TEXT NOT NULL,
    last_status TEXT,
    last_pickup TEXT,
    last_checked_at TIMESTAMP,
    status_changed_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS document_tracks_user_number_idx
    ON document_tracks(user_id, number);
