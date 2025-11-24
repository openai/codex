use alloc::vec::Vec;
use core::fmt;

use pki_types::CertificateRevocationListDer;
use webpki::{CertRevocationList, InvalidNameContext, OwnedCertRevocationList};

use crate::error::{
    CertRevocationListError, CertificateError, Error, ExtendedKeyPurpose, OtherError,
};
#[cfg(feature = "std")]
use crate::sync::Arc;

mod anchors;
mod client_verifier;
mod server_verifier;
mod verify;

pub use anchors::RootCertStore;
pub use client_verifier::{ClientCertVerifierBuilder, WebPkiClientVerifier};
pub use server_verifier::{ServerCertVerifierBuilder, WebPkiServerVerifier};
// Conditionally exported from crate.
#[allow(unreachable_pub)]
pub use verify::{
    ParsedCertificate, verify_server_cert_signed_by_trust_anchor, verify_server_name,
};
pub use verify::{
    WebPkiSupportedAlgorithms, verify_tls12_signature, verify_tls13_signature,
    verify_tls13_signature_with_raw_key,
};

/// An error that can occur when building a certificate verifier.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum VerifierBuilderError {
    /// No root trust anchors were provided.
    NoRootAnchors,
    /// A provided CRL could not be parsed.
    InvalidCrl(CertRevocationListError),
}

impl From<CertRevocationListError> for VerifierBuilderError {
    fn from(value: CertRevocationListError) -> Self {
        Self::InvalidCrl(value)
    }
}

