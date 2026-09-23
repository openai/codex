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
