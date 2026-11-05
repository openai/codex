//! Shared remote plugin mutations for the CLI and app-server.
//! The sync gate covers cache/backend changes and refresh scheduling. An install outcome keeps
//! its bundle protected from pruning until the caller finishes downstream setup and drops it.

use super::EffectivePluginsChangedCallback;
use super::PluginInstallOutcome;
use super::PluginsConfigInput;
use super::PluginsManager;
use crate::marketplace::MarketplacePluginAuthPolicy;
use crate::remote;
use crate::remote::RemoteInstalledPluginBundleSyncError;
use crate::remote::RemotePluginCacheMutationGuard;
use crate::remote::RemotePluginCatalogError;
use crate::remote::RemotePluginDetail;
use crate::remote_bundle;
use crate::remote_bundle::RemotePluginBundleInstallError;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginAvailability;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_login::CodexAuth;
use codex_plugin::PluginId;
use codex_plugin::PluginTelemetryMetadata;
use std::sync::Arc;

#[cfg(test)]
#[path = "remote_mutations_tests.rs"]
mod tests;

pub struct RemotePluginInstallRequest {
    pub marketplace_name: String,
    pub remote_plugin_id: String,
    pub install_attempt_id: Option<String>,
}

/// Keep this outcome alive throughout post-install setup to protect the installed bundle.
#[must_use]
pub struct RemotePluginInstallOutcome {
    pub installed: PluginInstallOutcome,
    pub detail: RemotePluginDetail,
    pub app_ids_needing_auth: Option<Vec<String>>,
    _cache_mutation: RemotePluginCacheMutationGuard,
}

pub struct RemotePluginUninstallOutcome {
    pub telemetry: PluginTelemetryMetadata,
    pub effective_plugins_changed: bool,
    /// The backend uninstall succeeded even when removing the local cache failed.
    pub cache_removal_error: Option<RemotePluginCatalogError>,
}

#[derive(Debug, thiserror::Error)]
#[error("{kind}")]
pub struct RemotePluginOperationError {
    pub plugin_id: Option<PluginId>,
    #[source]
    pub kind: Box<RemotePluginOperationErrorKind>,
}

#[derive(Debug, thiserror::Error)]
pub enum RemotePluginOperationErrorKind {
    #[error("{context}: {source}")]
    Catalog {
        context: &'static str,
        source: RemotePluginCatalogError,
    },
    #[error("install remote plugin bundle: {0}")]
    Bundle(#[source] RemotePluginBundleInstallError),
    #[error("{context}: {source}")]
    Sync {
        context: &'static str,
        source: RemoteInstalledPluginBundleSyncError,
    },
    #[error("remote plugin {0} is disabled by admin")]
    DisabledByAdmin(String),
    #[error("remote plugin {0} is not available for install")]
    NotAvailable(String),
    #[error("{0}")]
    InvalidRequest(String),
    #[error("{0}")]
    Internal(String),
}

impl PluginsManager {
    /// Resolve, validate, materialize, and install a remote plugin as one coordinated mutation.
    pub async fn install_remote_plugin(
        self: &Arc<Self>,
        config: &PluginsConfigInput,
        auth: Option<&CodexAuth>,
        request: RemotePluginInstallRequest,
        on_effective_plugins_changed: Option<EffectivePluginsChangedCallback>,
    ) -> Result<RemotePluginInstallOutcome, RemotePluginOperationError> {
        use RemotePluginOperationErrorKind as Error;
        let unresolved = |kind| RemotePluginOperationError {
            plugin_id: None,
            kind: Box::new(kind),
        };
        let RemotePluginInstallRequest {
            marketplace_name,
            remote_plugin_id,
            install_attempt_id,
        } = request;
        if !config.plugins_enabled {
            return Err(unresolved(Error::InvalidRequest(format!(
                "remote plugin install is not enabled for marketplace {marketplace_name}"
            ))));
        }
        remote::validate_remote_plugin_id(&remote_plugin_id)
            .map_err(|err| unresolved(Error::InvalidRequest(err.message)))?;
        let service = self
            .remote_plugin_service_config(config)
            .for_current_account();
        let detail = remote::fetch_remote_plugin_detail_with_download_urls(
            &service,
            auth,
            &marketplace_name,
            &remote_plugin_id,
        )
        .await
        .map_err(|source| {
            unresolved(Error::Catalog {
                context: "read remote plugin details before install",
                source,
            })
        })?;
        let plugin_id = PluginId::parse(&detail.summary.id).map_err(|err| {
            unresolved(Error::Internal(format!(
                "invalid resolved plugin id `{}`: {err}",
                detail.summary.id
            )))
        })?;
        let resolved = |kind| RemotePluginOperationError {
            plugin_id: Some(plugin_id.clone()),
            kind: Box::new(kind),
        };
        if detail.summary.availability == PluginAvailability::DisabledByAdmin {
            return Err(resolved(Error::DisabledByAdmin(remote_plugin_id)));
        }
        if detail.summary.install_policy == PluginInstallPolicy::NotAvailable {
            return Err(resolved(Error::NotAvailable(remote_plugin_id)));
        }
        let bundle = remote_bundle::validate_remote_plugin_bundle(
            &remote_plugin_id,
            &detail.marketplace_name,
            &detail.summary.name,
            detail.release_version.as_deref(),
            detail.bundle_download_url.as_deref(),
            detail.app_manifest.clone(),
        )
        .map_err(|err| resolved(Error::Bundle(err)))?;
        let _sync_guard = self
            .acquire_remote_installed_plugin_sync_guard()
            .await
            .map_err(|source| {
                resolved(Error::Sync {
                    context: "failed to coordinate remote plugin install",
                    source,
                })
            })?;
        let cache_mutation = remote::mark_remote_plugin_cache_mutation_in_flight(
            &self.codex_home,
            &detail.marketplace_name,
            &detail.summary.name,
        );
        // Materialize first: a failed download must never leave a backend installation behind.
        let installed = remote_bundle::download_and_install_remote_plugin_bundle(
            &service,
            self.codex_home.clone(),
            bundle,
        )
        .await
        .map_err(|err| resolved(Error::Bundle(err)))?;
        let install_result = if let Some(install_attempt_id) = install_attempt_id.as_deref() {
            remote::install_remote_plugin_with_install_attempt_id(
                &service,
                auth,
                &detail.marketplace_name,
                &remote_plugin_id,
                install_attempt_id,
            )
            .await
        } else {
            remote::install_remote_plugin(
                &service,
                auth,
                &detail.marketplace_name,
                &remote_plugin_id,
            )
            .await
        }
        .map_err(|source| {
            resolved(Error::Catalog {
                context: "install remote plugin",
                source,
            })
        })?;
        self.maybe_start_remote_installed_plugins_cache_refresh_after_mutation(
            config,
            auth.cloned(),
            on_effective_plugins_changed,
        );
        Ok(RemotePluginInstallOutcome {
            installed: PluginInstallOutcome {
                plugin_id: installed.plugin_id,
                plugin_version: installed.plugin_version,
                installed_path: installed.installed_path,
                auth_policy: match detail.summary.auth_policy {
                    PluginAuthPolicy::OnInstall => MarketplacePluginAuthPolicy::OnInstall,
                    PluginAuthPolicy::OnUse => MarketplacePluginAuthPolicy::OnUse,
                },
            },
            detail,
            app_ids_needing_auth: install_result.app_ids_needing_auth,
            _cache_mutation: cache_mutation,
        })
    }

