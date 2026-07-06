use super::*;

#[test]
fn remote_plugin_catalog_cache_is_scoped_by_plugin_service_preview() {
    let regular = RemotePluginCatalogCacheKey {
        chatgpt_base_url: "https://chatgpt.com/backend-api".to_string(),
        account_id: Some("account".to_string()),
        chatgpt_user_id: Some("user".to_string()),
        is_workspace_account: true,
        plugin_service_preview: false,
    };
    let preview = RemotePluginCatalogCacheKey {
        plugin_service_preview: true,
        ..regular.clone()
    };

    assert_ne!(
        cache_path(Path::new("/codex-home"), &regular),
        cache_path(Path::new("/codex-home"), &preview)
    );
}
