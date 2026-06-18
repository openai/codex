use std::sync::Once;

const REQUIRED_SIGNATURE_SCHEME: rustls::SignatureScheme =
    rustls::SignatureScheme::ECDSA_NISTP521_SHA512;

/// Ensures a process-wide rustls crypto provider is installed.
///
/// rustls cannot auto-select a provider when both `ring` and `aws-lc-rs`
/// features are enabled in the dependency graph.
pub fn ensure_rustls_crypto_provider() {
    static RUSTLS_PROVIDER_INIT: Once = Once::new();
    RUSTLS_PROVIDER_INIT.call_once(|| {
        // aws-lc-rs supports a broader WebPKI signature set than ring, including
        // ECDSA P-521/SHA-512 certs used by some enterprise TLS proxies.
        if rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .is_err()
        {
            // Preserve the previous best-effort behavior for embedded hosts that
            // install a process-global provider before Codex can install one.
            return;
        }

        let Some(provider) = rustls::crypto::CryptoProvider::get_default() else {
            panic!("aws-lc-rs rustls crypto provider should be installed");
        };
        assert!(
            provider_supports_required_signature_scheme(provider),
            "installed rustls crypto provider must support {REQUIRED_SIGNATURE_SCHEME:?}"
        );
    });
}

fn provider_supports_required_signature_scheme(provider: &rustls::crypto::CryptoProvider) -> bool {
    provider
        .signature_verification_algorithms
        .supported_schemes()
        .contains(&REQUIRED_SIGNATURE_SCHEME)
}
