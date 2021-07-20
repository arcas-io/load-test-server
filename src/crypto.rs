use crate::error::Result;
use crate::utils::log_error;
use openssl::asn1::Asn1Time;
use openssl::bn::{BigNum, MsbOption};
use openssl::error::ErrorStack;
use openssl::hash::MessageDigest;
use openssl::pkey::{PKey, Private};
use openssl::rsa::Rsa;
use openssl::x509::extension::{BasicConstraints, KeyUsage, SubjectKeyIdentifier};
use openssl::x509::{X509NameBuilder, X509};

pub(crate) fn certificate() -> Result<(X509, PKey<Private>)> {
    self_signed_certificate().map_err(|e| log_error("CreateCertificateError", &e.to_string()))
}

fn self_signed_certificate() -> std::result::Result<(X509, PKey<Private>), ErrorStack> {
    let rsa = Rsa::generate(2048)?;
    let key_pair = PKey::from_rsa(rsa)?;

    let mut x509_name = X509NameBuilder::new()?;
    x509_name.append_entry_by_text("C", "US")?;
    x509_name.append_entry_by_text("ST", "CO")?;
    x509_name.append_entry_by_text("O", "Some CO organization")?;
    x509_name.append_entry_by_text("CN", "co test")?;
    let x509_name = x509_name.build();

    let mut cert_builder = X509::builder()?;
    cert_builder.set_version(2)?;
    let serial_number = {
        let mut serial = BigNum::new()?;
        serial.rand(159, MsbOption::MAYBE_ZERO, false)?;
        serial.to_asn1_integer()?
    };
    cert_builder.set_serial_number(&serial_number)?;
    cert_builder.set_subject_name(&x509_name)?;
    cert_builder.set_issuer_name(&x509_name)?;
    cert_builder.set_pubkey(&key_pair)?;
    let not_before = Asn1Time::days_from_now(0)?;
    cert_builder.set_not_before(&not_before)?;
    let not_after = Asn1Time::days_from_now(365)?;
    cert_builder.set_not_after(&not_after)?;

    let subject_key_identifier =
        SubjectKeyIdentifier::new().build(&cert_builder.x509v3_context(None, None))?;
    cert_builder.append_extension(subject_key_identifier)?;

    cert_builder.sign(&key_pair, MessageDigest::sha256())?;
    let cert = cert_builder.build();

    Ok((cert, key_pair))
}

pub(crate) fn fingerprint(certificate: &X509) -> Result<String> {
    let hash = certificate
        .digest(MessageDigest::sha256())
        .map_err(|e| log_error("CreateFingerprintError", &e.to_string()))?;
    let fingerprint = hash
        .as_ref()
        .iter()
        .map(|b| format!(":{:02X}", b))
        .collect::<String>()
        .trim_start_matches(':') // remove leading colon
        .to_string();

    Ok(fingerprint)
}
