// Copyright 2023 Daniel McCarney.
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

use pki_types::{SignatureVerificationAlgorithm, UnixTime};

use crate::error::Error;
use crate::verify_cert::{Budget, PathNode, Role};
use crate::{der, public_values_eq};

use core::fmt::Debug;

mod types;
pub use types::{
    BorrowedCertRevocationList, BorrowedRevokedCert, CertRevocationList, RevocationReason,
};
#[cfg(feature = "alloc")]
pub use types::{OwnedCertRevocationList, OwnedRevokedCert};

/// Builds a RevocationOptions instance to control how revocation checking is performed.
#[derive(Debug, Copy, Clone)]
pub struct RevocationOptionsBuilder<'a> {
    crls: &'a [&'a CertRevocationList<'a>],

    depth: RevocationCheckDepth,

    status_policy: UnknownStatusPolicy,

    expiration_policy: ExpirationPolicy,
}

impl<'a> RevocationOptionsBuilder<'a> {
    /// Create a builder that will perform revocation checking using the provided certificate
    /// revocation lists (CRLs). At least one CRL must be provided.
    ///
    /// Use [RevocationOptionsBuilder::build] to create a [RevocationOptions] instance.
    ///
    /// By default revocation checking will be performed on both the end-entity (leaf) certificate
    /// and intermediate certificates. This can be customized using the
    /// [RevocationOptionsBuilder::with_depth] method.
    ///
    /// By default revocation checking will fail if the revocation status of a certificate cannot
    /// be determined. This can be customized using the
    /// [RevocationOptionsBuilder::with_status_policy] method.
    ///
    /// By default revocation checking will *not* fail if the verification time is beyond the time
    /// in the CRL nextUpdate field. This can be customized using the
    /// [RevocationOptionsBuilder::with_expiration_policy] method.
    pub fn new(crls: &'a [&'a CertRevocationList<'a>]) -> Result<Self, CrlsRequired> {
        if crls.is_empty() {
            return Err(CrlsRequired(()));
        }

        Ok(Self {
            crls,
            depth: RevocationCheckDepth::Chain,
            status_policy: UnknownStatusPolicy::Deny,
            expiration_policy: ExpirationPolicy::Ignore,
        })
    }

    /// Customize the depth at which revocation checking will be performed, controlling
    /// whether only the end-entity (leaf) certificate in the chain to a trust anchor will
    /// have its revocation status checked, or whether the intermediate certificates will as well.
    pub fn with_depth(mut self, depth: RevocationCheckDepth) -> Self {
        self.depth = depth;
        self
    }

    /// Customize whether unknown revocation status is an error, or permitted.
    pub fn with_status_policy(mut self, policy: UnknownStatusPolicy) -> Self {
        self.status_policy = policy;
        self
    }

    /// Customize whether the CRL nextUpdate field (i.e. expiration) is enforced.
    pub fn with_expiration_policy(mut self, policy: ExpirationPolicy) -> Self {
        self.expiration_policy = policy;
        self
    }

    /// Construct a [RevocationOptions] instance based on the builder's configuration.
    pub fn build(self) -> RevocationOptions<'a> {
        RevocationOptions {
            crls: self.crls,
            depth: self.depth,
            status_policy: self.status_policy,
            expiration_policy: self.expiration_policy,
        }
    }
}

/// Describes how revocation checking is performed, if at all. Can be constructed with a
/// [RevocationOptionsBuilder] instance.
#[derive(Debug, Copy, Clone)]
pub struct RevocationOptions<'a> {
    pub(crate) crls: &'a [&'a CertRevocationList<'a>],
    pub(crate) depth: RevocationCheckDepth,
    pub(crate) status_policy: UnknownStatusPolicy,
    pub(crate) expiration_policy: ExpirationPolicy,
}

