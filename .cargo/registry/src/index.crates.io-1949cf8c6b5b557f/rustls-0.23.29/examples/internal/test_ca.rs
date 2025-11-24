use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rcgen::string::Ia5String;
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, CertificateRevocationListParams,
    DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyIdMethod, KeyPair,
    KeyUsagePurpose, PKCS_ECDSA_P256_SHA256, PKCS_ECDSA_P384_SHA384, PKCS_ECDSA_P521_SHA512,
    PKCS_ED25519, PKCS_RSA_SHA256, PKCS_RSA_SHA384, PKCS_RSA_SHA512, RevocationReason,
    RevokedCertParams, RsaKeySize, SanType, SerialNumber, SignatureAlgorithm,
};
use time::OffsetDateTime;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut certified_keys = HashMap::<
        (Role, &'static SignatureAlgorithm),
        (Issuer<'static, KeyPair>, Certificate),
    >::with_capacity(ROLES.len() * SIG_ALGS.len());

    for role in ROLES {
        for alg in SIG_ALGS {
            // Generate a key pair and serialize it to a PEM encoded file.
            let key_pair = alg.key_pair();
            let mut key_pair_file = File::create(role.key_file_path(alg))?;
            key_pair_file.write_all(key_pair.serialize_pem().as_bytes())?;

            // Issue a certificate for the key pair. For trust anchors, this will be self-signed.
            // Otherwise we dig out the issuer and issuer_key for the issuer, which should have
            // been produced in earlier iterations based on the careful ordering of roles.
            let (params, cert) = match role {
                Role::TrustAnchor => {
                    let params = role.params(alg);
                    let cert = params.self_signed(&key_pair)?;
                    (params, cert)
                }
                Role::Intermediate => {
                    let issuer = certified_keys
                        .get(&(Role::TrustAnchor, alg.inner))
                        .unwrap();
                    let params = role.params(alg);
                    let cert = params.signed_by(&key_pair, &issuer.0)?;
                    (params, cert)
                }
                Role::EndEntity | Role::Client => {
                    let issuer = certified_keys
                        .get(&(Role::Intermediate, alg.inner))
                        .unwrap();
                    let params = role.params(alg);
                    let cert = params.signed_by(&key_pair, &issuer.0)?;
                    (params, cert)
                }
            };

            // Serialize the issued certificate to a PEM encoded file.
            let mut cert_file = File::create(role.cert_pem_file_path(alg))?;
            cert_file.write_all(cert.pem().as_bytes())?;
            // And to a DER encoded file.
            let mut cert_file = File::create(role.cert_der_file_path(alg))?;
            cert_file.write_all(cert.der())?;

            // If we're not a trust anchor, generate a CRL for the certificate we just issued.
            if role != Role::TrustAnchor {
                // The CRL will be signed by the issuer of the certificate being revoked. For
                // intermediates this will be the trust anchor, and for client/EE certs this will
                // be the intermediate.
                let issuer = match role {
                    Role::Intermediate => certified_keys
                        .get(&(Role::TrustAnchor, alg.inner))
                        .unwrap(),
                    Role::EndEntity | Role::Client => certified_keys
                        .get(&(Role::Intermediate, alg.inner))
                        .unwrap(),
                    _ => panic!("unexpected role for CRL generation: {role:?}"),
                };

                let revoked_crl =
                    crl_for_serial(params.serial_number.clone().unwrap()).signed_by(&issuer.0)?;
                let mut revoked_crl_file = File::create(
                    alg.output_directory()
                        .join(format!("{}.revoked.crl.pem", role.label())),
                )?;
                revoked_crl_file.write_all(revoked_crl.pem().unwrap().as_bytes())?;

                let expired_crl = expired_crl().signed_by(&issuer.0)?;
                let mut expired_crl_file = File::create(
                    alg.output_directory()
                        .join(format!("{}.expired.crl.pem", role.label())),
                )?;
                expired_crl_file.write_all(expired_crl.pem().unwrap().as_bytes())?;
            }

            // When we're issuing end entity or client certs we have a bit of extra work to do
            // now that we have full chains in hand.
            if matches!(role, Role::EndEntity | Role::Client) {
                let root = &certified_keys
                    .get(&(Role::TrustAnchor, alg.inner))
                    .unwrap()
                    .1;
                let intermediate = &certified_keys
                    .get(&(Role::Intermediate, alg.inner))
                    .unwrap()
                    .1;

                // Write the PEM chain and full chain files for the end entity and client certs.
                // Chain files include the intermediate and root certs, while full chain files include
                // the end entity or client cert as well.
                for f in [
                    ("chain", &[intermediate, root][..]),
                    ("fullchain", &[&cert, intermediate, root][..]),
                ] {
                    let mut chain_file = File::create(alg.output_directory().join(format!(
                        "{}.{}",
                        role.label(),
                        f.0
                    )))?;
                    for cert in f.1 {
                        chain_file.write_all(cert.pem().as_bytes())?;
                    }
                }

                // Write the PEM public key for the end entity and client.
                let mut raw_public_key_file = File::create(
                    alg.output_directory()
                        .join(format!("{}.spki.pem", role.label())),
                )?;
                raw_public_key_file.write_all(key_pair.public_key_pem().as_bytes())?;
            }

            certified_keys.insert((role, alg.inner), (Issuer::new(params, key_pair), cert));
        }
    }

    Ok(())
}

fn crl_for_serial(serial_number: SerialNumber) -> CertificateRevocationListParams {
    let now = OffsetDateTime::now_utc();
    CertificateRevocationListParams {
        this_update: now,
        next_update: now + Duration::from_secs(60 * 60 * 24 * 365 * 100), // 100 years
        crl_number: SerialNumber::from(1234),
        issuing_distribution_point: None,
        revoked_certs: vec![RevokedCertParams {
            serial_number,
            revocation_time: now,
            reason_code: Some(RevocationReason::KeyCompromise),
            invalidity_date: None,
        }],
        key_identifier_method: KeyIdMethod::Sha256,
    }
}

fn expired_crl() -> CertificateRevocationListParams {
    let now = OffsetDateTime::now_utc();
    CertificateRevocationListParams {
        this_update: now - Duration::from_secs(60),
        next_update: now,
        crl_number: SerialNumber::from(1234),
        issuing_distribution_point: None,
        revoked_certs: vec![],
        key_identifier_method: KeyIdMethod::Sha256,
    }
}

// Note: these are ordered such that the data dependencies for issuance are satisfied.
const ROLES: [Role; 4] = [
    Role::TrustAnchor,
    Role::Intermediate,
    Role::EndEntity,
    Role::Client,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum Role {
    Client,
    EndEntity,
    Intermediate,
    TrustAnchor,
}

impl Role {
    fn params(&self, alg: &'static SigAlgContext) -> CertificateParams {
        let mut params = CertificateParams::default();
        params.distinguished_name = self.common_name(alg);
        params.use_authority_key_identifier_extension = true;
        let serial = SERIAL_NUMBER.fetch_add(1, Ordering::SeqCst);
        params.serial_number = Some(SerialNumber::from_slice(&serial.to_be_bytes()[..]));

        match self {
            Self::TrustAnchor | Self::Intermediate => {
                params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
                params.key_usages = ISSUER_KEY_USAGES.to_vec();
                params.extended_key_usages = ISSUER_EXTENDED_KEY_USAGES.to_vec();
            }
            Self::EndEntity | Self::Client => {
                params.is_ca = IsCa::NoCa;
                params.key_usages = EE_KEY_USAGES.to_vec();
                params.subject_alt_names = vec![
                    SanType::DnsName(Ia5String::try_from("testserver.com".to_string()).unwrap()),
                    SanType::DnsName(
                        Ia5String::try_from("second.testserver.com".to_string()).unwrap(),
                    ),
                    SanType::DnsName(Ia5String::try_from("localhost".to_string()).unwrap()),
                    SanType::IpAddress(IpAddr::from_str("198.51.100.1").unwrap()),
                    SanType::IpAddress(IpAddr::from_str("2001:db8::1").unwrap()),
                ];
            }
        }

        // Client certificates additionally get the client auth EKU.
        if *self == Self::Client {
            params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
        }

        params
    }

    fn common_name(&self, alg: &'static SigAlgContext) -> DistinguishedName {
        let mut distinguished_name = DistinguishedName::new();
        distinguished_name.push(
            DnType::CommonName,
            match self {
                Self::Client => "ponytown client".to_owned(),
                Self::EndEntity => "testserver.com".to_owned(),
                Self::Intermediate => {
                    format!("ponytown {} level 2 intermediate", alg.issuer_cn)
                }
                Self::TrustAnchor => format!("ponytown {} CA", alg.issuer_cn),
            },
        );
        distinguished_name
    }

    fn key_file_path(&self, alg: &'static SigAlgContext) -> PathBuf {
        alg.output_directory()
            .join(format!("{}.key", self.label()))
    }

    fn cert_pem_file_path(&self, alg: &'static SigAlgContext) -> PathBuf {
        alg.output_directory()
            .join(format!("{}.cert", self.label()))
    }

    fn cert_der_file_path(&self, alg: &'static SigAlgContext) -> PathBuf {
        alg.output_directory()
            .join(format!("{}.der", self.label()))
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::EndEntity => "end",
            Self::Intermediate => "inter",
            Self::TrustAnchor => "ca",
        }
    }
}

// Note: for convenience we use the RSA sigalg digest algorithm to inform the RSA modulus
//   size, mapping SHA256 to RSA 2048, SHA384 to RSA 3072, and SHA512 to RSA 4096.
static SIG_ALGS: &[SigAlgContext] = &[
    SigAlgContext {
        inner: &PKCS_RSA_SHA256,
        issuer_cn: "RSA 2048",
    },
    SigAlgContext {
        inner: &PKCS_RSA_SHA384,
        issuer_cn: "RSA 3072",
    },
    SigAlgContext {
        inner: &PKCS_RSA_SHA512,
        issuer_cn: "RSA 4096",
    },
    SigAlgContext {
        inner: &PKCS_ECDSA_P256_SHA256,
        issuer_cn: "ECDSA p256",
    },
    SigAlgContext {
        inner: &PKCS_ECDSA_P384_SHA384,
        issuer_cn: "ECDSA p384",
    },
    SigAlgContext {
        inner: &PKCS_ECDSA_P521_SHA512,
        issuer_cn: "ECDSA p521",
    },
    SigAlgContext {
        inner: &PKCS_ED25519,
        issuer_cn: "EdDSA",
    },
];

struct SigAlgContext {
    pub(crate) inner: &'static SignatureAlgorithm,
    pub(crate) issuer_cn: &'static str,
}

impl SigAlgContext {
    fn output_directory(&self) -> PathBuf {
        let output_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../")
            .join("test-ca")
            .join(
                self.issuer_cn
                    .to_lowercase()
                    .replace(' ', "-"),
            );
        fs::create_dir_all(&output_dir).unwrap();
        output_dir
    }

    fn key_pair(&self) -> KeyPair {
        if *self.inner == PKCS_RSA_SHA256 {
            KeyPair::generate_rsa_for(&PKCS_RSA_SHA256, RsaKeySize::_2048)
        } else if *self.inner == PKCS_RSA_SHA384 {
            KeyPair::generate_rsa_for(&PKCS_RSA_SHA384, RsaKeySize::_3072)
        } else if *self.inner == PKCS_RSA_SHA512 {
            KeyPair::generate_rsa_for(&PKCS_RSA_SHA512, RsaKeySize::_4096)
        } else {
            KeyPair::generate_for(self.inner)
        }
        .unwrap()
    }
}

const ISSUER_KEY_USAGES: &[KeyUsagePurpose; 7] = &[
    KeyUsagePurpose::CrlSign,
    KeyUsagePurpose::KeyCertSign,
    KeyUsagePurpose::DigitalSignature,
    KeyUsagePurpose::ContentCommitment,
    KeyUsagePurpose::KeyEncipherment,
    KeyUsagePurpose::DataEncipherment,
    KeyUsagePurpose::KeyAgreement,
];

const ISSUER_EXTENDED_KEY_USAGES: &[ExtendedKeyUsagePurpose; 2] = &[
    ExtendedKeyUsagePurpose::ServerAuth,
    ExtendedKeyUsagePurpose::ClientAuth,
];

const EE_KEY_USAGES: &[KeyUsagePurpose; 2] = &[
    KeyUsagePurpose::DigitalSignature,
    KeyUsagePurpose::ContentCommitment,
];

static SERIAL_NUMBER: AtomicU64 = AtomicU64::new(1);
