use alloc::vec::Vec;

use pki_types::{CertificateDer, CertificateRevocationListDer, UnixTime};
use webpki::{CertRevocationList, ExpirationPolicy, RevocationCheckDepth, UnknownStatusPolicy};

use super::{VerifierBuilderError, pki_error};
#[cfg(doc)]
use crate::ConfigBuilder;
#[cfg(doc)]
use crate::crypto;
use crate::crypto::{CryptoProvider, WebPkiSupportedAlgorithms};
#[cfg(doc)]
use crate::server::ServerConfig;
use crate::sync::Arc;
use crate::verify::{
    ClientCertVerified, ClientCertVerifier, DigitallySignedStruct, HandshakeSignatureValid,
    NoClientAuth,
};
use crate::webpki::parse_crls;
use crate::webpki::verify::{ParsedCertificate, verify_tls12_signature, verify_tls13_signature};
use crate::{DistinguishedName, Error, RootCertStore, SignatureScheme};

/// A builder for configuring a `webpki` client certificate verifier.
///
/// For more information, see the [`WebPkiClientVerifier`] documentation.
#[derive(Debug, Clone)]
pub struct ClientCertVerifierBuilder {
    roots: Arc<RootCertStore>,
    root_hint_subjects: Vec<DistinguishedName>,
    crls: Vec<CertificateRevocationListDer<'static>>,
    revocation_check_depth: RevocationCheckDepth,
    unknown_revocation_policy: UnknownStatusPolicy,
    revocation_expiration_policy: ExpirationPolicy,
    anon_policy: AnonymousClientPolicy,
    supported_algs: WebPkiSupportedAlgorithms,
}

impl ClientCertVerifierBuilder {
    pub(crate) fn new(
        roots: Arc<RootCertStore>,
        supported_algs: WebPkiSupportedAlgorithms,
    ) -> Self {
        Self {
            root_hint_subjects: roots.subjects(),
            roots,
            crls: Vec::new(),
            anon_policy: AnonymousClientPolicy::Deny,
            revocation_check_depth: RevocationCheckDepth::Chain,
            unknown_revocation_policy: UnknownStatusPolicy::Deny,
            revocation_expiration_policy: ExpirationPolicy::Ignore,
            supported_algs,
        }
    }

    /// Clear the list of trust anchor hint subjects.
    ///
    /// By default, the client cert verifier will use the subjects provided by the root cert
    /// store configured for client authentication. Calling this function will remove these
    /// hint subjects, indicating the client should make a free choice of which certificate
    /// to send.
    ///
    /// See [`ClientCertVerifier::root_hint_subjects`] for more information on
    /// circumstances where you may want to clear the default hint subjects.
    pub fn clear_root_hint_subjects(mut self) -> Self {
        self.root_hint_subjects = Vec::default();
        self
    }

    /// Add additional [`DistinguishedName`]s to the list of trust anchor hint subjects.
    ///
    /// By default, the client cert verifier will use the subjects provided by the root cert
    /// store configured for client authentication. Calling this function will add to these
    /// existing hint subjects. Calling this function with empty `subjects` will have no
    /// effect.
    ///
    /// See [`ClientCertVerifier::root_hint_subjects`] for more information on
    /// circumstances where you may want to override the default hint subjects.
    pub fn add_root_hint_subjects(
        mut self,
        subjects: impl IntoIterator<Item = DistinguishedName>,
    ) -> Self {
        self.root_hint_subjects.extend(subjects);
        self
    }

    /// Verify the revocation state of presented client certificates against the provided
    /// certificate revocation lists (CRLs). Calling `with_crls` multiple times appends the
    /// given CRLs to the existing collection.
    ///
    /// By default all certificates in the verified chain built from the presented client
    /// certificate to a trust anchor will have their revocation status checked. Calling
    /// [`only_check_end_entity_revocation`][Self::only_check_end_entity_revocation] will
    /// change this behavior to only check the end entity client certificate.
    ///
    /// By default if a certificate's revocation status can not be determined using the
    /// configured CRLs, it will be treated as an error. Calling
    /// [`allow_unknown_revocation_status`][Self::allow_unknown_revocation_status] will change
    /// this behavior to allow unknown revocation status.
    pub fn with_crls(
        mut self,
        crls: impl IntoIterator<Item = CertificateRevocationListDer<'static>>,
    ) -> Self {
        self.crls.extend(crls);
        self
    }

