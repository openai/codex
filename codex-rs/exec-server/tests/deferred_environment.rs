use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_exec_server::EnvironmentReadyInfo;
use codex_exec_server::ExecServerError;
use codex_exec_server::NoiseChannelPublicKey;
use codex_exec_server::NoiseRendezvousConnectBundle;
use codex_exec_server::NoiseRendezvousConnectProvider;
use codex_exec_server_test_support::environment_manager_without_environments;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use futures::FutureExt;
use futures::future::BoxFuture;
use futures::poll;
use pretty_assertions::assert_eq;

#[derive(Default)]
struct FailingNoiseConnectProvider {
    calls: AtomicUsize,
}

impl FailingNoiseConnectProvider {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }
}

impl NoiseRendezvousConnectProvider for FailingNoiseConnectProvider {
    fn connect_bundle(
        &self,
        _: NoiseChannelPublicKey,
    ) -> BoxFuture<'_, Result<NoiseRendezvousConnectBundle, ExecServerError>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        async {
            Err(ExecServerError::Protocol(
                "test Noise provider called".to_string(),
            ))
        }
        .boxed()
    }
}

fn ready_info(root_id: &str, environment_id: &str) -> anyhow::Result<EnvironmentReadyInfo> {
    Ok(EnvironmentReadyInfo {
        selected_capability_roots: vec![SelectedCapabilityRoot {
            id: root_id.to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: environment_id.to_string(),
                path: PathUri::parse("file:///plugins/root")?,
            },
        }],
    })
}

#[tokio::test]
async fn readiness_before_materialization_creates_the_stable_environment() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let readiness_provider = Arc::new(FailingNoiseConnectProvider::default());
    let materialization_provider = Arc::new(FailingNoiseConnectProvider::default());
    let selected = ready_info("selected-root", "tools")?;

    let ready = manager
        .report_environment_provisioning_status(
            "tools".to_string(),
            Ok(selected.clone()),
            readiness_provider.clone(),
        )?
        .expect("readiness report should create the environment");
    let materialized = manager.materialize_pending_noise_environment(
        "tools".to_string(),
        materialization_provider.clone(),
    )?;

    assert!(Arc::ptr_eq(&ready, &materialized));
    assert_eq!(
        ready.selected_capability_roots(),
        selected.selected_capability_roots
    );
    let error = ready.wait_until_ready().await.unwrap_err();
    assert!(error.to_string().contains("test Noise provider called"));
    assert_eq!(readiness_provider.calls(), 1);
    assert_eq!(materialization_provider.calls(), 0);
    Ok(())
}

#[tokio::test]
async fn materialize_then_report_ready_reuses_the_pending_environment() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let pending_provider = Arc::new(FailingNoiseConnectProvider::default());
    let pending = manager
        .materialize_pending_noise_environment("tools".to_string(), pending_provider.clone())?;
    let mut pending_readiness = Box::pin(pending.wait_until_ready());
    assert!(poll!(&mut pending_readiness).is_pending());
    let ready = manager
        .report_environment_provisioning_status(
            "tools".to_string(),
            Ok(ready_info("selected-root", "tools")?),
            Arc::new(FailingNoiseConnectProvider::default()),
        )?
        .expect("provisioning report should apply to the pending environment");

    assert!(Arc::ptr_eq(&pending, &ready));
    let error = pending_readiness.await.unwrap_err();
    assert!(error.to_string().contains("test Noise provider called"));
    assert_eq!(pending_provider.calls(), 1);
    Ok(())
}

#[tokio::test]
async fn ordinary_environment_ignores_provisioning_reports() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    manager.upsert_environment(
        "tools".to_string(),
        "ws://127.0.0.1:1".to_string(),
        Some(std::time::Duration::from_millis(1)),
    )?;
    let existing_environment = manager
        .get_environment("tools")
        .expect("existing environment");

    let reported = manager.report_environment_provisioning_status(
        "tools".to_string(),
        Ok(ready_info("selected-root", "tools")?),
        Arc::new(FailingNoiseConnectProvider::default()),
    )?;

    let current_environment = manager
        .get_environment("tools")
        .expect("current environment");
    assert!(Arc::ptr_eq(&existing_environment, &current_environment));
    assert!(reported.is_none());
    assert!(existing_environment.selected_capability_roots().is_empty());
    Ok(())
}

#[tokio::test]
async fn failure_before_materialization_is_terminal_without_connecting() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let provider = Arc::new(FailingNoiseConnectProvider::default());

    let failed = manager
        .report_environment_provisioning_status(
            "tools".to_string(),
            Err("provisioning failed".to_string()),
            provider.clone(),
        )?
        .expect("failure report should create the environment");
    let materialized = manager.materialize_pending_noise_environment(
        "tools".to_string(),
        Arc::new(FailingNoiseConnectProvider::default()),
    )?;

    assert!(Arc::ptr_eq(&failed, &materialized));
    let error = failed.wait_until_ready().await.unwrap_err();
    assert!(error.to_string().ends_with("provisioning failed"));
    assert_eq!(provider.calls(), 0);
    Ok(())
}

#[tokio::test]
async fn failure_releases_the_existing_pending_environment_without_connecting() -> anyhow::Result<()>
{
    let manager = environment_manager_without_environments();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    let pending =
        manager.materialize_pending_noise_environment("tools".to_string(), provider.clone())?;

    let reported = manager
        .report_environment_provisioning_status(
            "tools".to_string(),
            Err("provisioning failed".to_string()),
            provider.clone(),
        )?
        .expect("failure report should apply to the pending environment");

    assert!(Arc::ptr_eq(&pending, &reported));
    let error = pending.wait_until_ready().await.unwrap_err();
    assert!(error.to_string().ends_with("provisioning failed"));
    assert_eq!(provider.calls(), 0);
    Ok(())
}

