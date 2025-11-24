use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
#[cfg(feature = "std")]
use std::time::SystemTimeError;

use pki_types::{AlgorithmIdentifier, ServerName, UnixTime};
use webpki::KeyUsage;

use crate::enums::{AlertDescription, ContentType, HandshakeType};
use crate::msgs::handshake::{EchConfigPayload, KeyExchangeAlgorithm};
use crate::rand;

/// rustls reports protocol errors using this type.
#[non_exhaustive]
#[derive(Debug, PartialEq, Clone)]
pub enum Error {
    /// We received a TLS message that isn't valid right now.
    /// `expect_types` lists the message types we can expect right now.
    /// `got_type` is the type we found.  This error is typically
    /// caused by a buggy TLS stack (the peer or this one), a broken
    /// network, or an attack.
    InappropriateMessage {
        /// Which types we expected
        expect_types: Vec<ContentType>,
        /// What type we received
        got_type: ContentType,
    },

    /// We received a TLS handshake message that isn't valid right now.
    /// `expect_types` lists the handshake message types we can expect
    /// right now.  `got_type` is the type we found.
    InappropriateHandshakeMessage {
        /// Which handshake type we expected
        expect_types: Vec<HandshakeType>,
        /// What handshake type we received
        got_type: HandshakeType,
    },

    /// An error occurred while handling Encrypted Client Hello (ECH).
    InvalidEncryptedClientHello(EncryptedClientHelloError),

    /// The peer sent us a TLS message with invalid contents.
    InvalidMessage(InvalidMessage),

    /// The peer didn't give us any certificates.
    NoCertificatesPresented,

    /// The certificate verifier doesn't support the given type of name.
    UnsupportedNameType,

    /// We couldn't decrypt a message.  This is invariably fatal.
    DecryptError,

    /// We couldn't encrypt a message because it was larger than the allowed message size.
    /// This should never happen if the application is using valid record sizes.
    EncryptError,

    /// The peer doesn't support a protocol version/feature we require.
    /// The parameter gives a hint as to what version/feature it is.
    PeerIncompatible(PeerIncompatible),

    /// The peer deviated from the standard TLS protocol.
    /// The parameter gives a hint where.
    PeerMisbehaved(PeerMisbehaved),

    /// We received a fatal alert.  This means the peer is unhappy.
    AlertReceived(AlertDescription),

    /// We saw an invalid certificate.
    ///
    /// The contained error is from the certificate validation trait
    /// implementation.
    InvalidCertificate(CertificateError),

    /// A provided certificate revocation list (CRL) was invalid.
    InvalidCertRevocationList(CertRevocationListError),

    /// A catch-all error for unlikely errors.
    General(String),

    /// We failed to figure out what time it currently is.
    FailedToGetCurrentTime,

    /// We failed to acquire random bytes from the system.
    FailedToGetRandomBytes,

    /// This function doesn't work until the TLS handshake
    /// is complete.
    HandshakeNotComplete,

    /// The peer sent an oversized record/fragment.
    PeerSentOversizedRecord,

    /// An incoming connection did not support any known application protocol.
    NoApplicationProtocol,

    /// The `max_fragment_size` value supplied in configuration was too small,
    /// or too large.
    BadMaxFragmentSize,

    /// Specific failure cases from [`keys_match`] or a [`crate::crypto::signer::SigningKey`] that cannot produce a corresponding public key.
    ///
    /// [`keys_match`]: crate::crypto::signer::CertifiedKey::keys_match
    InconsistentKeys(InconsistentKeys),

    /// Any other error.
    ///
    /// This variant should only be used when the error is not better described by a more
    /// specific variant. For example, if a custom crypto provider returns a
    /// provider specific error.
    ///
    /// Enums holding this variant will never compare equal to each other.
    Other(OtherError),
}

/// Specific failure cases from [`keys_match`] or a [`crate::crypto::signer::SigningKey`] that cannot produce a corresponding public key.
///
/// [`keys_match`]: crate::crypto::signer::CertifiedKey::keys_match
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InconsistentKeys {
    /// The public key returned by the [`SigningKey`] does not match the public key information in the certificate.
    ///
    /// [`SigningKey`]: crate::crypto::signer::SigningKey
    KeyMismatch,

    /// The [`SigningKey`] cannot produce its corresponding public key.
    ///
    /// [`SigningKey`]: crate::crypto::signer::SigningKey
    Unknown,
}

impl From<InconsistentKeys> for Error {
    #[inline]
    fn from(e: InconsistentKeys) -> Self {
        Self::InconsistentKeys(e)
    }
}

