use std::net::AddrParseError;
use std::sync::{MutexGuard, PoisonError};
use thiserror::Error;
use tonic::Status;

pub type Result<T> = std::result::Result<T, ServerError>;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Could not create peer connection: {0}")]
    CreatePeerConnectionError(String),

    #[error("Could not retrieve stats for session {0}, peer connection {1}")]
    GetStatsError(String, String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Session {0} does not exist")]
    InvalidSessionError(String),

    #[error("{0}")]
    InvalidStateError(String),

    #[error("TimeStamp {0} is invalid")]
    InvalidTimeStampError(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}

impl From<AddrParseError> for ServerError {
    fn from(error: AddrParseError) -> Self {
        ServerError::ParseError(error.to_string())
    }
}

impl<T> From<PoisonError<MutexGuard<'_, T>>> for ServerError {
    fn from(error: PoisonError<MutexGuard<T>>) -> Self {
        ServerError::InternalError(error.to_string())
    }
}

impl From<ServerError> for Status {
    fn from(err: ServerError) -> Status {
        Status::internal(err.to_string())
    }
}
