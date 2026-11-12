mod assertion;
mod exchange;
mod spiffe;

use std::path::PathBuf;
use std::sync::Arc;

pub use exchange::ApiTokenMetadata;
pub use exchange::ChatgptTokenMetadata;
pub use exchange::WorkloadIdentityExchange;
pub use exchange::WorkloadIdentityMetadata;
pub use exchange::WorkloadIdentityToken;
pub use spiffe::SpiffeWorkloadApi;
use thiserror::Error;

/// The configured source of the assertion exchanged for workload identity auth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssertionSource {
    File(PathBuf),
    Spiffe(SpiffeWorkloadApi),
}

impl From<PathBuf> for AssertionSource {
    fn from(path: PathBuf) -> Self {
        Self::File(path)
    }
}

/// The inputs needed to exchange an assertion for workload identity auth.
#[derive(Clone)]
pub struct WorkloadIdentityConfig {
    pub(crate) assertion_source: AssertionSource,
    pub(crate) federation_rule_id: String,
    pub(crate) workload_identity_context: Option<String>,
}

impl WorkloadIdentityConfig {
    pub fn new(
        federation_rule_id: String,
        assertion_source: impl Into<AssertionSource>,
        workload_identity_context: Option<String>,
    ) -> Result<Self, WorkloadIdentityError> {
        let federation_rule_id = federation_rule_id.trim();
        if federation_rule_id.is_empty() {
            return Err(WorkloadIdentityError::InvalidFederationRuleId);
        }
        let assertion_source = assertion_source.into();
        if let AssertionSource::File(path) = &assertion_source
            && !path.is_absolute()
        {
            return Err(WorkloadIdentityError::AssertionFileMustBeAbsolute);
        }
        Ok(Self {
            assertion_source,
            federation_rule_id: federation_rule_id.to_string(),
            workload_identity_context,
        })
    }
}

#[derive(Clone, Debug, Error)]
pub enum WorkloadIdentityError {
    #[error(transparent)]
    Policy(#[from] codex_http_client::NetworkPolicyDenied),
    #[error("the workload identity federation rule ID must not be empty")]
    InvalidFederationRuleId,
    #[error("the workload identity assertion file path must be absolute")]
    AssertionFileMustBeAbsolute,
    #[error("the workload identity assertion is invalid")]
    InvalidAssertion,
    #[error("the workload identity assertion exceeds 16 KiB")]
    AssertionTooLarge,
    #[error("the SPIFFE Workload API endpoint must be an absolute unix: socket URI")]
    InvalidSpiffeEndpoint,
    #[error("the SPIFFE JWT-SVID audience must not be empty or contain NUL")]
    InvalidSpiffeAudience,
    #[error("the requested SPIFFE ID is invalid")]
    InvalidSpiffeId,
    #[error("SPIFFE Workload API authentication requires a Unix platform")]
    UnsupportedSpiffePlatform,
    #[error("the SPIFFE Workload API is unavailable or timed out")]
    SpiffeUnavailable,
    #[error("the SPIFFE Workload API rejected the request with gRPC status {0}")]
    SpiffeRejected(tonic::Code),
    #[error("the SPIFFE Workload API returned multiple JWT-SVIDs; configure an explicit SPIFFE ID")]
    AmbiguousSpiffeIdentity,
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

impl WorkloadIdentityError {
    /// Whether retrying the operation may succeed without changing configuration.
    pub fn is_transient(&self) -> bool {
        match self {
            Self::AssertionFile { source, .. } => matches!(
                source.kind(),
                std::io::ErrorKind::Interrupted
                    | std::io::ErrorKind::NotFound
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::WouldBlock
            ),
            Self::ExchangeUnavailable
            | Self::SpiffeUnavailable
            | Self::ExchangeRejected(408 | 429 | 500..=599) => true,
            Self::SpiffeRejected(code) => matches!(
                code,
                tonic::Code::Unavailable
                    | tonic::Code::DeadlineExceeded
                    | tonic::Code::ResourceExhausted
                    | tonic::Code::Aborted
                    | tonic::Code::Cancelled
                    | tonic::Code::Internal
            ),
            Self::Policy(_)
            | Self::InvalidFederationRuleId
            | Self::AssertionFileMustBeAbsolute
            | Self::InvalidAssertion
            | Self::AssertionTooLarge
            | Self::InvalidSpiffeEndpoint
            | Self::InvalidSpiffeAudience
            | Self::InvalidSpiffeId
            | Self::UnsupportedSpiffePlatform
            | Self::AmbiguousSpiffeIdentity
            | Self::HttpClientConfiguration
            | Self::InvalidTokenUrl
            | Self::ExchangeRejected(_)
            | Self::InvalidExchangeResponse => false,
        }
    }
}