/// A corrupt TLS message payload that resulted in an error.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InvalidMessage {
    /// A certificate payload exceeded rustls's 64KB limit
    CertificatePayloadTooLarge,
    /// An advertised message was larger then expected.
    HandshakePayloadTooLarge,
    /// The peer sent us a syntactically incorrect ChangeCipherSpec payload.
    InvalidCcs,
    /// An unknown content type was encountered during message decoding.
    InvalidContentType,
    /// A peer sent an invalid certificate status type
    InvalidCertificateStatusType,
    /// Context was incorrectly attached to a certificate request during a handshake.
    InvalidCertRequest,
    /// A peer's DH params could not be decoded
    InvalidDhParams,
    /// A message was zero-length when its record kind forbids it.
    InvalidEmptyPayload,
    /// A peer sent an unexpected key update request.
    InvalidKeyUpdate,
    /// A peer's server name could not be decoded
    InvalidServerName,
    /// A TLS message payload was larger then allowed by the specification.
    MessageTooLarge,
    /// Message is shorter than the expected length
    MessageTooShort,
    /// Missing data for the named handshake payload value
    MissingData(&'static str),
    /// A peer did not advertise its supported key exchange groups.
    MissingKeyExchange,
    /// A peer sent an empty list of signature schemes
    NoSignatureSchemes,
    /// Trailing data found for the named handshake payload value
    TrailingData(&'static str),
    /// A peer sent an unexpected message type.
    UnexpectedMessage(&'static str),
    /// An unknown TLS protocol was encountered during message decoding.
    UnknownProtocolVersion,
    /// A peer sent a non-null compression method.
    UnsupportedCompression,
    /// A peer sent an unknown elliptic curve type.
    UnsupportedCurveType,
    /// A peer sent an unsupported key exchange algorithm.
    UnsupportedKeyExchangeAlgorithm(KeyExchangeAlgorithm),
    /// A server sent an empty ticket
    EmptyTicketValue,
    /// A peer sent an empty list of items, but a non-empty list is required.
    ///
    /// The argument names the context.
    IllegalEmptyList(&'static str),
    /// A peer sent an empty value, but a non-empty value is required.
    IllegalEmptyValue,
    /// A peer sent a message where a given extension type was repeated
    DuplicateExtension(u16),
    /// A peer sent a message with a PSK offer extension in wrong position
    PreSharedKeyIsNotFinalExtension,
    /// A server sent a HelloRetryRequest with an unknown extension
    UnknownHelloRetryRequestExtension,
    /// The peer sent a TLS1.3 Certificate with an unknown extension
    UnknownCertificateExtension,
}

impl From<InvalidMessage> for Error {
    #[inline]
    fn from(e: InvalidMessage) -> Self {
        Self::InvalidMessage(e)
    }
}

impl From<InvalidMessage> for AlertDescription {
    fn from(e: InvalidMessage) -> Self {
        match e {
            InvalidMessage::PreSharedKeyIsNotFinalExtension => Self::IllegalParameter,
            InvalidMessage::DuplicateExtension(_) => Self::IllegalParameter,
            InvalidMessage::UnknownHelloRetryRequestExtension => Self::UnsupportedExtension,
            _ => Self::DecodeError,
        }
    }
}

#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Clone)]
/// The set of cases where we failed to make a connection because we thought
/// the peer was misbehaving.
///
/// This is `non_exhaustive`: we might add or stop using items here in minor
/// versions.  We also don't document what they mean.  Generally a user of
/// rustls shouldn't vary its behaviour on these error codes, and there is
/// nothing it can do to improve matters.
///
/// Please file a bug against rustls if you see `Error::PeerMisbehaved` in
/// the wild.
pub enum PeerMisbehaved {
    AttemptedDowngradeToTls12WhenTls13IsSupported,
    BadCertChainExtensions,
    DisallowedEncryptedExtension,
    DuplicateClientHelloExtensions,
    DuplicateEncryptedExtensions,
    DuplicateHelloRetryRequestExtensions,
    DuplicateNewSessionTicketExtensions,
    DuplicateServerHelloExtensions,
    DuplicateServerNameTypes,
    EarlyDataAttemptedInSecondClientHello,
    EarlyDataExtensionWithoutResumption,
    EarlyDataOfferedWithVariedCipherSuite,
    HandshakeHashVariedAfterRetry,
    IllegalHelloRetryRequestWithEmptyCookie,
    IllegalHelloRetryRequestWithNoChanges,
    IllegalHelloRetryRequestWithOfferedGroup,
    IllegalHelloRetryRequestWithUnofferedCipherSuite,
    IllegalHelloRetryRequestWithUnofferedNamedGroup,
    IllegalHelloRetryRequestWithUnsupportedVersion,
    IllegalHelloRetryRequestWithWrongSessionId,
    IllegalHelloRetryRequestWithInvalidEch,
    IllegalMiddleboxChangeCipherSpec,
    IllegalTlsInnerPlaintext,
    IncorrectBinder,
    InvalidCertCompression,
    InvalidMaxEarlyDataSize,
    InvalidKeyShare,
    KeyEpochWithPendingFragment,
    KeyUpdateReceivedInQuicConnection,
    MessageInterleavedWithHandshakeMessage,
    MissingBinderInPskExtension,
    MissingKeyShare,
    MissingPskModesExtension,
    MissingQuicTransportParameters,
    OfferedDuplicateCertificateCompressions,
    OfferedDuplicateKeyShares,
    OfferedEarlyDataWithOldProtocolVersion,
    OfferedEmptyApplicationProtocol,
    OfferedIncorrectCompressions,
    PskExtensionMustBeLast,
    PskExtensionWithMismatchedIdsAndBinders,
    RefusedToFollowHelloRetryRequest,
    RejectedEarlyDataInterleavedWithHandshakeMessage,
    ResumptionAttemptedWithVariedEms,
    ResumptionOfferedWithVariedCipherSuite,
    ResumptionOfferedWithVariedEms,
    ResumptionOfferedWithIncompatibleCipherSuite,
    SelectedDifferentCipherSuiteAfterRetry,
    SelectedInvalidPsk,
    SelectedTls12UsingTls13VersionExtension,
    SelectedUnofferedApplicationProtocol,
    SelectedUnofferedCertCompression,
    SelectedUnofferedCipherSuite,
    SelectedUnofferedCompression,
    SelectedUnofferedKxGroup,
    SelectedUnofferedPsk,
    SelectedUnusableCipherSuiteForVersion,
    ServerEchoedCompatibilitySessionId,
    ServerHelloMustOfferUncompressedEcPoints,
    ServerNameDifferedOnRetry,
    ServerNameMustContainOneHostName,
    SignedKxWithWrongAlgorithm,
    SignedHandshakeWithUnadvertisedSigScheme,
    TooManyEmptyFragments,
    TooManyKeyUpdateRequests,
    TooManyRenegotiationRequests,
    TooManyWarningAlertsReceived,
    TooMuchEarlyDataReceived,
    UnexpectedCleartextExtension,
    UnsolicitedCertExtension,
    UnsolicitedEncryptedExtension,
    UnsolicitedSctList,
    UnsolicitedServerHelloExtension,
    WrongGroupForKeyShare,
    UnsolicitedEchExtension,
}

impl From<PeerMisbehaved> for Error {
    #[inline]
    fn from(e: PeerMisbehaved) -> Self {
        Self::PeerMisbehaved(e)
    }
}

#[non_exhaustive]
#[allow(missing_docs)]
#[derive(Debug, PartialEq, Clone)]
/// The set of cases where we failed to make a connection because a peer
/// doesn't support a TLS version/feature we require.
///
/// This is `non_exhaustive`: we might add or stop using items here in minor
/// versions.
pub enum PeerIncompatible {
    EcPointsExtensionRequired,
    ExtendedMasterSecretExtensionRequired,
    IncorrectCertificateTypeExtension,
    KeyShareExtensionRequired,
    NamedGroupsExtensionRequired,
    NoCertificateRequestSignatureSchemesInCommon,
    NoCipherSuitesInCommon,
    NoEcPointFormatsInCommon,
    NoKxGroupsInCommon,
    NoSignatureSchemesInCommon,
    NullCompressionRequired,
    ServerDoesNotSupportTls12Or13,
    ServerSentHelloRetryRequestWithUnknownExtension,
    ServerTlsVersionIsDisabledByOurConfig,
    SignatureAlgorithmsExtensionRequired,
    SupportedVersionsExtensionRequired,
    Tls12NotOffered,
    Tls12NotOfferedOrEnabled,
    Tls13RequiredForQuic,
    UncompressedEcPointsRequired,
    UnsolicitedCertificateTypeExtension,
    ServerRejectedEncryptedClientHello(Option<Vec<EchConfigPayload>>),
}

impl From<PeerIncompatible> for Error {
    #[inline]
    fn from(e: PeerIncompatible) -> Self {
        Self::PeerIncompatible(e)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
/// The ways in which certificate validators can express errors.
///
/// Note that the rustls TLS protocol code interprets specifically these
/// error codes to send specific TLS alerts.  Therefore, if a
/// custom certificate validator uses incorrect errors the library as
/// a whole will send alerts that do not match the standard (this is usually
/// a minor issue, but could be misleading).
pub enum CertificateError {
    /// The certificate is not correctly encoded.
    BadEncoding,

    /// The current time is after the `notAfter` time in the certificate.
    Expired,

    /// The current time is after the `notAfter` time in the certificate.
    ///
    /// This variant is semantically the same as `Expired`, but includes
    /// extra data to improve error reports.
    ExpiredContext {
        /// The validation time.
        time: UnixTime,
        /// The `notAfter` time of the certificate.
        not_after: UnixTime,
    },

    /// The current time is before the `notBefore` time in the certificate.
    NotValidYet,

    /// The current time is before the `notBefore` time in the certificate.
    ///
    /// This variant is semantically the same as `NotValidYet`, but includes
    /// extra data to improve error reports.
    NotValidYetContext {
        /// The validation time.
        time: UnixTime,
        /// The `notBefore` time of the certificate.
        not_before: UnixTime,
    },

    /// The certificate has been revoked.
    Revoked,

    /// The certificate contains an extension marked critical, but it was
    /// not processed by the certificate validator.
    UnhandledCriticalExtension,

    /// The certificate chain is not issued by a known root certificate.
    UnknownIssuer,

    /// The certificate's revocation status could not be determined.
    UnknownRevocationStatus,

    /// The certificate's revocation status could not be determined, because the CRL is expired.
    ExpiredRevocationList,

    /// The certificate's revocation status could not be determined, because the CRL is expired.
    ///
    /// This variant is semantically the same as `ExpiredRevocationList`, but includes
    /// extra data to improve error reports.
    ExpiredRevocationListContext {
        /// The validation time.
        time: UnixTime,
        /// The nextUpdate time of the CRL.
        next_update: UnixTime,
    },

    /// A certificate is not correctly signed by the key of its alleged
    /// issuer.
    BadSignature,

    /// A signature inside a certificate or on a handshake was made with an unsupported algorithm.
    #[deprecated(
        since = "0.23.29",
        note = "use `UnsupportedSignatureAlgorithmContext` instead"
    )]
    UnsupportedSignatureAlgorithm,

    /// A signature inside a certificate or on a handshake was made with an unsupported algorithm.
    UnsupportedSignatureAlgorithmContext {
        /// The signature algorithm OID that was unsupported.
        signature_algorithm_id: Vec<u8>,
        /// Supported algorithms that were available for signature verification.
        supported_algorithms: Vec<AlgorithmIdentifier>,
    },

    /// A signature was made with an algorithm that doesn't match the relevant public key.
    UnsupportedSignatureAlgorithmForPublicKeyContext {
        /// The signature algorithm OID.
        signature_algorithm_id: Vec<u8>,
        /// The public key algorithm OID.
        public_key_algorithm_id: Vec<u8>,
    },

    /// The subject names in an end-entity certificate do not include
    /// the expected name.
    NotValidForName,

    /// The subject names in an end-entity certificate do not include
    /// the expected name.
    ///
    /// This variant is semantically the same as `NotValidForName`, but includes
    /// extra data to improve error reports.
    NotValidForNameContext {
        /// Expected server name.
        expected: ServerName<'static>,

        /// The names presented in the end entity certificate.
        ///
        /// These are the subject names as present in the leaf certificate and may contain DNS names
        /// with or without a wildcard label as well as IP address names.
        presented: Vec<String>,
    },

    /// The certificate is being used for a different purpose than allowed.
    InvalidPurpose,

    /// The certificate is being used for a different purpose than allowed.
    ///
    /// This variant is semantically the same as `InvalidPurpose`, but includes
    /// extra data to improve error reports.
    InvalidPurposeContext {
        /// Extended key purpose that was required by the application.
        required: ExtendedKeyPurpose,
        /// Extended key purposes that were presented in the peer's certificate.
        presented: Vec<ExtendedKeyPurpose>,
    },

    /// The OCSP response provided to the verifier was invalid.
    ///
    /// This should be returned from [`ServerCertVerifier::verify_server_cert()`]
    /// when a verifier checks its `ocsp_response` parameter and finds it invalid.
    ///
    /// This maps to [`AlertDescription::BadCertificateStatusResponse`].
    ///
    /// [`ServerCertVerifier::verify_server_cert()`]: crate::client::danger::ServerCertVerifier::verify_server_cert
    InvalidOcspResponse,

    /// The certificate is valid, but the handshake is rejected for other
    /// reasons.
    ApplicationVerificationFailure,

    /// Any other error.
    ///
    /// This can be used by custom verifiers to expose the underlying error
    /// (where they are not better described by the more specific errors
    /// above).
    ///
    /// It is also used by the default verifier in case its error is
    /// not covered by the above common cases.
    ///
    /// Enums holding this variant will never compare equal to each other.
    Other(OtherError),
}

impl PartialEq<Self> for CertificateError {
    fn eq(&self, other: &Self) -> bool {
        use CertificateError::*;
        #[allow(clippy::match_like_matches_macro)]
        match (self, other) {
            (BadEncoding, BadEncoding) => true,
            (Expired, Expired) => true,
            (
                ExpiredContext {
                    time: left_time,
                    not_after: left_not_after,
                },
                ExpiredContext {
                    time: right_time,
                    not_after: right_not_after,
                },
            ) => (left_time, left_not_after) == (right_time, right_not_after),
            (NotValidYet, NotValidYet) => true,
            (
                NotValidYetContext {
                    time: left_time,
                    not_before: left_not_before,
                },
                NotValidYetContext {
                    time: right_time,
                    not_before: right_not_before,
                },
            ) => (left_time, left_not_before) == (right_time, right_not_before),
            (Revoked, Revoked) => true,
            (UnhandledCriticalExtension, UnhandledCriticalExtension) => true,
            (UnknownIssuer, UnknownIssuer) => true,
            (BadSignature, BadSignature) => true,
            #[allow(deprecated)]
            (UnsupportedSignatureAlgorithm, UnsupportedSignatureAlgorithm) => true,
            (
                UnsupportedSignatureAlgorithmContext {
                    signature_algorithm_id: left_signature_algorithm_id,
                    supported_algorithms: left_supported_algorithms,
                },
                UnsupportedSignatureAlgorithmContext {
                    signature_algorithm_id: right_signature_algorithm_id,
                    supported_algorithms: right_supported_algorithms,
                },
            ) => {
                (left_signature_algorithm_id, left_supported_algorithms)
                    == (right_signature_algorithm_id, right_supported_algorithms)
            }
            (
                UnsupportedSignatureAlgorithmForPublicKeyContext {
                    signature_algorithm_id: left_signature_algorithm_id,
                    public_key_algorithm_id: left_public_key_algorithm_id,
                },
                UnsupportedSignatureAlgorithmForPublicKeyContext {
                    signature_algorithm_id: right_signature_algorithm_id,
                    public_key_algorithm_id: right_public_key_algorithm_id,
                },
            ) => {
                (left_signature_algorithm_id, left_public_key_algorithm_id)
                    == (right_signature_algorithm_id, right_public_key_algorithm_id)
            }
            (NotValidForName, NotValidForName) => true,
            (
                NotValidForNameContext {
                    expected: left_expected,
                    presented: left_presented,
                },
                NotValidForNameContext {
                    expected: right_expected,
                    presented: right_presented,
                },
            ) => (left_expected, left_presented) == (right_expected, right_presented),
            (InvalidPurpose, InvalidPurpose) => true,
            (
                InvalidPurposeContext {
                    required: left_required,
                    presented: left_presented,
                },
                InvalidPurposeContext {
                    required: right_required,
                    presented: right_presented,
                },
            ) => (left_required, left_presented) == (right_required, right_presented),
            (InvalidOcspResponse, InvalidOcspResponse) => true,
            (ApplicationVerificationFailure, ApplicationVerificationFailure) => true,
            (UnknownRevocationStatus, UnknownRevocationStatus) => true,
            (ExpiredRevocationList, ExpiredRevocationList) => true,
            (
                ExpiredRevocationListContext {
                    time: left_time,
                    next_update: left_next_update,
                },
                ExpiredRevocationListContext {
                    time: right_time,
                    next_update: right_next_update,
                },
            ) => (left_time, left_next_update) == (right_time, right_next_update),
            _ => false,
        }
    }
}

// The following mapping are heavily referenced in:
// * [OpenSSL Implementation](https://github.com/openssl/openssl/blob/45bb98bfa223efd3258f445ad443f878011450f0/ssl/statem/statem_lib.c#L1434)
// * [BoringSSL Implementation](https://github.com/google/boringssl/blob/583c60bd4bf76d61b2634a58bcda99a92de106cb/ssl/ssl_x509.cc#L1323)
impl From<CertificateError> for AlertDescription {
    fn from(e: CertificateError) -> Self {
        use CertificateError::*;
        match e {
            BadEncoding
            | UnhandledCriticalExtension
            | NotValidForName
            | NotValidForNameContext { .. } => Self::BadCertificate,
            // RFC 5246/RFC 8446
            // certificate_expired
            //  A certificate has expired or **is not currently valid**.
            Expired | ExpiredContext { .. } | NotValidYet | NotValidYetContext { .. } => {
                Self::CertificateExpired
            }
            Revoked => Self::CertificateRevoked,
            // OpenSSL, BoringSSL and AWS-LC all generate an Unknown CA alert for
            // the case where revocation status can not be determined, so we do the same here.
            UnknownIssuer
            | UnknownRevocationStatus
            | ExpiredRevocationList
            | ExpiredRevocationListContext { .. } => Self::UnknownCA,
            InvalidOcspResponse => Self::BadCertificateStatusResponse,
            #[allow(deprecated)]
            BadSignature
            | UnsupportedSignatureAlgorithm
            | UnsupportedSignatureAlgorithmContext { .. }
            | UnsupportedSignatureAlgorithmForPublicKeyContext { .. } => Self::DecryptError,
            InvalidPurpose | InvalidPurposeContext { .. } => Self::UnsupportedCertificate,
            ApplicationVerificationFailure => Self::AccessDenied,
            // RFC 5246/RFC 8446
            // certificate_unknown
            //  Some other (unspecified) issue arose in processing the
            //  certificate, rendering it unacceptable.
            Other(..) => Self::CertificateUnknown,
        }
    }
}

impl fmt::Display for CertificateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "std")]
            Self::NotValidForNameContext {
                expected,
                presented,
            } => {
                write!(
                    f,
                    "certificate not valid for name {:?}; certificate ",
                    expected.to_str()
                )?;

                match presented.as_slice() {
                    &[] => write!(
                        f,
                        "is not valid for any names (according to its subjectAltName extension)"
                    ),
                    [one] => write!(f, "is only valid for {one}"),
                    many => {
                        write!(f, "is only valid for ")?;

                        let n = many.len();
                        let all_but_last = &many[..n - 1];
                        let last = &many[n - 1];

                        for (i, name) in all_but_last.iter().enumerate() {
                            write!(f, "{name}")?;
                            if i < n - 2 {
                                write!(f, ", ")?;
                            }
                        }
                        write!(f, " or {last}")
                    }
                }
            }

            Self::ExpiredContext { time, not_after } => write!(
                f,
                "certificate expired: verification time {} (UNIX), \
                 but certificate is not valid after {} \
                 ({} seconds ago)",
                time.as_secs(),
                not_after.as_secs(),
                time.as_secs()
                    .saturating_sub(not_after.as_secs())
            ),

            Self::NotValidYetContext { time, not_before } => write!(
                f,
                "certificate not valid yet: verification time {} (UNIX), \
                 but certificate is not valid before {} \
                 ({} seconds in future)",
                time.as_secs(),
                not_before.as_secs(),
                not_before
                    .as_secs()
                    .saturating_sub(time.as_secs())
            ),

            Self::ExpiredRevocationListContext { time, next_update } => write!(
                f,
                "certificate revocation list expired: \
                 verification time {} (UNIX), \
                 but CRL is not valid after {} \
                 ({} seconds ago)",
                time.as_secs(),
                next_update.as_secs(),
                time.as_secs()
                    .saturating_sub(next_update.as_secs())
            ),

            Self::InvalidPurposeContext {
                required,
                presented,
            } => {
                write!(
                    f,
                    "certificate does not allow extended key usage for {required}, allows "
                )?;
                for (i, eku) in presented.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{eku}")?;
                }
                Ok(())
            }

            other => write!(f, "{other:?}"),
        }
    }
}

