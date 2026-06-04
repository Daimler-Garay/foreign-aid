DROP INDEX IF EXISTS idx_player_ratings_games_played;
DROP INDEX IF EXISTS idx_player_ratings_rank_score;
DROP INDEX IF EXISTS idx_players_display_name;
DROP INDEX IF EXISTS idx_players_active;
DROP INDEX IF EXISTS idx_users_role;
DROP INDEX IF EXISTS idx_users_active;

DROP TRIGGER IF EXISTS player_ratings_set_updated_at ON player_ratings;
DROP TRIGGER IF EXISTS players_set_updated_at ON players;
DROP TRIGGER IF EXISTS users_set_updated_at ON users;

DROP TABLE IF EXISTS player_ratings;
DROP TABLE IF EXISTS players;
DROP TABLE IF EXISTS users;

DROP FUNCTION IF EXISTS set_updated_at();
