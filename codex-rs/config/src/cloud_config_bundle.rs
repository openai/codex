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
use futures::future::BoxFuture;
use futures::future::FutureExt;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use thiserror::Error;

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
    getter: Arc<
        dyn Fn()
                -> BoxFuture<'static, Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError>>
            + Send
            + Sync,
    >,
}

impl CloudConfigBundleLoader {
    pub fn new<F>(fut: F) -> Self
    where
        F: Future<Output = Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError>>
            + Send
            + 'static,
    {
        let fut = fut.boxed().shared();
        Self::from_getter(move || fut.clone())
    }

    /// Creates a loader that requests the latest bundle on every call.
    pub fn from_getter<F, Fut>(getter: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError>>
            + Send
            + 'static,
    {
        Self {
            getter: Arc::new(move || getter().boxed()),
        }
    }

    /// Returns the current bundle snapshot.
    pub async fn get(&self) -> Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError> {
        (self.getter)().await
    }
}

impl fmt::Debug for CloudConfigBundleLoader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CloudConfigBundleLoader").finish()
    }
}

impl Default for CloudConfigBundleLoader {
    fn default() -> Self {
        Self::new(async { Ok(None) })
    }
}

#[cfg(test)]
#[path = "cloud_config_bundle_tests.rs"]
mod tests;
