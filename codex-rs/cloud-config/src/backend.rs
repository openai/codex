use codex_backend_client::Client as BackendClient;
use codex_backend_client::ConfigBundleResponse;
use codex_backend_client::DeliveredManagedLayers;
use codex_backend_client::DeliveredTomlFragment;
use codex_config::CloudConfigBundle;
use codex_config::CloudConfigFragment;
use codex_config::CloudConfigTomlBundle;
use codex_config::CloudConfigTomlManagedLayers;
use codex_config::CloudRequirementsFragment;
use codex_config::CloudRequirementsTomlBundle;
use codex_config::CloudRequirementsTomlManagedLayers;
use codex_login::CodexAuth;
use std::future::Future;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RetryableFailureKind {
    BackendClientInit,
    Request { status_code: Option<u16> },
}

impl RetryableFailureKind {
    pub(crate) fn status_code(self) -> Option<u16> {
        match self {
            Self::BackendClientInit => None,
            Self::Request { status_code } => status_code,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BundleRequestError {
    Retryable(RetryableFailureKind),
    Unauthorized {
        status_code: Option<u16>,
        message: String,
    },
    InvalidBundle {
        message: String,
    },
}

/// Retrieves one cloud config bundle from the backend.
///
/// Implementations translate the backend response into the managed-only domain model. TOML
/// parsing, semantic validation, and caching remain service-layer responsibilities.
pub(crate) trait BundleClient: Send + Sync {
    fn get_bundle(
        &self,
        auth: &CodexAuth,
    ) -> impl Future<Output = Result<CloudConfigBundle, BundleRequestError>> + Send;
}

pub(crate) struct BackendBundleClient {
    base_url: String,
}

impl BackendBundleClient {
    pub(crate) fn new(base_url: String) -> Self {
        Self { base_url }
    }
}

impl BundleClient for BackendBundleClient {
    async fn get_bundle(&self, auth: &CodexAuth) -> Result<CloudConfigBundle, BundleRequestError> {
        let client = BackendClient::from_auth(self.base_url.clone(), auth)
            .inspect_err(|err| {
                tracing::warn!(
                    error = %err,
                    "Failed to construct backend client for cloud config bundle"
                );
            })
            .map_err(|_| BundleRequestError::Retryable(RetryableFailureKind::BackendClientInit))?;

        let response = client
            .get_config_bundle()
            .await
            .inspect_err(|err| {
                tracing::warn!(error = %err, "Failed to fetch cloud config bundle");
            })
            .map_err(|err| {
                let status_code = err.status().map(|status| status.as_u16());
                if err.is_unauthorized() {
                    BundleRequestError::Unauthorized {
                        status_code,
                        message: err.to_string(),
                    }
                } else {
                    BundleRequestError::Retryable(RetryableFailureKind::Request { status_code })
                }
            })?;

        bundle_from_response(response)
    }
}

pub(crate) fn bundle_from_response(
    response: ConfigBundleResponse,
) -> Result<CloudConfigBundle, BundleRequestError> {
    let config_toml = match response.config_toml.flatten() {
        Some(config_toml) => {
            let (baseline, system_overlay) = managed_fragments_from_delivered(
                config_toml.managed_layers,
                config_fragment_from_delivered,
                "config_toml",
            )?;
            CloudConfigTomlBundle {
                managed_layers: CloudConfigTomlManagedLayers {
                    baseline,
                    system_overlay,
                },
            }
        }
        None => CloudConfigTomlBundle::default(),
    };
    let requirements_toml = match response.requirements_toml.flatten() {
        Some(requirements_toml) => {
            let (baseline, system_overlay) = managed_fragments_from_delivered(
                requirements_toml.managed_layers,
                requirements_fragment_from_delivered,
                "requirements_toml",
            )?;
            CloudRequirementsTomlBundle {
                managed_layers: CloudRequirementsTomlManagedLayers {
                    baseline,
                    system_overlay,
                },
            }
        }
        None => CloudRequirementsTomlBundle::default(),
    };

    Ok(CloudConfigBundle {
        config_toml,
        requirements_toml,
    })
}

fn managed_fragments_from_delivered<T>(
    managed_layers: Option<Option<Box<DeliveredManagedLayers>>>,
    convert: fn(DeliveredTomlFragment) -> T,
    document_name: &str,
) -> Result<(Vec<T>, Vec<T>), BundleRequestError> {
    let managed_layers = managed_layers
        .flatten()
        .map(|managed_layers| *managed_layers)
        .ok_or_else(|| BundleRequestError::InvalidBundle {
            message: format!(
                "cloud config bundle {document_name} is present but managed_layers is missing or null"
            ),
        })?;
    let DeliveredManagedLayers {
        baseline,
        system_overlay,
    } = managed_layers;
    Ok((
        baseline.into_iter().map(convert).collect(),
        system_overlay.into_iter().map(convert).collect(),
    ))
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