impl From<CertificateError> for Error {
    #[inline]
    fn from(e: CertificateError) -> Self {
        Self::InvalidCertificate(e)
    }
}

/// Extended Key Usage (EKU) purpose values.
///
/// These are usually represented as OID values in the certificate's extension (if present), but
/// we represent the values that are most relevant to rustls as named enum variants.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExtendedKeyPurpose {
    /// Client authentication
    ClientAuth,
    /// Server authentication
    ServerAuth,
    /// Other EKU values
    ///
    /// Represented here as a `Vec<usize>` for human readability.
    Other(Vec<usize>),
}

impl ExtendedKeyPurpose {
    pub(crate) fn for_values(values: impl Iterator<Item = usize>) -> Self {
        let values = values.collect::<Vec<_>>();
        match &*values {
            KeyUsage::CLIENT_AUTH_REPR => Self::ClientAuth,
            KeyUsage::SERVER_AUTH_REPR => Self::ServerAuth,
            _ => Self::Other(values),
        }
    }
}

impl fmt::Display for ExtendedKeyPurpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClientAuth => write!(f, "client authentication"),
            Self::ServerAuth => write!(f, "server authentication"),
            Self::Other(values) => {
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{value}")?;
                }
                Ok(())
            }
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
/// The ways in which a certificate revocation list (CRL) can be invalid.
pub enum CertRevocationListError {
    /// The CRL had a bad signature from its issuer.
    BadSignature,

