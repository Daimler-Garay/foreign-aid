CREATE TABLE matches (
    id UUID PRIMARY KEY,

    played_at TIMESTAMPTZ NOT NULL,
    submitted_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,

    status TEXT NOT NULL DEFAULT 'confirmed' CHECK (status IN ('pending', 'confirmed', 'voided')),
    notes TEXT,

    rating_algorithm TEXT NOT NULL DEFAULT 'weng_lin',
    rating_algorithm_version INTEGER NOT NULL DEFAULT 1,

    corrected_from_match_id UUID REFERENCES matches(id) ON DELETE SET NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT matches_rating_algorithm_not_empty CHECK (length(trim(rating_algorithm)) > 0),
    CONSTRAINT matches_rating_algorithm_version_positive CHECK (rating_algorithm_version > 0),
    CONSTRAINT matches_not_self_corrected CHECK (corrected_from_match_id IS NULL OR corrected_from_match_id <> id)
);

CREATE TRIGGER matches_set_updated_at
    BEFORE UPDATE ON matches
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TABLE match_players (
    match_id UUID NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
    player_id UUID NOT NULL REFERENCES players(id) ON DELETE RESTRICT,

    placement INTEGER NOT NULL CHECK (placement >= 1),

    old_rating DOUBLE PRECISION NOT NULL,
    old_uncertainty DOUBLE PRECISION NOT NULL,
    new_rating DOUBLE PRECISION NOT NULL,
    new_uncertainty DOUBLE PRECISION NOT NULL,

    rating_delta DOUBLE PRECISION NOT NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (match_id, player_id),
    UNIQUE (match_id, placement),

    CONSTRAINT match_players_old_rating_positive CHECK (old_rating > 0),
    CONSTRAINT match_players_old_uncertainty_positive CHECK (old_uncertainty > 0),
    CONSTRAINT match_players_new_rating_positive CHECK (new_rating > 0),
    CONSTRAINT match_players_new_uncertainty_positive CHECK (new_uncertainty > 0)
);

CREATE TABLE audit_log (
    id UUID PRIMARY KEY,

    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,

    action TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id UUID,

    old_value JSONB,
    new_value JSONB,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT audit_log_action_not_empty CHECK (length(trim(action)) > 0),
    CONSTRAINT audit_log_entity_type_not_empty CHECK (length(trim(entity_type)) > 0)
);

CREATE TABLE rating_recalculation_runs (
    id UUID PRIMARY KEY,

    triggered_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    reason TEXT NOT NULL,

    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ,

    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
    error_message TEXT,

    CONSTRAINT rating_recalculation_runs_reason_not_empty CHECK (length(trim(reason)) > 0),
    CONSTRAINT rating_recalculation_runs_finished_after_started CHECK (finished_at IS NULL OR finished_at >= started_at)
);

CREATE INDEX idx_matches_status ON matches(status);
CREATE INDEX idx_matches_history_order ON matches(played_at DESC, created_at DESC, id DESC);
CREATE INDEX idx_matches_replay_order ON matches(played_at ASC, created_at ASC, id ASC)
    WHERE status = 'confirmed';
CREATE INDEX idx_matches_submitted_by_user_id ON matches(submitted_by_user_id);
CREATE INDEX idx_matches_corrected_from_match_id ON matches(corrected_from_match_id);

CREATE INDEX idx_match_players_player_id ON match_players(player_id);
CREATE INDEX idx_match_players_placement ON match_players(match_id, placement);

CREATE INDEX idx_audit_log_created_at ON audit_log(created_at DESC);
CREATE INDEX idx_audit_log_actor_user_id ON audit_log(actor_user_id);
CREATE INDEX idx_audit_log_entity ON audit_log(entity_type, entity_id);

CREATE INDEX idx_rating_recalculation_runs_status ON rating_recalculation_runs(status);
CREATE INDEX idx_rating_recalculation_runs_started_at ON rating_recalculation_runs(started_at DESC);
