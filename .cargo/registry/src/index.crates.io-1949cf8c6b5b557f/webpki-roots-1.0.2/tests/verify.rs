use core::time::Duration;
use std::convert::TryFrom;

use pki_types::{CertificateDer, ServerName, SignatureVerificationAlgorithm, UnixTime};
use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair, KeyUsagePurpose};
use webpki::{anchor_from_trusted_cert, EndEntityCert, Error, KeyUsage};
use x509_parser::extensions::{GeneralName, NameConstraints as X509ParserNameConstraints};
use x509_parser::prelude::FromDer;

use webpki_roots::TLS_SERVER_ROOTS;

#[test]
fn name_constraints() {
    for name_constraints in TLS_SERVER_ROOTS
        .iter()
        .filter_map(|ta| ta.name_constraints.as_ref())
    {
        let time = UnixTime::since_unix_epoch(Duration::from_secs(0x40000000)); // Time matching rcgen default.
        let test_case = ConstraintTest::new(name_constraints.as_ref());
        let trust_anchors = &[anchor_from_trusted_cert(&test_case.trust_anchor).unwrap()];

        // Each permitted EE should verify without error.
        for permitted_ee in test_case.permitted_certs {
            webpki::EndEntityCert::try_from(&permitted_ee)
                .unwrap()
                .verify_for_usage(
                    ALL_ALGORITHMS,
                    trust_anchors,
                    &[],
                    time,
                    KeyUsage::server_auth(),
                    None,
                    None,
                )
                .unwrap();
        }

        // Each forbidden EE should fail to verify with the expected name constraint error.
        for forbidden_ee in test_case.forbidden_certs {
            let ee = EndEntityCert::try_from(&forbidden_ee).unwrap();
            let result = ee.verify_for_usage(
                ALL_ALGORITHMS,
                trust_anchors,
                &[],
                time,
                KeyUsage::server_auth(),
                None,
                None,
            );
            assert!(matches!(result, Err(Error::NameConstraintViolation)));
        }
    }
}

struct ConstraintTest {
    trust_anchor: CertificateDer<'static>,
    permitted_certs: Vec<CertificateDer<'static>>,
    forbidden_certs: Vec<CertificateDer<'static>>,
}

impl ConstraintTest {
    fn new(webpki_name_constraints: &[u8]) -> Self {
        let name_constraints = rcgen_name_constraints(webpki_name_constraints);

        // Create a trust anchor CA certificate that has the name constraints we want to test.
        let trust_anchor = {
            let mut params = CertificateParams::new([]).unwrap();
            params
                .distinguished_name
                .push(DnType::CommonName, "Name Constraint Test CA");
            params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
            params.key_usages = vec![
                KeyUsagePurpose::KeyCertSign,
                KeyUsagePurpose::DigitalSignature,
            ];
            params.name_constraints = Some(name_constraints.clone());

            let key = KeyPair::generate().unwrap();
            let cert = params.self_signed(&key).unwrap();
            let issuer = Issuer::new(params, key);
            (issuer, cert)
        };

        let certs_for_subtrees = |suffix| {
            name_constraints
                .permitted_subtrees
                .iter()
                .filter_map(|subtree| match subtree {
                    rcgen::GeneralSubtree::DnsName(dns_name) => Some(rcgen_ee_for_name(
                        format!("valid{dns_name}{suffix}"),
                        &trust_anchor.0,
                    )),
                    _ => None,
                })
                .collect()
        };

        Self {
            // For each permitted subtree in the name constraints, issue an end entity certificate
            // that contains a DNS name matching the permitted subtree base.
            permitted_certs: certs_for_subtrees(""),
            // For each permitted subtree in the name constraints, issue an end entity certificate
            // that contains a DNS name that will **not** match the permitted subtree base.
            forbidden_certs: certs_for_subtrees(".invalid"),
            trust_anchor: trust_anchor.1.into(),
        }
    }
}