impl RevocationOptions<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn check(
        &self,
        path: &PathNode<'_>,
        issuer_subject: untrusted::Input<'_>,
        issuer_spki: untrusted::Input<'_>,
        issuer_ku: Option<untrusted::Input<'_>>,
        supported_sig_algs: &[&dyn SignatureVerificationAlgorithm],
        budget: &mut Budget,
        time: UnixTime,
    ) -> Result<Option<CertNotRevoked>, Error> {
        assert!(public_values_eq(path.cert.issuer, issuer_subject));

        // If the policy only specifies checking EndEntity revocation state and we're looking at an
        // issuer certificate, return early without considering the certificate's revocation state.
        if let (RevocationCheckDepth::EndEntity, Role::Issuer) = (self.depth, path.role()) {
            return Ok(None);
        }

        let crl = self
            .crls
            .iter()
            .find(|candidate_crl| candidate_crl.authoritative(path));

        use UnknownStatusPolicy::*;
        let crl = match (crl, self.status_policy) {
            (Some(crl), _) => crl,
            // If the policy allows unknown, return Ok(None) to indicate that the certificate
            // was not confirmed as CertNotRevoked, but that this isn't an error condition.
            (None, Allow) => return Ok(None),
            // Otherwise, this is an error condition based on the provided policy.
            (None, _) => return Err(Error::UnknownRevocationStatus),
        };

        // Verify the CRL signature with the issuer SPKI.
        // TODO(XXX): consider whether we can refactor so this happens once up-front, instead
        //            of per-lookup.
        //            https://github.com/rustls/webpki/issues/81
        crl.verify_signature(supported_sig_algs, issuer_spki, budget)
            .map_err(crl_signature_err)?;

        if self.expiration_policy == ExpirationPolicy::Enforce {
            crl.check_expiration(time)?;
        }

        // Verify that if the issuer has a KeyUsage bitstring it asserts cRLSign.
        KeyUsageMode::CrlSign.check(issuer_ku)?;

        // Try to find the cert serial in the verified CRL contents.
        let cert_serial = path.cert.serial.as_slice_less_safe();
        match crl.find_serial(cert_serial)? {
            None => Ok(Some(CertNotRevoked::assertion())),
            Some(_) => Err(Error::CertRevoked),
        }
    }
}

// https://www.rfc-editor.org/rfc/rfc5280#section-4.2.1.3
#[repr(u8)]
#[derive(Clone, Copy)]
enum KeyUsageMode {
    // DigitalSignature = 0,
    // ContentCommitment = 1,
    // KeyEncipherment = 2,
    // DataEncipherment = 3,
    // KeyAgreement = 4,
    // CertSign = 5,
    CrlSign = 6,
    // EncipherOnly = 7,
    // DecipherOnly = 8,
}

impl KeyUsageMode {
    // https://www.rfc-editor.org/rfc/rfc5280#section-4.2.1.3
    fn check(self, input: Option<untrusted::Input<'_>>) -> Result<(), Error> {
        let bit_string = match input {
            Some(input) => {
                der::expect_tag(&mut untrusted::Reader::new(input), der::Tag::BitString)?
            }
            // While RFC 5280 requires KeyUsage be present, historically the absence of a KeyUsage
            // has been treated as "Any Usage". We follow that convention here and assume the absence
            // of KeyUsage implies the required_ku_bit_if_present we're checking for.
            None => return Ok(()),
        };

        let flags = der::bit_string_flags(bit_string)?;
        #[allow(clippy::as_conversions)] // u8 always fits in usize.
        match flags.bit_set(self as usize) {
            true => Ok(()),
            false => Err(Error::IssuerNotCrlSigner),
        }
    }
}

// When verifying CRL signed data we want to disambiguate the context of possible errors by mapping
// them to CRL specific variants that a consumer can use to tell the issue was with the CRL's
// signature, not a certificate.
fn crl_signature_err(err: Error) -> Error {
    match err {
        #[allow(deprecated)]
        Error::UnsupportedSignatureAlgorithm => Error::UnsupportedCrlSignatureAlgorithm,
        Error::UnsupportedSignatureAlgorithmContext(cx) => {
            Error::UnsupportedCrlSignatureAlgorithmContext(cx)
        }
        #[allow(deprecated)]
        Error::UnsupportedSignatureAlgorithmForPublicKey => {
            Error::UnsupportedCrlSignatureAlgorithmForPublicKey
        }
        Error::UnsupportedSignatureAlgorithmForPublicKeyContext(cx) => {
            Error::UnsupportedCrlSignatureAlgorithmForPublicKeyContext(cx)
        }
        Error::InvalidSignatureForPublicKey => Error::InvalidCrlSignatureForPublicKey,
        _ => err,
    }
}

/// Describes how much of a certificate chain is checked for revocation status.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RevocationCheckDepth {
    /// Only check the end entity (leaf) certificate's revocation status.
    EndEntity,
    /// Check the revocation status of the end entity (leaf) and all intermediates.
    Chain,
}

/// Describes how to handle the case where a certificate's revocation status is unknown.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum UnknownStatusPolicy {
    /// Treat unknown revocation status permissively, acting as if the certificate were
    /// not revoked.
    Allow,
    /// Treat unknown revocation status as an error condition, yielding
    /// [Error::UnknownRevocationStatus].
    Deny,
}

/// Describes how to handle the nextUpdate field of the CRL (i.e. expiration).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExpirationPolicy {
    /// Enforce the verification time is before the time in the nextUpdate field.
    /// Treats an expired CRL as an error condition yielding [Error::CrlExpired].
    Enforce,
    /// Ignore the CRL nextUpdate field.
    Ignore,
}

// Zero-sized marker type representing positive assertion that revocation status was checked
// for a certificate and the result was that the certificate is not revoked.
pub(crate) struct CertNotRevoked(());

