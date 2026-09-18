//! Authenticated, immutable references for one executor plugin's measurements.

use crate::PluginsConfigInput;
use crate::TrustedPluginRoots;
use crate::manager::RemoteInstalledPluginsAuthIdentity;
use crate::remote::fetch_measurement_reference_bundle;
use crate::remote_bundle::download_remote_plugin_bundle;
use crate::remote_bundle::install_remote_plugin_bundle;
use crate::script_attribution::PluginMeasurementTarget;
use codex_login::CodexAuth;
use codex_plugin::PluginId;
use codex_utils_path_uri::PathConvention;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Mutex;
use tokio::sync::watch;

const MAX_CACHED_REFERENCES: usize = 4;

/// Owns the immutable files backing its roots. Keep the reference alive for the
/// full command/turn that consumes those roots, including asynchronous commands.
pub struct PluginMeasurementReference {
    roots: TrustedPluginRoots,
    home: Option<TempDir>,
}

impl Drop for PluginMeasurementReference {
    fn drop(&mut self) {
        let Some(home) = self.home.take() else {
            return;
        };
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn_blocking(move || drop(home));
        } else {
            drop(home);
        }
    }
}

impl PluginMeasurementReference {
    pub fn roots(&self) -> &TrustedPluginRoots {
        &self.roots
    }
}

#[derive(PartialEq, Eq)]
struct ReferenceKey {
    actor: RemoteInstalledPluginsAuthIdentity,
    endpoint: String,
    remote_id: String,
    plugin_id: PluginId,
    version: String,
}

type ReferenceCell = Arc<watch::Receiver<Option<Arc<PluginMeasurementReference>>>>;

fn is_preparing(cell: &ReferenceCell) -> bool {
    cell.has_changed().is_ok() && cell.borrow().is_none()
}

/// A thread-scoped cache, separate from the installed plugin/capability store.
/// Each version has its own store so installing a new version cannot remove
/// files still leased by an earlier turn. At most four entries are retained,
/// including active preparations; command-held leases can outlive eviction.
#[derive(Default)]
pub struct RemotePluginMeasurementCache {
    references: Mutex<Vec<(ReferenceKey, ReferenceCell)>>,
}

