#![cfg(test)]
//! Catalog origins survive merges, cache reloads, and changes of provider identity.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn catalog_origins_survive_reload_and_auth_change() {
    for (slug, reviewer_origin) in [
        ("codex-auto-review", ModelOrigin::Provider),
        ("gateway-model", ModelOrigin::Bundled),
    ] {
        let home = tempdir().unwrap();
        let model = remote_model_with_visibility(slug, "Gateway", /*priority*/ 0, "hide");
        let auth =
            AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
        let endpoint =
            TestAuthAwareModelsEndpoint::new(Some(auth.clone()), vec![vec![model.clone()]]);
        let manager =
            OpenAiModelsManager::new(home.path().into(), endpoint.clone(), Some(auth.clone()));
        let mut updates = manager.model_catalog_updates().unwrap();
        manager.set_api_key_model_discovery_enabled(/*enabled*/ true);
        let bundled = manager
            .model_catalog_with_origins(RefreshStrategy::Offline, DEFAULT_HTTP_CLIENT_FACTORY)
            .await;
        assert!(manager.refreshed_model_catalog().is_none());
        assert!(!updates.has_changed().unwrap());
        let catalog = manager
            .model_catalog_with_origins(RefreshStrategy::Online, DEFAULT_HTTP_CLIENT_FACTORY)
            .await;
        assert_eq!(
            manager.refreshed_model_catalog().as_deref(),
            Some([model.clone()].as_slice())
        );
        assert!(updates.has_changed().unwrap());
        updates.borrow_and_update();
        assert!(catalog.contains(&CatalogModel {
            model: model.into(),
            origin: ModelOrigin::Provider
        }));
        assert_eq!(
            catalog
                .iter()
                .find(|entry| entry.model.slug == "codex-auto-review")
                .map(|entry| entry.origin),
            Some(reviewer_origin)
        );

        let restarted = openai_manager_for_tests(home.path().to_path_buf(), endpoint.clone());
        assert_eq!(
            restarted
                .model_catalog_with_origins(RefreshStrategy::Offline, DEFAULT_HTTP_CLIENT_FACTORY)
                .await,
            catalog
        );
        assert_eq!(endpoint.fetch_count(), 1);
        assert!(
            restarted.refreshed_model_catalog().is_none(),
            "disk cache is not a fresh provider response"
        );
        assert_eq!(
            restarted
                .get_model_info(
                    &format!("gateway/{slug}-variant"),
                    &ModelsManagerConfig::default()
                )
                .await
                .origin,
            ModelOrigin::Provider
        );

        // An empty subsequent response restores bundled origins instead of retaining old IDs.
        assert_eq!(
            restarted
                .model_catalog_with_origins(RefreshStrategy::Online, DEFAULT_HTTP_CLIENT_FACTORY)
                .await,
            bundled
        );

        // The original manager still holds provider IDs, but its identity is no longer current.
        auth.set_external_auth(Arc::new(TestExternalApiKeyAuth))
            .await
            .unwrap();
        assert!(
            manager.refreshed_model_catalog().is_none(),
            "a previous identity cannot normalize settings"
        );
        assert_eq!(
            manager
                .model_catalog_with_origins(RefreshStrategy::Offline, DEFAULT_HTTP_CLIENT_FACTORY)
                .await,
            bundled
        );
        assert_eq!(endpoint.fetch_count(), 2);
        assert_eq!(
            manager
                .get_model_info("codex-auto-review", &ModelsManagerConfig::default())
                .await
                .origin,
            ModelOrigin::Bundled
        );
    }
}

#[tokio::test]
async fn static_catalog_records_its_supplied_origin() {
    let model = remote_model("reviewer", "Reviewer", /*priority*/ 0);
    for origin in [
        ModelOrigin::Bundled,
        ModelOrigin::Provider,
        ModelOrigin::ExplicitConfig,
    ] {
        let manager = StaticModelsManager::new(
            /*auth_manager*/ None,
            ModelsResponse {
                models: vec![model.clone()],
            },
            origin,
        );
        assert_eq!(
            manager
                .get_model_info(
                    "namespace/reviewer-variant",
                    &ModelsManagerConfig::default()
                )
                .await,
            CatalogModel {
                model: ModelInfo {
                    slug: "namespace/reviewer-variant".to_string(),
                    ..model.clone()
                }
                .into(),
                origin
            }
        );
        assert_eq!(
            manager
                .model_catalog_with_origins(RefreshStrategy::Offline, DEFAULT_HTTP_CLIENT_FACTORY)
                .await,
            vec![CatalogModel {
                model: model.clone().into(),
                origin
            }]
        );
    }
}

#[tokio::test]
async fn refreshing_retained_originators_keeps_catalog_notifications_isolated() {
    let root_model = remote_model("root-model", "Root", /*priority*/ 0);
    let work_model = remote_model("work-model", "Work", /*priority*/ 0);
    let root_endpoint = TestModelsEndpoint::new(vec![vec![root_model.clone()]]);
    let work_endpoint = TestModelsEndpoint::new(vec![vec![work_model.clone()]]);
    let root =
        OpenAiModelsManager::new_without_cache(root_endpoint.clone(), /*auth_manager*/ None);
    let work = Arc::new(OpenAiModelsManager::new_without_cache(
        work_endpoint.clone(),
        /*auth_manager*/ None,
    ));
    root.list_models(RefreshStrategy::Online, DEFAULT_HTTP_CLIENT_FACTORY)
        .await;
    let root_updates = root.model_catalog_updates().unwrap();
    let work_updates = work.model_catalog_updates().unwrap();
    root.originator_managers
        .lock()
        .unwrap()
        .insert(ModelCatalogOriginator::WorkDesktop, work.clone());
    root.refresh_originator_catalogs(DEFAULT_HTTP_CLIENT_FACTORY)
        .await;
    assert_eq!(
        root.refreshed_model_catalog().as_deref(),
        Some([root_model].as_slice())
    );
    assert_eq!(
        work.refreshed_model_catalog().as_deref(),
        Some([work_model].as_slice())
    );
    assert!(!root_updates.has_changed().unwrap());
    assert!(work_updates.has_changed().unwrap());
    assert_eq!(root_endpoint.fetch_count(), 1);
    assert_eq!(work_endpoint.fetch_count(), 1);
    assert_eq!(root.originator_managers.lock().unwrap().len(), 1);
}
