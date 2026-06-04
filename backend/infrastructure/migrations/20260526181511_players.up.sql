CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TABLE users (
    id UUID PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('admin', 'player')),
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT users_username_not_empty CHECK (length(trim(username)) > 0),
    CONSTRAINT users_password_hash_not_empty CHECK (length(trim(password_hash)) > 0)
);

CREATE TRIGGER users_set_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TABLE players (
    id UUID PRIMARY KEY,
    user_id UUID UNIQUE REFERENCES users(id) ON DELETE SET NULL,
    display_name TEXT NOT NULL UNIQUE,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT players_display_name_not_empty CHECK (length(trim(display_name)) > 0)
);

CREATE TRIGGER players_set_updated_at
    BEFORE UPDATE ON players
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TABLE player_ratings (
    player_id UUID PRIMARY KEY REFERENCES players(id) ON DELETE CASCADE,

    rating DOUBLE PRECISION NOT NULL DEFAULT 25.0,
    uncertainty DOUBLE PRECISION NOT NULL DEFAULT 8.3333333333,

    games_played INTEGER NOT NULL DEFAULT 0,
    wins INTEGER NOT NULL DEFAULT 0,
    losses INTEGER NOT NULL DEFAULT 0,
    total_placement INTEGER NOT NULL DEFAULT 0,

    last_played_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT player_ratings_rating_positive CHECK (rating > 0),
    CONSTRAINT player_ratings_uncertainty_positive CHECK (uncertainty > 0),
    CONSTRAINT player_ratings_games_played_non_negative CHECK (games_played >= 0),
    CONSTRAINT player_ratings_wins_non_negative CHECK (wins >= 0),
    CONSTRAINT player_ratings_losses_non_negative CHECK (losses >= 0),
    CONSTRAINT player_ratings_total_placement_non_negative CHECK (total_placement >= 0),
    CONSTRAINT player_ratings_wins_not_above_games CHECK (wins <= games_played),
    CONSTRAINT player_ratings_losses_not_above_games CHECK (losses <= games_played)
);

CREATE TRIGGER player_ratings_set_updated_at
    BEFORE UPDATE ON player_ratings
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE INDEX idx_users_active ON users(active);
CREATE INDEX idx_users_role ON users(role);
CREATE INDEX idx_players_active ON players(active);
CREATE INDEX idx_players_display_name ON players(display_name);
CREATE INDEX idx_player_ratings_rank_score ON player_ratings((rating - (3.0 * uncertainty)) DESC);
CREATE INDEX idx_player_ratings_games_played ON player_ratings(games_played DESC);
