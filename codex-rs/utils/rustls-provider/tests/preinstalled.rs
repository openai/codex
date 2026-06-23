use codex_utils_rustls_provider::ensure_rustls_crypto_provider;

const EMPTY_ALGORITHMS: rustls::crypto::WebPkiSupportedAlgorithms =
    rustls::crypto::WebPkiSupportedAlgorithms {
        all: &[],
        mapping: &[],
    };

#[test]
fn ensure_provider_preserves_preinstalled_provider() {
    let mut provider = rustls::crypto::aws_lc_rs::default_provider();
    provider.signature_verification_algorithms = EMPTY_ALGORITHMS;
    assert!(provider.install_default().is_ok());

    ensure_rustls_crypto_provider();

    let Some(provider) = rustls::crypto::CryptoProvider::get_default() else {
        panic!("preinstalled rustls provider should still be installed");
    };
    assert!(
        !provider
            .signature_verification_algorithms
            .supported_schemes()
            .contains(&rustls::SignatureScheme::ECDSA_NISTP521_SHA512)
    );
}