    /// Only check the end entity certificate revocation status when using CRLs.
    ///
    /// If CRLs are provided using [`with_crls`][Self::with_crls] only check the end entity
    /// certificate's revocation status. Overrides the default behavior of checking revocation
    /// status for each certificate in the verified chain built to a trust anchor
    /// (excluding the trust anchor itself).
    ///
    /// If no CRLs are provided then this setting has no effect. Neither the end entity certificate
    /// or any intermediates will have revocation status checked.
    pub fn only_check_end_entity_revocation(mut self) -> Self {
        self.revocation_check_depth = RevocationCheckDepth::EndEntity;
        self
    }

    /// Allow unauthenticated clients to connect.
    ///
    /// Clients that offer a client certificate issued by a trusted root, and clients that offer no
    /// client certificate will be allowed to connect.
    pub fn allow_unauthenticated(mut self) -> Self {
        self.anon_policy = AnonymousClientPolicy::Allow;
        self
    }

    /// Allow unknown certificate revocation status when using CRLs.
    ///
    /// If CRLs are provided with [`with_crls`][Self::with_crls] and it isn't possible to
    /// determine the revocation status of a certificate, do not treat it as an error condition.
    /// Overrides the default behavior where unknown revocation status is considered an error.
    ///
    /// If no CRLs are provided then this setting has no effect as revocation status checks
    /// are not performed.
    pub fn allow_unknown_revocation_status(mut self) -> Self {
        self.unknown_revocation_policy = UnknownStatusPolicy::Allow;
        self
    }

    /// Enforce the CRL nextUpdate field (i.e. expiration)
    ///
    /// If CRLs are provided with [`with_crls`][Self::with_crls] and the verification time is
    /// beyond the time in the CRL nextUpdate field, it is expired and treated as an error condition.
    /// Overrides the default behavior where expired CRLs are not treated as an error condition.
    ///
    /// If no CRLs are provided then this setting has no effect as revocation status checks
    /// are not performed.
    pub fn enforce_revocation_expiration(mut self) -> Self {
        self.revocation_expiration_policy = ExpirationPolicy::Enforce;
        self
    }

    /// Build a client certificate verifier. The built verifier will be used for the server to offer
    /// client certificate authentication, to control how offered client certificates are validated,
    /// and to determine what to do with anonymous clients that do not respond to the client
    /// certificate authentication offer with a client certificate.
    ///
    /// If `with_signature_verification_algorithms` was not called on the builder, a default set of
    /// signature verification algorithms is used, controlled by the selected [`CryptoProvider`].
    ///
    /// Once built, the provided `Arc<dyn ClientCertVerifier>` can be used with a Rustls
    /// [`ServerConfig`] to configure client certificate validation using
    /// [`with_client_cert_verifier`][ConfigBuilder<ClientConfig, WantsVerifier>::with_client_cert_verifier].
    ///
    /// # Errors
    /// This function will return a [`VerifierBuilderError`] if:
    /// 1. No trust anchors have been provided.
    /// 2. DER encoded CRLs have been provided that can not be parsed successfully.
    pub fn build(self) -> Result<Arc<dyn ClientCertVerifier>, VerifierBuilderError> {
        if self.roots.is_empty() {
            return Err(VerifierBuilderError::NoRootAnchors);
        }

        Ok(Arc::new(WebPkiClientVerifier::new(
            self.roots,
            self.root_hint_subjects,
            parse_crls(self.crls)?,
            self.revocation_check_depth,
            self.unknown_revocation_policy,
            self.revocation_expiration_policy,
            self.anon_policy,
            self.supported_algs,
        )))
    }
}

