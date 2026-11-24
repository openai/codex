#![cfg(test)]

use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider::ProviderAuthContext;
use pretty_assertions::assert_eq;
use serde_json::json;
use statsig_client::StatsigOptions;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::StatsigBootstrapContext;
use super::StatsigEvaluations;
use super::StatsigIdentity;

#[tokio::test]
async fn bootstraps_once_and_refreshes_directly_through_the_sdk() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    let home = std::env::temp_dir().join(format!("statsig-auth-{}", server.address().port()));
    let auth_manager = AuthManager::from_auth_for_testing_with_home(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        home.clone(),
    );
    let account = ProviderAuthContext {
        credentials: auth_manager.clone(),
        runtime_auth: None,
    };
    let context = StatsigBootstrapContext {
        target_app: "test-target-app",
        stable_id: "test-install".into(),
        app_version: "1".into(),
        locale: None,
        system_name: "test-os".into(),
        system_version: "1".into(),
        window_type: "cli",
        brand_name: "codex",
    };
    const PROBE: super::Gate = super::Gate::new("probe");
    assert!(!PROBE.is_enabled());
    assert!(super::DynamicConfig::new("probe").value().is_empty());
    let user = json!({"userID": "test-user", "customIDs": {"stableID": "test-install"}});
    let values = |enabled: bool, limit: u32| {
        json!({
            "has_updates": true,
            "hash_used": "none",
            "user": user,
            "feature_gates": {"probe": {"value": enabled}},
            "dynamic_configs": {"probe": {"value": {"limit": limit}}},
            "layer_configs": {}
        })
    };
    let bootstrap = json!({"statsigPayload": values(true, 1).to_string()});
    let failed = Mock::given(method("POST"))
        .and(path("/api/codex/statsig/bootstrap"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let runtime = super::StatsigRuntime::start_process(
        StatsigIdentity::Account(account.clone()),
        server.uri(),
        "client-test".into(),
        context.clone(),
        /*exposure_logging_enabled*/ false,
    );
    assert!(!PROBE.is_enabled());
    failed.wait_until_satisfied().await;
    drop(failed);
    let failed_retry = Mock::given(method("POST"))
        .and(path("/api/codex/statsig/bootstrap"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let token = "e30.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsidXNlcl9pZCI6InRlc3QtdXNlciJ9fQ";
    codex_login::auth::login_with_chatgpt_auth_tokens(
        &home,
        &format!("{token}.a"),
        "account_id",
        /*chatgpt_plan_type*/ None,
    )?;
    assert!(auth_manager.reload().await);
    failed_retry.wait_until_satisfied().await;
    drop(failed_retry);
    Mock::given(method("POST"))
        .and(path("/api/codex/statsig/bootstrap"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bootstrap))
        .expect(2)
        .mount(&server)
        .await;
    let owner = auth_manager.auth_owner();
    codex_login::auth::login_with_chatgpt_auth_tokens(
        &home,
        &format!("{token}.b"),
        "account_id",
        /*chatgpt_plan_type*/ None,
    )?;
    auth_manager.reload().await;
    assert!(owner.is_current());
    let mut evaluations = runtime.evaluations.clone();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        evaluations.wait_for(Option::is_some),
    )
    .await??;
    assert!(PROBE.is_enabled());
    assert_eq!(
        super::DynamicConfig::new("probe").value(),
        json!({"limit": 1}).as_object().unwrap().clone()
    );
    drop(evaluations);
    super::StatsigRuntime::finish(Some(runtime)).await;
    for response in [
        ResponseTemplate::new(503),
        ResponseTemplate::new(200).set_body_json(values(false, 2)),
    ] {
        Mock::given(method("POST"))
            .and(path("/initialize"))
            .and(body_partial_json(json!({"user": user})))
            .respond_with(response)
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
    }
    let options = StatsigOptions::new()
        .with_initialize_url(format!("{}/initialize", server.uri()))
        .with_initialization_request_attempts(/* count */ 1)
        .with_disable_all_logging(/* disabled */ true);
    let store = StatsigEvaluations::start(
        &StatsigIdentity::Account(account),
        &server.uri(),
        "client-test",
        context,
        options,
    )
    .await?;
    let read = || {
        (
            store.sdk.check_gate("probe"),
            json!(store.sdk.get_dynamic_config("probe").value()),
        )
    };
    assert_eq!(read(), (true, json!({"limit": 1})));
    assert!(store.refresh().await.is_err());
    assert_eq!(read(), (true, json!({"limit": 1})));
    store.refresh().await?;
    assert_eq!(read(), (false, json!({"limit": 2})));
    server.verify().await;
    store.shutdown().await;
    Ok(())
}
