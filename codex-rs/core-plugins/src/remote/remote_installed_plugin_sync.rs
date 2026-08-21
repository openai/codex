use super::REMOTE_CREATED_BY_ME_MARKETPLACE_NAME;
use super::REMOTE_GLOBAL_MARKETPLACE_NAME;
use super::REMOTE_WORKSPACE_MARKETPLACE_NAME;
use super::REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME;
use super::REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME;
use super::REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME;
use super::RemoteInstalledPlugin;
use super::RemoteInstalledPluginScope;
use super::RemotePluginCatalogError;
use super::RemotePluginScope;
use super::RemotePluginServiceConfig;
use super::RemotePluginShareDiscoverability;
use super::ensure_chatgpt_auth;
use super::fetch_installed_plugins;
use crate::store::PLUGINS_CACHE_DIR;
use crate::store::PluginStore;
use crate::store::PluginStoreError;
use codex_login::CodexAuth;
use codex_plugin::PluginId;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Weak;
use tokio::sync::Semaphore;
use tracing::warn;

static REMOTE_INSTALLED_PLUGIN_BUNDLE_SYNC_GATES: OnceLock<
    Mutex<HashMap<PathBuf, Weak<Semaphore>>>,
> = OnceLock::new();
static REMOTE_PLUGIN_CACHE_MUTATIONS_IN_FLIGHT: OnceLock<
    Mutex<HashMap<RemotePluginCacheMutationKey, usize>>,
> = OnceLock::new();

/// A remote plugin bundle newly installed or updated from an authenticated snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePluginMaterialization {
    pub plugin_id: PluginId,
    pub scope: RemotePluginScope,
    pub discoverability: Option<RemotePluginShareDiscoverability>,
    pub authenticated_account_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RemoteInstalledPluginBundleSyncOutcome {
    pub materialized_remote_plugins: Vec<RemotePluginMaterialization>,
    pub removed_cache_plugin_ids: Vec<String>,
    pub failed_remote_plugin_ids: Vec<String>,
    /// Failures that leave an otherwise valid installed plugin unavailable locally.
    pub failed_materialization_remote_plugin_ids: Vec<String>,
}

pub(crate) struct RemoteInstalledPluginBundleSyncResult {
    pub(crate) outcome: RemoteInstalledPluginBundleSyncOutcome,
    pub(crate) installed_plugins: Vec<RemoteInstalledPlugin>,
}

