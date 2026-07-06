use super::*;
use tempfile::tempdir;

#[test]
fn load_rejects_replaced_disk_cache_contents() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let config = RemotePluginServiceConfig {
        chatgpt_base_url: "https://chatgpt.com".to_string(),
    };
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    write_cached_global_directory_plugins(codex_home.path(), &config, &auth, &[]);
    assert_eq!(
        load_cached_global_directory_plugins(codex_home.path(), &config, &auth),
        Some(Vec::new())
    );

    let path = cache_path(
        codex_home.path(),
        &RemotePluginCatalogCacheKey::global(&config, &auth),
    );
    std::fs::write(&path, b"invalid json").expect("cache should be replaceable");

    assert_eq!(
        load_cached_global_directory_plugins(codex_home.path(), &config, &auth),
        None
    );
    assert!(!path.exists());
}

#[test]
fn memory_cache_is_bounded() {
    let mut cache = MemoryCache::default();
    for index in 0..=REMOTE_PLUGIN_CATALOG_MEMORY_CACHE_CAPACITY {
        cache.insert(
            PathBuf::from(format!("catalog-{index}.json")),
            Vec::new(),
            Vec::new(),
        );
    }

    assert_eq!(
        cache.entries.len(),
        REMOTE_PLUGIN_CATALOG_MEMORY_CACHE_CAPACITY
    );
    assert!(
        cache
            .entries
            .iter()
            .all(|entry| entry.path.as_path() != Path::new("catalog-0.json"))
    );
}
