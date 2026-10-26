-- Enable UUID extension if not already enabled
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- 1. Songs table (Option A: Array-based table-efficient schema)
CREATE TABLE IF NOT EXISTS songs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mbid TEXT UNIQUE NOT NULL,                  -- MusicBrainz Recording ID
    title TEXT NOT NULL,                        -- Song title
    release_year INT,                           -- Earliest release year
    duration_ms INT,                            -- Track length in milliseconds
    artists TEXT[] NOT NULL DEFAULT '{}',       -- Main credited artist(s)
    albums TEXT[] NOT NULL DEFAULT '{}',        -- Album(s) / release(s)
    credits TEXT[] NOT NULL DEFAULT '{}',       -- Producers, songwriters, composers
    locations TEXT[] NOT NULL DEFAULT '{}',     -- Recording studios and cities
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Fast GIN indexes for array intersection queries
CREATE INDEX IF NOT EXISTS idx_songs_title ON songs(title);
CREATE INDEX IF NOT EXISTS idx_songs_year ON songs(release_year);
CREATE INDEX IF NOT EXISTS idx_songs_artists ON songs USING GIN (artists);
CREATE INDEX IF NOT EXISTS idx_songs_albums ON songs USING GIN (albums);
CREATE INDEX IF NOT EXISTS idx_songs_credits ON songs USING GIN (credits);
CREATE INDEX IF NOT EXISTS idx_songs_locations ON songs USING GIN (locations);

-- 2. Users table (for authentication)
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username TEXT UNIQUE NOT NULL,
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    games_played INT NOT NULL DEFAULT 0,
    games_won INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 3. Matches table (PvP match sessions)
CREATE TABLE IF NOT EXISTS matches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    player1_id UUID NOT NULL REFERENCES users(id),
    player2_id UUID REFERENCES users(id),
    winner_id UUID REFERENCES users(id),
    status TEXT NOT NULL DEFAULT 'waiting',     -- 'waiting', 'active', 'finished', 'abandoned'
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ
);

-- 4. Match moves table (Turn history)
CREATE TABLE IF NOT EXISTS match_moves (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    match_id UUID NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
    player_id UUID NOT NULL REFERENCES users(id),
    turn_number INT NOT NULL,
    song_id UUID NOT NULL REFERENCES songs(id),
    link_type TEXT,                             -- 'artist', 'album', 'year', 'credit', 'location' (NULL on turn 1)
    link_value TEXT,                            -- e.g. "Queen", "1975", "Abbey Road Studios"
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (match_id, turn_number)
);

CREATE INDEX IF NOT EXISTS idx_match_moves_match ON match_moves(match_id);
