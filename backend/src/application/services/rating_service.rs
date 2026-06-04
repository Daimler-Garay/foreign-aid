use std::collections::HashSet;

use skillratings::{
    MultiTeamOutcome,
    weng_lin::{WengLinConfig, WengLinRating, weng_lin_multi_team},
};
use thiserror::Error;
use uuid::Uuid;

pub const DEFAULT_RATING: f64 = 25.0;
pub const DEFAULT_UNCERTAINTY: f64 = DEFAULT_RATING / 3.0;
pub const DISPLAY_RATING_SCALE: f64 = 40.0;
// this is is here to guard against a new player immediately dominating the ratings
// after a few matches
pub const CONSERVATIVE_UNCERTAINTY_MULTIPLIER: f64 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RatingInput {
    pub player_id: Uuid,
    pub rating: f64,
    pub uncertainty: f64,
    pub placement: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RatingUpdate {
    pub player_id: Uuid,
    pub placement: i32,
    pub old_rating: f64,
    pub old_uncertainty: f64,
    pub new_rating: f64,
    pub new_uncertainty: f64,
    pub rating_delta: f64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RatingError {
    #[error("rating calculation requires at least two players")]
    RequiresAtLeastTwoPlayers,
    #[error("placements must be positive")]
    InvalidPlacement,
    #[error("placements must be unique")]
    DuplicatePlacement,
}

pub fn default_rating() -> WengLinRating {
    WengLinRating::new()
}

pub fn display_rating(rating: f64) -> i32 {
    (rating * DISPLAY_RATING_SCALE).round() as i32
}

pub fn conservative_rank_score(rating: f64, uncertainty: f64) -> i32 {
    ((rating - (CONSERVATIVE_UNCERTAINTY_MULTIPLIER * uncertainty)) * DISPLAY_RATING_SCALE).round()
        as i32
}

// core function it does the following:
// 1. validation
// 2. convert each player to a single 'team'
// 3. pairs with MultiTeamOutcome::new(placement), lower equals better
// 4. calls the algorithm (wenglin) and map the ratings
pub fn rate_ranked_free_for_all(players: &[RatingInput]) -> Result<Vec<RatingUpdate>, RatingError> {
    validate_rating_inputs(players)?;

    let teams: Vec<Vec<WengLinRating>> = players
        .iter()
        .map(|player| {
            vec![WengLinRating {
                rating: player.rating,
                uncertainty: player.uncertainty,
            }]
        })
        .collect();
    let teams_and_ranks: Vec<(&[WengLinRating], MultiTeamOutcome)> = teams
        .iter()
        .zip(players.iter())
        .map(|(team, player)| (&team[..], MultiTeamOutcome::new(player.placement as usize)))
        .collect();
    let new_teams = weng_lin_multi_team(&teams_and_ranks, &WengLinConfig::new());

    Ok(players
        .iter()
        .zip(new_teams.iter())
        .map(|(old, new_team)| {
            let new_rating = new_team[0];
            RatingUpdate {
                player_id: old.player_id,
                placement: old.placement,
                old_rating: old.rating,
                old_uncertainty: old.uncertainty,
                new_rating: new_rating.rating,
                new_uncertainty: new_rating.uncertainty,
                rating_delta: new_rating.rating - old.rating,
            }
        })
        .collect())
}

fn validate_rating_inputs(players: &[RatingInput]) -> Result<(), RatingError> {
    // there has to be more than two players
    if players.len() < 2 {
        return Err(RatingError::RequiresAtLeastTwoPlayers);
    }

    let mut placements = HashSet::with_capacity(players.len());
    for player in players {
        if player.placement < 1 {
            return Err(RatingError::InvalidPlacement);
        }
        if !placements.insert(player.placement) {
            return Err(RatingError::DuplicatePlacement);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(placement: i32) -> RatingInput {
        RatingInput {
            player_id: Uuid::new_v4(),
            rating: DEFAULT_RATING,
            uncertainty: DEFAULT_UNCERTAINTY,
            placement,
        }
    }

    #[test]
    fn default_internal_rating_matches_weng_lin_defaults() {
        let rating = default_rating();

        assert_eq!(rating.rating, DEFAULT_RATING);
        assert!((rating.uncertainty - DEFAULT_UNCERTAINTY).abs() < f64::EPSILON);
    }

    #[test]
    fn display_rating_starts_at_1000() {
        assert_eq!(display_rating(DEFAULT_RATING), 1000);
    }

    #[test]
    fn conservative_score_is_lower_for_uncertain_players() {
        assert_eq!(
            conservative_rank_score(DEFAULT_RATING, DEFAULT_UNCERTAINTY),
            0
        );
        assert!(conservative_rank_score(30.0, DEFAULT_UNCERTAINTY) < display_rating(30.0));
    }

    #[test]
    fn winner_gains_rating_after_two_player_match() {
        let players = [input(1), input(2)];

        let updates = rate_ranked_free_for_all(&players).expect("ratings should update");

        assert!(updates[0].new_rating > updates[0].old_rating);
    }

    #[test]
    fn loser_loses_rating_after_two_player_match() {
        let players = [input(1), input(2)];

        let updates = rate_ranked_free_for_all(&players).expect("ratings should update");

        assert!(updates[1].new_rating < updates[1].old_rating);
    }

    #[test]
    fn first_place_gains_more_than_second_place() {
        let players = [input(1), input(2), input(3), input(4)];

        let updates = rate_ranked_free_for_all(&players).expect("ratings should update");

        assert!(updates[0].rating_delta > updates[1].rating_delta);
    }

    #[test]
    fn last_place_loses_more_than_middle_players() {
        let players = [input(1), input(2), input(3), input(4)];

        let updates = rate_ranked_free_for_all(&players).expect("ratings should update");

        assert!(updates[3].rating_delta < updates[2].rating_delta);
        assert!(updates[3].rating_delta < updates[1].rating_delta);
    }

    #[test]
    fn rating_output_preserves_input_order_and_player_ids() {
        let players = [input(2), input(1), input(3)];

        let updates = rate_ranked_free_for_all(&players).expect("ratings should update");

        assert_eq!(updates[0].player_id, players[0].player_id);
        assert_eq!(updates[1].player_id, players[1].player_id);
        assert_eq!(updates[2].player_id, players[2].player_id);
    }

    #[test]
    fn rejects_less_than_two_players() {
        let error = rate_ranked_free_for_all(&[input(1)]).expect_err("single player should fail");

        assert_eq!(error, RatingError::RequiresAtLeastTwoPlayers);
    }

    #[test]
    fn rejects_duplicate_placements() {
        let error = rate_ranked_free_for_all(&[input(1), input(1)])
            .expect_err("duplicate placements should fail");

        assert_eq!(error, RatingError::DuplicatePlacement);
    }
}
