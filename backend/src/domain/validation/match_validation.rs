use std::collections::HashSet;

use uuid::Uuid;

use crate::domain::{models::matches::PlacementRequest, validation::ValidationError};

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
}
