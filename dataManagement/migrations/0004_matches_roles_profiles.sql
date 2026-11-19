-- Migration 0004_matches_roles_profiles.sql
-- Supports 2-player matches, turn tracking, versions, moderator roles, and user profiles

-- 1. Users: add bio
ALTER TABLE users ADD COLUMN IF NOT EXISTS bio TEXT NULL;

-- 2. Games: update status check to allow 'waiting', add match/turn columns
ALTER TABLE games DROP CONSTRAINT IF EXISTS games_status_check;
ALTER TABLE games ADD CONSTRAINT games_status_check CHECK (status IN ('waiting', 'active', 'ended'));

ALTER TABLE games ADD COLUMN IF NOT EXISTS invite_code TEXT UNIQUE NULL;
ALTER TABLE games ADD COLUMN IF NOT EXISTS turn_user_id UUID NULL REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE games ADD COLUMN IF NOT EXISTS turn_number INT NOT NULL DEFAULT 0;
ALTER TABLE games ADD COLUMN IF NOT EXISTS turn_deadline TIMESTAMPTZ NULL;
ALTER TABLE games ADD COLUMN IF NOT EXISTS version INT NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS games_invite_code_idx ON games (invite_code);
CREATE INDEX IF NOT EXISTS games_turn_user_id_idx ON games (turn_user_id);

-- 3. Reports: add decision (upheld or dismissed, null while open)
ALTER TABLE reports ADD COLUMN IF NOT EXISTS decision TEXT NULL CHECK (decision IN ('upheld', 'dismissed'));

-- 4. Match Queue: for matchmaking queue (PBI 24)
CREATE TABLE IF NOT EXISTS match_queue (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
