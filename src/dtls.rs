use crate::error::Result;
use crate::utils::{log_error, CERTIFICATE};
use openssl::ssl::{
    Ssl, SslAcceptor, SslAcceptorBuilder, SslConnector, SslConnectorBuilder, SslMethod,
};
use std::sync::Arc;

fn ssl_connector(method: SslMethod) -> Result<SslConnectorBuilder> {
    let mut ssl_ctx =
        SslConnector::builder(method).map_err(|e| log_error("SslConnectorError", e))?;

    ssl_ctx
        .set_tlsext_use_srtp("SRTP_AES128_CM_SHA1_80:SRTP_AES128_CM_SHA1_32")
        .map_err(|e| log_error("SslSrtpError", e))?;
    ssl_ctx
        .set_certificate(&(*CERTIFICATE).0)
        .map_err(|e| log_error("SslCertificateError", e))?;
    ssl_ctx
        .set_private_key(&(*CERTIFICATE).1)
        .map_err(|e| log_error("SslPrivateKeyError", e))?;

    Ok(ssl_ctx)
}

fn ssl_acceptor(method: SslMethod) -> Result<SslAcceptorBuilder> {
    let mut ssl_ctx =
        SslAcceptor::mozilla_modern(method).map_err(|e| log_error("SslAcceptorError", e))?;

    ssl_ctx
        .set_tlsext_use_srtp("SRTP_AES128_CM_SHA1_80:SRTP_AEAD_AES_128_GCM")
        .map_err(|e| log_error("SslSrtpError", e))?;
    ssl_ctx
        .set_certificate(&(*CERTIFICATE).0)
        .map_err(|e| log_error("SslCertificateError", e))?;
    ssl_ctx
        .set_private_key(&(*CERTIFICATE).1)
        .map_err(|e| log_error("SslPrivateKeyError", e))?;

    Ok(ssl_ctx)
}

pub(crate) fn ssl_client(method: SslMethod) -> Result<Ssl> {
    let ssl_connector = ssl_connector(method)?;

    let mut ssl = ssl_connector
        .build()
        .configure()
        .map_err(|e| log_error("SslConfigureError", e))?
        .into_ssl("localhost")
        .map_err(|e| log_error("SslIntoSslError", e))?;

    // required in local testing b/c the cert is self-signed
    ssl.set_verify(openssl::ssl::SslVerifyMode::NONE);

    Ok(ssl)
}

pub(crate) fn ssl_server(method: SslMethod) -> Result<Arc<SslAcceptor>> {
    let ssl_acceptor = ssl_acceptor(method)?;

    Ok(Arc::new(ssl_acceptor.build()))
}
