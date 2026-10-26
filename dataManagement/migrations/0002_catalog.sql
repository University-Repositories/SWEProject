CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TABLE recordings (
    id UUID PRIMARY KEY,
    title TEXT NOT NULL,
    artist_name TEXT NOT NULL,
    year INT NULL,
    release_id UUID NULL
);

CREATE INDEX recordings_title_trgm_idx ON recordings USING gin (title gin_trgm_ops);
CREATE INDEX recordings_artist_name_trgm_idx ON recordings USING gin (artist_name gin_trgm_ops);

CREATE TABLE credit_entities (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('person', 'label', 'album')),
    name TEXT NOT NULL
);

CREATE TABLE recording_credits (
    recording_id UUID NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
    credit_id UUID NOT NULL REFERENCES credit_entities(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('artist', 'feature', 'producer', 'writer', 'label', 'album')),
    PRIMARY KEY (recording_id, credit_id, role)
);

CREATE INDEX recording_credits_credit_id_idx ON recording_credits (credit_id);