    /// The CRL had an unsupported signature from its issuer.
    #[deprecated(
        since = "0.23.29",
        note = "use `UnsupportedSignatureAlgorithmContext` instead"
    )]
    UnsupportedSignatureAlgorithm,

    /// A signature inside a certificate or on a handshake was made with an unsupported algorithm.
    UnsupportedSignatureAlgorithmContext {
        /// The signature algorithm OID that was unsupported.
        signature_algorithm_id: Vec<u8>,
        /// Supported algorithms that were available for signature verification.
        supported_algorithms: Vec<AlgorithmIdentifier>,
    },

    /// A signature was made with an algorithm that doesn't match the relevant public key.
    UnsupportedSignatureAlgorithmForPublicKeyContext {
        /// The signature algorithm OID.
        signature_algorithm_id: Vec<u8>,
        /// The public key algorithm OID.
        public_key_algorithm_id: Vec<u8>,
    },

    /// The CRL contained an invalid CRL number.
    InvalidCrlNumber,

    /// The CRL contained a revoked certificate with an invalid serial number.
    InvalidRevokedCertSerialNumber,

    /// The CRL issuer does not specify the cRLSign key usage.
    IssuerInvalidForCrl,

    /// The CRL is invalid for some other reason.
    ///
    /// Enums holding this variant will never compare equal to each other.
    Other(OtherError),

    /// The CRL is not correctly encoded.
    ParseError,

    /// The CRL is not a v2 X.509 CRL.
    UnsupportedCrlVersion,

    /// The CRL, or a revoked certificate in the CRL, contained an unsupported critical extension.
    UnsupportedCriticalExtension,

    /// The CRL is an unsupported delta CRL, containing only changes relative to another CRL.
    UnsupportedDeltaCrl,

    /// The CRL is an unsupported indirect CRL, containing revoked certificates issued by a CA
    /// other than the issuer of the CRL.
    UnsupportedIndirectCrl,

    /// The CRL contained a revoked certificate with an unsupported revocation reason.
    /// See RFC 5280 Section 5.3.1[^1] for a list of supported revocation reasons.
    ///
    /// [^1]: <https://www.rfc-editor.org/rfc/rfc5280#section-5.3.1>
    UnsupportedRevocationReason,
}

