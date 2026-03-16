use std::sync::Once;

/// Ensures a process-wide rustls crypto provider is installed.
///
/// rustls cannot auto-select a provider when both `ring` and `aws-lc-rs`
/// features are enabled in the dependency graph.
pub fn ensure_rustls_crypto_provider() {
    static RUSTLS_PROVIDER_INIT: Once = Once::new();
    RUSTLS_PROVIDER_INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}
