use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;
use url::Host;
use url::Url;

const DEFAULT_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const GITHUB_ACTIONS_REQUEST_URL_ENV: &str = "ACTIONS_ID_TOKEN_REQUEST_URL";
const GITHUB_ACTIONS_REQUEST_TOKEN_ENV: &str = "ACTIONS_ID_TOKEN_REQUEST_TOKEN";

/// Configuration for exchanging a workload credential for a Codex access token.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkloadIdentityConfig {
    /// OpenAI workload identity provider selected by the workspace administrator.
    pub identity_provider_id: String,

    /// Administrator-created mapping from external claims to a ChatGPT principal.
    pub identity_provider_mapping_id: String,

    /// Audience requested from the external workload identity provider.
    pub audience: String,

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
    /// An OIDC token injected directly into the Codex process environment.
    Environment {
        /// Name of the environment variable. The value is never serialized.
        variable: String,
    },

    /// An OIDC token projected into a file by the workload runtime.
    File {
        /// Absolute path to the projected token.
        path: PathBuf,
    },

    /// A projected Azure workload identity token.
    Azure {
        /// Token path. When omitted, Codex reads `AZURE_FEDERATED_TOKEN_FILE`.
        #[serde(default)]
        token_file: Option<PathBuf>,
    },

    /// A GCE or GKE service-account identity token from the metadata service.
    Gcp {
        /// Service account email or `default` when omitted.
        #[serde(default)]
        service_account: Option<String>,
    },

    /// A GitHub Actions OIDC token requested from the runner token service.
    #[serde(rename = "github_actions")]
    GithubActions {},

    /// A JWT-SVID requested from the local SPIFFE Workload API.
    Spiffe {
        /// Workload API endpoint. Defaults to `SPIFFE_ENDPOINT_SOCKET`.
        #[serde(default)]
        endpoint_socket: Option<String>,
        /// Explicit SPIFFE ID when the workload exposes more than one identity.
        #[serde(default)]
        spiffe_id: Option<String>,
    },

    /// Reserved for the approved AWS workload proof contract.
    Aws {},
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
    #[error("workload_identity.credential_source.path must be an absolute path")]
    RelativeCredentialFile,
    #[error("workload_identity.credential_source.path must not be empty")]
    EmptyCredentialFile,
    #[error("workload_identity.credential_source.variable is invalid")]
    InvalidEnvironmentVariable,
    #[error("workload_identity.credential_source.{0} must not be empty")]
    EmptySourceField(&'static str),
    #[error("workload_identity.credential_source.spiffe_id must be a SPIFFE ID")]
    InvalidSpiffeId,
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
            ("audience", self.audience.as_str()),
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
            CredentialSourceConfig::Environment { variable }
                if !is_valid_environment_variable_name(variable) =>
            {
                Err(WorkloadIdentityConfigError::InvalidEnvironmentVariable)
            }
            CredentialSourceConfig::Environment { .. } => Ok(()),
            CredentialSourceConfig::File { path } if path.as_os_str().is_empty() => {
                Err(WorkloadIdentityConfigError::EmptyCredentialFile)
            }
            CredentialSourceConfig::File { path } if !path.is_absolute() => {
                Err(WorkloadIdentityConfigError::RelativeCredentialFile)
            }
            CredentialSourceConfig::File { .. } => Ok(()),
            CredentialSourceConfig::Azure {
                token_file: Some(token_file),
            } if token_file.as_os_str().is_empty() => {
                Err(WorkloadIdentityConfigError::EmptyTokenFile)
            }
            CredentialSourceConfig::Azure {
                token_file: Some(token_file),
            } if !token_file.is_absolute() => Err(WorkloadIdentityConfigError::RelativeTokenFile),
            CredentialSourceConfig::Azure { .. } => Ok(()),
            CredentialSourceConfig::Gcp {
                service_account: Some(service_account),
            } if service_account.trim().is_empty() => Err(
                WorkloadIdentityConfigError::EmptySourceField("service_account"),
            ),
            CredentialSourceConfig::Gcp { .. } => Ok(()),
            CredentialSourceConfig::GithubActions {} => Ok(()),
            CredentialSourceConfig::Spiffe {
                endpoint_socket: Some(endpoint_socket),
                ..
            } if endpoint_socket.trim().is_empty() => Err(
                WorkloadIdentityConfigError::EmptySourceField("endpoint_socket"),
            ),
            CredentialSourceConfig::Spiffe {
                spiffe_id: Some(spiffe_id),
                ..
            } if !spiffe_id.starts_with("spiffe://") => {
                Err(WorkloadIdentityConfigError::InvalidSpiffeId)
            }
            CredentialSourceConfig::Spiffe { .. } => Ok(()),
            CredentialSourceConfig::Aws {} => Ok(()),
        }
    }
}

impl CredentialSourceConfig {
    pub const fn source_name(&self) -> &'static str {
        match self {
            Self::Environment { .. } => "environment",
            Self::File { .. } => "file",
            Self::Azure { .. } => "azure",
            Self::Gcp { .. } => "gcp",
            Self::GithubActions {} => "github_actions",
            Self::Spiffe { .. } => "spiffe",
            Self::Aws {} => "aws",
        }
    }

    pub fn sensitive_environment_variables(&self) -> Vec<&str> {
        match self {
            Self::Environment { variable } => vec![variable.as_str()],
            Self::GithubActions {} => vec![
                GITHUB_ACTIONS_REQUEST_URL_ENV,
                GITHUB_ACTIONS_REQUEST_TOKEN_ENV,
            ],
            Self::File { .. }
            | Self::Azure { .. }
            | Self::Gcp { .. }
            | Self::Spiffe { .. }
            | Self::Aws {} => Vec::new(),
        }
    }
}

fn is_valid_environment_variable_name(variable: &str) -> bool {
    let mut characters = variable.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
