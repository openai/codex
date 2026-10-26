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
fn catalog_cache_paths_are_isolated_by_scope_and_collection() {
    let codex_home = Path::new("/tmp/codex-home");
    let cache_key_for_scope = |scope| RemotePluginCatalogCacheKey {
        chatgpt_base_url: "https://chatgpt.com/backend-api".to_string(),
        account_id: Some("account-id".to_string()),
        chatgpt_user_id: Some("user-id".to_string()),
        is_workspace_account: true,
        scope: (scope != RemotePluginScope::Global).then_some(scope),
        collection: None,
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

    let mut collection_key = cache_key_for_scope(RemotePluginScope::Global);
    collection_key.collection = Some("vertical".to_string());
    assert!(!paths.contains(&cache_path(codex_home, &collection_key)));
}

#[test]
fn global_catalog_cache_reuses_legacy_cache_file() {
    let codex_home = tempfile::tempdir().expect("create codex home");
    let config = RemotePluginServiceConfig::new(
        "https://chatgpt.com/backend-api".to_string(),
        crate::test_support::test_http_client_factory(),
        /*product_sku*/ None,
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
    codex_utils_path::write_atomically(&legacy_cache_path, &legacy_cache.to_string())
        .expect("write legacy cache");

    let cached = load_cached_directory_plugins::<RemotePluginDirectoryItem>(
        codex_home.path(),
        &config,
        &auth,
        RemotePluginScope::Global,
        /*collection*/ None,
        RemotePluginCatalogCacheMode::PreferCache,
    )
    .expect("load legacy global cache");
    assert!(cached.plugins.is_empty());
    assert_eq!(cached.freshness, RemotePluginCatalogCacheFreshness::Stale);

    write_cached_directory_plugins(
        codex_home.path(),
        &config,
        &auth,
        RemotePluginScope::Global,
        /*collection*/ None,
        &[],
    );
    assert!(has_cached_directory_plugins(
        codex_home.path(),
        &config,
        &auth,
        RemotePluginScope::Global,
        RemotePluginCatalogCacheMode::PreferFreshCache,
    ));
}

#[test]
fn freshness_probe_defers_plugin_validation_until_full_read() {
    let codex_home = tempfile::tempdir().expect("create codex home");
    let config = RemotePluginServiceConfig::new(
        "https://chatgpt.com/backend-api".to_string(),
        crate::test_support::test_http_client_factory(),
        /*product_sku*/ None,
    );
    let auth = CodexAuth::Headers(AuthHeaders::new(http::HeaderMap::new()));
    let scope = RemotePluginScope::Global;
    let key = RemotePluginCatalogCacheKey::new(&config, &auth, scope, /*collection*/ None)
        .expect("global cache key");
    let path = cache_path(codex_home.path(), &key);
    let probe = |mode| has_cached_directory_plugins(codex_home.path(), &config, &auth, scope, mode);
    let version = REMOTE_PLUGIN_CATALOG_DISK_CACHE_SCHEMA_VERSION;
    let now = serde_json::to_string(&Utc::now()).expect("serialize timestamp");
    let expired_at = "2000-01-01T00:00:00Z";
    let padding = " ".repeat(512);
    for contents in [
        serde_json::json!({"schema_version": version + 1, "plugins": []}).to_string(),
        serde_json::json!({"schema_version": version}).to_string(),
        serde_json::json!({"schema_version": version, "plugins": {}}).to_string(),
        format!(r#"{{"schema_version":{version},"plugins":[{{"#),
        format!(r#"{{"schema_version":{version},"fetched_at":{now},"plugins":[{padding}"#),
        format!(
            r#"{{"details":{{"fetched_at":"{expired_at}"}},"schema_version":{version},"plugins":[{padding}"#
        ),
        format!(
            r#"{{"schema_version":{version},"fetched_at":"{expired_at}","fetched_at":{now},"plugins":[{padding}"#
        ),
        format!(
            r#"{{"schema_version":{version},"fetched_at":"{expired_at}","plugins":{{}}}}{padding}"#
        ),
    ] {
        codex_utils_path::write_atomically(&path, &contents).expect("write invalid cache");
        assert!(
            !probe(RemotePluginCatalogCacheMode::PreferFreshCache),
            "{contents}"
        );
        assert!(!path.exists());
    }

    let expired = format!(
        r#"{{"note":"[","fetched\u005fat":"{expired_at}","schema_version":{version},"plugins":[{padding}"#
    );
    codex_utils_path::write_atomically(&path, &expired).expect("write expired cache");
    assert!(!probe(RemotePluginCatalogCacheMode::PreferFreshCache));
    assert!(path.exists());
    assert!(!probe(RemotePluginCatalogCacheMode::PreferCache));
    assert!(!path.exists());

    let reordered =
        format!(r#"{{"plugins":[{padding}],"schema_version":{version},"fetched_at":{now}}}"#);
    codex_utils_path::write_atomically(&path, &reordered).expect("write reordered cache");
    assert!(probe(RemotePluginCatalogCacheMode::PreferFreshCache));

    let contents = serde_json::json!({
        "schema_version": version,
        "fetched_at": Utc::now(),
        "plugins": [{}],
    });
    codex_utils_path::write_atomically(&path, &contents.to_string()).expect("write invalid plugin");
    assert!(probe(RemotePluginCatalogCacheMode::PreferFreshCache));
    assert!(
        load_cached_directory_plugins::<RemotePluginDirectoryItem>(
            codex_home.path(),
            &config,
            &auth,
            scope,
            /*collection*/ None,
            RemotePluginCatalogCacheMode::PreferCache,
        )
        .is_none()
    );
    assert!(!path.exists());
}

#[test]
fn header_auth_does_not_cache_private_catalogs_without_a_stable_identity() {
    let codex_home = tempfile::tempdir().expect("create codex home");
    let config = RemotePluginServiceConfig::new(
        "https://chatgpt.com/backend-api".to_string(),
        crate::test_support::test_http_client_factory(),
        /*product_sku*/ None,
    );
    let auth = CodexAuth::Headers(AuthHeaders::new(http::HeaderMap::new()));

    write_cached_directory_plugins(
        codex_home.path(),
        &config,
        &auth,
        RemotePluginScope::Global,
        /*collection*/ None,
        &[],
    );
    assert!(
        load_cached_directory_plugins::<RemotePluginDirectoryItem>(
            codex_home.path(),
            &config,
            &auth,
            RemotePluginScope::Global,
            /*collection*/ None,
            RemotePluginCatalogCacheMode::PreferCache,
        )
        .is_some()
    );

    for scope in [RemotePluginScope::User, RemotePluginScope::Workspace] {
        let insecure_cache_key = RemotePluginCatalogCacheKey {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
            account_id: None,
            chatgpt_user_id: None,
            is_workspace_account: false,
            scope: Some(scope),
            collection: None,
        };
        let insecure_cache_path = cache_path(codex_home.path(), &insecure_cache_key);

        write_cached_directory_plugins(
            codex_home.path(),
            &config,
            &auth,
            scope,
            /*collection*/ None,
            &[],
        );
        assert!(!insecure_cache_path.exists());

        let insecure_cache = RemotePluginCatalogDiskCache {
            schema_version: REMOTE_PLUGIN_CATALOG_DISK_CACHE_SCHEMA_VERSION,
            fetched_at: Some(Utc::now()),
            plugins: Vec::<RemotePluginDirectoryItem>::new(),
        };
        let contents = serde_json::to_string_pretty(&insecure_cache).expect("serialize cache");
        codex_utils_path::write_atomically(&insecure_cache_path, &contents)
            .expect("write insecure cache");

        assert!(
            load_cached_directory_plugins::<RemotePluginDirectoryItem>(
                codex_home.path(),
                &config,
                &auth,
                scope,
                /*collection*/ None,
                RemotePluginCatalogCacheMode::PreferCache,
            )
            .is_none()
        );
    }
}
