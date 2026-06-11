use super::REMOTE_GLOBAL_MARKETPLACE_NAME;
use super::REMOTE_WORKSPACE_MARKETPLACE_NAME;
use super::REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME;
use super::REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME;
use super::REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME;
use super::RemotePluginCatalogError;
use super::RemotePluginScope;
use super::RemotePluginServiceConfig;
use super::ensure_chatgpt_auth;
use super::fetch_installed_plugins_for_scope_with_download_url;
use super::remote_plugin_canonical_marketplace_name;
use crate::store::PLUGINS_CACHE_DIR;
use crate::store::PluginStore;
use crate::store::PluginStoreError;
use codex_login::CodexAuth;
use codex_plugin::PluginId;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use tracing::info;
use tracing::warn;

static REMOTE_INSTALLED_PLUGIN_BUNDLE_SYNC_IN_FLIGHT: OnceLock<
    Mutex<HashSet<RemoteInstalledPluginBundleSyncKey>>,
> = OnceLock::new();
static REMOTE_PLUGIN_CACHE_MUTATIONS_IN_FLIGHT: OnceLock<
    Mutex<HashMap<RemotePluginCacheMutationKey, usize>>,
> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RemoteInstalledPluginBundleSyncOutcome {
    pub installed_plugin_ids: Vec<String>,
    pub removed_cache_plugin_ids: Vec<String>,
    pub failed_remote_plugin_ids: Vec<String>,
}

