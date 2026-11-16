CREATE TABLE games (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mode TEXT NOT NULL CHECK (mode IN ('solo', 'live')),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'ended')),
    opening_recording_id UUID NOT NULL REFERENCES recordings(id),
    accepted_links INT NULL,
    winner_user_id UUID NULL REFERENCES users(id),
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ended_at TIMESTAMPTZ NULL
);

CREATE INDEX games_status_idx ON games (status);
CREATE INDEX games_opening_recording_id_idx ON games (opening_recording_id);

CREATE TABLE game_players (
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    lives_remaining INT NULL,
    PRIMARY KEY (game_id, user_id)
);

CREATE INDEX game_players_user_id_idx ON game_players (user_id);

CREATE TABLE moves (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    turn_number INT NOT NULL,
    recording_id UUID NOT NULL REFERENCES recordings(id),
    credit_id UUID NOT NULL REFERENCES credit_entities(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT moves_game_id_turn_number_key UNIQUE (game_id, turn_number)
);

CREATE INDEX moves_game_id_idx ON moves (game_id);

CREATE TABLE reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    reporter_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    move_id UUID NULL REFERENCES moves(id) ON DELETE SET NULL,
    credit_id UUID NULL REFERENCES credit_entities(id) ON DELETE SET NULL,
    explanation TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved')),
    resolution_reason TEXT NULL,
    reviewer_id UUID NULL REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ NULL,
    CONSTRAINT reports_target_check CHECK (move_id IS NOT NULL OR credit_id IS NOT NULL)
);

CREATE INDEX reports_reporter_id_idx ON reports (reporter_id);
CREATE INDEX reports_status_idx ON reports (status);
