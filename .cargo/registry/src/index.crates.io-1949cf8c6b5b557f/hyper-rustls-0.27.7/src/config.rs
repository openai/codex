#[cfg(feature = "rustls-native-certs")]
use std::io;

#[cfg(any(
    feature = "rustls-platform-verifier",
    feature = "rustls-native-certs",
    feature = "webpki-roots"
))]
use rustls::client::WantsClientCert;
use rustls::{ClientConfig, ConfigBuilder, WantsVerifier};
#[cfg(feature = "rustls-native-certs")]
use rustls_native_certs::CertificateResult;
#[cfg(feature = "rustls-platform-verifier")]
use rustls_platform_verifier::BuilderVerifierExt;

/// Methods for configuring roots
///
/// This adds methods (gated by crate features) for easily configuring
/// TLS server roots a rustls ClientConfig will trust.
pub trait ConfigBuilderExt: sealed::Sealed {
    /// Use the platform's native verifier to verify server certificates.
    ///
    /// See the documentation for [rustls-platform-verifier] for more details.
    ///
    /// # Panics
    ///
    /// Since 0.27.7, this method will panic if the platform verifier cannot be initialized.
    /// Use `try_with_platform_verifier()` instead to handle errors gracefully.
    ///
    /// [rustls-platform-verifier]: https://docs.rs/rustls-platform-verifier
    #[deprecated(since = "0.27.7", note = "use `try_with_platform_verifier` instead")]
    #[cfg(feature = "rustls-platform-verifier")]
    fn with_platform_verifier(self) -> ConfigBuilder<ClientConfig, WantsClientCert>;

    /// Use the platform's native verifier to verify server certificates.
    ///
    /// See the documentation for [rustls-platform-verifier] for more details.
    ///
    /// [rustls-platform-verifier]: https://docs.rs/rustls-platform-verifier
    #[cfg(feature = "rustls-platform-verifier")]
    fn try_with_platform_verifier(
        self,
    ) -> Result<ConfigBuilder<ClientConfig, WantsClientCert>, rustls::Error>;

    /// This configures the platform's trusted certs, as implemented by
    /// rustls-native-certs
    ///
    /// This will return an error if no valid certs were found. In that case,
    /// it's recommended to use `with_webpki_roots`.
    #[cfg(feature = "rustls-native-certs")]
    fn with_native_roots(self) -> Result<ConfigBuilder<ClientConfig, WantsClientCert>, io::Error>;

    /// This configures the webpki roots, which are Mozilla's set of
    /// trusted roots as packaged by webpki-roots.
    #[cfg(feature = "webpki-roots")]
    fn with_webpki_roots(self) -> ConfigBuilder<ClientConfig, WantsClientCert>;
}

impl ConfigBuilderExt for ConfigBuilder<ClientConfig, WantsVerifier> {
    #[cfg(feature = "rustls-platform-verifier")]
    fn with_platform_verifier(self) -> ConfigBuilder<ClientConfig, WantsClientCert> {
        self.try_with_platform_verifier()
            .expect("failure to initialize platform verifier")
    }

    #[cfg(feature = "rustls-platform-verifier")]
    fn try_with_platform_verifier(
        self,
    ) -> Result<ConfigBuilder<ClientConfig, WantsClientCert>, rustls::Error> {
        BuilderVerifierExt::with_platform_verifier(self)
    }

    #[cfg(feature = "rustls-native-certs")]
    #[cfg_attr(not(feature = "logging"), allow(unused_variables))]
    fn with_native_roots(self) -> Result<ConfigBuilder<ClientConfig, WantsClientCert>, io::Error> {
        let mut roots = rustls::RootCertStore::empty();
        let mut valid_count = 0;
        let mut invalid_count = 0;

        let CertificateResult { certs, errors, .. } = rustls_native_certs::load_native_certs();
        if !errors.is_empty() {
            crate::log::warn!("native root CA certificate loading errors: {errors:?}");
        }

        if certs.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no native root CA certificates found (errors: {errors:?})"),
            ));
        }

        for cert in certs {
            match roots.add(cert) {
                Ok(_) => valid_count += 1,
                Err(err) => {
                    crate::log::debug!("certificate parsing failed: {:?}", err);
                    invalid_count += 1
                }
            }
        }

        crate::log::debug!(
            "with_native_roots processed {} valid and {} invalid certs",
            valid_count,
            invalid_count
        );
        if roots.is_empty() {
            crate::log::debug!("no valid native root CA certificates found");
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no valid native root CA certificates found ({invalid_count} invalid)"),
            ))?
        }

        Ok(self.with_root_certificates(roots))
    }

    #[cfg(feature = "webpki-roots")]
    fn with_webpki_roots(self) -> ConfigBuilder<ClientConfig, WantsClientCert> {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(
            webpki_roots::TLS_SERVER_ROOTS
                .iter()
                .cloned(),
        );
        self.with_root_certificates(roots)
    }
}

mod sealed {
    use super::*;

    pub trait Sealed {}

    impl Sealed for ConfigBuilder<ClientConfig, WantsVerifier> {}
}
