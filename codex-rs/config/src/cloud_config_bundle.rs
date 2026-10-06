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

impl CloudRequirementsTomlBundle {
    pub(crate) fn into_layers(self) -> Vec<RequirementsLayerEntry> {
        // Bundle fragments arrive highest-priority first; requirements merge in reverse order.
        self.enterprise_managed
            .into_iter()
            .rev()
            .map(|fragment| {
                // Older backends used this fragment to deny every accelerated tier.
                // Preserve that meaning through mixed deployments and rollbacks.
                if fragment.id == "rbac-fast-mode"
                    && let Ok(mut contents) = toml::from_str::<toml::Value>(&fragment.contents)
                    && let Some(features) = contents
                        .get_mut("features")
                        .and_then(toml::Value::as_table_mut)
                    && features.get("fast_mode").and_then(toml::Value::as_bool) == Some(false)
                {
                    features.insert("ultrafast_mode".to_string(), toml::Value::Boolean(false));
                    return RequirementsLayerEntry::from_toml_value(
                        RequirementSource::EnterpriseManaged {
                            id: fragment.id,
                            name: fragment.name,
                        },
                        contents,
                    );
                }
                RequirementsLayerEntry::from_toml(
                    RequirementSource::EnterpriseManaged {
                        id: fragment.id,
                        name: fragment.name,
                    },
                    fragment.contents,
                )
            })
            .collect()
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
            requirements_toml,
        } = bundle;

        let enterprise_managed_config = if strict_config {
            cloud_config_layers_from_fragments_strict(config_enterprise_managed, base_dir)?
        } else {
            cloud_config_layers_from_fragments(config_enterprise_managed, base_dir)?
        };

        let enterprise_managed_requirements = requirements_toml
            .into_layers()
            .into_iter()
            .map(|layer| layer.with_base_dir(base_dir.clone()))
            .collect::<Vec<_>>();

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

type CloudConfigBundleGetter = dyn Fn() -> BoxFuture<'static, Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError>>
    + Send
    + Sync;

/// Tracks which delivered policy may authorize new enterprise MCP requests.
#[derive(Clone)]
pub struct CloudConfigBundlePolicy(watch::Sender<CloudConfigBundlePolicyState>);

struct CloudConfigBundlePolicyState {
    revision: u64,
    bundle: Option<CloudConfigBundle>,
    phase: CloudConfigBundlePolicyPhase,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum CloudConfigBundlePolicyPhase {
    Uninitialized,
    Suspended,
    Active,
    Retired,
}

impl Default for CloudConfigBundlePolicy {
    fn default() -> Self {
        Self(
            watch::channel(CloudConfigBundlePolicyState {
                revision: 0,
                bundle: None,
                phase: CloudConfigBundlePolicyPhase::Uninitialized,
            })
            .0,
        )
    }
}

impl fmt::Debug for CloudConfigBundlePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CloudConfigBundlePolicy").finish()
    }
}

impl CloudConfigBundlePolicy {
    /// Suspends old EMA admission before a changed delivered bundle is validated or cached.
    pub fn observe_remote_bundle(
        &self,
        bundle: &CloudConfigBundle,
    ) -> Option<CloudConfigBundlePolicyRevision> {
        let bundle = (!bundle.is_empty()).then(|| bundle.clone());
        let mut observed_revision = None;
        self.0.send_if_modified(|state| {
            if state.phase == CloudConfigBundlePolicyPhase::Retired {
                return false;
            }
            if state.phase != CloudConfigBundlePolicyPhase::Uninitialized && state.bundle == bundle
            {
                observed_revision = Some(state.revision);
                return false;
            }
            if state.phase != CloudConfigBundlePolicyPhase::Uninitialized {
                let Some(revision) = state.revision.checked_add(1) else {
                    state.phase = CloudConfigBundlePolicyPhase::Retired;
                    return true;
                };
                state.revision = revision;
            }
            state.bundle = bundle;
            state.phase = CloudConfigBundlePolicyPhase::Suspended;
            observed_revision = Some(state.revision);
            true
        });
        observed_revision.map(|revision| CloudConfigBundlePolicyRevision {
            revision,
            policy: self.clone(),
        })
    }

    /// Pairs a service result with its EMA revision before the snapshot becomes visible.
    pub fn publish_snapshot(&self, snapshot: &mut CloudConfigBundleSnapshot) {
        let Ok(bundle) = &snapshot.bundle else {
            return;
        };
        self.0.send_if_modified(|state| {
            let current = state.phase != CloudConfigBundlePolicyPhase::Retired
                && (state.phase == CloudConfigBundlePolicyPhase::Uninitialized
                    || state.bundle == *bundle);
            snapshot.binding = Some(CloudConfigBundleBinding {
                revision: current.then_some(state.revision),
                policy: self.clone(),
            });
            if current && state.phase != CloudConfigBundlePolicyPhase::Active {
                state.bundle = bundle.clone();
                state.phase = CloudConfigBundlePolicyPhase::Active;
                true
            } else {
                false
            }
        });
    }

