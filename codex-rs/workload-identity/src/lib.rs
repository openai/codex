mod assertion;
mod exchange;

use std::path::PathBuf;
use std::sync::Arc;

pub use exchange::WorkloadIdentityExchange;
pub use exchange::WorkloadIdentityToken;
use thiserror::Error;

/// The inputs needed to exchange a file-backed assertion for ChatGPT auth.
#[derive(Clone)]
pub struct WorkloadIdentityConfig {
    pub(crate) assertion_file: PathBuf,
    pub(crate) federation_rule_id: String,
}

impl WorkloadIdentityConfig {
    pub fn new(
        federation_rule_id: String,
        assertion_file: PathBuf,
    ) -> Result<Self, WorkloadIdentityError> {
        let federation_rule_id = federation_rule_id.trim();
        if federation_rule_id.is_empty() {
            return Err(WorkloadIdentityError::InvalidFederationRuleId);
        }
        if !assertion_file.is_absolute() {
            return Err(WorkloadIdentityError::AssertionFileMustBeAbsolute);
        }
        Ok(Self {
            assertion_file,
            federation_rule_id: federation_rule_id.to_string(),
        })
    }
}

#[derive(Clone, Debug, Error)]
pub enum WorkloadIdentityError {
    #[error("the workload identity federation rule ID must not be empty")]
    InvalidFederationRuleId,
    #[error("the workload identity assertion file path must be absolute")]
    AssertionFileMustBeAbsolute,
    #[error("the workload identity assertion is invalid")]
    InvalidAssertion,
    #[error("the workload identity assertion exceeds 16 KiB")]
    AssertionTooLarge,
    #[error("could not read workload identity assertion file {path}")]
    AssertionFile {
        path: PathBuf,
        #[source]
        source: Arc<std::io::Error>,
    },
    #[error("could not configure the workload identity HTTP client")]
    HttpClientConfiguration,
    #[error("the workload identity token URL must use HTTPS or loopback HTTP")]
    InvalidTokenUrl,
    #[error("the workload identity token exchange is unavailable")]
    ExchangeUnavailable,
    #[error("the workload identity token exchange was rejected with HTTP {0}")]
    ExchangeRejected(u16),
    #[error("the workload identity token exchange returned an invalid response")]
    InvalidExchangeResponse,
}