impl PartialEq<Self> for CertRevocationListError {
    fn eq(&self, other: &Self) -> bool {
        use CertRevocationListError::*;
        #[allow(clippy::match_like_matches_macro)]
        match (self, other) {
            (BadSignature, BadSignature) => true,
            #[allow(deprecated)]
            (UnsupportedSignatureAlgorithm, UnsupportedSignatureAlgorithm) => true,
            (
                UnsupportedSignatureAlgorithmContext {
                    signature_algorithm_id: left_signature_algorithm_id,
                    supported_algorithms: left_supported_algorithms,
                },
                UnsupportedSignatureAlgorithmContext {
                    signature_algorithm_id: right_signature_algorithm_id,
                    supported_algorithms: right_supported_algorithms,
                },
            ) => {
                (left_signature_algorithm_id, left_supported_algorithms)
                    == (right_signature_algorithm_id, right_supported_algorithms)
            }
            (
                UnsupportedSignatureAlgorithmForPublicKeyContext {
                    signature_algorithm_id: left_signature_algorithm_id,
                    public_key_algorithm_id: left_public_key_algorithm_id,
                },
                UnsupportedSignatureAlgorithmForPublicKeyContext {
                    signature_algorithm_id: right_signature_algorithm_id,
                    public_key_algorithm_id: right_public_key_algorithm_id,
                },
            ) => {
                (left_signature_algorithm_id, left_public_key_algorithm_id)
                    == (right_signature_algorithm_id, right_public_key_algorithm_id)
            }
            (InvalidCrlNumber, InvalidCrlNumber) => true,
            (InvalidRevokedCertSerialNumber, InvalidRevokedCertSerialNumber) => true,
            (IssuerInvalidForCrl, IssuerInvalidForCrl) => true,
            (ParseError, ParseError) => true,
            (UnsupportedCrlVersion, UnsupportedCrlVersion) => true,
            (UnsupportedCriticalExtension, UnsupportedCriticalExtension) => true,
            (UnsupportedDeltaCrl, UnsupportedDeltaCrl) => true,
            (UnsupportedIndirectCrl, UnsupportedIndirectCrl) => true,
            (UnsupportedRevocationReason, UnsupportedRevocationReason) => true,
            _ => false,
        }
    }
}

