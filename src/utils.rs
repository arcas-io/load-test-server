use lazy_static::lazy_static;
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use std::string::ToString;

use crate::crypto::certificate;
use crate::OfferWebSocketError;

lazy_static! {
    pub static ref CERTIFICATE: (X509, PKey<Private>) =
        certificate().expect("Fatal error, could not load cert");
}

// helper function to reduce boilerplate
// TODO: upgrade to using finer grained error types via thiserror
pub(crate) fn log_error<T: ToString>(prefix: &str, message: T) -> OfferWebSocketError {
    let message = message.to_string();
    log::error!("[{}] {:?}", prefix, message);
    OfferWebSocketError::InternalError(message)
}
