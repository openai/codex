use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;
use url::Host;
use url::Url;

const DEFAULT_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";

/// Configuration for exchanging a workload credential for a Codex access token.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkloadIdentityConfig {
    /// OpenAI workload identity provider selected by the workspace administrator.
    pub identity_provider_id: String,

    /// Administrator-created mapping from external claims to a ChatGPT principal.
    pub identity_provider_mapping_id: String,

    /// OAuth token endpoint. The override is primarily useful for local development.
    #[serde(default = "default_token_url")]
    pub token_url: String,

    /// Runtime-specific mechanism used to obtain the external subject token.
    pub credential_source: CredentialSourceConfig,
}

/// Supported runtime credential sources. Implementations are independently feature gated.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "type")]
pub enum CredentialSourceConfig {
    /// A projected Azure workload identity token.
    Azure {
        /// Token path. When omitted, Codex reads `AZURE_FEDERATED_TOKEN_FILE`.
        #[serde(default)]
        token_file: Option<PathBuf>,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkloadIdentityConfigError {
    #[error("workload_identity.{0} must not be empty")]
    EmptyField(&'static str),
    #[error("workload_identity.token_url is invalid: {0}")]
    InvalidTokenUrl(String),
    #[error("workload_identity.token_url must use https or loopback http")]
    UnsupportedTokenUrlScheme,
    #[error("workload_identity.credential_source.token_file must be an absolute path")]
    RelativeTokenFile,
    #[error("workload_identity.credential_source.token_file must not be empty")]
    EmptyTokenFile,
}

pub fn default_token_url() -> String {
    DEFAULT_TOKEN_URL.to_string()
}

impl WorkloadIdentityConfig {
    pub fn validate(&self) -> Result<(), WorkloadIdentityConfigError> {
        for (field, value) in [
            ("identity_provider_id", self.identity_provider_id.as_str()),
            (
                "identity_provider_mapping_id",
                self.identity_provider_mapping_id.as_str(),
            ),
            ("token_url", self.token_url.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(WorkloadIdentityConfigError::EmptyField(field));
            }
        }

        let token_url = Url::parse(&self.token_url)
            .map_err(|error| WorkloadIdentityConfigError::InvalidTokenUrl(error.to_string()))?;
        let loopback_http = token_url.scheme() == "http"
            && token_url.host().is_some_and(|host| match host {
                Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
                Host::Ipv4(address) => address.is_loopback(),
                Host::Ipv6(address) => address.is_loopback(),
            });
        if token_url.scheme() != "https" && !loopback_http {
            return Err(WorkloadIdentityConfigError::UnsupportedTokenUrlScheme);
        }

        match &self.credential_source {
            CredentialSourceConfig::Azure {
                token_file: Some(token_file),
            } if token_file.as_os_str().is_empty() => {
                Err(WorkloadIdentityConfigError::EmptyTokenFile)
            }
            CredentialSourceConfig::Azure {
                token_file: Some(token_file),
            } if !token_file.is_absolute() => Err(WorkloadIdentityConfigError::RelativeTokenFile),
            CredentialSourceConfig::Azure { .. } => Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
