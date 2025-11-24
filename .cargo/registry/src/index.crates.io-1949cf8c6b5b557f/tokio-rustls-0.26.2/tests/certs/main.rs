//! An ignored-by-default integration test that regenerates vendored certs.
//! Run with `cargo test -- --ignored` when test certificates need updating.
//! Suitable for test certificates only. Not a production CA ;-)

use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    KeyPair, KeyUsagePurpose,
};
use std::fs::File;
use std::io::Write;

#[test]
#[ignore]
fn regenerate_certs() {
    let root_key = KeyPair::generate().unwrap();
    let root_ca = issuer_params("Rustls Robust Root")
        .self_signed(&root_key)
        .unwrap();

    let mut root_file = File::create("tests/certs/root.pem").unwrap();
    root_file.write_all(root_ca.pem().as_bytes()).unwrap();

    let intermediate_key = KeyPair::generate().unwrap();
    let intermediate_ca = issuer_params("Rustls Robust Root - Rung 2")
        .signed_by(&intermediate_key, &root_ca, &root_key)
        .unwrap();

    let end_entity_key = KeyPair::generate().unwrap();
    let mut end_entity_params =
        CertificateParams::new(vec![utils::TEST_SERVER_DOMAIN.to_string()]).unwrap();
    end_entity_params.is_ca = IsCa::ExplicitNoCa;
    end_entity_params.extended_key_usages = vec![
        ExtendedKeyUsagePurpose::ServerAuth,
        ExtendedKeyUsagePurpose::ClientAuth,
    ];
    let end_entity = end_entity_params
        .signed_by(&end_entity_key, &intermediate_ca, &intermediate_key)
        .unwrap();

    let mut chain_file = File::create("tests/certs/chain.pem").unwrap();
    chain_file.write_all(end_entity.pem().as_bytes()).unwrap();
    chain_file
        .write_all(intermediate_ca.pem().as_bytes())
        .unwrap();

    let mut key_file = File::create("tests/certs/end.key").unwrap();
    key_file
        .write_all(end_entity_key.serialize_pem().as_bytes())
        .unwrap();
}

fn issuer_params(common_name: &str) -> CertificateParams {
    let mut issuer_name = DistinguishedName::new();
    issuer_name.push(DnType::CommonName, common_name);
    let mut issuer_params = CertificateParams::default();
    issuer_params.distinguished_name = issuer_name;
    issuer_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    issuer_params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    issuer_params
}

// For the server name constant.
include!("../utils.rs");
