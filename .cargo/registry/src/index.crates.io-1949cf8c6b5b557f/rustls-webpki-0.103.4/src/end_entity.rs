// Copyright 2015-2021 Brian Smith.
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

use core::ops::Deref;

use pki_types::{
    CertificateDer, ServerName, SignatureVerificationAlgorithm, TrustAnchor, UnixTime,
};

use crate::crl::RevocationOptions;
use crate::error::Error;
use crate::subject_name::{verify_dns_names, verify_ip_address_names};
use crate::verify_cert::{self, KeyUsage, VerifiedPath};
use crate::{cert, signed_data};

/// An end-entity certificate.
///
/// Server certificate processing in a TLS connection consists of several
/// steps. All of these steps are necessary:
///
/// * [`EndEntityCert::verify_for_usage()`]: Verify that the peer's certificate
///   is valid for the current usage scenario. For server authentication, use
///   [`KeyUsage::server_auth()`].
/// * [`EndEntityCert::verify_is_valid_for_subject_name()`]: Verify that the server's
///   certificate is valid for the host or IP address that is being connected to.
/// * [`EndEntityCert::verify_signature()`]: Verify that the signature of server's
///   `ServerKeyExchange` message is valid for the server's certificate.
///
/// Client certificate processing in a TLS connection consists of analogous
/// steps. All of these steps are necessary:
///
/// * [`EndEntityCert::verify_for_usage()`]: Verify that the peer's certificate
///   is valid for the current usage scenario. For client authentication, use
///   [`KeyUsage::client_auth()`].
/// * [`EndEntityCert::verify_signature()`]: Verify that the signature of client's
///   `CertificateVerify` message is valid using the public key from the
///   client's certificate.
///
/// Although it would be less error-prone to combine all these steps into a
/// single function call, some significant optimizations are possible if the
/// three steps are processed separately (in parallel). It does not matter much
/// which order the steps are done in, but **all of these steps must completed
/// before application data is sent and before received application data is
/// processed**. The [`TryFrom`] conversion from `&CertificateDer<'_>` is an
/// inexpensive operation and is deterministic, so if these tasks are done in
/// multiple threads, it is probably best to just create multiple [`EndEntityCert`]
/// instances for the same DER-encoded ASN.1 certificate bytes.
pub struct EndEntityCert<'a> {
    inner: cert::Cert<'a>,
}

impl<'a> TryFrom<&'a CertificateDer<'a>> for EndEntityCert<'a> {
    type Error = Error;

    /// Parse the ASN.1 DER-encoded X.509 encoding of the certificate
    /// `cert_der`.
    fn try_from(cert: &'a CertificateDer<'a>) -> Result<Self, Self::Error> {
        Ok(Self {
            inner: cert::Cert::from_der(untrusted::Input::from(cert.as_ref()))?,
        })
    }
}