impl From<CertRevocationListError> for Error {
    #[inline]
    fn from(e: CertRevocationListError) -> Self {
        Self::InvalidCertRevocationList(e)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Eq, PartialEq)]
/// An error that occurred while handling Encrypted Client Hello (ECH).
pub enum EncryptedClientHelloError {
    /// The provided ECH configuration list was invalid.
    InvalidConfigList,
    /// No compatible ECH configuration.
    NoCompatibleConfig,
    /// The client configuration has server name indication (SNI) disabled.
    SniRequired,
}

impl From<EncryptedClientHelloError> for Error {
    #[inline]
    fn from(e: EncryptedClientHelloError) -> Self {
        Self::InvalidEncryptedClientHello(e)
    }
}

fn join<T: fmt::Debug>(items: &[T]) -> String {
    items
        .iter()
        .map(|x| format!("{x:?}"))
        .collect::<Vec<String>>()
        .join(" or ")
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InappropriateMessage {
                expect_types,
                got_type,
            } => write!(
                f,
                "received unexpected message: got {:?} when expecting {}",
                got_type,
                join::<ContentType>(expect_types)
            ),
            Self::InappropriateHandshakeMessage {
                expect_types,
                got_type,
            } => write!(
                f,
                "received unexpected handshake message: got {:?} when expecting {}",
                got_type,
                join::<HandshakeType>(expect_types)
            ),
            Self::InvalidMessage(typ) => {
                write!(f, "received corrupt message of type {typ:?}")
            }
            Self::PeerIncompatible(why) => write!(f, "peer is incompatible: {why:?}"),
            Self::PeerMisbehaved(why) => write!(f, "peer misbehaved: {why:?}"),
            Self::AlertReceived(alert) => write!(f, "received fatal alert: {alert:?}"),
            Self::InvalidCertificate(err) => {
                write!(f, "invalid peer certificate: {err}")
            }
            Self::InvalidCertRevocationList(err) => {
                write!(f, "invalid certificate revocation list: {err:?}")
            }
            Self::NoCertificatesPresented => write!(f, "peer sent no certificates"),
            Self::UnsupportedNameType => write!(f, "presented server name type wasn't supported"),
            Self::DecryptError => write!(f, "cannot decrypt peer's message"),
            Self::InvalidEncryptedClientHello(err) => {
                write!(f, "encrypted client hello failure: {err:?}")
            }
            Self::EncryptError => write!(f, "cannot encrypt message"),
            Self::PeerSentOversizedRecord => write!(f, "peer sent excess record size"),
            Self::HandshakeNotComplete => write!(f, "handshake not complete"),
            Self::NoApplicationProtocol => write!(f, "peer doesn't support any known protocol"),
            Self::FailedToGetCurrentTime => write!(f, "failed to get current time"),
            Self::FailedToGetRandomBytes => write!(f, "failed to get random bytes"),
            Self::BadMaxFragmentSize => {
                write!(f, "the supplied max_fragment_size was too small or large")
            }
            Self::InconsistentKeys(why) => {
                write!(f, "keys may not be consistent: {why:?}")
            }
            Self::General(err) => write!(f, "unexpected error: {err}"),
            Self::Other(err) => write!(f, "other error: {err}"),
        }
    }
}

