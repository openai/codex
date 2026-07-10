mod assertion;
mod exchange;

use std::path::PathBuf;

pub use assertion::WorkloadIdentityAssertionSource;
pub use exchange::WorkloadIdentityExchange;
pub use exchange::WorkloadIdentityToken;
use thiserror::Error;

pub const FEDERATION_RULE_ID_ENV_VAR: &str = "OPENAI_FEDERATION_RULE_ID";
pub const IDENTITY_TOKEN_ENV_VAR: &str = "OPENAI_IDENTITY_TOKEN";
pub const IDENTITY_TOKEN_FILE_ENV_VAR: &str = "OPENAI_IDENTITY_TOKEN_FILE";
pub const PRINCIPAL_ID_ENV_VAR: &str = "OPENAI_PRINCIPAL_ID";
pub const TENANT_ID_ENV_VAR: &str = "OPENAI_TENANT_ID";
pub const WORKSPACE_ID_ENV_VAR: &str = "OPENAI_WORKSPACE_ID";

/// Identifies the pre-provisioned OpenAI principal selected by a federation rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkloadIdentityTarget {
    pub federation_rule_id: String,
    pub principal_id: String,
    pub tenant_id: String,
    pub workspace_id: String,
}

/// Complete input for exchanging an upstream assertion for ChatGPT auth.
#[derive(Clone)]
pub struct WorkloadIdentityConfig {
    pub(crate) assertion_source: WorkloadIdentityAssertionSource,
    pub(crate) target: WorkloadIdentityTarget,
}

impl WorkloadIdentityConfig {
    pub fn new(
        target: WorkloadIdentityTarget,
        assertion_source: WorkloadIdentityAssertionSource,
    ) -> Result<Self, WorkloadIdentityError> {
        let target = WorkloadIdentityTarget {
            federation_rule_id: normalized_field(target.federation_rule_id, "federation_rule_id")?,
            principal_id: normalized_field(target.principal_id, "principal_id")?,
            tenant_id: normalized_field(target.tenant_id, "tenant_id")?,
            workspace_id: normalized_field(target.workspace_id, "workspace_id")?,
        };
        Ok(Self {
            assertion_source,
            target,
        })
    }
}

fn normalized_field(value: String, name: &'static str) -> Result<String, WorkloadIdentityError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(WorkloadIdentityError::InvalidConfigurationField(name));
    }
    Ok(value.to_string())
}

#[derive(Debug, Error)]
pub enum WorkloadIdentityError {
    #[error("workload identity field {0} must not be empty")]
    InvalidConfigurationField(&'static str),
    #[error("the workload identity assertion is invalid")]
    InvalidAssertion,
    #[error("the workload identity assertion exceeds 16 KiB")]
    AssertionTooLarge,
    #[error("could not read workload identity token file {path}")]
    TokenFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not configure the workload identity HTTP client")]
    HttpClientConfiguration,
    #[error("CODEX_WIF_TOKEN_URL_OVERRIDE must use loopback HTTP(S)")]
    InvalidTokenUrl,
    #[error("the workload identity token exchange is unavailable")]
    ExchangeUnavailable,
    #[error("the workload identity token exchange was rejected with HTTP {0}")]
    ExchangeRejected(u16),
    #[error("the workload identity token exchange returned an invalid response")]
    InvalidExchangeResponse,
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
