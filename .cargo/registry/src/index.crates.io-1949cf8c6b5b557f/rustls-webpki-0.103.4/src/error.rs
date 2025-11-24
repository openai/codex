// Copyright 2015 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHORS DISCLAIM ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::fmt;
use core::ops::ControlFlow;

use pki_types::UnixTime;
#[cfg(feature = "alloc")]
use pki_types::{AlgorithmIdentifier, ServerName};

use crate::verify_cert::RequiredEkuNotFoundContext;

/// An error that occurs during certificate validation or name validation.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The encoding of some ASN.1 DER-encoded item is invalid.
    BadDer,

    /// The encoding of an ASN.1 DER-encoded time is invalid.
    BadDerTime,

    /// A CA certificate is being used as an end-entity certificate.
    CaUsedAsEndEntity,

    /// The certificate is expired; i.e. the time it is being validated for is
    /// later than the certificate's notAfter time.
    CertExpired {
        /// The validation time.
        time: UnixTime,
        /// The notAfter time of the certificate.
        not_after: UnixTime,
    },

    /// The certificate is not valid for the name it is being validated for.
    CertNotValidForName(InvalidNameContext),

    /// The certificate is not valid yet; i.e. the time it is being validated
    /// for is earlier than the certificate's notBefore time.
    CertNotValidYet {
        /// The validation time.
        time: UnixTime,
        /// The notBefore time of the certificate.
        not_before: UnixTime,
    },

    /// The certificate, or one of its issuers, has been revoked.
    CertRevoked,

    /// The CRL is expired; i.e. the verification time is not before the time
    /// in the CRL nextUpdate field.
    CrlExpired {
        /// The validation time.
        time: UnixTime,
        /// The nextUpdate time of the CRL.
        next_update: UnixTime,
    },

    /// An end-entity certificate is being used as a CA certificate.
    EndEntityUsedAsCa,

    /// An X.509 extension is invalid.
    ExtensionValueInvalid,

    /// The certificate validity period (notBefore, notAfter) is invalid; e.g.
    /// the notAfter time is earlier than the notBefore time.
    InvalidCertValidity,

    /// A CRL number extension was invalid:
    ///  - it was mis-encoded
    ///  - it was negative
    ///  - it was too long
    InvalidCrlNumber,

    /// A iPAddress name constraint was invalid:
    /// - it had a sparse network mask (ie, cannot be written in CIDR form).
    /// - it was too long or short
    InvalidNetworkMaskConstraint,

    /// A serial number was invalid:
    ///  - it was misencoded
    ///  - it was negative
    ///  - it was too long
    InvalidSerialNumber,

    /// The CRL signature is invalid for the issuer's public key.
    InvalidCrlSignatureForPublicKey,

    /// The signature is invalid for the given public key.
    InvalidSignatureForPublicKey,

    /// A CRL was signed by an issuer that has a KeyUsage bitstring that does not include
    /// the cRLSign key usage bit.
    IssuerNotCrlSigner,

    /// A presented or reference DNS identifier was malformed, potentially
    /// containing invalid characters or invalid labels.
    MalformedDnsIdentifier,

    /// The certificate extensions are malformed.
    ///
    /// In particular, webpki requires the DNS name(s) be in the subjectAltName
    /// extension as required by the CA/Browser Forum Baseline Requirements
    /// and as recommended by RFC6125.
    MalformedExtensions,

    /// A name constraint was malformed, potentially containing invalid characters or
    /// invalid labels.
    MalformedNameConstraint,

    /// The maximum number of name constraint comparisons has been reached.
    MaximumNameConstraintComparisonsExceeded,

    /// The maximum number of internal path building calls has been reached. Path complexity is too great.
    MaximumPathBuildCallsExceeded,

    /// The path search was terminated because it became too deep.
    MaximumPathDepthExceeded,

    /// The maximum number of signature checks has been reached. Path complexity is too great.
    MaximumSignatureChecksExceeded,

    /// The certificate violates one or more name constraints.
    NameConstraintViolation,

    /// The certificate violates one or more path length constraints.
    PathLenConstraintViolated,

    /// The certificate is not valid for the Extended Key Usage for which it is
    /// being validated.
    #[deprecated(since = "0.103.2", note = "use RequiredEkuNotFoundContext instead")]
    RequiredEkuNotFound,

    /// The certificate is not valid for the Extended Key Usage for which it is
    /// being validated.
    RequiredEkuNotFoundContext(RequiredEkuNotFoundContext),

    /// The algorithm in the TBSCertificate "signature" field of a certificate
    /// does not match the algorithm in the signature of the certificate.
    SignatureAlgorithmMismatch,

    /// Trailing data was found while parsing DER-encoded input for the named type.
    TrailingData(DerTypeId),

    /// A valid issuer for the certificate could not be found.
    UnknownIssuer,

    /// The certificate's revocation status could not be determined.
    UnknownRevocationStatus,

    /// The certificate is not a v3 X.509 certificate.
    ///
    /// This error may be also reported if the certificate version field
    /// is malformed.
    UnsupportedCertVersion,

    /// The certificate contains an unsupported critical extension.
    UnsupportedCriticalExtension,

    /// The CRL contains an issuing distribution point with no distribution point name,
    /// or a distribution point name relative to an issuer.
    UnsupportedCrlIssuingDistributionPoint,

    /// The CRL is not a v2 X.509 CRL.
    ///
    /// The RFC 5280 web PKI profile mandates only version 2 be used. See section
    /// 5.1.2.1 for more information.
    ///
    /// This error may also be reported if the CRL version field is malformed.
    UnsupportedCrlVersion,

    /// The CRL is an unsupported "delta" CRL.
    UnsupportedDeltaCrl,

    /// The CRL contains unsupported "indirect" entries.
    UnsupportedIndirectCrl,

    /// The `ServerName` contained an unsupported type of value.
    UnsupportedNameType,

    /// The revocation reason is not in the set of supported revocation reasons.
    UnsupportedRevocationReason,

    /// The CRL is partitioned by revocation reasons.
    UnsupportedRevocationReasonsPartitioning,

    /// The signature algorithm for a signature over a CRL is not in the set of supported
    /// signature algorithms given.
    #[deprecated(
        since = "0.103.4",
        note = "use UnsupportedCrlSignatureAlgorithmContext instead"
    )]
    UnsupportedCrlSignatureAlgorithm,

    /// The signature algorithm for a signature is not in the set of supported
    /// signature algorithms given.
    UnsupportedCrlSignatureAlgorithmContext(UnsupportedSignatureAlgorithmContext),

    /// The signature algorithm for a signature is not in the set of supported
    /// signature algorithms given.
    #[deprecated(
        since = "0.103.4",
        note = "use UnsupportedSignatureAlgorithmContext instead"
    )]
    UnsupportedSignatureAlgorithm,

    /// The signature algorithm for a signature is not in the set of supported
    /// signature algorithms given.
    UnsupportedSignatureAlgorithmContext(UnsupportedSignatureAlgorithmContext),

    /// The CRL signature's algorithm does not match the algorithm of the issuer
    /// public key it is being validated for. This may be because the public key
    /// algorithm's OID isn't recognized (e.g. DSA), or the public key
    /// algorithm's parameters don't match the supported parameters for that
    /// algorithm (e.g. ECC keys for unsupported curves), or the public key
    /// algorithm and the signature algorithm simply don't match (e.g.
    /// verifying an RSA signature with an ECC public key).
    #[deprecated(
        since = "0.103.4",
        note = "use UnsupportedCrlSignatureAlgorithmForPublicKeyContext instead"
    )]
    UnsupportedCrlSignatureAlgorithmForPublicKey,

    /// The signature's algorithm does not match the algorithm of the public
    /// key it is being validated for. This may be because the public key
    /// algorithm's OID isn't recognized (e.g. DSA), or the public key
    /// algorithm's parameters don't match the supported parameters for that
    /// algorithm (e.g. ECC keys for unsupported curves), or the public key
    /// algorithm and the signature algorithm simply don't match (e.g.
    /// verifying an RSA signature with an ECC public key).
    UnsupportedCrlSignatureAlgorithmForPublicKeyContext(
        UnsupportedSignatureAlgorithmForPublicKeyContext,
    ),

    /// The signature's algorithm does not match the algorithm of the public
    /// key it is being validated for. This may be because the public key
    /// algorithm's OID isn't recognized (e.g. DSA), or the public key
    /// algorithm's parameters don't match the supported parameters for that
    /// algorithm (e.g. ECC keys for unsupported curves), or the public key
    /// algorithm and the signature algorithm simply don't match (e.g.
    /// verifying an RSA signature with an ECC public key).
    #[deprecated(
        since = "0.103.4",
        note = "use UnsupportedSignatureAlgorithmForPublicKeyContext instead"
    )]
    UnsupportedSignatureAlgorithmForPublicKey,

    /// The signature's algorithm does not match the algorithm of the public
    /// key it is being validated for. This may be because the public key
    /// algorithm's OID isn't recognized (e.g. DSA), or the public key
    /// algorithm's parameters don't match the supported parameters for that
    /// algorithm (e.g. ECC keys for unsupported curves), or the public key
    /// algorithm and the signature algorithm simply don't match (e.g.
    /// verifying an RSA signature with an ECC public key).
    UnsupportedSignatureAlgorithmForPublicKeyContext(
        UnsupportedSignatureAlgorithmForPublicKeyContext,
    ),
}

