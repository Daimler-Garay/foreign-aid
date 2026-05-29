CREATE TABLE matches (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid (),
    host_player_id uuid NOT NULL REFERENCES players (id),
    status text NOT NULL DEFAULT 'lobby',
    CONSTRAINT match_status CHECK (status IN ('lobby', 'in_progress', 'completed')),
    notes text,
    created_at timestamptz NOT NULL DEFAULT NOW(),
    started_at timestamptz,
    completed_at timestamptz
);

CREATE TABLE match_players (
    match_id uuid NOT NULL REFERENCES matches (id) ON DELETE CASCADE,
    player_id uuid NOT NULL REFERENCES players (id),
    placement integer, -- null until match completes or host sets it
    joined_at timestamptz NOT NULL DEFAULT NOW(),
    eliminated_at timestamptz, -- null = still active / winner
    -- Rating snapshot
    old_rating float8,
    old_rating_deviation float8,
    new_rating float8,
    new_rating_deviation float8,
    rating_delta float8,
    PRIMARY KEY (match_id, player_id)
);