#[cfg(feature = "std")]
impl From<SystemTimeError> for Error {
    #[inline]
    fn from(_: SystemTimeError) -> Self {
        Self::FailedToGetCurrentTime
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

impl From<rand::GetRandomFailed> for Error {
    fn from(_: rand::GetRandomFailed) -> Self {
        Self::FailedToGetRandomBytes
    }
}

mod other_error {
    use core::fmt;
    #[cfg(feature = "std")]
    use std::error::Error as StdError;

    use super::Error;
    #[cfg(feature = "std")]
    use crate::sync::Arc;

    /// Any other error that cannot be expressed by a more specific [`Error`] variant.
    ///
    /// For example, an `OtherError` could be produced by a custom crypto provider
    /// exposing a provider specific error.
    ///
    /// Enums holding this type will never compare equal to each other.
    #[derive(Debug, Clone)]
    pub struct OtherError(#[cfg(feature = "std")] pub Arc<dyn StdError + Send + Sync>);

    impl PartialEq<Self> for OtherError {
        fn eq(&self, _other: &Self) -> bool {
            false
        }
    }

    impl From<OtherError> for Error {
        fn from(value: OtherError) -> Self {
            Self::Other(value)
        }
    }

    impl fmt::Display for OtherError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            #[cfg(feature = "std")]
            {
                write!(f, "{}", self.0)
            }
            #[cfg(not(feature = "std"))]
            {
                f.write_str("no further information available")
            }
        }
    }

    #[cfg(feature = "std")]
    impl StdError for OtherError {
        fn source(&self) -> Option<&(dyn StdError + 'static)> {
            Some(self.0.as_ref())
        }
    }
}

pub use other_error::OtherError;

#[cfg(test)]
mod tests {
    use core::time::Duration;
    use std::prelude::v1::*;
    use std::{println, vec};

    use pki_types::ServerName;

    use super::{
        CertRevocationListError, Error, InconsistentKeys, InvalidMessage, OtherError, UnixTime,
    };
    #[cfg(feature = "std")]
    use crate::sync::Arc;

    #[test]
    fn certificate_error_equality() {
        use super::CertificateError::*;
        assert_eq!(BadEncoding, BadEncoding);
        assert_eq!(Expired, Expired);
        let context = ExpiredContext {
            time: UnixTime::since_unix_epoch(Duration::from_secs(1234)),
            not_after: UnixTime::since_unix_epoch(Duration::from_secs(123)),
        };
        assert_eq!(context, context);
        assert_ne!(
            context,
            ExpiredContext {
                time: UnixTime::since_unix_epoch(Duration::from_secs(12345)),
                not_after: UnixTime::since_unix_epoch(Duration::from_secs(123)),
            }
        );
        assert_ne!(
            context,
            ExpiredContext {
                time: UnixTime::since_unix_epoch(Duration::from_secs(1234)),
                not_after: UnixTime::since_unix_epoch(Duration::from_secs(1234)),
            }
        );
        assert_eq!(NotValidYet, NotValidYet);
        let context = NotValidYetContext {
            time: UnixTime::since_unix_epoch(Duration::from_secs(123)),
            not_before: UnixTime::since_unix_epoch(Duration::from_secs(1234)),
        };
        assert_eq!(context, context);
        assert_ne!(
            context,
            NotValidYetContext {
                time: UnixTime::since_unix_epoch(Duration::from_secs(1234)),
                not_before: UnixTime::since_unix_epoch(Duration::from_secs(1234)),
            }
        );
        assert_ne!(
            context,
            NotValidYetContext {
                time: UnixTime::since_unix_epoch(Duration::from_secs(123)),
                not_before: UnixTime::since_unix_epoch(Duration::from_secs(12345)),
            }
        );
        assert_eq!(Revoked, Revoked);
        assert_eq!(UnhandledCriticalExtension, UnhandledCriticalExtension);
        assert_eq!(UnknownIssuer, UnknownIssuer);
        assert_eq!(ExpiredRevocationList, ExpiredRevocationList);
        assert_eq!(UnknownRevocationStatus, UnknownRevocationStatus);
        let context = ExpiredRevocationListContext {
            time: UnixTime::since_unix_epoch(Duration::from_secs(1234)),
            next_update: UnixTime::since_unix_epoch(Duration::from_secs(123)),
        };
        assert_eq!(context, context);
        assert_ne!(
            context,
            ExpiredRevocationListContext {
                time: UnixTime::since_unix_epoch(Duration::from_secs(12345)),
                next_update: UnixTime::since_unix_epoch(Duration::from_secs(123)),
            }
        );
        assert_ne!(
            context,
            ExpiredRevocationListContext {
                time: UnixTime::since_unix_epoch(Duration::from_secs(1234)),
                next_update: UnixTime::since_unix_epoch(Duration::from_secs(1234)),
            }
        );
        assert_eq!(BadSignature, BadSignature);
        #[allow(deprecated)]
        {
            assert_eq!(UnsupportedSignatureAlgorithm, UnsupportedSignatureAlgorithm);
        }
        assert_eq!(
            UnsupportedSignatureAlgorithmContext {
                signature_algorithm_id: vec![1, 2, 3],
                supported_algorithms: vec![]
            },
            UnsupportedSignatureAlgorithmContext {
                signature_algorithm_id: vec![1, 2, 3],
                supported_algorithms: vec![]
            }
        );
        assert_eq!(
            UnsupportedSignatureAlgorithmForPublicKeyContext {
                signature_algorithm_id: vec![1, 2, 3],
                public_key_algorithm_id: vec![4, 5, 6]
            },
            UnsupportedSignatureAlgorithmForPublicKeyContext {
                signature_algorithm_id: vec![1, 2, 3],
                public_key_algorithm_id: vec![4, 5, 6]
            }
        );
        assert_eq!(NotValidForName, NotValidForName);
        let context = NotValidForNameContext {
            expected: ServerName::try_from("example.com")
                .unwrap()
                .to_owned(),
            presented: vec!["other.com".into()],
        };
        assert_eq!(context, context);
        assert_ne!(
            context,
            NotValidForNameContext {
                expected: ServerName::try_from("example.com")
                    .unwrap()
                    .to_owned(),
                presented: vec![]
            }
        );
        assert_ne!(
            context,
            NotValidForNameContext {
                expected: ServerName::try_from("huh.com")
                    .unwrap()
                    .to_owned(),
                presented: vec!["other.com".into()],
            }
        );
        assert_eq!(InvalidPurpose, InvalidPurpose);
        assert_eq!(
            ApplicationVerificationFailure,
            ApplicationVerificationFailure
        );
        assert_eq!(InvalidOcspResponse, InvalidOcspResponse);
        let other = Other(OtherError(
            #[cfg(feature = "std")]
            Arc::from(Box::from("")),
        ));
        assert_ne!(other, other);
        assert_ne!(BadEncoding, Expired);
    }