/// A client certificate verifier that uses the `webpki` crate[^1] to perform client certificate
/// validation.
///
/// It must be created via the [`WebPkiClientVerifier::builder()`] or
/// [`WebPkiClientVerifier::builder_with_provider()`] functions.
///
/// Once built, the provided `Arc<dyn ClientCertVerifier>` can be used with a Rustls [`ServerConfig`]
/// to configure client certificate validation using [`with_client_cert_verifier`][ConfigBuilder<ClientConfig, WantsVerifier>::with_client_cert_verifier].
///
/// Example:
///
/// To require all clients present a client certificate issued by a trusted CA:
/// ```no_run
/// # #[cfg(any(feature = "ring", feature = "aws_lc_rs"))] {
/// # use rustls::RootCertStore;
/// # use rustls::server::WebPkiClientVerifier;
/// # let roots = RootCertStore::empty();
/// let client_verifier = WebPkiClientVerifier::builder(roots.into())
///   .build()
///   .unwrap();
/// # }
/// ```
///
/// Or, to allow clients presenting a client certificate authenticated by a trusted CA, or
/// anonymous clients that present no client certificate:
/// ```no_run
/// # #[cfg(any(feature = "ring", feature = "aws_lc_rs"))] {
/// # use rustls::RootCertStore;
/// # use rustls::server::WebPkiClientVerifier;
/// # let roots = RootCertStore::empty();
/// let client_verifier = WebPkiClientVerifier::builder(roots.into())
///   .allow_unauthenticated()
///   .build()
///   .unwrap();
/// # }
/// ```
///
/// If you wish to disable advertising client authentication:
/// ```no_run
/// # use rustls::RootCertStore;
/// # use rustls::server::WebPkiClientVerifier;
/// # let roots = RootCertStore::empty();
/// let client_verifier = WebPkiClientVerifier::no_client_auth();
/// ```
///
/// You can also configure the client verifier to check for certificate revocation with
/// client certificate revocation lists (CRLs):
/// ```no_run
/// # #[cfg(any(feature = "ring", feature = "aws_lc_rs"))] {
/// # use rustls::RootCertStore;
/// # use rustls::server::{WebPkiClientVerifier};
/// # let roots = RootCertStore::empty();
/// # let crls = Vec::new();
/// let client_verifier = WebPkiClientVerifier::builder(roots.into())
///   .with_crls(crls)
///   .build()
///   .unwrap();
/// # }
/// ```
///
/// [^1]: <https://github.com/rustls/webpki>
#[derive(Debug)]
pub struct WebPkiClientVerifier {
    roots: Arc<RootCertStore>,
    root_hint_subjects: Vec<DistinguishedName>,
    crls: Vec<CertRevocationList<'static>>,
    revocation_check_depth: RevocationCheckDepth,
    unknown_revocation_policy: UnknownStatusPolicy,
    revocation_expiration_policy: ExpirationPolicy,
    anonymous_policy: AnonymousClientPolicy,
    supported_algs: WebPkiSupportedAlgorithms,
}

impl WebPkiClientVerifier {
    /// Create a builder for the `webpki` client certificate verifier configuration using
    /// the [process-default `CryptoProvider`][CryptoProvider#using-the-per-process-default-cryptoprovider].
    ///
    /// Client certificate authentication will be offered by the server, and client certificates
    /// will be verified using the trust anchors found in the provided `roots`. If you
    /// wish to disable client authentication use [`WebPkiClientVerifier::no_client_auth()`] instead.
    ///
    /// Use [`Self::builder_with_provider`] if you wish to specify an explicit provider.
    ///
    /// For more information, see the [`ClientCertVerifierBuilder`] documentation.
    pub fn builder(roots: Arc<RootCertStore>) -> ClientCertVerifierBuilder {
        Self::builder_with_provider(
            roots,
            CryptoProvider::get_default_or_install_from_crate_features().clone(),
        )
    }