impl RemoteInstalledPluginBundleSyncOutcome {
    pub fn changed_local_cache(&self) -> bool {
        !self.materialized_remote_plugins.is_empty() || !self.removed_cache_plugin_ids.is_empty()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RemoteInstalledPluginBundleSyncError {
    #[error("{0}")]
    Catalog(#[from] RemotePluginCatalogError),

    #[error("{0}")]
    Store(#[from] PluginStoreError),

    #[error("timed out waiting for another remote plugin cache mutation; retry")]
    LockTimeout,

    #[error("remote plugin state changed during reconciliation; retry reconciliation")]
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RemotePluginCacheMutationKey {
    plugin_cache_root: PathBuf,
    marketplace_name: String,
    plugin_name: String,
}

pub struct RemotePluginCacheMutationGuard {
    key: RemotePluginCacheMutationKey,
}

pub(crate) fn remote_installed_plugin_bundle_sync_gate(codex_home: &Path) -> Arc<Semaphore> {
    let plugin_cache_root = remote_plugin_cache_root(codex_home);
    let gates =
        REMOTE_INSTALLED_PLUGIN_BUNDLE_SYNC_GATES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut gates = match gates.lock() {
        Ok(gates) => gates,
        Err(err) => err.into_inner(),
    };
    if let Some(gate) = gates.get(&plugin_cache_root).and_then(Weak::upgrade) {
        return gate;
    }
    let gate = Arc::new(Semaphore::new(/*permits*/ 1));
    gates.insert(plugin_cache_root, Arc::downgrade(&gate));
    gate
}

pub async fn sync_remote_installed_plugin_bundles_once(
    codex_home: PathBuf,
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
) -> Result<RemoteInstalledPluginBundleSyncOutcome, RemoteInstalledPluginBundleSyncError> {
    let result =
        sync_remote_installed_plugin_bundles_once_with_snapshot(codex_home, config, auth).await?;
    Ok(result.outcome)
}

pub(crate) async fn sync_remote_installed_plugin_bundles_once_with_snapshot(
    codex_home: PathBuf,
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
) -> Result<RemoteInstalledPluginBundleSyncResult, RemoteInstalledPluginBundleSyncError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let authenticated_account_id = auth.get_account_id();
    let fetched_installed_plugins = fetch_installed_plugins(
        config,
        auth,
        RemoteInstalledPluginScope::All,
        /*include_download_urls*/ true,
    )
    .await?;
    // `/installed` is an authoritative full snapshot. If any row cannot be
    // canonicalized to a local cache key, omitting it makes an installed plugin
    // indistinguishable from an uninstall, so reject the pass before downloads,
    // publication, or stale cleanup.
    let mut validated_installed_plugins = Vec::with_capacity(fetched_installed_plugins.len());
    for installed_plugin in fetched_installed_plugins {
        let cached_plugin = super::remote_installed_plugin_to_cache_entry(&installed_plugin)?;
        let plugin_id = PluginId::new(
            installed_plugin.plugin.name.clone(),
            cached_plugin.marketplace_name.clone(),
        )
        .map_err(|err| {
            RemotePluginCatalogError::UnexpectedResponse(format!(
                "remote installed plugin `{}` has an invalid local cache id: {err}",
                installed_plugin.plugin.id
            ))
        })?;
        validated_installed_plugins.push((installed_plugin, cached_plugin, plugin_id));
    }
    let store = PluginStore::try_new(codex_home.clone())?;
    let mut installed_plugin_names_by_marketplace =
        BTreeMap::<String, BTreeSet<String>>::from_iter([
            (REMOTE_GLOBAL_MARKETPLACE_NAME.to_string(), BTreeSet::new()),
            (
                REMOTE_CREATED_BY_ME_MARKETPLACE_NAME.to_string(),
                BTreeSet::new(),
            ),
            (
                REMOTE_WORKSPACE_MARKETPLACE_NAME.to_string(),
                BTreeSet::new(),
            ),
            (
                REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME.to_string(),
                BTreeSet::new(),
            ),
            (
                REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME.to_string(),
                BTreeSet::new(),
            ),
            (
                REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME.to_string(),
                BTreeSet::new(),
            ),
        ]);
    let mut materialized_remote_plugins = BTreeMap::new();
    let mut failed_remote_plugin_ids = BTreeSet::new();
    let mut failed_materialization_remote_plugin_ids = BTreeSet::new();
    let mut installed_plugins = Vec::new();

    for (installed_plugin, cached_plugin, plugin_id) in validated_installed_plugins {
        let marketplace_name = cached_plugin.marketplace_name.clone();
        let plugin = installed_plugin.plugin;
        let scope = plugin.scope;
        let discoverability = plugin.discoverability;
        installed_plugin_names_by_marketplace
            .entry(marketplace_name.clone())
            .or_default()
            .insert(plugin.name.clone());
        let release_version = plugin
            .release
            .version
            .as_deref()
            .map(str::trim)
            .filter(|version| !version.is_empty());
        if let Some(release_version) = release_version
            && store.active_plugin_version(&plugin_id).as_deref() == Some(release_version)
        {
            if let Err(err) = store.write_remote_plugin_id(&plugin_id, &plugin.id) {
                warn!(
                    remote_plugin_id = %plugin.id,
                    plugin = %plugin.name,
                    marketplace = %marketplace_name,
                    error = %err,
                    "failed to persist identity for cached remote installed plugin"
                );
                failed_remote_plugin_ids.insert(plugin.id);
            }
            installed_plugins.push(cached_plugin);
            continue;
        }

        let bundle = match crate::remote_bundle::validate_remote_plugin_bundle(
            &plugin.id,
            &marketplace_name,
            &plugin.name,
            release_version,
            plugin.release.bundle_download_url.as_deref(),
            plugin.release.app_manifest.clone(),
        ) {
            Ok(bundle) => bundle,
            Err(err) => {
                warn!(
                    remote_plugin_id = %plugin.id,
                    plugin = %plugin.name,
                    marketplace = %marketplace_name,
                    error = %err,
                    "skipping remote installed plugin bundle download"
                );
                failed_remote_plugin_ids.insert(plugin.id.clone());
                failed_materialization_remote_plugin_ids.insert(plugin.id);
                installed_plugins.push(cached_plugin);
                continue;
            }
        };

        match crate::remote_bundle::download_and_install_remote_plugin_bundle(
            config,
            codex_home.clone(),
            bundle,
        )
        .await
        {
            Ok(result) => {
                let plugin_id = result.plugin_id;
                materialized_remote_plugins.insert(
                    plugin_id.as_key(),
                    RemotePluginMaterialization {
                        plugin_id,
                        scope,
                        discoverability,
                        authenticated_account_id: authenticated_account_id.clone(),
                    },
                );
            }
            Err(err) => {
                warn!(
                    remote_plugin_id = %plugin.id,
                    plugin = %plugin.name,
                    marketplace = %marketplace_name,
                    error = %err,
                    "failed to download remote installed plugin bundle"
                );
                failed_remote_plugin_ids.insert(plugin.id.clone());
                failed_materialization_remote_plugin_ids.insert(plugin.id);
            }
        }
        installed_plugins.push(cached_plugin);
    }

    installed_plugins.sort_by(|left, right| {
        left.marketplace_name
            .cmp(&right.marketplace_name)
            .then_with(|| left.id.cmp(&right.id))
    });
    let stale_cache_cleanup = tokio::task::spawn_blocking(move || {
        let mut removed_cache_plugin_ids = Vec::new();
        let cleanup_error = remove_stale_remote_plugin_caches(
            codex_home.as_path(),
            &installed_plugin_names_by_marketplace,
            &mut removed_cache_plugin_ids,
        )
        .err();
        removed_cache_plugin_ids.sort();
        (removed_cache_plugin_ids, cleanup_error)
    })
    .await;
    let (removed_cache_plugin_ids, cleanup_error) = match stale_cache_cleanup {
        Ok(result) => result,
        Err(err) => {
            warn!(error = %err, "failed to join stale remote plugin cache cleanup task");
            (Vec::new(), Some(err.to_string()))
        }
    };
    if let Some(err) = cleanup_error {
        warn!(error = %err, "failed to remove stale remote plugin cache entries");
    }

    Ok(RemoteInstalledPluginBundleSyncResult {
        outcome: RemoteInstalledPluginBundleSyncOutcome {
            materialized_remote_plugins: materialized_remote_plugins.into_values().collect(),
            removed_cache_plugin_ids,
            failed_remote_plugin_ids: failed_remote_plugin_ids.into_iter().collect(),
            failed_materialization_remote_plugin_ids: failed_materialization_remote_plugin_ids
                .into_iter()
                .collect(),
        },
        installed_plugins,
    })
}

pub fn mark_remote_plugin_cache_mutation_in_flight(
    codex_home: &Path,
    marketplace_name: &str,
    plugin_name: &str,
) -> RemotePluginCacheMutationGuard {
    let key = RemotePluginCacheMutationKey {
        plugin_cache_root: remote_plugin_cache_root(codex_home),
        marketplace_name: marketplace_name.to_string(),
        plugin_name: plugin_name.to_string(),
    };
    let mutations =
        REMOTE_PLUGIN_CACHE_MUTATIONS_IN_FLIGHT.get_or_init(|| Mutex::new(HashMap::new()));
    let mut mutations = match mutations.lock() {
        Ok(mutations) => mutations,
        Err(err) => err.into_inner(),
    };
    *mutations.entry(key.clone()).or_default() += 1;
    RemotePluginCacheMutationGuard { key }
}

impl Drop for RemotePluginCacheMutationGuard {
    fn drop(&mut self) {
        let Some(mutations) = REMOTE_PLUGIN_CACHE_MUTATIONS_IN_FLIGHT.get() else {
            return;
        };
        let mut mutations = match mutations.lock() {
            Ok(mutations) => mutations,
            Err(err) => err.into_inner(),
        };
        if let Some(count) = mutations.get_mut(&self.key) {
            *count -= 1;
            if *count == 0 {
                mutations.remove(&self.key);
            }
        }
    }
}

fn remove_stale_remote_plugin_caches(
    codex_home: &Path,
    installed_plugin_names_by_marketplace: &BTreeMap<String, BTreeSet<String>>,
    removed_cache_plugin_ids: &mut Vec<String>,
) -> Result<(), String> {
    for marketplace_name in [
        REMOTE_GLOBAL_MARKETPLACE_NAME,
        REMOTE_CREATED_BY_ME_MARKETPLACE_NAME,
        REMOTE_WORKSPACE_MARKETPLACE_NAME,
        REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME,
        REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME,
        REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME,
    ] {
        let marketplace_root = codex_home.join(PLUGINS_CACHE_DIR).join(marketplace_name);
        if !marketplace_root.exists() {
            continue;
        }
        let installed_plugin_names = installed_plugin_names_by_marketplace
            .get(marketplace_name)
            .cloned()
            .unwrap_or_default();
        for entry in fs::read_dir(&marketplace_root).map_err(|err| {
            format!(
                "failed to read remote plugin cache directory {}: {err}",
                marketplace_root.display()
            )
        })? {
            let entry = entry.map_err(|err| {
                format!(
                    "failed to enumerate remote plugin cache directory {}: {err}",
                    marketplace_root.display()
                )
            })?;
            let plugin_name = entry.file_name().into_string().map_err(|file_name| {
                format!(
                    "remote plugin cache entry under {} is not valid UTF-8: {:?}",
                    marketplace_root.display(),
                    file_name
                )
            })?;
            if installed_plugin_names.contains(&plugin_name) {
                continue;
            }
            if is_remote_plugin_cache_mutation_in_flight(codex_home, marketplace_name, &plugin_name)
            {
                continue;
            }

            let cache_path = entry.path();
            if cache_path.is_dir() {
                fs::remove_dir_all(&cache_path).map_err(|err| {
                    format!(
                        "failed to remove stale remote plugin cache entry {}: {err}",
                        cache_path.display()
                    )
                })?;
            } else {
                fs::remove_file(&cache_path).map_err(|err| {
                    format!(
                        "failed to remove stale remote plugin cache entry {}: {err}",
                        cache_path.display()
                    )
                })?;
            }
            let plugin_key = PluginId::new(plugin_name.clone(), marketplace_name.to_string())
                .map(|plugin_id| plugin_id.as_key())
                .unwrap_or_else(|_| format!("{plugin_name}@{marketplace_name}"));
            removed_cache_plugin_ids.push(plugin_key);
        }
    }

    Ok(())
}

fn remote_plugin_cache_root(codex_home: &Path) -> PathBuf {
    codex_home.join(PLUGINS_CACHE_DIR)
}

fn is_remote_plugin_cache_mutation_in_flight(
    codex_home: &Path,
    marketplace_name: &str,
    plugin_name: &str,
) -> bool {
    let Some(mutations) = REMOTE_PLUGIN_CACHE_MUTATIONS_IN_FLIGHT.get() else {
        return false;
    };
    let mutations = match mutations.lock() {
        Ok(mutations) => mutations,
        Err(err) => err.into_inner(),
    };
    mutations.contains_key(&RemotePluginCacheMutationKey {
        plugin_cache_root: remote_plugin_cache_root(codex_home),
        marketplace_name: marketplace_name.to_string(),
        plugin_name: plugin_name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;
    use wiremock::matchers::query_param;
    use wiremock::matchers::query_param_is_missing;

    #[tokio::test]
    async fn sync_same_version_backfills_metadata_and_missing_version_requires_materialization() {
        let server = MockServer::start().await;
        let codex_home = tempfile::tempdir().expect("create codex home");
        let cached_manifest = codex_home
            .path()
            .join(PLUGINS_CACHE_DIR)
            .join(REMOTE_GLOBAL_MARKETPLACE_NAME)
            .join("linear")
            .join("1.2.3")
            .join(".codex-plugin")
            .join("plugin.json");
        std::fs::create_dir_all(cached_manifest.parent().expect("manifest parent"))
            .expect("create cached plugin manifest parent");
        std::fs::write(&cached_manifest, r#"{"name":"linear","version":"1.2.3"}"#)
            .expect("write cached plugin manifest");
        let remote_plugin_id = "plugins~Plugin_linear";
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/installed"))
            .and(query_param_is_missing("scope"))
            .and(query_param("limit", "200"))
            .and(query_param("includeDownloadUrls", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "plugins": [
                    {
                        "id": remote_plugin_id,
                        "name": "linear",
                        "scope": "GLOBAL",
                        "installation_policy": "AVAILABLE",
                        "authentication_policy": "ON_USE",
                        "status": "ENABLED",
                        "release": {
                            "version": "1.2.3",
                            "display_name": "Linear",
                            "description": "Track work",
                            "interface": {},
                        },
                        "enabled": true,
                    },
                    {
                        "id": "plugins~Plugin_missing_version",
                        "name": "missing-version",
                        "scope": "GLOBAL",
                        "installation_policy": "AVAILABLE",
                        "authentication_policy": "ON_USE",
                        "status": "ENABLED",
                        "release": {
                            "display_name": "Missing version",
                            "description": "Needs materialization",
                            "interface": {},
                        },
                        "enabled": true,
                    },
                ],
                "pagination": {"next_page_token": null},
            })))
            .expect(1)
            .mount(&server)
            .await;
        let config = RemotePluginServiceConfig::new(
            format!("{}/backend-api", server.uri()),
            crate::test_support::test_http_client_factory(),
        );
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

        let outcome = sync_remote_installed_plugin_bundles_once(
            codex_home.path().to_path_buf(),
            &config,
            Some(&auth),
        )
        .await
        .expect("sync current remote plugin bundle");

        assert_eq!(
            outcome,
            RemoteInstalledPluginBundleSyncOutcome {
                failed_remote_plugin_ids: vec!["plugins~Plugin_missing_version".to_string()],
                failed_materialization_remote_plugin_ids: vec![
                    "plugins~Plugin_missing_version".to_string()
                ],
                ..RemoteInstalledPluginBundleSyncOutcome::default()
            }
        );
        let plugin_id = PluginId::new(
            "linear".to_string(),
            REMOTE_GLOBAL_MARKETPLACE_NAME.to_string(),
        )
        .expect("valid plugin id");
        let metadata_path = PluginStore::new(codex_home.path().to_path_buf())
            .plugin_base_root(&plugin_id)
            .join(".codex-remote-plugin-install.json");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &std::fs::read_to_string(metadata_path.as_path())
                    .expect("read remote plugin install metadata")
            )
            .expect("parse remote plugin install metadata"),
            json!({
                "schema_version": 1,
                "remote_plugin_id": remote_plugin_id,
            })
        );
        assert!(
            !codex_home
                .path()
                .join(PLUGINS_CACHE_DIR)
                .join(REMOTE_GLOBAL_MARKETPLACE_NAME)
                .join("missing-version")
                .exists()
        );
    }

    #[tokio::test]
    async fn sync_all_scopes_paginates_and_reconciles_each_marketplace() {
        let server = MockServer::start().await;
        let codex_home = tempfile::tempdir().expect("create codex home");
        let cached_plugins = [
            (
                REMOTE_GLOBAL_MARKETPLACE_NAME,
                "global-plugin",
                "GLOBAL",
                None,
            ),
            (
                REMOTE_CREATED_BY_ME_MARKETPLACE_NAME,
                "user-plugin",
                "USER",
                None,
            ),
            (
                REMOTE_WORKSPACE_MARKETPLACE_NAME,
                "workspace-plugin",
                "WORKSPACE",
                Some("LISTED"),
            ),
            (
                REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME,
                "shared-plugin",
                "WORKSPACE",
                Some("PRIVATE"),
            ),
        ];
        for (marketplace_name, plugin_name, _, _) in cached_plugins {
            for cached_plugin_name in [plugin_name, "stale"] {
                let manifest = codex_home
                    .path()
                    .join(PLUGINS_CACHE_DIR)
                    .join(marketplace_name)
                    .join(cached_plugin_name)
                    .join("1.2.3")
                    .join(".codex-plugin")
                    .join("plugin.json");
                std::fs::create_dir_all(manifest.parent().expect("manifest parent"))
                    .expect("create cached plugin manifest parent");
                std::fs::write(
                    &manifest,
                    format!(r#"{{"name":"{cached_plugin_name}","version":"1.2.3"}}"#),
                )
                .expect("write cached plugin manifest");
            }
        }
        let installed_plugins = cached_plugins
            .iter()
            .map(|(_, plugin_name, scope, discoverability)| {
                let mut plugin = json!({
                    "id": format!("plugins~Plugin_{plugin_name}"),
                    "name": plugin_name,
                    "scope": scope,
                    "installation_policy": "AVAILABLE",
                    "authentication_policy": "ON_USE",
                    "status": "ENABLED",
                    "release": {
                        "version": "1.2.3",
                        "display_name": plugin_name,
                        "description": "Installed plugin",
                        "interface": {},
                    },
                    "enabled": true,
                });
                if let Some(discoverability) = discoverability {
                    plugin["discoverability"] = json!(discoverability);
                }
                plugin
            })
            .collect::<Vec<_>>();
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/installed"))
            .and(query_param_is_missing("scope"))
            .and(query_param("limit", "200"))
            .and(query_param("includeDownloadUrls", "true"))
            .and(query_param_is_missing("pageToken"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "plugins": &installed_plugins[..2],
                "pagination": {"next_page_token": "page-2"},
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/installed"))
            .and(query_param_is_missing("scope"))
            .and(query_param("limit", "200"))
            .and(query_param("includeDownloadUrls", "true"))
            .and(query_param("pageToken", "page-2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "plugins": &installed_plugins[2..],
                "pagination": {"next_page_token": null},
            })))
            .expect(1)
            .mount(&server)
            .await;
        let (config, selected_urls) = crate::test_support::recording_remote_plugin_service_config(
            format!("{}/backend-api", server.uri()),
        );
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

        let outcome = sync_remote_installed_plugin_bundles_once(
            codex_home.path().to_path_buf(),
            &config,
            Some(&auth),
        )
        .await
        .expect("sync installed plugins across every marketplace");
        let mut removed_cache_plugin_ids = cached_plugins
            .iter()
            .map(|(marketplace_name, _, _, _)| format!("stale@{marketplace_name}"))
            .collect::<Vec<_>>();
        removed_cache_plugin_ids.sort();

        assert_eq!(
            outcome,
            RemoteInstalledPluginBundleSyncOutcome {
                materialized_remote_plugins: Vec::new(),
                removed_cache_plugin_ids,
                failed_remote_plugin_ids: Vec::new(),
                failed_materialization_remote_plugin_ids: Vec::new(),
            }
        );
        assert_eq!(
            crate::test_support::recorded_http_client_urls(&selected_urls),
            vec![
                format!(
                    "{}/backend-api/ps/plugins/installed?limit=200&includeDownloadUrls=true",
                    server.uri()
                ),
                format!(
                    "{}/backend-api/ps/plugins/installed?limit=200&includeDownloadUrls=true&pageToken=page-2",
                    server.uri()
                ),
            ]
        );
        for (marketplace_name, plugin_name, _, _) in cached_plugins {
            let plugin_root = codex_home
                .path()
                .join(PLUGINS_CACHE_DIR)
                .join(marketplace_name)
                .join(plugin_name);
            assert!(
                plugin_root
                    .join("1.2.3/.codex-plugin/plugin.json")
                    .is_file()
            );
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(
                    &std::fs::read_to_string(plugin_root.join(".codex-remote-plugin-install.json"))
                        .expect("read remote plugin install metadata")
                )
                .expect("parse remote plugin install metadata"),
                json!({
                    "schema_version": 1,
                    "remote_plugin_id": format!("plugins~Plugin_{plugin_name}"),
                })
            );
            assert!(
                !codex_home
                    .path()
                    .join(PLUGINS_CACHE_DIR)
                    .join(marketplace_name)
                    .join("stale")
                    .exists()
            );
        }
    }

    #[test]
    fn stale_remote_plugin_cleanup_skips_cache_mutations_in_progress() {
        let codex_home = tempfile::tempdir().expect("create codex home");
        let cached_manifest = codex_home
            .path()
            .join(PLUGINS_CACHE_DIR)
            .join(REMOTE_GLOBAL_MARKETPLACE_NAME)
            .join("linear")
            .join("1.2.3")
            .join(".codex-plugin")
            .join("plugin.json");
        std::fs::create_dir_all(cached_manifest.parent().expect("manifest parent"))
            .expect("create cached plugin manifest parent");
        std::fs::write(&cached_manifest, r#"{"name":"linear"}"#)
            .expect("write cached plugin manifest");
        let installed_plugin_names_by_marketplace =
            BTreeMap::<String, BTreeSet<String>>::from_iter([
                (REMOTE_GLOBAL_MARKETPLACE_NAME.to_string(), BTreeSet::new()),
                (
                    REMOTE_WORKSPACE_MARKETPLACE_NAME.to_string(),
                    BTreeSet::new(),
                ),
                (
                    REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME.to_string(),
                    BTreeSet::new(),
                ),
                (
                    REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME.to_string(),
                    BTreeSet::new(),
                ),
            ]);

        let guard = mark_remote_plugin_cache_mutation_in_flight(
            codex_home.path(),
            REMOTE_GLOBAL_MARKETPLACE_NAME,
            "linear",
        );
        let second_guard = mark_remote_plugin_cache_mutation_in_flight(
            codex_home.path(),
            REMOTE_GLOBAL_MARKETPLACE_NAME,
            "linear",
        );
        let mut removed = Vec::new();
        remove_stale_remote_plugin_caches(
            codex_home.path(),
            &installed_plugin_names_by_marketplace,
            &mut removed,
        )
        .expect("cleanup while install is guarded");
        assert_eq!(removed, Vec::<String>::new());
        assert!(cached_manifest.is_file());

        drop(guard);
        let mut removed = Vec::new();
        remove_stale_remote_plugin_caches(
            codex_home.path(),
            &installed_plugin_names_by_marketplace,
            &mut removed,
        )
        .expect("cleanup while second install guard is still active");
        assert_eq!(removed, Vec::<String>::new());
        assert!(cached_manifest.is_file());

        drop(second_guard);
        let mut removed = Vec::new();
        remove_stale_remote_plugin_caches(
            codex_home.path(),
            &installed_plugin_names_by_marketplace,
            &mut removed,
        )
        .expect("cleanup after install guard is dropped");
        assert_eq!(removed, vec!["linear@openai-curated-remote".to_string()]);
        assert!(!cached_manifest.exists());
    }

    #[test]
    fn stale_remote_plugin_cleanup_removes_stale_marketplace_caches_and_keeps_canonical_cache() {
        let codex_home = tempfile::tempdir().expect("create codex home");
        let created_by_me_cached_manifest = codex_home
            .path()
            .join(PLUGINS_CACHE_DIR)
            .join(REMOTE_CREATED_BY_ME_MARKETPLACE_NAME)
            .join("created-by-me-plugin")
            .join("1.2.3")
            .join(".codex-plugin")
            .join("plugin.json");
        std::fs::create_dir_all(
            created_by_me_cached_manifest
                .parent()
                .expect("manifest parent"),
        )
        .expect("create cached plugin manifest parent");
        std::fs::write(
            &created_by_me_cached_manifest,
            r#"{"name":"created-by-me-plugin"}"#,
        )
        .expect("write cached plugin manifest");
        let cached_manifest = codex_home
            .path()
            .join(PLUGINS_CACHE_DIR)
            .join(REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME)
            .join("private-plugin")
            .join("1.2.3")
            .join(".codex-plugin")
            .join("plugin.json");
        std::fs::create_dir_all(cached_manifest.parent().expect("manifest parent"))
            .expect("create cached plugin manifest parent");
        std::fs::write(&cached_manifest, r#"{"name":"private-plugin"}"#)
            .expect("write cached plugin manifest");
        let canonical_cached_manifest = codex_home
            .path()
            .join(PLUGINS_CACHE_DIR)
            .join(REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME)
            .join("shared-plugin")
            .join("1.2.3")
            .join(".codex-plugin")
            .join("plugin.json");
        std::fs::create_dir_all(canonical_cached_manifest.parent().expect("manifest parent"))
            .expect("create canonical cached plugin manifest parent");
        std::fs::write(&canonical_cached_manifest, r#"{"name":"shared-plugin"}"#)
            .expect("write canonical cached plugin manifest");
        let installed_plugin_names_by_marketplace =
            BTreeMap::<String, BTreeSet<String>>::from_iter([
                (REMOTE_GLOBAL_MARKETPLACE_NAME.to_string(), BTreeSet::new()),
                (
                    REMOTE_CREATED_BY_ME_MARKETPLACE_NAME.to_string(),
                    BTreeSet::new(),
                ),
                (
                    REMOTE_WORKSPACE_MARKETPLACE_NAME.to_string(),
                    BTreeSet::new(),
                ),
                (
                    REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME.to_string(),
                    BTreeSet::from(["shared-plugin".to_string()]),
                ),
                (
                    REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME.to_string(),
                    BTreeSet::new(),
                ),
                (
                    REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME.to_string(),
                    BTreeSet::new(),
                ),
            ]);

        let mut removed = Vec::new();
        remove_stale_remote_plugin_caches(
            codex_home.path(),
            &installed_plugin_names_by_marketplace,
            &mut removed,
        )
        .expect("cleanup private shared-with-me cache");

        assert_eq!(
            removed,
            vec![
                "created-by-me-plugin@created-by-me-remote".to_string(),
                "private-plugin@workspace-shared-with-me-private".to_string(),
            ]
        );
        assert!(!created_by_me_cached_manifest.exists());
        assert!(!cached_manifest.exists());
        assert!(canonical_cached_manifest.is_file());
    }
}
