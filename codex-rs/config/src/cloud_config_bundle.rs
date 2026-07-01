//! Cloud config bundle domain model and shared in-memory loader.
//!
//! The backend bundle groups cloud-delivered config and requirements fragments
//! by source bucket. `CloudConfigBundleLayers` converts those raw buckets into
//! layer entries while preserving each bucket's insertion semantics.

use crate::CloudConfigFragment;
use crate::ConfigLayerEntry;
use crate::RequirementSource;
use crate::RequirementsLayerEntry;
use crate::cloud_config_layers::CloudConfigLayerError;
use crate::cloud_config_layers::cloud_config_layers_from_fragments_strict;
use crate::cloud_config_layers_from_fragments;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use thiserror::Error;
use tokio::sync::watch;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudConfigBundle {
    pub config_toml: CloudConfigTomlBundle,
    pub requirements_toml: CloudRequirementsTomlBundle,
}

impl CloudConfigBundle {
    pub fn is_empty(&self) -> bool {
        let CloudConfigBundle {
            config_toml,
            requirements_toml,
        } = self;
        let CloudConfigTomlBundle {
            enterprise_managed: config_enterprise_managed,
        } = config_toml;
        let CloudRequirementsTomlBundle {
            enterprise_managed: requirements_enterprise_managed,
        } = requirements_toml;

        config_enterprise_managed.is_empty() && requirements_enterprise_managed.is_empty()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudConfigTomlBundle {
    pub enterprise_managed: Vec<CloudConfigFragment>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudRequirementsTomlBundle {
    pub enterprise_managed: Vec<CloudRequirementsFragment>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudRequirementsFragment {
    pub id: String,
    pub name: String,
    pub contents: String,
}

/// Cloud config bundle converted into semantic layer buckets.
///
/// This is not a final config stack. Callers still decide where each bucket is
/// inserted relative to local/system/user layers.
#[derive(Clone, Debug)]
pub struct CloudConfigBundleLayers {
    /// Enterprise-managed config layers in `ConfigLayerStack` order.
    pub enterprise_managed_config: Vec<ConfigLayerEntry>,
    /// Enterprise-managed requirements layers in requirements layer merge order.
    pub enterprise_managed_requirements: Vec<RequirementsLayerEntry>,
}

impl CloudConfigBundleLayers {
    pub fn from_bundle(
        bundle: CloudConfigBundle,
        base_dir: &AbsolutePathBuf,
    ) -> Result<Self, CloudConfigLayerError> {
        Self::from_bundle_impl(bundle, base_dir, /*strict_config*/ false)
    }

    pub fn from_bundle_strict_config(
        bundle: CloudConfigBundle,
        base_dir: &AbsolutePathBuf,
    ) -> Result<Self, CloudConfigLayerError> {
        Self::from_bundle_impl(bundle, base_dir, /*strict_config*/ true)
    }

    fn from_bundle_impl(
        bundle: CloudConfigBundle,
        base_dir: &AbsolutePathBuf,
        strict_config: bool,
    ) -> Result<Self, CloudConfigLayerError> {
        // Keep this destructuring exhaustive so adding a new bundle bucket forces
        // an explicit choice about how it becomes layer data.
        let CloudConfigBundle {
            config_toml:
                CloudConfigTomlBundle {
                    enterprise_managed: config_enterprise_managed,
                },
            requirements_toml:
                CloudRequirementsTomlBundle {
                    enterprise_managed: requirements_enterprise_managed,
                },
        } = bundle;

        let enterprise_managed_config = if strict_config {
            cloud_config_layers_from_fragments_strict(config_enterprise_managed, base_dir)?
        } else {
            cloud_config_layers_from_fragments(config_enterprise_managed, base_dir)?
        };

        let mut enterprise_managed_requirements = requirements_enterprise_managed
            .into_iter()
            .map(|fragment| {
                RequirementsLayerEntry::from_toml(
                    RequirementSource::EnterpriseManaged {
                        id: fragment.id,
                        name: fragment.name,
                    },
                    fragment.contents,
                )
                .with_base_dir(base_dir.clone())
            })
            .collect::<Vec<_>>();
        // Bundle fragments arrive highest-priority first, while requirements
        // layers are merged lowest-priority to highest-priority.
        enterprise_managed_requirements.reverse();

        Ok(Self {
            enterprise_managed_config,
            enterprise_managed_requirements,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloudConfigBundleLoadErrorCode {
    Auth,
    Timeout,
    RequestFailed,
    InvalidBundle,
    Internal,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct CloudConfigBundleLoadError {
    code: CloudConfigBundleLoadErrorCode,
    message: String,
    status_code: Option<u16>,
}

impl CloudConfigBundleLoadError {
    pub fn new(
        code: CloudConfigBundleLoadErrorCode,
        status_code: Option<u16>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            status_code,
        }
    }

    pub fn code(&self) -> CloudConfigBundleLoadErrorCode {
        self.code
    }

    pub fn status_code(&self) -> Option<u16> {
        self.status_code
    }
}

#[derive(Clone)]
pub struct CloudConfigBundleLoader {
    state: watch::Receiver<CloudConfigBundleState>,
}

#[derive(Clone, Debug)]
enum CloudConfigBundleState {
    Pending,
    Ready(Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError>),
}

/// Publishes results to a [`CloudConfigBundleLoader`].
#[derive(Debug)]
pub struct CloudConfigBundlePublisher {
    state: watch::Sender<CloudConfigBundleState>,
}

impl CloudConfigBundleLoader {
    /// Creates a loader that immediately returns the supplied result.
    pub fn from_result(
        result: Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError>,
    ) -> Self {
        let (_publisher, state) = watch::channel(CloudConfigBundleState::Ready(result));
        Self { state }
    }

    /// Creates a pending loader and its publisher.
    pub fn pending() -> (Self, CloudConfigBundlePublisher) {
        let (publisher, state) = watch::channel(CloudConfigBundleState::Pending);
        (
            Self { state },
            CloudConfigBundlePublisher { state: publisher },
        )
    }

    /// Returns the bundle result currently published by this loader.
    ///
    /// A pending loader waits for its initial result. Once a result is ready,
    /// this returns that snapshot immediately; later refreshes are observed by
    /// future calls. If the publisher is dropped before the initial result,
    /// this returns an internal lifecycle error.
    pub async fn get(&self) -> Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError> {
        // Each call uses its own watch cursor so concurrent callers can wait
        // for the initial publication independently.
        let mut state = self.state.clone();
        loop {
            let result = match &*state.borrow_and_update() {
                CloudConfigBundleState::Pending => None,
                CloudConfigBundleState::Ready(result) => Some(result.clone()),
            };
            if let Some(result) = result {
                return result;
            }
            if state.changed().await.is_err() {
                return Err(CloudConfigBundleLoadError::new(
                    CloudConfigBundleLoadErrorCode::Internal,
                    /*status_code*/ None,
                    "cloud config bundle lifecycle ended before startup completed",
                ));
            }
        }
    }
}

impl CloudConfigBundlePublisher {
    /// Waits until every associated loader and clone has been dropped.
    pub async fn closed(&self) {
        self.state.closed().await;
    }

    /// Publishes the latest load result.
    ///
    /// An error resolves a pending loader, but does not replace a result that
    /// was already published. Successful results always replace the current
    /// value.
    pub fn publish(&self, result: Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError>) {
        self.state.send_if_modified(move |state| {
            if matches!(state, CloudConfigBundleState::Ready(_)) && result.is_err() {
                return false;
            }
            *state = CloudConfigBundleState::Ready(result);
            true
        });
    }
}

impl fmt::Debug for CloudConfigBundleLoader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CloudConfigBundleLoader").finish()
    }
}

impl Default for CloudConfigBundleLoader {
    fn default() -> Self {
        Self::from_result(Ok(None))
    }
}

#[cfg(test)]
#[path = "cloud_config_bundle_tests.rs"]
mod tests;