    /// Create a builder for the `webpki` client certificate verifier configuration using
    /// a specified [`CryptoProvider`].
    ///
    /// Client certificate authentication will be offered by the server, and client certificates
    /// will be verified using the trust anchors found in the provided `roots`. If you
    /// wish to disable client authentication use [WebPkiClientVerifier::no_client_auth()] instead.
    ///
    /// The cryptography used comes from the specified [`CryptoProvider`].
    ///
    /// For more information, see the [`ClientCertVerifierBuilder`] documentation.
    pub fn builder_with_provider(
        roots: Arc<RootCertStore>,
        provider: Arc<CryptoProvider>,
    ) -> ClientCertVerifierBuilder {
        ClientCertVerifierBuilder::new(roots, provider.signature_verification_algorithms)
    }

    /// Create a new `WebPkiClientVerifier` that disables client authentication. The server will
    /// not offer client authentication and anonymous clients will be accepted.
    ///
    /// This is in contrast to using `WebPkiClientVerifier::builder().allow_unauthenticated().build()`,
    /// which will produce a verifier that will offer client authentication, but not require it.
    pub fn no_client_auth() -> Arc<dyn ClientCertVerifier> {
        Arc::new(NoClientAuth {})
    }

    /// Construct a new `WebpkiClientVerifier`.
    ///
    /// * `roots` is a list of trust anchors to use for certificate validation.
    /// * `root_hint_subjects` is a list of distinguished names to use for hinting acceptable
    ///   certificate authority subjects to a client.
    /// * `crls` is a `Vec` of owned certificate revocation lists (CRLs) to use for
    ///   client certificate validation.
    /// * `revocation_check_depth` controls which certificates have their revocation status checked
    ///   when `crls` are provided.
    /// * `unknown_revocation_policy` controls how certificates with an unknown revocation status
    ///   are handled when `crls` are provided.
    /// * `anonymous_policy` controls whether client authentication is required, or if anonymous
    ///   clients can connect.
    /// * `supported_algs` specifies which signature verification algorithms should be used.
    pub(crate) fn new(
        roots: Arc<RootCertStore>,
        root_hint_subjects: Vec<DistinguishedName>,
        crls: Vec<CertRevocationList<'static>>,
        revocation_check_depth: RevocationCheckDepth,
        unknown_revocation_policy: UnknownStatusPolicy,
        revocation_expiration_policy: ExpirationPolicy,
        anonymous_policy: AnonymousClientPolicy,
        supported_algs: WebPkiSupportedAlgorithms,
    ) -> Self {
        Self {
            roots,
            root_hint_subjects,
            crls,
            revocation_check_depth,
            unknown_revocation_policy,
            revocation_expiration_policy,
            anonymous_policy,
            supported_algs,
        }
    }
}

