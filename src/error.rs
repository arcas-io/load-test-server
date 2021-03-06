use libwebrtc::error::WebRTCError;
use log::error;
use std::net::AddrParseError;
use std::sync::{MutexGuard, PoisonError};
use thiserror::Error;
use tonic::Status;

pub(crate) type Result<T> = std::result::Result<T, ServerError>;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Could not create peer connection: {0}")]
    CreatePeerConnectionError(String),

    #[error("Could not create answer: {0}")]
    CouldNotCreateAnswer(String),

    #[error("Could not create offer: {0}")]
    CouldNotCreateOffer(String),

    #[error("Could not create track: {0}")]
    CouldNotCreateTrack(String),

    #[error("Could not create transceiver: {0}")]
    CouldNotAddTransceiver(String),

    #[error("Could not parse SDP: {0}")]
    CouldNotParseSdp(String),

    #[error("Could not set SDP: {0}")]
    CouldNotSetSdp(String),

    #[error("Could not retrieve stats for session {0}, peer connection {1}")]
    GetStatsError(String, String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("PeerConnection {0} does not exist")]
    InvalidPeerConnection(String),

    #[error("Session {0} does not exist")]
    InvalidSessionError(String),

    #[error("{0}")]
    InvalidStateError(String),

    #[error("TimeStamp {0} is invalid")]
    InvalidTimeStampError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("WebRTC error: {0}")]
    WebRTCError(String),
}

impl From<AddrParseError> for ServerError {
    fn from(error: AddrParseError) -> Self {
        error!("{:?}", error);
        ServerError::ParseError(error.to_string())
    }
}

impl<T> From<PoisonError<MutexGuard<'_, T>>> for ServerError {
    fn from(error: PoisonError<MutexGuard<T>>) -> Self {
        error!("{:?}", error);
        ServerError::InternalError(error.to_string())
    }
}

impl From<ServerError> for Status {
    fn from(error: ServerError) -> Status {
        error!("{:?}", error);
        Status::internal(error.to_string())
    }
}

impl From<&ServerError> for Status {
    fn from(error: &ServerError) -> Status {
        error!("{:?}", error);
        Status::internal(error.to_string())
    }
}

impl From<WebRTCError> for ServerError {
    fn from(value: WebRTCError) -> Self {
        // A little lazy to have a single variant but we can improve this in the future.
        Self::WebRTCError(value.to_string())
    }
}