impl RemotePluginMeasurementCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rechecks current installation authorization, then prepares only the
    /// requested version. Previously authenticated versions remain reusable after
    /// a release update; a cache miss can download only the current release.
    /// Callers apply analytics consent and product gates.
    /// Concurrent callers share a download even if one caller is cancelled.
    /// Returns no reference when all cache entries are actively preparing.
    /// Failures omit authenticated URLs and response bodies from diagnostics.
    pub async fn prepare(
        &self,
        config: &PluginsConfigInput,
        auth: &CodexAuth,
        target: &PluginMeasurementTarget,
    ) -> anyhow::Result<Option<Arc<PluginMeasurementReference>>> {
        if !config.plugins_enabled || !config.remote_plugin_enabled || !auth.uses_codex_backend() {
            return Ok(None);
        }
        // Like normal remote loading, the authenticated installed catalog is
        // authoritative for enabled state, including default installations.
        // GLOBAL plugins have already passed server-side admission; local
        // marketplace product policy and skill capability loading are separate.
        let service = config.remote_plugin_service_config();
        let Some((remote_id, bundle)) = fetch_measurement_reference_bundle(&service, auth, target)
            .await
            .map_err(|_| anyhow::anyhow!("plugin measurement catalog lookup failed"))?
        else {
            return Ok(None);
        };
        // The executor hint follows its filesystem's case rules. The cache and
        // local store use the authenticated bundle's exact identity instead.
        let plugin_id = bundle.plugin_id.clone();
        let version = bundle.plugin_version.clone();
        let current_version = match target.path_convention {
            PathConvention::Windows => target.version.eq_ignore_ascii_case(&version),
            PathConvention::Posix => target.version == version,
        };
        let key = ReferenceKey {
            actor: RemoteInstalledPluginsAuthIdentity::from_auth(Some(auth)),
            endpoint: config.chatgpt_base_url.clone(),
            remote_id: remote_id.clone(),
            plugin_id: plugin_id.clone(),
            version: if current_version {
                version.clone()
            } else {
                target.version.clone()
            },
        };
        let (cell, initializer, retired) = {
            let mut references = self.references.lock().await;
            let mut retired = Vec::new();
            // Keep active preparations admitted until they finish, even after an
            // identity change; they can never match the new key.
            for index in (0..references.len()).rev() {
                let (existing, cell) = &references[index];
                // A closed, empty result is a completed failure. Only a new
                // caller can admit a retry; existing waiters observe that failure.
                if (cell.has_changed().is_err() && cell.borrow().is_none())
                    || (!is_preparing(cell)
                        && (existing.actor != key.actor
                            || existing.endpoint != key.endpoint
                            || (existing.plugin_id == key.plugin_id
                                && existing.remote_id != key.remote_id)))
                {
                    retired.push(references.remove(index));
                }
            }
            let mut initializer = None;
            let mut matching = references.iter().enumerate().filter(|(_, (existing, _))| {
                if current_version {
                    return existing == &key;
                }
                existing.actor == key.actor
                    && existing.endpoint == key.endpoint
                    && existing.remote_id == key.remote_id
                    && existing.plugin_id == key.plugin_id
                    && match target.path_convention {
                        PathConvention::Windows => {
                            existing.version.eq_ignore_ascii_case(&key.version)
                        }
                        PathConvention::Posix => existing.version == key.version,
                    }
            });
            // Historical Windows hints must not select between case-distinct releases.
            let cached_index = match (matching.next(), matching.next()) {
                (Some((index, _)), None) => Some(index),
                _ => None,
            };
            let cell = if let Some(index) = cached_index {
                let entry = references.remove(index);
                let cell = Arc::clone(&entry.1);
                references.push(entry);
                Some(cell)
            } else if !current_version {
                // The catalog cannot supply a trusted historical bundle.
                None
            } else if references.len() < MAX_CACHED_REFERENCES {
                let (sender, receiver) = watch::channel(None);
                let cell = Arc::new(receiver);
                initializer = Some(sender);
                references.push((key, Arc::clone(&cell)));
                Some(cell)
            } else if let Some(index) = references.iter().position(|(_, cell)| !is_preparing(cell))
            {
                retired.push(references.remove(index));
                let (sender, receiver) = watch::channel(None);
                let cell = Arc::new(receiver);
                initializer = Some(sender);
                references.push((key, Arc::clone(&cell)));
                Some(cell)
            } else {
                None
            };
            (cell, initializer, retired)
        };
        // Final reference drops schedule store cleanup outside the cache mutex.
        drop(retired);
        let Some(cell) = cell else {
            return Ok(None);
        };
        if let Some(sender) = initializer {
            // The cache keeps the receiver alive when individual callers cancel.
            // Once the cache and all callers disappear, stop waiting/downloading.
            tokio::spawn(async move {
                let reference = tokio::select! {
                    biased;
                    _ = sender.closed() => return,
                    reference = async {
                        let bytes = download_remote_plugin_bundle(&service, &bundle).await?;
                        // Blocking extraction cannot be aborted once started. Its
                        // owner must keep the store alive even if this task stops.
                        tokio::task::spawn_blocking(move || {
                            let home = tempfile::tempdir()?;
                            install_remote_plugin_bundle(home.path().to_owned(), bundle, bytes)?;
                            let roots = TrustedPluginRoots::from_measurement_reference(
                                home.path(),
                                &plugin_id,
                                &version,
                                &remote_id,
                            )
                            .ok_or_else(|| anyhow::anyhow!("invalid plugin measurement reference"))?;
                            Ok::<_, anyhow::Error>(Arc::new(PluginMeasurementReference {
                                roots,
                                home: Some(home),
                            }))
                        }).await?
                    } => reference,
                };
                if let Ok(reference) = reference {
                    sender.send_replace(Some(reference));
                }
                // Closing an empty channel reports failure without retaining
                // authenticated URLs or response bodies in the shared result.
            });
        }
        let mut receiver = cell.as_ref().clone();
        let reference = receiver
            .wait_for(Option::is_some)
            .await
            .map_err(|_| anyhow::anyhow!("plugin measurement preparation failed"))?;
        Ok(reference.clone())
    }
}

#[cfg(test)]
#[path = "plugin_measurement_references_tests.rs"]
mod tests;
