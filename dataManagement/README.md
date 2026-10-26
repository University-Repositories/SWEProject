# Database & Data Management (`data_management`)

This crate manages the PostgreSQL database, schema migrations, gameplay queries, and catalog imports from MusicBrainz for the Beat4Beat PVP game.

---

## 1. Running the Tests

Ensure the PostgreSQL container is running:
```bash
docker compose up -d
```

Run the database integration tests:
```bash
cargo test -p data_management --test game_db_test
```

To see real-time console output during testing:
```bash
cargo test -p data_management --test game_db_test -- --nocapture
```

---

## 2. Table Schemas

Migrations are defined in [`migrations/0001_init.sql`](migrations/0001_init.sql).

### `songs` (Song Catalog)
Stores the dictionary of playable songs and pre-computed metadata for 0-join link verification.

```sql
CREATE TABLE songs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    mbid TEXT UNIQUE NOT NULL,                  -- MusicBrainz Recording MBID
    title TEXT NOT NULL,                        -- Song title
    release_year INT,                           -- Earliest release year
    duration_ms INT,                            -- Track duration in milliseconds
    artists TEXT[] NOT NULL DEFAULT '{}',       -- Main credited artist(s)
    albums TEXT[] NOT NULL DEFAULT '{}',        -- Release(s) / album(s)
    credits TEXT[] NOT NULL DEFAULT '{}',       -- Producers, songwriters, composers
    locations TEXT[] NOT NULL DEFAULT '{}',     -- Recording studios and cities
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- GIN indexes for fast array-overlap link queries
CREATE INDEX idx_songs_title ON songs(title);
CREATE INDEX idx_songs_year ON songs(release_year);
CREATE INDEX idx_songs_artists ON songs USING GIN (artists);
CREATE INDEX idx_songs_albums ON songs USING GIN (albums);
CREATE INDEX idx_songs_credits ON songs USING GIN (credits);
CREATE INDEX idx_songs_locations ON songs USING GIN (locations);
```

### `users` (Player Accounts)
```sql
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username TEXT UNIQUE NOT NULL,
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    games_played INT NOT NULL DEFAULT 0,
    games_won INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### `matches` (PVP Match Sessions)
```sql
CREATE TABLE matches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    player1_id UUID NOT NULL REFERENCES users(id),
    player2_id UUID REFERENCES users(id),
    winner_id UUID REFERENCES users(id),
    status TEXT NOT NULL DEFAULT 'waiting',     -- 'waiting', 'active', 'finished', 'abandoned'
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ
);
```

### `match_moves` (Turn History & Move Ledger)
```sql
CREATE TABLE match_moves (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    match_id UUID NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
    player_id UUID NOT NULL REFERENCES users(id),
    turn_number INT NOT NULL,
    song_id UUID NOT NULL REFERENCES songs(id),
    link_type TEXT,                             -- 'artist', 'album', 'year', 'credit', 'location'
    link_value TEXT,                            -- e.g. "Queen", "1975", "Abbey Road Studios"
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (match_id, turn_number)
);

CREATE INDEX idx_match_moves_match ON match_moves(match_id);
```

---

## 3. The 5 Linking Mechanics

Link checking is executed directly inside PostgreSQL via `check_song_link(pool, song1_id, song2_id)`:

| Link Type | Table Field | Validation Rule |
| --- | --- | --- |
| **Main Artist** | `artists TEXT[]` | Array overlap (`s1.artists && s2.artists`) |
| **Album** | `albums TEXT[]` | Array overlap (`s1.albums && s2.albums`) |
| **Release Year** | `release_year INT` | Value match (`s1.release_year = s2.release_year`) |
| **Producer / Songwriter** | `credits TEXT[]` | Array overlap (`s1.credits && s2.credits`) |
| **Studio / City** | `locations TEXT[]` | Array overlap (`s1.locations && s2.locations`) |

---

## 4. Populating the `songs` Table

To populate the `songs` table with 1,000 songs from MusicBrainz in the pop genre, run:

```bash
cargo run -p data_management --example populate_pop
```
