//! Preserve policy denials through terminal connection state and allow later policy recovery.

use codex_http_client::NetworkPolicyDenied;
use codex_http_client::RouteAwareRequestError;

use super::ExecServerError;

#[derive(Clone, thiserror::Error, Debug)]
pub(super) enum ConnectionFailure {
    #[error("{0}")]
    Disconnected(String),
    #[error(transparent)]
    Policy(NetworkPolicyDenied),
}

impl From<ConnectionFailure> for ExecServerError {
    fn from(failure: ConnectionFailure) -> Self {
        match failure {
            ConnectionFailure::Disconnected(message) => Self::Disconnected(message),
            ConnectionFailure::Policy(denial) => Self::ApplicationNetworkPolicy(denial),
        }
    }
}

impl ExecServerError {
    pub(crate) fn application_network_policy_denial(&self) -> Option<NetworkPolicyDenied> {
        match self {
            Self::ConnectionAttempt(error) => error.application_network_policy_denial(),
            Self::ApplicationNetworkPolicy(denial)
            | Self::EnvironmentRegistryRequest(RouteAwareRequestError::Policy(denial)) => {
                Some(*denial)
            }
            Self::WebSocketConnect { source, .. } => {
                codex_websocket_client::network_policy_denial(source)
            }
            Self::Spawn(_)
            | Self::WebSocketConnectTimeout { .. }
            | Self::WebSocketConfiguration(_)
            | Self::InitializeTimedOut { .. }
            | Self::Closed
            | Self::Disconnected(_)
            | Self::ProvisioningFailed(_)
            | Self::Json(_)
            | Self::HttpRequest(_)
            | Self::Protocol(_)
            | Self::ProvisioningModeConflict { .. }
            | Self::Server { .. }
            | Self::EnvironmentRegistryHttp { .. }
            | Self::EnvironmentRegistryConfig(_)
            | Self::EnvironmentRegistryAuth(_)
            | Self::EnvironmentRegistryRequest(
                RouteAwareRequestError::Request(_)
                | RouteAwareRequestError::Route(_)
                | RouteAwareRequestError::Build(_)
                | RouteAwareRequestError::UnsupportedRedirectScheme(_)
                | RouteAwareRequestError::TooManyRedirects
                | RouteAwareRequestError::Timeout,
            ) => None,
        }
    }
}

pub(crate) fn can_retry_connection_attempt(error: &ExecServerError) -> bool {
    error.application_network_policy_denial().is_some()
        || super::recovery::is_retryable_recovery_error(error)
}
