//! Checks that overlapping policy reloads cannot publish out of order.

use super::ConfigManager;
use anyhow::Result;
use codex_config::CloudConfigBundleLoader;
use codex_config::LoaderOverrides;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_http_client::NetworkPolicyDenied;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::Semaphore;
use tokio::time::timeout;

#[tokio::test]
async fn embedded_transports_stay_blocked_until_the_same_policy_is_published() -> Result<()> {
    use crate::in_process::EmbeddedNetworkPolicy;
    use codex_core::config::ConfigBuilder;
    use codex_http_client::ClientRouteClass;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(2)
        .mount(&server)
        .await;
    let home = tempdir()?;
    let requirements = home.path().join("requirements.toml");
    std::fs::write(&requirements, "[application.network]")?;
    let overrides = LoaderOverrides {
        system_requirements_path: Some(requirements.clone()),
        ..LoaderOverrides::without_managed_config_for_tests()
    };
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(overrides.clone())
        .build()
        .await?;
    config.chatgpt_base_url = server.uri();
    let cloud_endpoint = format!("{}/api/codex/config/bundle", server.uri());
    let policy = EmbeddedNetworkPolicy::load(&overrides).await;
    let bootstrap_auth = policy.bind_bootstrap_auth(config.auth_config());
    let bootstrap = bootstrap_auth
        .auth_route_config
        .http_client_factory()
        .build_client(&cloud_endpoint, ClientRouteClass::Auth)?;
    let application = policy
        .bind(config.http_client_factory())
        .build_client(&server.uri(), ClientRouteClass::Other)?;
    assert!(bootstrap.get(&cloud_endpoint).send().await.is_err());
    assert!(application.get(server.uri()).send().await.is_err());
    policy.activate(&mut config);
    let caller_factory = config.http_client_factory();
    let caller_client = caller_factory.build_client(&server.uri(), ClientRouteClass::Other)?;
    let worktree_bootstrap = policy.clone().bind_bootstrap_auth(config.auth_config());
    let worktree_client = worktree_bootstrap
        .auth_route_config
        .http_client_factory()
        .build_client(&cloud_endpoint, ClientRouteClass::Auth)?;
    assert!(caller_client.get(server.uri()).send().await.is_err());
    assert!(worktree_client.get(&cloud_endpoint).send().await.is_err());
    assert!(
        caller_factory
            .network_policy()
            .acquire_for_unsupported_sdk()
            .is_err()
    );

    let manager = ConfigManager::new_for_tests(
        home.path().to_path_buf(),
        Vec::new(),
        overrides,
        CloudConfigBundleLoader::default(),
    )
    .with_embedded_network_policy(policy);
    drop(manager.refresh_application_network_policy().await?);
    assert!(application.get(server.uri()).send().await.is_err());
    assert!(server.received_requests().await.unwrap().is_empty());

    std::fs::remove_file(&requirements)?;
    let unrestricted = manager.refresh_application_network_policy().await?;
    let unrelated_endpoint = format!("{}/not-the-config-endpoint", server.uri()).parse()?;
    assert_eq!(
        bootstrap_auth
            .auth_route_config
            .http_client_factory()
            .network_policy()
            .acquire(&unrelated_endpoint)
            .map(|_| ()),
        Err(NetworkPolicyDenied::Destination),
    );
    caller_client.get(server.uri()).send().await?;
    worktree_client.get(&cloud_endpoint).send().await?;
    assert!(
        caller_factory
            .network_policy()
            .acquire_for_unsupported_sdk()
            .is_ok()
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
    std::fs::write(&requirements, "[malformed")?;
    assert!(manager.refresh_application_network_policy().await.is_err());
    assert!(caller_client.get(server.uri()).send().await.is_err());
    assert!(worktree_client.get(&cloud_endpoint).send().await.is_err());
    assert!(
        caller_factory
            .network_policy()
            .acquire_for_unsupported_sdk()
            .is_err()
    );
    assert!(
        manager
            .check_application_policy_load(&unrestricted)
            .is_err()
    );
    std::fs::remove_file(requirements)?;
    let recovered = manager.refresh_application_network_policy().await?;
    manager.check_application_policy_load(&recovered)?;
    assert!(
        manager
            .check_application_policy_load(&unrestricted)
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn embedded_caller_activation_uses_cloud_requirements() -> Result<()> {
    use crate::in_process::EmbeddedNetworkPolicy;
    use codex_core::config::ConfigBuilder;

    let home = tempdir()?;
    let overrides = LoaderOverrides::without_managed_config_for_tests();
    let policy = EmbeddedNetworkPolicy::load(&overrides).await;
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(overrides)
        .cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement("[application.network]"),
        )
        .build()
        .await?;
    policy.activate(&mut config);
    let factory = config.http_client_factory();
    assert!(
        factory
            .network_policy()
            .acquire_for_unsupported_sdk()
            .is_err()
    );
    let url = "https://denied.example".parse()?;
    assert!(factory.network_policy().acquire(&url).is_err());
    Ok(())
}

#[tokio::test]
async fn overlapping_policy_reloads_publish_in_order() -> Result<()> {
    let home = tempdir()?;
    let entered = Arc::new(Semaphore::new(/*permits*/ 0));
    let release = Arc::new(Semaphore::new(/*permits*/ 0));
    let calls = Arc::new(AtomicUsize::new(/*v*/ 0));
    let loader = CloudConfigBundleLoader::from_getter({
        let (entered, release, calls) = (entered.clone(), release.clone(), calls.clone());
        move || {
            let (entered, release, calls) = (entered.clone(), release.clone(), calls.clone());
            async move {
                let call = calls.fetch_add(/*val*/ 1, Ordering::SeqCst);
                let host = if call == 0 {
                    entered.add_permits(/*n*/ 1);
                    release
                        .acquire()
                        .await
                        .expect("release first reload")
                        .forget();
                    "old.example"
                } else if call == 3 {
                    "old.example"
                } else {
                    "new.example"
                };
                Ok(Some(
                    CloudConfigBundleFixture::enterprise_requirement(format!(
                        "[application.network.domains]\n'{host}' = 'allow'"
                    ))
                    .into_bundle(),
                ))
            }
        }
    });
    let manager = ConfigManager::new_for_tests(
        home.path().to_path_buf(),
        Vec::new(),
        LoaderOverrides::without_managed_config_for_tests(),
        loader,
    );
    let first = tokio::spawn({
        let manager = manager.clone();
        async move { manager.refresh_application_network_policy().await }
    });
    entered.acquire().await?.forget();
    let mut second = tokio::spawn({
        let manager = manager.clone();
        async move { manager.refresh_application_network_policy().await }
    });
    assert!(
        timeout(Duration::from_millis(/*millis*/ 100), &mut second)
            .await
            .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    release.add_permits(/*n*/ 1);
    let first_load = first.await??;
    let second_load = timeout(Duration::from_secs(/*secs*/ 5), second).await???;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    manager.check_application_policy_load(&second_load)?;
    let stale = manager
        .check_application_policy_load(&first_load)
        .unwrap_err();
    assert_eq!(
        stale.get_ref().and_then(|error| error.downcast_ref()),
        Some(&NetworkPolicyDenied::Revoked)
    );

    let policy = manager.network_policy.policy();
    assert_eq!(
        policy.acquire(&"https://old.example".parse()?).map(|_| ()),
        Err(NetworkPolicyDenied::Destination)
    );
    assert_eq!(
        policy.acquire(&"https://new.example".parse()?).map(|_| ()),
        Ok(())
    );
    let unchanged = manager.refresh_application_network_policy().await?;
    manager.check_application_policy_load(&second_load)?;
    manager.check_application_policy_load(&unchanged)?;
    let restored = manager.refresh_application_network_policy().await?;
    manager.check_application_policy_load(&restored)?;
    assert!(manager.check_application_policy_load(&first_load).is_err());
    assert!(manager.check_application_policy_load(&second_load).is_err());
    Ok(())
}

#[tokio::test]
async fn cloud_config_change_supersedes_load_even_when_network_rules_are_unchanged() -> Result<()> {
    let home = tempdir()?;
    let calls = AtomicUsize::new(/*v*/ 0);
    let loader = CloudConfigBundleLoader::from_getter(move || {
        let model = if calls.fetch_add(/*val*/ 1, Ordering::SeqCst) == 0 {
            "old"
        } else {
            "new"
        };
        async move {
            Ok(Some(
                CloudConfigBundleFixture::enterprise_config(format!("model = '{model}'"))
                    .into_bundle(),
            ))
        }
    });
    let manager = ConfigManager::new_for_tests(
        home.path().to_path_buf(),
        Vec::new(),
        LoaderOverrides::without_managed_config_for_tests(),
        loader,
    );
    let first = manager.refresh_application_network_policy().await?;
    let second = manager.refresh_application_network_policy().await?;
    manager.check_application_policy_load(&second)?;
    let stale = manager.check_application_policy_load(&first).unwrap_err();
    assert_eq!(
        stale.get_ref().and_then(|error| error.downcast_ref()),
        Some(&NetworkPolicyDenied::Revoked)
    );
    Ok(())
}
