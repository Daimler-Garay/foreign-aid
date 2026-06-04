use std::collections::HashSet;

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::domain::{models::matches::PlacementRequest, validation::ValidationError};

pub const MAX_PLAYED_AT_FUTURE_SKEW_MINUTES: i64 = 5;

pub fn validate_match_submission(
    played_at: DateTime<Utc>,
    placements: &[PlacementRequest],
    active_player_ids: &HashSet<Uuid>,
    now: DateTime<Utc>,
) -> Result<(), ValidationError> {
    validate_placements(placements)?;
    validate_active_players(placements, active_player_ids)?;
    validate_played_at(played_at, now)?;

    Ok(())
}

pub fn validate_placements(placements: &[PlacementRequest]) -> Result<(), ValidationError> {
    if placements.len() < 2 {
        return Err(ValidationError::MatchRequiresAtLeastTwoPlayers);
    }

    let mut player_ids = HashSet::<Uuid>::new();
    for placement in placements {
        if !player_ids.insert(placement.player_id) {
            return Err(ValidationError::DuplicatePlayers);
        }
    }

    let mut ranks: Vec<i32> = placements
        .iter()
        .map(|placement| placement.placement)
        .collect();
    ranks.sort_unstable();

    if ranks.windows(2).any(|window| window[0] == window[1]) {
        return Err(ValidationError::DuplicatePlacements);
    }

    let expected: Vec<i32> = (1..=placements.len() as i32).collect();
    if ranks != expected {
        return Err(ValidationError::NonSequentialPlacements);
    }

    Ok(())
}

fn validate_active_players(
    placements: &[PlacementRequest],
    active_player_ids: &HashSet<Uuid>,
) -> Result<(), ValidationError> {
    if placements
        .iter()
        .any(|placement| !active_player_ids.contains(&placement.player_id))
    {
        return Err(ValidationError::MissingOrInactivePlayers);
    }

    Ok(())
}

fn validate_played_at(played_at: DateTime<Utc>, now: DateTime<Utc>) -> Result<(), ValidationError> {
    if played_at > now + Duration::minutes(MAX_PLAYED_AT_FUTURE_SKEW_MINUTES) {
        return Err(ValidationError::PlayedAtTooFarInFuture);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(player_id: u128, placement: i32) -> PlacementRequest {
        PlacementRequest {
            player_id: Uuid::from_u128(player_id),
            placement,
        }
    }

    #[test]
    fn rejects_match_with_less_than_two_players() {
        let error = validate_placements(&[placement(1, 1)]).expect_err("match should fail");

        assert_eq!(error, ValidationError::MatchRequiresAtLeastTwoPlayers);
    }

    #[test]
    fn rejects_duplicate_players() {
        let placements = [placement(1, 1), placement(1, 2)];

        let error = validate_placements(&placements).expect_err("match should fail");

        assert_eq!(error, ValidationError::DuplicatePlayers);
    }

    #[test]
    fn rejects_duplicate_placements() {
        let placements = [placement(1, 1), placement(2, 1)];

        let error = validate_placements(&placements).expect_err("match should fail");

        assert_eq!(error, ValidationError::DuplicatePlacements);
    }

    #[test]
    fn rejects_non_sequential_placements() {
        let placements = [placement(1, 1), placement(2, 3)];

        let error = validate_placements(&placements).expect_err("match should fail");

        assert_eq!(error, ValidationError::NonSequentialPlacements);
    }

    #[test]
    fn accepts_valid_placements() {
        let placements = [placement(1, 2), placement(2, 1), placement(3, 3)];

        validate_placements(&placements).expect("match should pass");
    }

    #[test]
    fn rejects_placement_zero() {
        let placements = [placement(1, 0), placement(2, 1)];

        let error = validate_placements(&placements).expect_err("match should fail");

        assert_eq!(error, ValidationError::NonSequentialPlacements);
    }

    #[test]
    fn rejects_missing_or_inactive_players() {
        let placements = [placement(1, 1), placement(2, 2)];
        let active_player_ids = HashSet::from([Uuid::from_u128(1)]);

        let error =
            validate_match_submission(Utc::now(), &placements, &active_player_ids, Utc::now())
                .expect_err("match should fail");

        assert_eq!(error, ValidationError::MissingOrInactivePlayers);
    }

    #[test]
    fn rejects_played_at_too_far_in_future() {
        let now = Utc::now();
        let placements = [placement(1, 1), placement(2, 2)];
        let active_player_ids = HashSet::from([Uuid::from_u128(1), Uuid::from_u128(2)]);

        let error = validate_match_submission(
            now + Duration::minutes(MAX_PLAYED_AT_FUTURE_SKEW_MINUTES + 1),
            &placements,
            &active_player_ids,
            now,
        )
        .expect_err("match should fail");

        assert_eq!(error, ValidationError::PlayedAtTooFarInFuture);
    }

    #[test]
    fn accepts_valid_two_player_match() {
        let now = Utc::now();
        let placements = [placement(1, 1), placement(2, 2)];
        let active_player_ids = HashSet::from([Uuid::from_u128(1), Uuid::from_u128(2)]);

        validate_match_submission(now, &placements, &active_player_ids, now)
            .expect("match should pass");
    }

    #[test]
    fn accepts_valid_six_player_match() {
        let now = Utc::now();
        let placements = [
            placement(1, 1),
            placement(2, 2),
            placement(3, 3),
            placement(4, 4),
            placement(5, 5),
            placement(6, 6),
        ];
        let active_player_ids = HashSet::from([
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            Uuid::from_u128(3),
            Uuid::from_u128(4),
            Uuid::from_u128(5),
            Uuid::from_u128(6),
        ]);

        validate_match_submission(now, &placements, &active_player_ids, now)
            .expect("match should pass");
    }
}
