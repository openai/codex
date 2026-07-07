//! Cloud config bundle domain model and shared in-memory loader.
//!
//! The backend bundle groups cloud-delivered config and requirements fragments
//! by source bucket. `CloudConfigBundleLayers` converts those raw buckets into
//! layer entries while preserving each bucket's insertion semantics.

use crate::CloudConfigFragment;
use crate::CloudManagedLayer;
use crate::ConfigLayerEntry;
use crate::RequirementSource;
use crate::RequirementsLayerEntry;
use crate::cloud_config_layers::CloudConfigLayerError;
use crate::cloud_config_layers::cloud_managed_config_layers_from_fragments;
use crate::cloud_config_layers::cloud_managed_config_layers_from_fragments_strict;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use futures::future::Shared;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use std::future::Future;
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
            managed_layers: config_managed_layers,
        } = config_toml;
        let CloudRequirementsTomlBundle {
            managed_layers: requirements_managed_layers,
        } = requirements_toml;

        config_managed_layers.baseline.is_empty()
            && config_managed_layers.system_overlay.is_empty()
            && requirements_managed_layers.baseline.is_empty()
            && requirements_managed_layers.system_overlay.is_empty()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudConfigTomlBundle {
    #[serde(
        default,
        skip_serializing_if = "CloudConfigTomlManagedLayers::is_empty"
    )]
    pub managed_layers: CloudConfigTomlManagedLayers,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudConfigTomlManagedLayers {
    pub baseline: Vec<CloudConfigFragment>,
    pub system_overlay: Vec<CloudConfigFragment>,
}

impl CloudConfigTomlManagedLayers {
    fn is_empty(&self) -> bool {
        self.baseline.is_empty() && self.system_overlay.is_empty()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudRequirementsTomlBundle {
    #[serde(
        default,
        skip_serializing_if = "CloudRequirementsTomlManagedLayers::is_empty"
    )]
    pub managed_layers: CloudRequirementsTomlManagedLayers,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudRequirementsTomlManagedLayers {
    pub baseline: Vec<CloudRequirementsFragment>,
    pub system_overlay: Vec<CloudRequirementsFragment>,
}

impl CloudRequirementsTomlManagedLayers {
    fn is_empty(&self) -> bool {
        self.baseline.is_empty() && self.system_overlay.is_empty()
    }
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
    /// Baseline config layers in `ConfigLayerStack` order.
    pub baseline_config: Vec<ConfigLayerEntry>,
    /// System-overlay config layers in `ConfigLayerStack` order.
    pub system_overlay_config: Vec<ConfigLayerEntry>,
    /// Baseline requirements layers in requirements merge order.
    pub baseline_requirements: Vec<RequirementsLayerEntry>,
    /// System-overlay requirements layers in requirements merge order.
    pub system_overlay_requirements: Vec<RequirementsLayerEntry>,
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
                    managed_layers:
                        CloudConfigTomlManagedLayers {
                            baseline: config_baseline,
                            system_overlay: config_system_overlay,
                        },
                },
            requirements_toml:
                CloudRequirementsTomlBundle {
                    managed_layers:
                        CloudRequirementsTomlManagedLayers {
                            baseline: requirements_baseline,
                            system_overlay: requirements_system_overlay,
                        },
                },
        } = bundle;

        let parse_managed_config_fragments = |fragments, layer| {
            if strict_config {
                cloud_managed_config_layers_from_fragments_strict(fragments, base_dir, layer)
            } else {
                cloud_managed_config_layers_from_fragments(fragments, base_dir, layer)
            }
        };
        let baseline_config =
            parse_managed_config_fragments(config_baseline, CloudManagedLayer::Baseline)?;
        let system_overlay_config = parse_managed_config_fragments(
            config_system_overlay,
            CloudManagedLayer::SystemOverlay,
        )?;

        let baseline_requirements =
            requirements_layers_from_fragments(requirements_baseline, base_dir, |id, name| {
                RequirementSource::CloudManaged {
                    layer: CloudManagedLayer::Baseline,
                    id,
                    name,
                }
            });
        let system_overlay_requirements = requirements_layers_from_fragments(
            requirements_system_overlay,
            base_dir,
            |id, name| RequirementSource::CloudManaged {
                layer: CloudManagedLayer::SystemOverlay,
                id,
                name,
            },
        );

        Ok(Self {
            baseline_config,
            system_overlay_config,
            baseline_requirements,
            system_overlay_requirements,
        })
    }
}

fn requirements_layers_from_fragments(
    fragments: Vec<CloudRequirementsFragment>,
    base_dir: &AbsolutePathBuf,
    source_for_fragment: impl Fn(String, String) -> RequirementSource,
) -> Vec<RequirementsLayerEntry> {
    let mut layers = fragments
        .into_iter()
        .map(|fragment| {
            RequirementsLayerEntry::from_toml(
                source_for_fragment(fragment.id, fragment.name),
                fragment.contents,
            )
            .with_base_dir(base_dir.clone())
        })
        .collect::<Vec<_>>();
    // Bundle fragments arrive highest-priority first, while requirements
    // layers are merged lowest-priority to highest-priority.
    layers.reverse();
    layers
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
    fut: Shared<BoxFuture<'static, Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError>>>,
}

impl CloudConfigBundleLoader {
    pub fn new<F>(fut: F) -> Self
    where
        F: Future<Output = Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError>>
            + Send
            + 'static,
    {
        Self {
            fut: fut.boxed().shared(),
        }
    }

    pub async fn get(&self) -> Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError> {
        self.fut.clone().await
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