fn rcgen_ee_for_name(name: String, issuer: &Issuer<'_, KeyPair>) -> CertificateDer<'static> {
    let mut ee = CertificateParams::new(vec![name.clone()]).unwrap();
    ee.distinguished_name.push(DnType::CommonName, name);
    ee.is_ca = IsCa::NoCa;
    let key_pair = KeyPair::generate().unwrap();
    ee.signed_by(&key_pair, issuer).unwrap().into()
}

/// Convert the webpki trust anchor DER encoding of name constraints to rcgen NameConstraints.
fn rcgen_name_constraints(der: &[u8]) -> rcgen::NameConstraints {
    // x509 parser expects the outer SEQUENCE that the webpki trust anchor representation elides
    // so wrap the DER up.
    let wrapped_der = yasna::construct_der(|writer| {
        writer.write_sequence(|writer| {
            writer.next().write_der(der);
        })
    });

    // Constraints should parse with no trailing data.
    let (trailing, constraints) = X509ParserNameConstraints::from_der(&wrapped_der).unwrap();
    assert!(
        trailing.is_empty(),
        "unexpected trailing DER in name constraint"
    );

    // There should be at least one permitted subtree.
    assert!(
        constraints.permitted_subtrees.is_some(),
        "empty permitted subtrees in constraints"
    );

    // We don't expect any excluded subtrees as this time.
    assert!(constraints.excluded_subtrees.is_none());

    // Collect all of the DNS names from the x509-parser representation, mapping to the rcgen
    // representation usable in cert parameters. We don't expect to find any other types of general
    // name and x509-parser doesn't parse the subtree minimum and maximum (which we would assert to
    // be missing for proper encoding anyway).
    let permitted_subtrees = match constraints.permitted_subtrees {
        None => Vec::default(),
        Some(subtrees) => subtrees
            .iter()
            .map(|subtree| match &subtree.base {
                GeneralName::DNSName(base) => rcgen::GeneralSubtree::DnsName(base.to_string()),
                name => panic!("unexpected subtree base general name type: {name}"),
            })
            .collect(),
    };

    rcgen::NameConstraints {
        permitted_subtrees,
        excluded_subtrees: Vec::default(),
    }
}

#[test]
fn tubitak_name_constraint_works() {
    let root = CertificateDer::from(&include_bytes!("data/tubitak/root.der")[..]);
    let inter = CertificateDer::from(&include_bytes!("data/tubitak/inter.der")[..]);
    let subj = CertificateDer::from(&include_bytes!("data/tubitak/subj.der")[..]);

    let roots = [anchor_from_trusted_cert(&root).unwrap().to_owned()];
    let now = UnixTime::since_unix_epoch(Duration::from_secs(1493668479));
    let cert = EndEntityCert::try_from(&subj).unwrap();
    cert.verify_for_usage(
        ALL_ALGORITHMS,
        &roots,
        &[inter, root],
        now,
        KeyUsage::server_auth(),
        None,
        None,
    )
    .unwrap();

    let subject = ServerName::try_from("testssl.kamusm.gov.tr").unwrap();
    cert.verify_is_valid_for_subject_name(&subject).unwrap();
}

static ALL_ALGORITHMS: &[&dyn SignatureVerificationAlgorithm] = &[
    webpki::ring::ECDSA_P256_SHA256,
    webpki::ring::ECDSA_P256_SHA384,
    webpki::ring::ECDSA_P384_SHA256,
    webpki::ring::ECDSA_P384_SHA384,
    webpki::ring::RSA_PKCS1_2048_8192_SHA256,
    webpki::ring::RSA_PKCS1_2048_8192_SHA384,
    webpki::ring::RSA_PKCS1_2048_8192_SHA512,
    webpki::ring::RSA_PKCS1_3072_8192_SHA384,
    webpki::ring::RSA_PSS_2048_8192_SHA256_LEGACY_KEY,
    webpki::ring::RSA_PSS_2048_8192_SHA384_LEGACY_KEY,
    webpki::ring::RSA_PSS_2048_8192_SHA512_LEGACY_KEY,
];