impl fmt::Display for VerifierBuilderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRootAnchors => write!(f, "no root trust anchors were provided"),
            Self::InvalidCrl(e) => write!(f, "provided CRL could not be parsed: {e:?}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for VerifierBuilderError {}

fn pki_error(error: webpki::Error) -> Error {
    use webpki::Error::*;
    match error {
        BadDer | BadDerTime | TrailingData(_) => CertificateError::BadEncoding.into(),
        CertNotValidYet { time, not_before } => {
            CertificateError::NotValidYetContext { time, not_before }.into()
        }
        CertExpired { time, not_after } => {
            CertificateError::ExpiredContext { time, not_after }.into()
        }
        InvalidCertValidity => CertificateError::Expired.into(),
        UnknownIssuer => CertificateError::UnknownIssuer.into(),
        CertNotValidForName(InvalidNameContext {
            expected,
            presented,
        }) => CertificateError::NotValidForNameContext {
            expected,
            presented,
        }
        .into(),
        CertRevoked => CertificateError::Revoked.into(),
        UnknownRevocationStatus => CertificateError::UnknownRevocationStatus.into(),
        CrlExpired { time, next_update } => {
            CertificateError::ExpiredRevocationListContext { time, next_update }.into()
        }
        IssuerNotCrlSigner => CertRevocationListError::IssuerInvalidForCrl.into(),

        InvalidSignatureForPublicKey => CertificateError::BadSignature.into(),
        #[allow(deprecated)]
        UnsupportedSignatureAlgorithm | UnsupportedSignatureAlgorithmForPublicKey => {
            CertificateError::UnsupportedSignatureAlgorithm.into()
        }
        UnsupportedSignatureAlgorithmContext(cx) => {
            CertificateError::UnsupportedSignatureAlgorithmContext {
                signature_algorithm_id: cx.signature_algorithm_id,
                supported_algorithms: cx.supported_algorithms,
            }
            .into()
        }
        UnsupportedSignatureAlgorithmForPublicKeyContext(cx) => {
            CertificateError::UnsupportedSignatureAlgorithmForPublicKeyContext {
                signature_algorithm_id: cx.signature_algorithm_id,
                public_key_algorithm_id: cx.public_key_algorithm_id,
            }
            .into()
        }

        InvalidCrlSignatureForPublicKey => CertRevocationListError::BadSignature.into(),
        #[allow(deprecated)]
        UnsupportedCrlSignatureAlgorithm | UnsupportedCrlSignatureAlgorithmForPublicKey => {
            CertRevocationListError::UnsupportedSignatureAlgorithm.into()
        }
        UnsupportedCrlSignatureAlgorithmContext(cx) => {
            CertRevocationListError::UnsupportedSignatureAlgorithmContext {
                signature_algorithm_id: cx.signature_algorithm_id,
                supported_algorithms: cx.supported_algorithms,
            }
            .into()
        }
        UnsupportedCrlSignatureAlgorithmForPublicKeyContext(cx) => {
            CertRevocationListError::UnsupportedSignatureAlgorithmForPublicKeyContext {
                signature_algorithm_id: cx.signature_algorithm_id,
                public_key_algorithm_id: cx.public_key_algorithm_id,
            }
            .into()
        }

        #[allow(deprecated)]
        RequiredEkuNotFound => CertificateError::InvalidPurpose.into(),
        RequiredEkuNotFoundContext(webpki::RequiredEkuNotFoundContext { required, present }) => {
            CertificateError::InvalidPurposeContext {
                required: ExtendedKeyPurpose::for_values(required.oid_values()),
                presented: present
                    .into_iter()
                    .map(|eku| ExtendedKeyPurpose::for_values(eku.into_iter()))
                    .collect(),
            }
            .into()
        }

        _ => CertificateError::Other(OtherError(
            #[cfg(feature = "std")]
            Arc::new(error),
        ))
        .into(),
    }
}

fn crl_error(e: webpki::Error) -> CertRevocationListError {
    use webpki::Error::*;
    match e {
        InvalidCrlSignatureForPublicKey => CertRevocationListError::BadSignature,
        #[allow(deprecated)]
        UnsupportedCrlSignatureAlgorithm | UnsupportedCrlSignatureAlgorithmForPublicKey => {
            CertRevocationListError::UnsupportedSignatureAlgorithm
        }
        UnsupportedCrlSignatureAlgorithmContext(cx) => {
            CertRevocationListError::UnsupportedSignatureAlgorithmContext {
                signature_algorithm_id: cx.signature_algorithm_id,
                supported_algorithms: cx.supported_algorithms,
            }
        }
        UnsupportedSignatureAlgorithmForPublicKeyContext(cx) => {
            CertRevocationListError::UnsupportedSignatureAlgorithmForPublicKeyContext {
                signature_algorithm_id: cx.signature_algorithm_id,
                public_key_algorithm_id: cx.public_key_algorithm_id,
            }
        }
        InvalidCrlNumber => CertRevocationListError::InvalidCrlNumber,
        InvalidSerialNumber => CertRevocationListError::InvalidRevokedCertSerialNumber,
        IssuerNotCrlSigner => CertRevocationListError::IssuerInvalidForCrl,
        MalformedExtensions | BadDer | BadDerTime => CertRevocationListError::ParseError,
        UnsupportedCriticalExtension => CertRevocationListError::UnsupportedCriticalExtension,
        UnsupportedCrlVersion => CertRevocationListError::UnsupportedCrlVersion,
        UnsupportedDeltaCrl => CertRevocationListError::UnsupportedDeltaCrl,
        UnsupportedIndirectCrl => CertRevocationListError::UnsupportedIndirectCrl,
        UnsupportedRevocationReason => CertRevocationListError::UnsupportedRevocationReason,

        _ => CertRevocationListError::Other(OtherError(
            #[cfg(feature = "std")]
            Arc::new(e),
        )),
    }
}

fn parse_crls(
    crls: Vec<CertificateRevocationListDer<'_>>,
) -> Result<Vec<CertRevocationList<'_>>, CertRevocationListError> {
    crls.iter()
        .map(|der| OwnedCertRevocationList::from_der(der.as_ref()).map(Into::into))
        .collect::<Result<Vec<_>, _>>()
        .map_err(crl_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn pki_crl_errors() {
        // CRL signature errors should be turned into BadSignature.
        assert_eq!(
            pki_error(webpki::Error::InvalidCrlSignatureForPublicKey),
            Error::InvalidCertRevocationList(CertRevocationListError::BadSignature),
        );

        #[allow(deprecated)]
        {
            assert_eq!(
                pki_error(webpki::Error::UnsupportedCrlSignatureAlgorithm),
                Error::InvalidCertRevocationList(
                    CertRevocationListError::UnsupportedSignatureAlgorithm
                ),
            );
            assert_eq!(
                pki_error(webpki::Error::UnsupportedCrlSignatureAlgorithmForPublicKey),
                Error::InvalidCertRevocationList(
                    CertRevocationListError::UnsupportedSignatureAlgorithm
                ),
            );
        }

        assert_eq!(
            pki_error(webpki::Error::UnsupportedCrlSignatureAlgorithmContext(
                webpki::UnsupportedSignatureAlgorithmContext {
                    signature_algorithm_id: vec![],
                    supported_algorithms: vec![],
                }
            )),
            Error::InvalidCertRevocationList(
                CertRevocationListError::UnsupportedSignatureAlgorithmContext {
                    signature_algorithm_id: vec![],
                    supported_algorithms: vec![],
                }
            )
        );
        assert_eq!(
            pki_error(
                webpki::Error::UnsupportedCrlSignatureAlgorithmForPublicKeyContext(
                    webpki::UnsupportedSignatureAlgorithmForPublicKeyContext {
                        signature_algorithm_id: vec![],
                        public_key_algorithm_id: vec![],
                    }
                )
            ),
            Error::InvalidCertRevocationList(
                CertRevocationListError::UnsupportedSignatureAlgorithmForPublicKeyContext {
                    signature_algorithm_id: vec![],
                    public_key_algorithm_id: vec![],
                }
            )
        );

        // Revoked cert errors should be turned into Revoked.
        assert_eq!(
            pki_error(webpki::Error::CertRevoked),
            Error::InvalidCertificate(CertificateError::Revoked),
        );

        // Issuer not CRL signer errors should be turned into IssuerInvalidForCrl
        assert_eq!(
            pki_error(webpki::Error::IssuerNotCrlSigner),
            Error::InvalidCertRevocationList(CertRevocationListError::IssuerInvalidForCrl)
        );
    }

    #[test]
    fn crl_error_from_webpki() {
        use CertRevocationListError::*;
        #[allow(deprecated)]
        let testcases = &[
            (webpki::Error::InvalidCrlSignatureForPublicKey, BadSignature),
            (
                webpki::Error::UnsupportedCrlSignatureAlgorithm,
                UnsupportedSignatureAlgorithm,
            ),
            (
                webpki::Error::UnsupportedCrlSignatureAlgorithmForPublicKey,
                UnsupportedSignatureAlgorithm,
            ),
            (webpki::Error::InvalidCrlNumber, InvalidCrlNumber),
            (
                webpki::Error::InvalidSerialNumber,
                InvalidRevokedCertSerialNumber,
            ),
            (webpki::Error::IssuerNotCrlSigner, IssuerInvalidForCrl),
            (webpki::Error::MalformedExtensions, ParseError),
            (webpki::Error::BadDer, ParseError),
            (webpki::Error::BadDerTime, ParseError),
            (
                webpki::Error::UnsupportedCriticalExtension,
                UnsupportedCriticalExtension,
            ),
            (webpki::Error::UnsupportedCrlVersion, UnsupportedCrlVersion),
            (webpki::Error::UnsupportedDeltaCrl, UnsupportedDeltaCrl),
            (
                webpki::Error::UnsupportedIndirectCrl,
                UnsupportedIndirectCrl,
            ),
            (
                webpki::Error::UnsupportedRevocationReason,
                UnsupportedRevocationReason,
            ),
        ];
        for t in testcases {
            assert_eq!(crl_error(t.0.clone()), t.1);
        }

        assert!(matches!(
            crl_error(webpki::Error::NameConstraintViolation),
            Other(..)
        ));
    }
}
