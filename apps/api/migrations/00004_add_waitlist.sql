-- Add waitlist flag to users (default true - everyone starts on waitlist)
ALTER TABLE users ADD COLUMN on_waitlist BOOLEAN NOT NULL DEFAULT true;
