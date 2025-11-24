use std::ascii::escape_default;
use std::fmt::Write;
use std::fs;

use pki_types::CertificateDer;
use ring::digest;
use webpki::anchor_from_trusted_cert;
use webpki_ccadb::fetch_ccadb_roots;
use x509_parser::prelude::AttributeTypeAndValue;
use x509_parser::x509::X509Name;

#[tokio::test]
async fn new_generated_code_is_fresh() {
    let tls_roots_map = fetch_ccadb_roots().await;
    let mut code = String::with_capacity(256 * 1_024);
    code.push_str(HEADER);
    code.push_str("pub const TLS_SERVER_ROOTS: &[TrustAnchor<'static>] = &[\n");
    let (mut subject, mut spki, mut name_constraints) =
        (String::new(), String::new(), String::new());

    for root in tls_roots_map.values() {
        // Verify the DER FP matches the metadata FP.
        let der = root.der();
        let calculated_fp = digest::digest(&digest::SHA256, &der);
        let metadata_fp = hex::decode(&root.sha256_fingerprint).expect("malformed fingerprint");
        assert_eq!(calculated_fp.as_ref(), metadata_fp.as_slice());

        let ta_der = CertificateDer::from(der.as_ref());
        let ta = anchor_from_trusted_cert(&ta_der).expect("malformed trust anchor der");
        subject.clear();
        for &b in ta.subject.as_ref() {
            write!(&mut subject, "{}", escape_default(b)).unwrap();
        }

        spki.clear();
        for &b in ta.subject_public_key_info.as_ref() {
            write!(&mut spki, "{}", escape_default(b)).unwrap();
        }

        name_constraints.clear();
        if let Some(nc) = &root.mozilla_applied_constraints() {
            for &b in nc.iter() {
                write!(&mut name_constraints, "{}", escape_default(b)).unwrap();
            }
        }

        let (_, parsed_cert) =
            x509_parser::parse_x509_certificate(&der).expect("malformed x509 der");
        let issuer = name_to_string(parsed_cert.issuer());
        let subject_str = name_to_string(parsed_cert.subject());
        let label = root.common_name_or_certificate_name.clone();
        let serial = root.serial().to_string();
        let sha256_fp = root.sha256_fp();

        // Write comment
        code.push_str("  /*\n");
        code.push_str(&format!("   * Issuer: {issuer}\n"));
        code.push_str(&format!("   * Subject: {subject_str}\n"));
        code.push_str(&format!("   * Label: {label:?}\n"));
        code.push_str(&format!("   * Serial: {serial}\n"));
        code.push_str(&format!("   * SHA256 Fingerprint: {sha256_fp}\n"));
        for ln in root.pem().lines() {
            code.push_str("   * ");
            code.push_str(ln.trim());
            code.push('\n');
        }
        code.push_str("   */\n");

        // Write the code
        code.push_str("  TrustAnchor {\n");
        code.write_fmt(format_args!(
            "    subject: Der::from_slice(b\"{subject}\"),\n"
        ))
        .unwrap();
        code.write_fmt(format_args!(
            "    subject_public_key_info: Der::from_slice(b\"{spki}\"),\n"
        ))
        .unwrap();
        match name_constraints.is_empty() {
            false => code
                .write_fmt(format_args!(
                    "    name_constraints: Some(Der::from_slice(b\"{name_constraints}\"))\n"
                ))
                .unwrap(),
            true => code.push_str("    name_constraints: None\n"),
        }
        code.push_str("  },\n\n");
    }
    code.push_str("];\n");

    // Check that the generated code matches the checked-in code
    let old = fs::read_to_string("src/lib.rs").unwrap();
    if old != code {
        fs::write("src/lib.rs", code).unwrap();
        panic!("generated code changed");
    }
}

/// The built-in x509_parser::X509Name Display impl uses a different sort order than
/// the one historically used by mkcert.org^[0]. We re-create that sort order here to
/// avoid unnecessary churn in the generated code.
///
/// [0]: <https://github.com/Lukasa/mkcert/blob/6911a8f68681f4d6a795c1f6db7b063f75b03b5a/certs/convert_mozilla_certdata.go#L405-L428>
fn name_to_string(name: &X509Name) -> String {
    let mut ret = String::with_capacity(256);

    if let Some(cn) = name
        .iter_common_name()
        .next()
        .and_then(|cn| cn.as_str().ok())
    {
        write!(ret, "CN={cn}").unwrap();
    }

    let mut append_attrs = |attrs: Vec<&AttributeTypeAndValue>, label| {
        let str_parts = attrs
            .iter()
            .filter_map(|attr| attr.as_str().ok())
            .collect::<Vec<_>>()
            .join("/");
        if !str_parts.is_empty() {
            if !ret.is_empty() {
                ret.push(' ');
            }
            write!(ret, "{label}={str_parts}").unwrap();
        }
    };

    append_attrs(name.iter_organization().collect(), "O");
    append_attrs(name.iter_organizational_unit().collect(), "OU");

    ret
}

const HEADER: &str = r#"//! A compiled-in copy of the root certificates trusted by Mozilla.
//!
//! To use this library with rustls 0.22:
//!
//! ```rust
//! let root_store = rustls::RootCertStore {
//!   roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
//! };
//! ```
//!
//! This library is suitable for use in applications that can always be recompiled and instantly deployed.
//! For applications that are deployed to end-users and cannot be recompiled, or which need certification
//! before deployment, consider a library that uses the platform native certificate verifier such as
//! [rustls-platform-verifier]. This has the additional benefit of supporting OS provided CA constraints
//! and revocation data.
//!
//! [rustls-platform-verifier]: https://docs.rs/rustls-platform-verifier
//
// This library is automatically generated from the Mozilla
// IncludedCACertificateReportPEMCSV report via ccadb.org. Don't edit it.
//
// The generation is done deterministically so you can verify it
// yourself by inspecting and re-running the generation process.

#![no_std]
#![forbid(unsafe_code, unstable_features)]
#![deny(
    elided_lifetimes_in_paths,
    trivial_casts,
    trivial_numeric_casts,
    unused_import_braces,
    unused_extern_crates,
    unused_qualifications
)]

use pki_types::{Der, TrustAnchor};

"#;
