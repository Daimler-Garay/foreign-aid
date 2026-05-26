DROP INDEX IF EXISTS idx_players_rating_desc;

DROP INDEX IF EXISTS idx_players_active;

DROP TRIGGER IF EXISTS players_set_updated_at ON players;

-- droopping players drops associated indexes but ill keep it on for clarify
DROP TABLE IF EXISTS players;

DROP FUNCTION IF EXISTS set_updated_at ();

