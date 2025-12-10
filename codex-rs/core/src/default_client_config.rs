//! Configuration types for the default HTTP client.

use codex_utils_absolute_path::AbsolutePathBuf;

/// TLS configuration for HTTP clients.
/// Used when building reqwest clients with custom CA certificates or mTLS.
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Path to a custom CA certificate (PEM format)
    pub ca_certificate: Option<AbsolutePathBuf>,
    /// Path to the client certificate (PEM format) for mutual TLS
    pub client_certificate: Option<AbsolutePathBuf>,
    /// Path to the client private key (PEM format) for mutual TLS
    pub client_private_key: Option<AbsolutePathBuf>,
}