impl Error {
    // Compare the Error with the new error by rank, returning the higher rank of the two as
    // the most specific error.
    pub(crate) fn most_specific(self, new: Self) -> Self {
        // Assign an error a numeric value ranking it by specificity.
        if self.rank() >= new.rank() { self } else { new }
    }

    // Return a numeric indication of how specific the error is, where an error with a higher rank
    // is considered more useful to an end user than an error with a lower rank. This is used by
    // Error::most_specific to compare two errors in order to return which is more specific.
    #[allow(clippy::as_conversions)] // We won't exceed u32 errors.
    pub(crate) fn rank(&self) -> u32 {
        match &self {
            // Errors related to certificate validity
            Self::CertNotValidYet { .. } | Self::CertExpired { .. } => 290,
            Self::CertNotValidForName(_) => 280,
            Self::CertRevoked | Self::UnknownRevocationStatus | Self::CrlExpired { .. } => 270,
            Self::InvalidCrlSignatureForPublicKey | Self::InvalidSignatureForPublicKey => 260,
            Self::SignatureAlgorithmMismatch => 250,
            #[allow(deprecated)]
            Self::RequiredEkuNotFound | Self::RequiredEkuNotFoundContext(_) => 240,
            Self::NameConstraintViolation => 230,
            Self::PathLenConstraintViolated => 220,
            Self::CaUsedAsEndEntity | Self::EndEntityUsedAsCa => 210,
            Self::IssuerNotCrlSigner => 200,

            // Errors related to supported features used in an invalid way.
            Self::InvalidCertValidity => 190,
            Self::InvalidNetworkMaskConstraint => 180,
            Self::InvalidSerialNumber => 170,
            Self::InvalidCrlNumber => 160,

            // Errors related to unsupported features.
            #[allow(deprecated)]
            Self::UnsupportedCrlSignatureAlgorithmForPublicKey
            | Self::UnsupportedCrlSignatureAlgorithmForPublicKeyContext(_)
            | Self::UnsupportedSignatureAlgorithmForPublicKey
            | Self::UnsupportedSignatureAlgorithmForPublicKeyContext(_) => 150,
            #[allow(deprecated)]
            Self::UnsupportedCrlSignatureAlgorithm
            | Self::UnsupportedCrlSignatureAlgorithmContext(_)
            | Self::UnsupportedSignatureAlgorithm
            | Self::UnsupportedSignatureAlgorithmContext(_) => 140,
            Self::UnsupportedCriticalExtension => 130,
            Self::UnsupportedCertVersion => 130,
            Self::UnsupportedCrlVersion => 120,
            Self::UnsupportedDeltaCrl => 110,
            Self::UnsupportedIndirectCrl => 100,
            Self::UnsupportedNameType => 95,
            Self::UnsupportedRevocationReason => 90,
            Self::UnsupportedRevocationReasonsPartitioning => 80,
            Self::UnsupportedCrlIssuingDistributionPoint => 70,
            Self::MaximumPathDepthExceeded => 61,

            // Errors related to malformed data.
            Self::MalformedDnsIdentifier => 60,
            Self::MalformedNameConstraint => 50,
            Self::MalformedExtensions | Self::TrailingData(_) => 40,
            Self::ExtensionValueInvalid => 30,

            // Generic DER errors.
            Self::BadDerTime => 20,
            Self::BadDer => 10,

            // Special case errors - not subject to ranking.
            Self::MaximumSignatureChecksExceeded => 0,
            Self::MaximumPathBuildCallsExceeded => 0,
            Self::MaximumNameConstraintComparisonsExceeded => 0,

            // Default catch all error - should be renamed in the future.
            Self::UnknownIssuer => 0,
        }
    }

