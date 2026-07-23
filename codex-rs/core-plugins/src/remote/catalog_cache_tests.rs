use super::*;
use chrono::TimeDelta;
use codex_login::AuthHeaders;
use pretty_assertions::assert_eq;

#[test]
fn catalog_cache_freshness_honors_ttl() {
    let now = Utc::now();

    assert!(!is_fresh(/*fetched_at*/ None, now));
    assert!(is_fresh(Some(now), now));
    assert!(is_fresh(Some(now - TimeDelta::hours(3)), now,));
    assert!(!is_fresh(
        Some(now - TimeDelta::hours(3) - TimeDelta::milliseconds(1)),
        now,
    ));
    assert!(!is_fresh(Some(now + TimeDelta::milliseconds(1)), now));
}

#[test]
fn catalog_cache_paths_are_isolated_by_scope() {
    let codex_home = Path::new("/tmp/codex-home");
    let cache_key_for_scope = |scope| RemotePluginCatalogCacheKey {
        chatgpt_base_url: "https://chatgpt.com/backend-api".to_string(),
        account_id: Some("account-id".to_string()),
        chatgpt_user_id: Some("user-id".to_string()),
        is_workspace_account: true,
        scope: (scope != RemotePluginScope::Global).then_some(scope),
    };

    let paths = [
        RemotePluginScope::Global,
        RemotePluginScope::User,
        RemotePluginScope::Workspace,
    ]
    .map(|scope| cache_path(codex_home, &cache_key_for_scope(scope)));

    assert_ne!(paths[0], paths[1]);
    assert_ne!(paths[0], paths[2]);
    assert_ne!(paths[1], paths[2]);
}

#[test]
fn global_catalog_cache_reuses_legacy_cache_file() {
    let codex_home = tempfile::tempdir().expect("create codex home");
    let config = RemotePluginServiceConfig::new(
        "https://chatgpt.com/backend-api".to_string(),
        crate::test_support::test_http_client_factory(),
    );
    let auth = CodexAuth::Headers(AuthHeaders::new(http::HeaderMap::new()));
    let legacy_cache_path = codex_home
        .path()
        .join(REMOTE_PLUGIN_CATALOG_DISK_CACHE_DIR)
        .join("f22564d6f8ca89f6.json");
    let legacy_cache = serde_json::json!({
        "schema_version": REMOTE_PLUGIN_CATALOG_DISK_CACHE_SCHEMA_VERSION,
        "plugins": [],
    });
    let contents = serde_json::to_string_pretty(&legacy_cache).expect("serialize legacy cache");
    codex_utils_path::write_atomically(&legacy_cache_path, &contents).expect("write legacy cache");

    let cached =
        load_cached_directory_plugins(codex_home.path(), &config, &auth, RemotePluginScope::Global)
            .expect("load legacy global cache");
    assert!(cached.plugins.is_empty());
    assert_eq!(cached.freshness, RemotePluginCatalogCacheFreshness::Stale);

    write_cached_directory_plugins(
        codex_home.path(),
        &config,
        &auth,
        RemotePluginScope::Global,
        &[],
    );
    let refreshed_cache: RemotePluginCatalogDiskCache = serde_json::from_slice(
        &std::fs::read(&legacy_cache_path).expect("read refreshed legacy cache"),
    )
    .expect("parse refreshed legacy cache");
    assert!(refreshed_cache.fetched_at.is_some());
}

#[test]
fn header_auth_does_not_cache_private_catalogs_without_a_stable_identity() {
    let codex_home = tempfile::tempdir().expect("create codex home");
    let config = RemotePluginServiceConfig::new(
        "https://chatgpt.com/backend-api".to_string(),
        crate::test_support::test_http_client_factory(),
    );
    let auth = CodexAuth::Headers(AuthHeaders::new(http::HeaderMap::new()));

    write_cached_directory_plugins(
        codex_home.path(),
        &config,
        &auth,
        RemotePluginScope::Global,
        &[],
    );
    assert!(
        load_cached_directory_plugins(codex_home.path(), &config, &auth, RemotePluginScope::Global)
            .is_some()
    );

    for scope in [RemotePluginScope::User, RemotePluginScope::Workspace] {
        let insecure_cache_key = RemotePluginCatalogCacheKey {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
            account_id: None,
            chatgpt_user_id: None,
            is_workspace_account: false,
            scope: Some(scope),
        };
        let insecure_cache_path = cache_path(codex_home.path(), &insecure_cache_key);

        write_cached_directory_plugins(codex_home.path(), &config, &auth, scope, &[]);
        assert!(!insecure_cache_path.exists());

        let insecure_cache = RemotePluginCatalogDiskCache {
            schema_version: REMOTE_PLUGIN_CATALOG_DISK_CACHE_SCHEMA_VERSION,
            fetched_at: Some(Utc::now()),
            plugins: Vec::new(),
        };
        let contents = serde_json::to_string_pretty(&insecure_cache).expect("serialize cache");
        codex_utils_path::write_atomically(&insecure_cache_path, &contents)
            .expect("write insecure cache");

        assert!(load_cached_directory_plugins(codex_home.path(), &config, &auth, scope).is_none());
    }
}