    fn retire(&self) {
        self.0
            .send_modify(|state| state.phase = CloudConfigBundlePolicyPhase::Retired);
    }
}

/// An observed remote policy revision permitted to commit staged persistence.
pub struct CloudConfigBundlePolicyRevision {
    revision: u64,
    policy: CloudConfigBundlePolicy,
}

impl CloudConfigBundlePolicyRevision {
    /// Executes a synchronous commit only while this observation is still current.
    /// Prepare expensive work first; the guard blocks newer observations and retirement.
    pub fn commit_if_current<E>(self, commit: impl FnOnce() -> Result<(), E>) -> Result<(), E> {
        let state = self.policy.0.borrow();
        if state.revision == self.revision
            && matches!(
                state.phase,
                CloudConfigBundlePolicyPhase::Suspended | CloudConfigBundlePolicyPhase::Active
            )
        {
            commit()
        } else {
            Ok(())
        }
    }
}

/// The policy revision used to resolve one set of enterprise MCP inputs.
#[derive(Clone, Debug)]
pub struct CloudConfigBundleBinding {
    revision: Option<u64>,
    policy: CloudConfigBundlePolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloudConfigBundleBindingStatus {
    Current,
    Stale,
    Suspended,
}

impl CloudConfigBundleBinding {
    /// Keeps the policy observation stable until this guard is dropped.
    pub fn read(&self) -> CloudConfigBundleBindingGuard<'_> {
        let state = self.policy.0.borrow();
        let status = match (state.phase, self.revision) {
            (CloudConfigBundlePolicyPhase::Active, Some(expected))
                if state.revision == expected =>
            {
                CloudConfigBundleBindingStatus::Current
            }
            (CloudConfigBundlePolicyPhase::Active, Some(_)) => {
                CloudConfigBundleBindingStatus::Stale
            }
            _ => CloudConfigBundleBindingStatus::Suspended,
        };
        CloudConfigBundleBindingGuard {
            _state: state,
            status,
        }
    }
}

pub struct CloudConfigBundleBindingGuard<'a> {
    _state: watch::Ref<'a, CloudConfigBundlePolicyState>,
    pub status: CloudConfigBundleBindingStatus,
}

pub struct CloudConfigBundlePolicyChanges(watch::Receiver<CloudConfigBundlePolicyState>);

impl CloudConfigBundlePolicyChanges {
    pub fn is_active(&self) -> bool {
        self.0.borrow().phase == CloudConfigBundlePolicyPhase::Active
    }

    pub async fn changed(&mut self) -> Result<(), watch::error::RecvError> {
        self.0.changed().await
    }
}

#[derive(Clone)]
pub struct CloudConfigBundleSnapshot {
    pub bundle: Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError>,
    pub binding: Option<CloudConfigBundleBinding>,
}

type CloudConfigBundleSnapshotGetter =
    dyn Fn() -> BoxFuture<'static, CloudConfigBundleSnapshot> + Send + Sync;

#[derive(Clone)]
pub struct CloudConfigBundleLoader {
    getter: Arc<CloudConfigBundleGetter>,
    snapshot_getter: Option<Arc<CloudConfigBundleSnapshotGetter>>,
    ema_policy: Option<CloudConfigBundlePolicy>,
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
            snapshot_getter: None,
            ema_policy: None,
        }
    }

    /// Freezes the delivered bundle while retaining its live policy revision binding.
    pub fn from_snapshot(snapshot: CloudConfigBundleSnapshot) -> Self {
        let bundle = snapshot.bundle.clone();
        let mut loader = Self::from_getter(move || std::future::ready(bundle.clone()));
        loader.ema_policy = snapshot
            .binding
            .as_ref()
            .map(|binding| binding.policy.clone());
        loader.snapshot_getter = Some(Arc::new(move || {
            std::future::ready(snapshot.clone()).boxed()
        }));
        loader
    }

    pub fn with_ema_policy_snapshots<F, Fut>(
        mut self,
        policy: CloudConfigBundlePolicy,
        getter: F,
    ) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = CloudConfigBundleSnapshot> + Send + 'static,
    {
        self.ema_policy = Some(policy);
        self.snapshot_getter = Some(Arc::new(move || getter().boxed()));
        self
    }

    pub async fn get_snapshot(&self) -> CloudConfigBundleSnapshot {
        match &self.snapshot_getter {
            Some(getter) => getter().await,
            None => CloudConfigBundleSnapshot {
                bundle: self.get().await,
                binding: None,
            },
        }
    }

    pub fn ema_policy_changes(&self) -> Option<CloudConfigBundlePolicyChanges> {
        self.ema_policy
            .as_ref()
            .map(|policy| CloudConfigBundlePolicyChanges(policy.0.subscribe()))
    }

    /// Stops new EMA admission from this owner without changing generic loader behavior.
    pub fn retire_ema_policy(&self) {
        if let Some(policy) = &self.ema_policy {
            policy.retire();
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