impl ClientCertVerifier for WebPkiClientVerifier {
    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_mandatory(&self) -> bool {
        match self.anonymous_policy {
            AnonymousClientPolicy::Allow => false,
            AnonymousClientPolicy::Deny => true,
        }
    }

    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &self.root_hint_subjects
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        now: UnixTime,
    ) -> Result<ClientCertVerified, Error> {
        let cert = ParsedCertificate::try_from(end_entity)?;

        let crl_refs = self.crls.iter().collect::<Vec<_>>();

        let revocation = if self.crls.is_empty() {
            None
        } else {
            Some(
                webpki::RevocationOptionsBuilder::new(&crl_refs)
                    // Note: safe to unwrap here - new is only fallible if no CRLs are provided
                    //       and we verify this above.
                    .unwrap()
                    .with_depth(self.revocation_check_depth)
                    .with_status_policy(self.unknown_revocation_policy)
                    .with_expiration_policy(self.revocation_expiration_policy)
                    .build(),
            )
        };

        cert.0
            .verify_for_usage(
                self.supported_algs.all,
                &self.roots.roots,
                intermediates,
                now,
                webpki::KeyUsage::client_auth(),
                revocation,
                None,
            )
            .map_err(pki_error)
            .map(|_| ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(message, cert, dss, &self.supported_algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(message, cert, dss, &self.supported_algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.supported_algs.supported_schemes()
    }
}

/// Controls how the [WebPkiClientVerifier] handles anonymous clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnonymousClientPolicy {
    /// Clients that do not present a client certificate are allowed.
    Allow,
    /// Clients that do not present a client certificate are denied.
    Deny,
}

#[cfg(test)]
#[macro_rules_attribute::apply(test_for_each_provider)]
mod tests {
    use std::prelude::v1::*;
    use std::{format, println, vec};

    use pki_types::pem::PemObject;
    use pki_types::{CertificateDer, CertificateRevocationListDer};

    use super::{WebPkiClientVerifier, provider};
    use crate::RootCertStore;
    use crate::server::VerifierBuilderError;
    use crate::sync::Arc;

    fn load_crls(crls_der: &[&[u8]]) -> Vec<CertificateRevocationListDer<'static>> {
        crls_der
            .iter()
            .map(|pem_bytes| CertificateRevocationListDer::from_pem_slice(pem_bytes).unwrap())
            .collect()
    }

    fn test_crls() -> Vec<CertificateRevocationListDer<'static>> {
        load_crls(&[
            include_bytes!("../../../test-ca/ecdsa-p256/client.revoked.crl.pem").as_slice(),
            include_bytes!("../../../test-ca/rsa-2048/client.revoked.crl.pem").as_slice(),
        ])
    }

    fn load_roots(roots_der: &[&[u8]]) -> Arc<RootCertStore> {
        let mut roots = RootCertStore::empty();
        roots_der.iter().for_each(|der| {
            roots
                .add(CertificateDer::from(der.to_vec()))
                .unwrap()
        });
        roots.into()
    }

    fn test_roots() -> Arc<RootCertStore> {
        load_roots(&[
            include_bytes!("../../../test-ca/ecdsa-p256/ca.der").as_slice(),
            include_bytes!("../../../test-ca/rsa-2048/ca.der").as_slice(),
        ])
    }

    #[test]
    fn test_client_verifier_no_auth() {
        // We should be able to build a verifier that turns off client authentication.
        WebPkiClientVerifier::no_client_auth();
    }

    #[test]
    fn test_client_verifier_required_auth() {
        // We should be able to build a verifier that requires client authentication, and does
        // no revocation checking.
        let builder = WebPkiClientVerifier::builder_with_provider(
            test_roots(),
            provider::default_provider().into(),
        );
        // The builder should be Debug.
        println!("{builder:?}");
        builder.build().unwrap();
    }

    #[test]
    fn test_client_verifier_optional_auth() {
        // We should be able to build a verifier that allows client authentication, and anonymous
        // access, and does no revocation checking.
        let builder = WebPkiClientVerifier::builder_with_provider(
            test_roots(),
            provider::default_provider().into(),
        )
        .allow_unauthenticated();
        // The builder should be Debug.
        println!("{builder:?}");
        builder.build().unwrap();
    }

    #[test]
    fn test_client_verifier_without_crls_required_auth() {
        // We should be able to build a verifier that requires client authentication, and does
        // no revocation checking, that hasn't been configured to determine how to handle
        // unauthenticated clients yet.
        let builder = WebPkiClientVerifier::builder_with_provider(
            test_roots(),
            provider::default_provider().into(),
        );
        // The builder should be Debug.
        println!("{builder:?}");
        builder.build().unwrap();
    }

    #[test]
    fn test_client_verifier_without_crls_opptional_auth() {
        // We should be able to build a verifier that allows client authentication,
        // and anonymous access, that does no revocation checking.
        let builder = WebPkiClientVerifier::builder_with_provider(
            test_roots(),
            provider::default_provider().into(),
        )
        .allow_unauthenticated();
        // The builder should be Debug.
        println!("{builder:?}");
        builder.build().unwrap();
    }

    #[test]
    fn test_with_invalid_crls() {
        // Trying to build a client verifier with invalid CRLs should error at build time.
        let result = WebPkiClientVerifier::builder_with_provider(
            test_roots(),
            provider::default_provider().into(),
        )
        .with_crls(vec![CertificateRevocationListDer::from(vec![0xFF])])
        .build();
        assert!(matches!(result, Err(VerifierBuilderError::InvalidCrl(_))));
    }

    #[test]
    fn test_with_crls_multiple_calls() {
        // We should be able to call `with_crls` on a client verifier multiple times.
        let initial_crls = test_crls();
        let extra_crls =
            load_crls(&[
                include_bytes!("../../../test-ca/eddsa/client.revoked.crl.pem").as_slice(),
            ]);
        let builder = WebPkiClientVerifier::builder_with_provider(
            test_roots(),
            provider::default_provider().into(),
        )
        .with_crls(initial_crls.clone())
        .with_crls(extra_crls.clone());

        // There should be the expected number of crls.
        assert_eq!(builder.crls.len(), initial_crls.len() + extra_crls.len());
        // The builder should be Debug.
        println!("{builder:?}");
        builder.build().unwrap();
    }

    #[test]
    fn test_client_verifier_with_crls_required_auth_implicit() {
        // We should be able to build a verifier that requires client authentication, and that does
        // revocation checking with CRLs, and that does not allow any anonymous access.
        let builder = WebPkiClientVerifier::builder_with_provider(
            test_roots(),
            provider::default_provider().into(),
        )
        .with_crls(test_crls());
        // The builder should be Debug.
        println!("{builder:?}");
        builder.build().unwrap();
    }

    #[test]
    fn test_client_verifier_with_crls_optional_auth() {
        // We should be able to build a verifier that supports client authentication, that does
        // revocation checking with CRLs, and that allows anonymous access.
        let builder = WebPkiClientVerifier::builder_with_provider(
            test_roots(),
            provider::default_provider().into(),
        )
        .with_crls(test_crls())
        .allow_unauthenticated();
        // The builder should be Debug.
        println!("{builder:?}");
        builder.build().unwrap();
    }

    #[test]
    fn test_client_verifier_ee_only() {
        // We should be able to build a client verifier that only checks EE revocation status.
        let builder = WebPkiClientVerifier::builder_with_provider(
            test_roots(),
            provider::default_provider().into(),
        )
        .with_crls(test_crls())
        .only_check_end_entity_revocation();
        // The builder should be Debug.
        println!("{builder:?}");
        builder.build().unwrap();
    }

    #[test]
    fn test_client_verifier_allow_unknown() {
        // We should be able to build a client verifier that allows unknown revocation status
        let builder = WebPkiClientVerifier::builder_with_provider(
            test_roots(),
            provider::default_provider().into(),
        )
        .with_crls(test_crls())
        .allow_unknown_revocation_status();
        // The builder should be Debug.
        println!("{builder:?}");
        builder.build().unwrap();
    }

    #[test]
    fn test_client_verifier_enforce_expiration() {
        // We should be able to build a client verifier that allows unknown revocation status
        let builder = WebPkiClientVerifier::builder_with_provider(
            test_roots(),
            provider::default_provider().into(),
        )
        .with_crls(test_crls())
        .enforce_revocation_expiration();
        // The builder should be Debug.
        println!("{builder:?}");
        builder.build().unwrap();
    }

    #[test]
    fn test_builder_no_roots() {
        // Trying to create a client verifier builder with no trust anchors should fail at build time
        let result = WebPkiClientVerifier::builder_with_provider(
            RootCertStore::empty().into(),
            provider::default_provider().into(),
        )
        .build();
        assert!(matches!(result, Err(VerifierBuilderError::NoRootAnchors)));
    }

    #[test]
    fn smoke() {
        let all = vec![
            VerifierBuilderError::NoRootAnchors,
            VerifierBuilderError::InvalidCrl(crate::CertRevocationListError::ParseError),
        ];

        for err in all {
            let _ = format!("{err:?}");
            let _ = format!("{err}");
        }
    }
}