impl RemoteInstalledPluginBundleSyncOutcome {
    pub fn changed_local_cache(&self) -> bool {
        !self.installed_plugin_ids.is_empty() || !self.removed_cache_plugin_ids.is_empty()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RemoteInstalledPluginBundleSyncError {
    #[error("{0}")]
    Catalog(#[from] RemotePluginCatalogError),

    #[error("{0}")]
    Store(#[from] PluginStoreError),

    #[error("failed to join stale remote plugin cache cleanup task: {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("failed to remove stale remote plugin cache entries: {0}")]
    CacheRemove(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RemoteInstalledPluginBundleSyncKey {
    plugin_cache_root: PathBuf,
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

pub(crate) fn maybe_start_remote_installed_plugin_bundle_sync(
    codex_home: PathBuf,
    config: RemotePluginServiceConfig,
    auth: Option<CodexAuth>,
    on_local_cache_changed: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
) {
    let Some(auth) = auth else {
        return;
    };
    let key = RemoteInstalledPluginBundleSyncKey {
        plugin_cache_root: remote_plugin_cache_root(&codex_home),
    };
    if !mark_remote_installed_plugin_bundle_sync_in_flight(key.clone()) {
        return;
    }

    tokio::spawn(async move {
        let result =
            sync_remote_installed_plugin_bundles_once(codex_home, &config, Some(&auth)).await;
        match result {
            Ok(outcome) => {
                if outcome.changed_local_cache()
                    && let Some(on_local_cache_changed) = on_local_cache_changed
                {
                    on_local_cache_changed();
                }
                info!(
                    installed_plugin_ids = ?outcome.installed_plugin_ids,
                    removed_cache_plugin_ids = ?outcome.removed_cache_plugin_ids,
                    failed_remote_plugin_ids = ?outcome.failed_remote_plugin_ids,
                    "completed remote installed plugin bundle sync"
                );
            }
            Err(err) => {
                warn!(
                    error = %err,
                    "remote installed plugin bundle sync failed"
                );
            }
        }
        clear_remote_installed_plugin_bundle_sync_in_flight(&key);
    });
}

pub async fn sync_remote_installed_plugin_bundles_once(
    codex_home: PathBuf,
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
) -> Result<RemoteInstalledPluginBundleSyncOutcome, RemoteInstalledPluginBundleSyncError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let global = async {
        let scope = RemotePluginScope::Global;
        let installed_plugins = fetch_installed_plugins_for_scope_with_download_url(
            config, auth, scope, /*include_download_urls*/ true,
        )
        .await?;
        Ok::<_, RemotePluginCatalogError>((scope, installed_plugins))
    };
    let workspace = async {
        let scope = RemotePluginScope::Workspace;
        let installed_plugins = fetch_installed_plugins_for_scope_with_download_url(
            config, auth, scope, /*include_download_urls*/ true,
        )
        .await?;
        Ok::<_, RemotePluginCatalogError>((scope, installed_plugins))
    };

    let (global, workspace) = tokio::try_join!(global, workspace)?;
    let store = PluginStore::try_new(codex_home.clone())?;
    let mut installed_plugin_names_by_marketplace =
        BTreeMap::<String, BTreeSet<String>>::from_iter([
            (REMOTE_GLOBAL_MARKETPLACE_NAME.to_string(), BTreeSet::new()),
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
    let mut installed_plugin_ids = BTreeSet::new();
    let mut failed_remote_plugin_ids = BTreeSet::new();

    for (_scope, installed_plugins) in [global, workspace] {
        for installed_plugin in installed_plugins {
            let plugin = installed_plugin.plugin;
            let marketplace_name = remote_plugin_canonical_marketplace_name(&plugin)?.to_string();
            installed_plugin_names_by_marketplace
                .entry(marketplace_name.clone())
                .or_default()
                .insert(plugin.name.clone());
            let plugin_id = match PluginId::new(plugin.name.clone(), marketplace_name.clone()) {
                Ok(plugin_id) => plugin_id,
                Err(err) => {
                    warn!(
                        remote_plugin_id = %plugin.id,
                        plugin = %plugin.name,
                        marketplace = %marketplace_name,
                        error = %err,
                        "skipping remote installed plugin with invalid local cache id"
                    );
                    failed_remote_plugin_ids.insert(plugin.id);
                    continue;
                }
            };
            let release_version = plugin
                .release
                .version
                .as_deref()
                .map(str::trim)
                .filter(|version| !version.is_empty());
            if store.active_plugin_version(&plugin_id).as_deref() == release_version {
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
                    failed_remote_plugin_ids.insert(plugin.id);
                    continue;
                }
            };

            match crate::remote_bundle::download_and_install_remote_plugin_bundle(
                codex_home.clone(),
                bundle,
            )
            .await
            {
                Ok(result) => {
                    installed_plugin_ids.insert(result.plugin_id.as_key());
                }
                Err(err) => {
                    warn!(
                        remote_plugin_id = %plugin.id,
                        plugin = %plugin.name,
                        marketplace = %marketplace_name,
                        error = %err,
                        "failed to download remote installed plugin bundle"
                    );
                    failed_remote_plugin_ids.insert(plugin.id);
                }
            }
        }
    }

    let removed_cache_plugin_ids = tokio::task::spawn_blocking(move || {
        remove_stale_remote_plugin_caches(
            codex_home.as_path(),
            &installed_plugin_names_by_marketplace,
        )
    })
    .await?
    .map_err(RemoteInstalledPluginBundleSyncError::CacheRemove)?;

    Ok(RemoteInstalledPluginBundleSyncOutcome {
        installed_plugin_ids: installed_plugin_ids.into_iter().collect(),
        removed_cache_plugin_ids,
        failed_remote_plugin_ids: failed_remote_plugin_ids.into_iter().collect(),
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
) -> Result<Vec<String>, String> {
    let mut removed_cache_plugin_ids = Vec::new();
    for marketplace_name in [
        REMOTE_GLOBAL_MARKETPLACE_NAME,
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

    removed_cache_plugin_ids.sort();
    Ok(removed_cache_plugin_ids)
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

fn mark_remote_installed_plugin_bundle_sync_in_flight(
    key: RemoteInstalledPluginBundleSyncKey,
) -> bool {
    let syncs =
        REMOTE_INSTALLED_PLUGIN_BUNDLE_SYNC_IN_FLIGHT.get_or_init(|| Mutex::new(HashSet::new()));
    let mut syncs = match syncs.lock() {
        Ok(syncs) => syncs,
        Err(err) => err.into_inner(),
    };
    syncs.insert(key)
}

fn clear_remote_installed_plugin_bundle_sync_in_flight(key: &RemoteInstalledPluginBundleSyncKey) {
    let Some(syncs) = REMOTE_INSTALLED_PLUGIN_BUNDLE_SYNC_IN_FLIGHT.get() else {
        return;
    };
    let mut syncs = match syncs.lock() {
        Ok(syncs) => syncs,
        Err(err) => err.into_inner(),
    };
    syncs.remove(key);
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

    #[test]
    fn remote_installed_plugin_sync_in_flight_dedupes_by_cache_root() {
        let codex_home = tempfile::tempdir().expect("create codex home");
        let key = RemoteInstalledPluginBundleSyncKey {
            plugin_cache_root: remote_plugin_cache_root(codex_home.path()),
        };

        assert!(mark_remote_installed_plugin_bundle_sync_in_flight(
            key.clone()
        ));
        assert!(!mark_remote_installed_plugin_bundle_sync_in_flight(
            key.clone()
        ));

        clear_remote_installed_plugin_bundle_sync_in_flight(&key);
        assert!(mark_remote_installed_plugin_bundle_sync_in_flight(
            key.clone()
        ));
        clear_remote_installed_plugin_bundle_sync_in_flight(&key);
    }

    #[tokio::test]
    async fn sync_backfills_remote_plugin_install_metadata_for_current_bundle() {
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
            .and(query_param("scope", "GLOBAL"))
            .and(query_param("includeDownloadUrls", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "plugins": [{
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
                }],
                "pagination": {"next_page_token": null},
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/installed"))
            .and(query_param("scope", "WORKSPACE"))
            .and(query_param("includeDownloadUrls", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "plugins": [],
                "pagination": {"next_page_token": null},
            })))
            .expect(1)
            .mount(&server)
            .await;
        let config = RemotePluginServiceConfig {
            chatgpt_base_url: format!("{}/backend-api", server.uri()),
        };
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

        let outcome = sync_remote_installed_plugin_bundles_once(
            codex_home.path().to_path_buf(),
            &config,
            Some(&auth),
        )
        .await
        .expect("sync current remote plugin bundle");

        assert_eq!(outcome, RemoteInstalledPluginBundleSyncOutcome::default());
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
        let removed = remove_stale_remote_plugin_caches(
            codex_home.path(),
            &installed_plugin_names_by_marketplace,
        )
        .expect("cleanup while install is guarded");
        assert_eq!(removed, Vec::<String>::new());
        assert!(cached_manifest.is_file());

        drop(guard);
        let removed = remove_stale_remote_plugin_caches(
            codex_home.path(),
            &installed_plugin_names_by_marketplace,
        )
        .expect("cleanup while second install guard is still active");
        assert_eq!(removed, Vec::<String>::new());
        assert!(cached_manifest.is_file());

        drop(second_guard);
        let removed = remove_stale_remote_plugin_caches(
            codex_home.path(),
            &installed_plugin_names_by_marketplace,
        )
        .expect("cleanup after install guard is dropped");
        assert_eq!(removed, vec!["linear@openai-curated-remote".to_string()]);
        assert!(!cached_manifest.exists());
    }

    #[test]
    fn stale_remote_plugin_cleanup_removes_old_shared_with_me_cache_and_keeps_canonical_cache() {
        let codex_home = tempfile::tempdir().expect("create codex home");
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

        let removed = remove_stale_remote_plugin_caches(
            codex_home.path(),
            &installed_plugin_names_by_marketplace,
        )
        .expect("cleanup private shared-with-me cache");

        assert_eq!(
            removed,
            vec!["private-plugin@workspace-shared-with-me-private".to_string()]
        );
        assert!(!cached_manifest.exists());
        assert!(canonical_cached_manifest.is_file());
    }
}
