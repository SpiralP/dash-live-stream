// copied from https://github.com/sfackler/rust-openssl/blob/master/openssl/examples/mk_certs.rs

use crate::error::*;
use openssl::{
    asn1::Asn1Time,
    bn::{BigNum, MsbOption},
    ec::{EcGroup, EcKey},
    hash::MessageDigest,
    nid::Nid,
    pkey::{PKey, Private},
    x509::{extension::SubjectAlternativeName, X509NameBuilder, X509},
};

pub fn generate_cert_and_key() -> Result<(X509, PKey<Private>)> {
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1)?;
    let ec = EcKey::generate(&group)?;
    let privkey = PKey::from_ec_key(ec)?;

    let mut x509_name = X509NameBuilder::new()?;
    x509_name.append_entry_by_text("CN", "127.0.0.1")?;
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
    cert_builder.set_pubkey(&privkey)?;

    let not_before = Asn1Time::days_from_now(0)?;
    cert_builder.set_not_before(&not_before)?;
    let not_after = Asn1Time::days_from_now(1)?;
    cert_builder.set_not_after(&not_after)?;

    // cert_builder.append_extension(BasicConstraints::new().critical().ca().build()?)?;
    // cert_builder.append_extension(
    //     KeyUsage::new()
    //         .critical()
    //         .key_cert_sign()
    //         .crl_sign()
    //         .build()?,
    // )?;

    let alternative_name = SubjectAlternativeName::new()
        .ip("127.0.0.1")
        .dns("127.0.0.1")
        .build(&cert_builder.x509v3_context(None, None))?;
    cert_builder.append_extension(alternative_name)?;

    cert_builder.sign(&privkey, MessageDigest::sha256())?;
    let cert = cert_builder.build();

    Ok((cert, privkey))
}