impl EndEntityCert<'_> {
    /// Verifies that the end-entity certificate is valid for use against the
    /// specified Extended Key Usage (EKU).
    ///
    /// * `supported_sig_algs` is the list of signature algorithms that are
    ///   trusted for use in certificate signatures; the end-entity certificate's
    ///   public key is not validated against this list.
    /// * `trust_anchors` is the list of root CAs to trust in the built path.
    /// * `intermediate_certs` is the sequence of intermediate certificates that
    ///   a peer sent for the purpose of path building.
    /// * `time` is the time for which the validation is effective (usually the
    ///   current time).
    /// * `usage` is the intended usage of the certificate, indicating what kind
    ///   of usage we're verifying the certificate for.
    /// * `crls` is the list of certificate revocation lists to check
    ///   the certificate against.
    /// * `verify_path` is an optional verification function for path candidates.
    ///
    /// If successful, yields a `VerifiedPath` type that can be used to inspect a verified chain
    /// of certificates that leads from the `end_entity` to one of the `self.trust_anchors`.
    ///
    /// `verify_path` will only be called for potentially verified paths, that is, paths that
    /// have been verified up to the trust anchor. As such, `verify_path()` cannot be used to
    /// verify a path that doesn't satisfy the constraints listed above; it can only be used to
    /// reject a path that does satisfy the aforementioned constraints. If `verify_path` returns
    /// an error, path building will continue in order to try other options.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_for_usage<'p>(
        &'p self,
        supported_sig_algs: &[&dyn SignatureVerificationAlgorithm],
        trust_anchors: &'p [TrustAnchor<'_>],
        intermediate_certs: &'p [CertificateDer<'p>],
        time: UnixTime,
        usage: KeyUsage,
        revocation: Option<RevocationOptions<'_>>,
        verify_path: Option<&dyn Fn(&VerifiedPath<'_>) -> Result<(), Error>>,
    ) -> Result<VerifiedPath<'p>, Error> {
        verify_cert::ChainOptions {
            eku: usage,
            supported_sig_algs,
            trust_anchors,
            intermediate_certs,
            revocation,
        }
        .build_chain(self, time, verify_path)
    }

    /// Verifies that the certificate is valid for the given Subject Name.
    pub fn verify_is_valid_for_subject_name(
        &self,
        server_name: &ServerName<'_>,
    ) -> Result<(), Error> {
        match server_name {
            ServerName::DnsName(dns_name) => verify_dns_names(dns_name, &self.inner),
            // IP addresses are not compared against the subject field;
            // only against Subject Alternative Names.
            ServerName::IpAddress(ip_address) => verify_ip_address_names(ip_address, &self.inner),
            _ => Err(Error::UnsupportedNameType),
        }
    }

    /// Verifies the signature `signature` of message `msg` using the
    /// certificate's public key.
    ///
    /// `signature_alg` is the algorithm to use to
    /// verify the signature; the certificate's public key is verified to be
    /// compatible with this algorithm.
    ///
    /// For TLS 1.2, `signature` corresponds to TLS's
    /// `DigitallySigned.signature` and `signature_alg` corresponds to TLS's
    /// `DigitallySigned.algorithm` of TLS type `SignatureAndHashAlgorithm`. In
    /// TLS 1.2 a single `SignatureAndHashAlgorithm` may map to multiple
    /// `SignatureVerificationAlgorithm`s. For example, a TLS 1.2
    /// `SignatureAndHashAlgorithm` of (ECDSA, SHA-256) may map to any or all
    /// of {`ECDSA_P256_SHA256`, `ECDSA_P384_SHA256`}, depending on how the TLS
    /// implementation is configured.
    ///
    /// For current TLS 1.3 drafts, `signature_alg` corresponds to TLS's
    /// `algorithm` fields of type `SignatureScheme`. There is (currently) a
    /// one-to-one correspondence between TLS 1.3's `SignatureScheme` and
    /// `SignatureVerificationAlgorithm`.
    pub fn verify_signature(
        &self,
        signature_alg: &dyn SignatureVerificationAlgorithm,
        msg: &[u8],
        signature: &[u8],
    ) -> Result<(), Error> {
        signed_data::verify_signature(
            signature_alg,
            self.inner.spki,
            untrusted::Input::from(msg),
            untrusted::Input::from(signature),
        )
    }
}

impl<'a> Deref for EndEntityCert<'a> {
    type Target = cert::Cert<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[cfg(feature = "alloc")]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;
    use crate::test_utils::RCGEN_SIGNATURE_ALG;
    use std::prelude::v1::*;

    // This test reproduces https://github.com/rustls/webpki/issues/167 --- an
    // end-entity cert where the common name is a `PrintableString` rather than
    // a `UTF8String` cannot iterate over its subject alternative names.
    #[test]
    fn printable_string_common_name() {
        const DNS_NAME: &str = "test.example.com";

        let issuer = test_utils::make_issuer("Test").0;

        let ee_cert = {
            let mut params = test_utils::end_entity_params(vec![DNS_NAME.to_string()]);
            // construct a certificate that uses `PrintableString` as the
            // common name value, rather than `UTF8String`.
            params.distinguished_name.push(
                rcgen::DnType::CommonName,
                rcgen::DnValue::PrintableString(
                    rcgen::string::PrintableString::try_from("example.com").unwrap(),
                ),
            );
            params
                .signed_by(
                    &rcgen::KeyPair::generate_for(RCGEN_SIGNATURE_ALG).unwrap(),
                    &issuer,
                )
                .expect("failed to make ee cert (this is a test bug)")
        };

        expect_dns_name(ee_cert.der(), DNS_NAME);
    }

    // This test reproduces https://github.com/rustls/webpki/issues/167 --- an
    // end-entity cert where the common name is an empty SEQUENCE.
    #[test]
    fn empty_sequence_common_name() {
        let ee_cert_der = {
            // handcrafted cert DER produced using `ascii2der`, since `rcgen` is
            // unwilling to generate this particular weird cert.
            let bytes = include_bytes!("../tests/misc/empty_sequence_common_name.der");
            CertificateDer::from(&bytes[..])
        };
        expect_dns_name(&ee_cert_der, "example.com");
    }

    fn expect_dns_name(der: &CertificateDer<'_>, name: &str) {
        let cert =
            EndEntityCert::try_from(der).expect("should parse end entity certificate correctly");

        let mut names = cert.valid_dns_names();
        assert_eq!(names.next(), Some(name));
        assert_eq!(names.next(), None);
    }
}