    /// Returns true for errors that should be considered fatal during path building. Errors of
    /// this class should halt any further path building and be returned immediately.
    #[inline]
    pub(crate) fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::MaximumSignatureChecksExceeded
                | Self::MaximumPathBuildCallsExceeded
                | Self::MaximumNameConstraintComparisonsExceeded
        )
    }
}

impl From<Error> for ControlFlow<Error, Error> {
    fn from(value: Error) -> Self {
        match value {
            // If an error is fatal, we've exhausted the potential for continued search.
            err if err.is_fatal() => Self::Break(err),
            // Otherwise we've rejected one candidate chain, but may continue to search for others.
            err => Self::Continue(err),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(feature = "std")]
impl ::std::error::Error for Error {}

/// Additional context for the `CertNotValidForName` error variant.
///
/// The contents of this type depend on whether the `alloc` feature is enabled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidNameContext {
    /// Expected server name.
    #[cfg(feature = "alloc")]
    pub expected: ServerName<'static>,
    /// The names presented in the end entity certificate.
    ///
    /// These are the subject names as present in the leaf certificate and may contain DNS names
    /// with or without a wildcard label as well as IP address names.
    #[cfg(feature = "alloc")]
    pub presented: Vec<String>,
}

/// Additional context for the `UnsupportedSignatureAlgorithmForPublicKey` error variant.
///
/// The contents of this type depend on whether the `alloc` feature is enabled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsupportedSignatureAlgorithmForPublicKeyContext {
    /// The signature algorithm OID.
    #[cfg(feature = "alloc")]
    pub signature_algorithm_id: Vec<u8>,
    /// The public key algorithm OID.
    #[cfg(feature = "alloc")]
    pub public_key_algorithm_id: Vec<u8>,
}