    /// Remove the backend installation and local cache, then schedule an installed-state refresh.
    pub async fn uninstall_remote_plugin(
        self: &Arc<Self>,
        config: &PluginsConfigInput,
        auth: Option<&CodexAuth>,
        remote_plugin_id: &str,
        on_effective_plugins_changed: Option<EffectivePluginsChangedCallback>,
    ) -> Result<RemotePluginUninstallOutcome, RemotePluginOperationError> {
        use RemotePluginOperationErrorKind as Error;
        let unresolved = |kind| RemotePluginOperationError {
            plugin_id: None,
            kind: Box::new(kind),
        };
        if !config.plugins_enabled {
            return Err(unresolved(Error::InvalidRequest(
                "remote plugin uninstall is not enabled".to_string(),
            )));
        }
        remote::validate_remote_plugin_id(remote_plugin_id)
            .map_err(|err| unresolved(Error::InvalidRequest(err.message)))?;
        let service = self.remote_plugin_service_config(config);
        let target =
            remote::resolve_remote_plugin_uninstall_target(&service, auth, remote_plugin_id)
                .await
                .map_err(|source| {
                    unresolved(Error::Catalog {
                        context: "resolve remote plugin before uninstall",
                        source,
                    })
                })?;
        let mut telemetry = self
            .telemetry_metadata_for_installed_plugin_with_remote_id(
                &target.plugin_id,
                remote_plugin_id,
            )
            .await;
        if telemetry.capability_summary.is_none() {
            telemetry.capability_summary = Some(target.fallback_capability_summary.clone());
        }
        let plugin_id = target.plugin_id.clone();
        let resolved = |kind| RemotePluginOperationError {
            plugin_id: Some(plugin_id.clone()),
            kind: Box::new(kind),
        };
        let _sync_guard = self
            .acquire_remote_installed_plugin_sync_guard()
            .await
            .map_err(|source| {
                resolved(Error::Sync {
                    context: "failed to coordinate remote plugin uninstall",
                    source,
                })
            })?;
        let _cache_mutation = remote::mark_remote_plugin_cache_mutation_in_flight(
            &self.codex_home,
            &plugin_id.marketplace_name,
            &plugin_id.plugin_name,
        );
        let cache_removal_error =
            match remote::uninstall_remote_plugin(&service, auth, self.codex_home.clone(), target)
                .await
            {
                Ok(()) => None,
                Err(err @ RemotePluginCatalogError::CacheRemove(_)) => Some(err),
                Err(source) => {
                    return Err(resolved(Error::Catalog {
                        context: "uninstall remote plugin",
                        source,
                    }));
                }
            };
        let effective_plugins_changed = self.clear_remote_installed_plugins_cache();
        self.maybe_start_remote_installed_plugins_cache_refresh_after_mutation(
            config,
            auth.cloned(),
            on_effective_plugins_changed,
        );
        Ok(RemotePluginUninstallOutcome {
            telemetry,
            effective_plugins_changed,
            cache_removal_error,
        })
    }
}
