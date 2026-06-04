use thiserror::Error;

pub mod match_validation;
pub mod player_validation;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("display name cannot be blank")]
    BlankDisplayName,
    #[error("match must include at least two players")]
    MatchRequiresAtLeastTwoPlayers,
    #[error("match cannot include duplicate players")]
    DuplicatePlayers,
    #[error("placements must be unique")]
    DuplicatePlacements,
    #[error("placements must start at 1 and be sequential")]
    NonSequentialPlacements,
    #[error("match includes missing or inactive players")]
    MissingOrInactivePlayers,
    #[error("played_at cannot be too far in the future")]
    PlayedAtTooFarInFuture,
}
