DROP INDEX IF EXISTS idx_rating_recalculation_runs_started_at;
DROP INDEX IF EXISTS idx_rating_recalculation_runs_status;

DROP INDEX IF EXISTS idx_audit_log_entity;
DROP INDEX IF EXISTS idx_audit_log_actor_user_id;
DROP INDEX IF EXISTS idx_audit_log_created_at;

DROP INDEX IF EXISTS idx_match_players_placement;
DROP INDEX IF EXISTS idx_match_players_player_id;

DROP INDEX IF EXISTS idx_matches_corrected_from_match_id;
DROP INDEX IF EXISTS idx_matches_submitted_by_user_id;
DROP INDEX IF EXISTS idx_matches_replay_order;
DROP INDEX IF EXISTS idx_matches_history_order;
DROP INDEX IF EXISTS idx_matches_status;

DROP TABLE IF EXISTS rating_recalculation_runs;
DROP TABLE IF EXISTS audit_log;
DROP TABLE IF EXISTS match_players;

DROP TRIGGER IF EXISTS matches_set_updated_at ON matches;
DROP TABLE IF EXISTS matches;