/// Additional context for the `UnsupportedSignatureAlgorithm` error variant.
///
/// The contents of this type depend on whether the `alloc` feature is enabled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsupportedSignatureAlgorithmContext {
    /// The signature algorithm OID that was unsupported.
    #[cfg(feature = "alloc")]
    pub signature_algorithm_id: Vec<u8>,
    /// Supported algorithms that were available for signature verification.
    #[cfg(feature = "alloc")]
    pub supported_algorithms: Vec<AlgorithmIdentifier>,
}

/// Trailing data was found while parsing DER-encoded input for the named type.
#[allow(missing_docs)]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerTypeId {
    BitString,
    Bool,
    Certificate,
    CertificateExtensions,
    CertificateTbsCertificate,
    CertRevocationList,
    CertRevocationListExtension,
    CrlDistributionPoint,
    CommonNameInner,
    CommonNameOuter,
    DistributionPointName,
    Extension,
    GeneralName,
    RevocationReason,
    Signature,
    SignatureAlgorithm,
    SignedData,
    SubjectPublicKeyInfo,
    Time,
    TrustAnchorV1,
    TrustAnchorV1TbsCertificate,
    U8,
    RevokedCertificate,
    RevokedCertificateExtension,
    RevokedCertEntry,
    IssuingDistributionPoint,
}
