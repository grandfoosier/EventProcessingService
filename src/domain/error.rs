use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("event not found")]
    NotFound,

    #[error("invalid transition")]
    InvalidTransition,
}
