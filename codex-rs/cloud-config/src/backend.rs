use codex_backend_client::Client as BackendClient;
use codex_backend_client::ConfigBundleResponse;
use codex_backend_client::DeliveredTomlFragment;
use codex_config::CloudConfigBundle;
use codex_config::CloudConfigFragment;
use codex_config::CloudConfigTomlBundle;
use codex_config::CloudRequirementsFragment;
use codex_config::CloudRequirementsTomlBundle;
use codex_http_client::HttpClientFactory;
use codex_login::CodexAuth;
use std::future::Future;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RetryableFailureKind {
    Request { status_code: Option<u16> },
    AuthRecovery { status_code: Option<u16> },
}

impl RetryableFailureKind {
    pub(crate) fn status_code(self) -> Option<u16> {
        match self {
            Self::Request { status_code } | Self::AuthRecovery { status_code } => status_code,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BundleRequestError {
    Policy(codex_http_client::NetworkPolicyDenied),
    Fatal {
        error: codex_config::CloudConfigBundleLoadError,
        metric_reason: Option<&'static str>,
    },
    Retryable(RetryableFailureKind),
    Unauthorized {
        status_code: Option<u16>,
        message: String,
    },
}

impl BundleRequestError {
    pub(crate) fn auth_failure(
        status_code: Option<u16>,
        message: impl Into<String>,
        metric_reason: Option<&'static str>,
    ) -> Self {
        Self::Fatal {
            error: codex_config::CloudConfigBundleLoadError::new(
                codex_config::CloudConfigBundleLoadErrorCode::Auth,
                status_code,
                message,
            ),
            metric_reason,
        }
    }
}

/// Retrieves one cloud config bundle from the backend.
///
/// Implementations should return the backend-selected bundle exactly as delivered and leave
/// validation, caching, and config/requirements parsing decisions to the service layer.
pub(crate) trait BundleClient: Send + Sync {
    fn get_bundle(
        &self,
        auth: &CodexAuth,
    ) -> impl Future<Output = Result<CloudConfigBundle, BundleRequestError>> + Send;
}

pub(crate) struct BackendBundleClient {
    base_url: String,
    http_client_factory: HttpClientFactory,
}

impl BackendBundleClient {
    pub(crate) fn new(base_url: String, http_client_factory: HttpClientFactory) -> Self {
        Self {
            base_url,
            http_client_factory,
        }
    }
}

impl BundleClient for BackendBundleClient {
    async fn get_bundle(&self, auth: &CodexAuth) -> Result<CloudConfigBundle, BundleRequestError> {
        let client = BackendClient::from_auth(
            self.base_url.clone(),
            auth,
            self.http_client_factory.clone(),
        );

        fetch_bundle(&client).await
    }
}

pub(crate) async fn fetch_bundle(
    client: &BackendClient,
) -> Result<CloudConfigBundle, BundleRequestError> {
    client
        .get_config_bundle()
        .await
        .inspect_err(|err| {
            tracing::warn!(error = %err, "Failed to fetch cloud config bundle");
        })
        .map(bundle_from_response)
        .map_err(bundle_request_error)
}

fn bundle_request_error(error: codex_backend_client::RequestError) -> BundleRequestError {
    if let codex_backend_client::RequestError::Policy(denied) = &error {
        return BundleRequestError::Policy(*denied);
    }
    let status_code = error.status().map(|status| status.as_u16());
    if error.is_unauthorized() {
        BundleRequestError::Unauthorized {
            status_code,
            message: "Managed configuration sign-in is required".into(),
        }
    } else {
        BundleRequestError::Retryable(RetryableFailureKind::Request { status_code })
    }
}

pub(crate) fn bundle_from_response(response: ConfigBundleResponse) -> CloudConfigBundle {
    let config_toml = response
        .config_toml
        .flatten()
        .map(|config_toml| *config_toml)
        .and_then(|config_toml| config_toml.enterprise_managed.flatten())
        .unwrap_or_default()
        .into_iter()
        .map(config_fragment_from_delivered)
        .collect();
    let requirements_toml = response
        .requirements_toml
        .flatten()
        .map(|requirements_toml| *requirements_toml)
        .and_then(|requirements_toml| requirements_toml.enterprise_managed.flatten())
        .unwrap_or_default()
        .into_iter()
        .map(requirements_fragment_from_delivered)
        .collect();

    CloudConfigBundle {
        config_toml: CloudConfigTomlBundle {
            enterprise_managed: config_toml,
        },
        requirements_toml: CloudRequirementsTomlBundle {
            enterprise_managed: requirements_toml,
        },
    }
}

fn config_fragment_from_delivered(fragment: DeliveredTomlFragment) -> CloudConfigFragment {
    CloudConfigFragment {
        id: fragment.id,
        name: fragment.name,
        contents: fragment.contents,
    }
}

fn requirements_fragment_from_delivered(
    fragment: DeliveredTomlFragment,
) -> CloudRequirementsFragment {
    CloudRequirementsFragment {
        id: fragment.id,
        name: fragment.name,
        contents: fragment.contents,
    }
}
