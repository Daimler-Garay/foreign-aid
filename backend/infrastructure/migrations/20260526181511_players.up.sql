CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE players (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid (),
    display_name text NOT NULL UNIQUE,
    active boolean NOT NULL DEFAULT TRUE,
    rating double precision NOT NULL DEFAULT 1000.0,
    rating_deviation double precision NOT NULL DEFAULT 350.0,
    volatility double precision NOT NULL DEFAULT 0.06,
    games_played integer NOT NULL DEFAULT 0,
    wins integer NOT NULL DEFAULT 0,
    losses integer NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT players_display_name_not_empty CHECK (length(trim(display_name)) > 0),
    CONSTRAINT players_rating_positive CHECK (rating > 0),
    CONSTRAINT players_rating_deviation_positive CHECK (rating_deviation > 0),
    CONSTRAINT players_volatility_positive CHECK (volatility > 0),
    CONSTRAINT players_games_played_non_negative CHECK (games_played >= 0),
    CONSTRAINT players_wins_non_negative CHECK (wins >= 0),
    CONSTRAINT players_losses_non_negative CHECK (losses >= 0),
    CONSTRAINT players_record_valid CHECK (wins + losses <= games_played)
);

CREATE OR REPLACE FUNCTION set_updated_at ()
    RETURNS TRIGGER
    AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$
LANGUAGE plpgsql;

CREATE TRIGGER players_set_updated_at
    BEFORE UPDATE ON players
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at ();

CREATE INDEX idx_players_active ON players (active);

CREATE INDEX idx_players_rating_desc ON players (rating DESC)
WHERE
    active = TRUE;

