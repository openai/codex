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
        manager.set_api_key_model_discovery_enabled(/*enabled*/ true);
        let bundled = manager
            .model_catalog_with_origins(RefreshStrategy::Offline, DEFAULT_HTTP_CLIENT_FACTORY)
            .await;
        let catalog = manager
            .model_catalog_with_origins(RefreshStrategy::Online, DEFAULT_HTTP_CLIENT_FACTORY)
            .await;
        assert!(catalog.contains(&CatalogModel {
            model,
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
        assert_eq!(
            manager
                .model_catalog_with_origins(RefreshStrategy::Offline, DEFAULT_HTTP_CLIENT_FACTORY)
                .await,
            bundled
        );
        assert_eq!(endpoint.fetch_count(), 2);
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
                .model_catalog_with_origins(RefreshStrategy::Offline, DEFAULT_HTTP_CLIENT_FACTORY)
                .await,
            vec![CatalogModel {
                model: model.clone(),
                origin
            }]
        );
    }
}