impl CertNotRevoked {
    // Construct a CertNotRevoked marker.
    fn assertion() -> Self {
        Self(())
    }
}

#[derive(Debug, Copy, Clone)]
/// An opaque error indicating the caller must provide at least one CRL when building a
/// [RevocationOptions] instance.
pub struct CrlsRequired(pub(crate) ());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // redundant clone, clone_on_copy allowed to verify derived traits.
    #[allow(clippy::redundant_clone, clippy::clone_on_copy)]
    fn test_revocation_opts_builder() {
        // Trying to build a RevocationOptionsBuilder w/o CRLs should err.
        let result = RevocationOptionsBuilder::new(&[]);
        assert!(matches!(result, Err(CrlsRequired(_))));

        // The CrlsRequired error should be debug and clone when alloc is enabled.
        #[cfg(feature = "alloc")]
        {
            let err = result.unwrap_err();
            std::println!("{:?}", err.clone());
        }

        // It should be possible to build a revocation options builder with defaults.
        let crl = include_bytes!("../../tests/crls/crl.valid.der");
        let crl = BorrowedCertRevocationList::from_der(&crl[..])
            .unwrap()
            .into();
        let crls = [&crl];
        let builder = RevocationOptionsBuilder::new(&crls).unwrap();
        #[cfg(feature = "alloc")]
        {
            // The builder should be debug, and clone when alloc is enabled
            std::println!("{builder:?}");
            _ = builder.clone();
        }
        let opts = builder.build();
        assert_eq!(opts.depth, RevocationCheckDepth::Chain);
        assert_eq!(opts.status_policy, UnknownStatusPolicy::Deny);
        assert_eq!(opts.expiration_policy, ExpirationPolicy::Ignore);
        assert_eq!(opts.crls.len(), 1);

        // It should be possible to build a revocation options builder with custom depth.
        let opts = RevocationOptionsBuilder::new(&crls)
            .unwrap()
            .with_depth(RevocationCheckDepth::EndEntity)
            .build();
        assert_eq!(opts.depth, RevocationCheckDepth::EndEntity);
        assert_eq!(opts.status_policy, UnknownStatusPolicy::Deny);
        assert_eq!(opts.expiration_policy, ExpirationPolicy::Ignore);
        assert_eq!(opts.crls.len(), 1);

        // It should be possible to build a revocation options builder that allows unknown
        // revocation status.
        let opts = RevocationOptionsBuilder::new(&crls)
            .unwrap()
            .with_status_policy(UnknownStatusPolicy::Allow)
            .build();
        assert_eq!(opts.depth, RevocationCheckDepth::Chain);
        assert_eq!(opts.status_policy, UnknownStatusPolicy::Allow);
        assert_eq!(opts.expiration_policy, ExpirationPolicy::Ignore);
        assert_eq!(opts.crls.len(), 1);

        // It should be possible to specify both depth and unknown status policy together.
        let opts = RevocationOptionsBuilder::new(&crls)
            .unwrap()
            .with_status_policy(UnknownStatusPolicy::Allow)
            .with_depth(RevocationCheckDepth::EndEntity)
            .build();
        assert_eq!(opts.depth, RevocationCheckDepth::EndEntity);
        assert_eq!(opts.status_policy, UnknownStatusPolicy::Allow);
        assert_eq!(opts.expiration_policy, ExpirationPolicy::Ignore);
        assert_eq!(opts.crls.len(), 1);

        // The same should be true for explicitly forbidding unknown status.
        let opts = RevocationOptionsBuilder::new(&crls)
            .unwrap()
            .with_status_policy(UnknownStatusPolicy::Deny)
            .with_depth(RevocationCheckDepth::EndEntity)
            .build();
        assert_eq!(opts.depth, RevocationCheckDepth::EndEntity);
        assert_eq!(opts.status_policy, UnknownStatusPolicy::Deny);
        assert_eq!(opts.expiration_policy, ExpirationPolicy::Ignore);
        assert_eq!(opts.crls.len(), 1);

        // It should be possible to build a revocation options builder that allows unknown
        // revocation status.
        let opts = RevocationOptionsBuilder::new(&crls)
            .unwrap()
            .with_expiration_policy(ExpirationPolicy::Enforce)
            .build();
        assert_eq!(opts.depth, RevocationCheckDepth::Chain);
        assert_eq!(opts.status_policy, UnknownStatusPolicy::Deny);
        assert_eq!(opts.expiration_policy, ExpirationPolicy::Enforce);
        assert_eq!(opts.crls.len(), 1);

        // Built revocation options should be debug and clone when alloc is enabled.
        #[cfg(feature = "alloc")]
        {
            std::println!("{:?}", opts.clone());
        }
    }
}
