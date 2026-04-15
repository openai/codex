use std::io;

use thiserror::Error;

pub const CODEX_CA_CERT_ENV: &str = "CODEX_CA_CERTIFICATE";
pub const SSL_CERT_FILE_ENV: &str = "SSL_CERT_FILE";

#[derive(Debug, Error)]
pub enum BuildCustomCaTransportError {
    #[error("failed to build HTTP client while using browser transport defaults: {0}")]
    BuildClientWithSystemRoots(#[source] reqwest::Error),
}

impl From<BuildCustomCaTransportError> for io::Error {
    fn from(error: BuildCustomCaTransportError) -> Self {
        io::Error::other(error)
    }
}

pub fn build_reqwest_client_with_custom_ca(
    builder: reqwest::ClientBuilder,
) -> Result<reqwest::Client, BuildCustomCaTransportError> {
    builder
        .build()
        .map_err(BuildCustomCaTransportError::BuildClientWithSystemRoots)
}

pub fn build_reqwest_client_for_subprocess_tests(
    builder: reqwest::ClientBuilder,
) -> Result<reqwest::Client, BuildCustomCaTransportError> {
    build_reqwest_client_with_custom_ca(builder)
}

pub fn maybe_build_rustls_client_config_with_custom_ca()
-> Result<Option<()>, BuildCustomCaTransportError> {
    Ok(None)
}