#[tokio::test]
async fn repeated_failure_preserves_the_first_error_and_rejects_ready() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    let failed = manager
        .report_environment_provisioning_status(
            "tools".to_string(),
            Err("first failure".to_string()),
            provider.clone(),
        )?
        .expect("failure report should create the environment");

    let repeated = manager
        .report_environment_provisioning_status(
            "tools".to_string(),
            Err("different failure".to_string()),
            provider.clone(),
        )?
        .expect("repeated failure should be idempotent");
    assert!(Arc::ptr_eq(&failed, &repeated));

    let error = manager
        .report_environment_provisioning_status(
            "tools".to_string(),
            Ok(ready_info("selected-root", "tools")?),
            provider.clone(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("first failure"));
    assert!(failed.selected_capability_roots().is_empty());
    let error = failed.wait_until_ready().await.unwrap_err();
    assert!(error.to_string().ends_with("first failure"));
    assert_eq!(provider.calls(), 0);
    Ok(())
}

#[tokio::test]
async fn ready_environment_rejects_a_later_failure() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    let ready = manager
        .report_environment_provisioning_status(
            "tools".to_string(),
            Ok(ready_info("selected-root", "tools")?),
            provider.clone(),
        )?
        .expect("ready report should create the environment");

    let error = manager
        .report_environment_provisioning_status(
            "tools".to_string(),
            Err("late failure".to_string()),
            provider,
        )
        .unwrap_err();

    assert!(error.to_string().contains("already ready"));
    assert_eq!(ready.selected_capability_roots().len(), 1);
    Ok(())
}

#[tokio::test]
async fn existing_environment_accepts_matching_readiness() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    let ready_info = ready_info("selected-root", "tools")?;

    let environment = manager
        .report_environment_provisioning_status(
            "tools".to_string(),
            Ok(ready_info.clone()),
            provider.clone(),
        )?
        .expect("readiness report should create the environment");
    manager.report_environment_provisioning_status(
        "tools".to_string(),
        Ok(ready_info.clone()),
        provider,
    )?;
    assert_eq!(
        environment.selected_capability_roots(),
        ready_info.selected_capability_roots
    );
    Ok(())
}

#[tokio::test]
async fn existing_environment_overwrites_reported_readiness() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    let environment = manager
        .report_environment_provisioning_status(
            "tools".to_string(),
            Ok(ready_info("selected-root", "tools")?),
            provider.clone(),
        )?
        .expect("readiness report should create the environment");

    let updated_ready_info = ready_info("different-root", "tools")?;
    manager.report_environment_provisioning_status(
        "tools".to_string(),
        Ok(updated_ready_info.clone()),
        provider,
    )?;
    assert_eq!(
        environment.selected_capability_roots(),
        updated_ready_info.selected_capability_roots
    );
    assert!(Arc::ptr_eq(
        &environment,
        &manager.get_environment("tools").expect("environment")
    ));
    Ok(())
}

#[tokio::test]
async fn invalid_ready_report_fails_the_provisioning_gate() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    let environment =
        manager.materialize_pending_noise_environment("tools".to_string(), provider.clone())?;
    let readiness = Box::pin(environment.wait_until_ready());

    let error = manager
        .report_environment_provisioning_status(
            "tools".to_string(),
            Ok(ready_info("selected-root", "other")?),
            provider.clone(),
        )
        .unwrap_err();

    assert!(matches!(error, ExecServerError::Protocol(_)));
    let readiness_error = readiness.await.unwrap_err();
    assert!(readiness_error.to_string().contains(&error.to_string()));
    assert!(environment.selected_capability_roots().is_empty());
    assert_eq!(provider.calls(), 0);

    let later_ready_error = manager
        .report_environment_provisioning_status(
            "tools".to_string(),
            Ok(ready_info("selected-root", "tools")?),
            provider.clone(),
        )
        .unwrap_err();
    assert!(
        later_ready_error
            .to_string()
            .contains("provisioning already failed")
    );
    assert!(environment.selected_capability_roots().is_empty());
    assert_eq!(provider.calls(), 0);
    Ok(())
}

#[tokio::test]
async fn duplicate_materialization_reuses_the_pending_environment() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    let environment =
        manager.materialize_pending_noise_environment("tools".to_string(), provider.clone())?;
    let replacement_provider = Arc::new(FailingNoiseConnectProvider::default());

    let current = manager
        .materialize_pending_noise_environment("tools".to_string(), replacement_provider.clone())?;
    assert!(Arc::ptr_eq(&environment, &current));
    assert_eq!(provider.calls(), 0);
    assert_eq!(replacement_provider.calls(), 0);
    Ok(())
}

#[tokio::test]
async fn deferred_materialization_conflicts_with_an_existing_ordinary_environment()
-> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    manager.upsert_environment(
        "tools".to_string(),
        "ws://127.0.0.1:1".to_string(),
        Some(std::time::Duration::from_millis(1)),
    )?;
    let existing_environment = manager.get_environment("tools").expect("environment");
    let deferred_provider = Arc::new(FailingNoiseConnectProvider::default());

    let error = manager
        .materialize_pending_noise_environment("tools".to_string(), deferred_provider.clone())
        .unwrap_err();

    assert!(matches!(
        error,
        ExecServerError::ProvisioningModeConflict { environment_id }
            if environment_id == "tools"
    ));
    let current_environment = manager
        .get_environment("tools")
        .expect("ordinary environment should remain registered");
    assert!(Arc::ptr_eq(&existing_environment, &current_environment));
    assert_eq!(deferred_provider.calls(), 0);
    Ok(())
}
