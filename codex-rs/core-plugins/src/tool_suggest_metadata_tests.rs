use super::*;
use codex_utils_absolute_path::AbsolutePathBuf;

#[test]
fn expired_metadata_is_not_reused() {
    let cache = ToolSuggestMetadataCache::new();
    let artifact = PluginArtifactIdentity {
        plugin_id: "sample@test".to_string(),
        source: MarketplacePluginSource::Local {
            path: AbsolutePathBuf::try_from("/tmp/sample").expect("absolute path"),
        },
    };
    let entry = Ok(Arc::new(ToolSuggestMetadataFragment {
        config_name: "sample@test".to_string(),
        display_name: "sample".to_string(),
        description: None,
        mcp_server_names: Vec::new(),
        app_declarations: Vec::new(),
        skill_inventory: None,
    }));
    assert!(cache.cache_entry_if_current(cache.generation(), artifact.clone(), entry));
    cache
        .state
        .write()
        .expect("cache lock should not be poisoned")
        .entries
        .get_mut(&artifact)
        .expect("entry should be cached")
        .expires_at = Instant::now();

    assert!(cache.cached_entry(&artifact).is_none());
}