    #[test]
    fn crl_error_equality() {
        use super::CertRevocationListError::*;
        assert_eq!(BadSignature, BadSignature);
        #[allow(deprecated)]
        {
            assert_eq!(UnsupportedSignatureAlgorithm, UnsupportedSignatureAlgorithm);
        }
        assert_eq!(
            UnsupportedSignatureAlgorithmContext {
                signature_algorithm_id: vec![1, 2, 3],
                supported_algorithms: vec![]
            },
            UnsupportedSignatureAlgorithmContext {
                signature_algorithm_id: vec![1, 2, 3],
                supported_algorithms: vec![]
            }
        );
        assert_eq!(
            UnsupportedSignatureAlgorithmForPublicKeyContext {
                signature_algorithm_id: vec![1, 2, 3],
                public_key_algorithm_id: vec![4, 5, 6]
            },
            UnsupportedSignatureAlgorithmForPublicKeyContext {
                signature_algorithm_id: vec![1, 2, 3],
                public_key_algorithm_id: vec![4, 5, 6]
            }
        );
        assert_eq!(InvalidCrlNumber, InvalidCrlNumber);
        assert_eq!(
            InvalidRevokedCertSerialNumber,
            InvalidRevokedCertSerialNumber
        );
        assert_eq!(IssuerInvalidForCrl, IssuerInvalidForCrl);
        assert_eq!(ParseError, ParseError);
        assert_eq!(UnsupportedCriticalExtension, UnsupportedCriticalExtension);
        assert_eq!(UnsupportedCrlVersion, UnsupportedCrlVersion);
        assert_eq!(UnsupportedDeltaCrl, UnsupportedDeltaCrl);
        assert_eq!(UnsupportedIndirectCrl, UnsupportedIndirectCrl);
        assert_eq!(UnsupportedRevocationReason, UnsupportedRevocationReason);
        let other = Other(OtherError(
            #[cfg(feature = "std")]
            Arc::from(Box::from("")),
        ));
        assert_ne!(other, other);
        assert_ne!(BadSignature, InvalidCrlNumber);
    }

    #[test]
    #[cfg(feature = "std")]
    fn other_error_equality() {
        let other_error = OtherError(Arc::from(Box::from("")));
        assert_ne!(other_error, other_error);
        let other: Error = other_error.into();
        assert_ne!(other, other);
    }

    #[test]
    fn smoke() {
        use crate::enums::{AlertDescription, ContentType, HandshakeType};

        let all = vec![
            Error::InappropriateMessage {
                expect_types: vec![ContentType::Alert],
                got_type: ContentType::Handshake,
            },
            Error::InappropriateHandshakeMessage {
                expect_types: vec![HandshakeType::ClientHello, HandshakeType::Finished],
                got_type: HandshakeType::ServerHello,
            },
            Error::InvalidMessage(InvalidMessage::InvalidCcs),
            Error::NoCertificatesPresented,
            Error::DecryptError,
            super::PeerIncompatible::Tls12NotOffered.into(),
            super::PeerMisbehaved::UnsolicitedCertExtension.into(),
            Error::AlertReceived(AlertDescription::ExportRestriction),
            super::CertificateError::Expired.into(),
            super::CertificateError::NotValidForNameContext {
                expected: ServerName::try_from("example.com")
                    .unwrap()
                    .to_owned(),
                presented: vec![],
            }
            .into(),
            super::CertificateError::NotValidForNameContext {
                expected: ServerName::try_from("example.com")
                    .unwrap()
                    .to_owned(),
                presented: vec!["DnsName(\"hello.com\")".into()],
            }
            .into(),
            super::CertificateError::NotValidForNameContext {
                expected: ServerName::try_from("example.com")
                    .unwrap()
                    .to_owned(),
                presented: vec![
                    "DnsName(\"hello.com\")".into(),
                    "DnsName(\"goodbye.com\")".into(),
                ],
            }
            .into(),
            super::CertificateError::NotValidYetContext {
                time: UnixTime::since_unix_epoch(Duration::from_secs(300)),
                not_before: UnixTime::since_unix_epoch(Duration::from_secs(320)),
            }
            .into(),
            super::CertificateError::ExpiredContext {
                time: UnixTime::since_unix_epoch(Duration::from_secs(320)),
                not_after: UnixTime::since_unix_epoch(Duration::from_secs(300)),
            }
            .into(),
            super::CertificateError::ExpiredRevocationListContext {
                time: UnixTime::since_unix_epoch(Duration::from_secs(320)),
                next_update: UnixTime::since_unix_epoch(Duration::from_secs(300)),
            }
            .into(),
            super::CertificateError::InvalidOcspResponse.into(),
            Error::General("undocumented error".to_string()),
            Error::FailedToGetCurrentTime,
            Error::FailedToGetRandomBytes,
            Error::HandshakeNotComplete,
            Error::PeerSentOversizedRecord,
            Error::NoApplicationProtocol,
            Error::BadMaxFragmentSize,
            Error::InconsistentKeys(InconsistentKeys::KeyMismatch),
            Error::InconsistentKeys(InconsistentKeys::Unknown),
            Error::InvalidCertRevocationList(CertRevocationListError::BadSignature),
            Error::Other(OtherError(
                #[cfg(feature = "std")]
                Arc::from(Box::from("")),
            )),
        ];

        for err in all {
            println!("{err:?}:");
            println!("  fmt '{err}'");
        }
    }

    #[test]
    fn rand_error_mapping() {
        use super::rand;
        let err: Error = rand::GetRandomFailed.into();
        assert_eq!(err, Error::FailedToGetRandomBytes);
    }

    #[cfg(feature = "std")]
    #[test]
    fn time_error_mapping() {
        use std::time::SystemTime;

        let time_error = SystemTime::UNIX_EPOCH
            .duration_since(SystemTime::now())
            .unwrap_err();
        let err: Error = time_error.into();
        assert_eq!(err, Error::FailedToGetCurrentTime);
    }
}
