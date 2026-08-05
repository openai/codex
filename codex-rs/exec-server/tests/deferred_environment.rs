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
async fn deferred_environment_waits_before_connecting() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    let registration =
        manager.register_deferred_noise_environment("tools".to_string(), provider.clone())?;
    let environment = manager.get_environment("tools").expect("environment");
    let connection_state = environment
        .subscribe_connection_state()
        .expect("remote environment connection state");
    let mut readiness = Box::pin(environment.wait_until_ready());

    assert!(poll!(&mut readiness).is_pending());
    assert_eq!(provider.calls(), 0);
    assert!(environment.selected_capability_roots().is_empty());

    let ready_info = ready_info("selected-root", "tools")?;
    registration.complete(Ok(ready_info.clone()))?;
    assert_eq!(
        environment.selected_capability_roots(),
        ready_info.selected_capability_roots
    );
    let error = readiness.await.unwrap_err();
    assert!(error.to_string().contains("test Noise provider called"));
    assert_eq!(provider.calls(), 1);
    assert!(!connection_state.has_changed()?);
    Ok(())
}

#[tokio::test]
async fn deferred_registration_replaces_an_ordinary_noise_environment() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    manager.upsert_noise_environment(
        "tools".to_string(),
        Arc::new(FailingNoiseConnectProvider::default()),
    )?;
    let ordinary = manager
        .get_environment("tools")
        .expect("ordinary environment");
    let deferred_provider = Arc::new(FailingNoiseConnectProvider::default());

    let registration = manager
        .register_deferred_noise_environment("tools".to_string(), deferred_provider.clone())?;
    let deferred = manager
        .get_environment("tools")
        .expect("deferred environment");
    assert!(!Arc::ptr_eq(&ordinary, &deferred));
    let mut readiness = Box::pin(deferred.wait_until_ready());
    assert!(poll!(&mut readiness).is_pending());

    registration.complete(Ok(ready_info("selected-root", "tools")?))?;
    let error = readiness.await.unwrap_err();
    assert!(error.to_string().contains("test Noise provider called"));
    assert_eq!(deferred_provider.calls(), 1);
    Ok(())
}

#[tokio::test]
async fn ordinary_noise_environment_replaces_a_deferred_registration() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let deferred_provider = Arc::new(FailingNoiseConnectProvider::default());
    let registration = manager
        .register_deferred_noise_environment("tools".to_string(), deferred_provider.clone())?;
    let deferred = manager
        .get_environment("tools")
        .expect("deferred environment");
    let ordinary_provider = Arc::new(FailingNoiseConnectProvider::default());

    manager.upsert_noise_environment("tools".to_string(), ordinary_provider.clone())?;
    let ordinary = manager
        .get_environment("tools")
        .expect("ordinary environment");
    assert!(!Arc::ptr_eq(&deferred, &ordinary));

    drop(registration);
    let error = deferred.wait_until_ready().await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("registration ended before completion")
    );
    assert_eq!(deferred_provider.calls(), 0);
    let error = ordinary.wait_until_ready().await.unwrap_err();
    assert!(error.to_string().contains("test Noise provider called"));
    assert_eq!(ordinary_provider.calls(), 1);
    Ok(())
}

#[tokio::test]
async fn existing_environment_publishes_readiness_without_replacement() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let existing_provider = Arc::new(FailingNoiseConnectProvider::default());
    manager.upsert_noise_environment("tools".to_string(), existing_provider)?;
    let existing_environment = manager
        .get_environment("tools")
        .expect("existing environment");

    let ready_info = ready_info("selected-root", "tools")?;
    manager.publish_ready_info("tools", ready_info.clone())?;

    let current_environment = manager
        .get_environment("tools")
        .expect("current environment");
    assert!(Arc::ptr_eq(&existing_environment, &current_environment));
    assert_eq!(
        existing_environment.selected_capability_roots(),
        ready_info.selected_capability_roots
    );
    Ok(())
}

#[tokio::test]
async fn readiness_updates_the_current_environment_after_replacement() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    manager.upsert_noise_environment(
        "tools".to_string(),
        Arc::new(FailingNoiseConnectProvider::default()),
    )?;
    let captured_environment = manager
        .get_environment("tools")
        .expect("captured environment");
    manager.upsert_noise_environment(
        "tools".to_string(),
        Arc::new(FailingNoiseConnectProvider::default()),
    )?;
    let current_environment = manager
        .get_environment("tools")
        .expect("replacement environment");
    let ready_info = ready_info("selected-root", "tools")?;

    manager.publish_ready_info("tools", ready_info.clone())?;

    assert!(!Arc::ptr_eq(&captured_environment, &current_environment));
    assert!(captured_environment.selected_capability_roots().is_empty());
    assert_eq!(
        current_environment.selected_capability_roots(),
        ready_info.selected_capability_roots
    );
    Ok(())
}

#[tokio::test]
async fn publishing_readiness_requires_existing_environment() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();

    let error = manager
        .publish_ready_info("tools", ready_info("selected-root", "tools")?)
        .unwrap_err();

    assert!(matches!(error, ExecServerError::Protocol(_)));
    assert!(manager.get_environment("tools").is_none());
    Ok(())
}

#[tokio::test]
async fn existing_environment_accepts_matching_readiness() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    manager.upsert_noise_environment(
        "tools".to_string(),
        Arc::new(FailingNoiseConnectProvider::default()),
    )?;
    let environment = manager.get_environment("tools").expect("environment");
    let ready_info = ready_info("selected-root", "tools")?;

    for _ in 0..2 {
        manager.publish_ready_info("tools", ready_info.clone())?;
    }
    assert_eq!(
        environment.selected_capability_roots(),
        ready_info.selected_capability_roots
    );
    Ok(())
}

#[tokio::test]
async fn existing_environment_overwrites_published_readiness() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    manager.upsert_noise_environment(
        "tools".to_string(),
        Arc::new(FailingNoiseConnectProvider::default()),
    )?;
    let environment = manager.get_environment("tools").expect("environment");
    let selected_ready_info = ready_info("selected-root", "tools")?;
    manager.publish_ready_info("tools", selected_ready_info)?;

    let updated_ready_info = ready_info("different-root", "tools")?;
    manager.publish_ready_info("tools", updated_ready_info.clone())?;
    assert_eq!(
        environment.selected_capability_roots(),
        updated_ready_info.selected_capability_roots
    );
    assert!(Arc::ptr_eq(
        &environment,
        &manager.get_environment("tools").expect("environment")
    ));

    manager.publish_ready_info("tools", EnvironmentReadyInfo::default())?;
    assert!(environment.selected_capability_roots().is_empty());
    Ok(())
}

#[tokio::test]
async fn publishing_readiness_before_deferred_completion_preserves_the_gate() -> anyhow::Result<()>
{
    let manager = environment_manager_without_environments();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    let registration =
        manager.register_deferred_noise_environment("tools".to_string(), provider.clone())?;
    let environment = manager.get_environment("tools").expect("environment");
    let mut readiness = Box::pin(environment.wait_until_ready());

    manager.publish_ready_info("tools", ready_info("published-root", "tools")?)?;
    assert!(poll!(&mut readiness).is_pending());
    assert_eq!(provider.calls(), 0);

    let completed_ready_info = ready_info("completed-root", "tools")?;
    registration.complete(Ok(completed_ready_info.clone()))?;
    assert_eq!(
        environment.selected_capability_roots(),
        completed_ready_info.selected_capability_roots
    );
    let error = readiness.await.unwrap_err();
    assert!(error.to_string().contains("test Noise provider called"));
    assert_eq!(provider.calls(), 1);
    Ok(())
}

#[tokio::test]
async fn existing_environment_rejects_invalid_readiness() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    manager.upsert_noise_environment(
        "tools".to_string(),
        Arc::new(FailingNoiseConnectProvider::default()),
    )?;
    let existing_environment = manager
        .get_environment("tools")
        .expect("existing environment");
    let error = manager
        .publish_ready_info("tools", ready_info("selected-root", "other")?)
        .unwrap_err();

    assert!(matches!(error, ExecServerError::Protocol(_)));
    assert!(existing_environment.selected_capability_roots().is_empty());
    let current_environment = manager
        .get_environment("tools")
        .expect("current environment");
    assert!(Arc::ptr_eq(&existing_environment, &current_environment));
    Ok(())
}

#[tokio::test]
async fn failure_and_dropped_registration_are_terminal() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let failed_provider = Arc::new(FailingNoiseConnectProvider::default());
    let failed = manager
        .register_deferred_noise_environment("failed".to_string(), failed_provider.clone())?;
    let failed_environment = manager.get_environment("failed").expect("environment");
    failed.complete(Err("provisioning failed".to_string()))?;
    let error = failed_environment.wait_until_ready().await.unwrap_err();
    assert!(
        error
            .to_string()
            .ends_with("environment unavailable: provisioning failed")
    );
    assert_eq!(failed_provider.calls(), 0);

    let dropped_provider = Arc::new(FailingNoiseConnectProvider::default());
    let dropped = manager
        .register_deferred_noise_environment("dropped".to_string(), dropped_provider.clone())?;
    let dropped_environment = manager.get_environment("dropped").expect("environment");
    drop(dropped);
    let error = dropped_environment.wait_until_ready().await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("registration ended before completion")
    );
    assert_eq!(dropped_provider.calls(), 0);
    assert!(manager.get_environment("failed").is_some());
    assert!(manager.get_environment("dropped").is_some());
    Ok(())
}

#[tokio::test]
async fn invalid_ready_info_is_terminal() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    let registration =
        manager.register_deferred_noise_environment("tools".to_string(), provider.clone())?;
    let environment = manager.get_environment("tools").expect("environment");

    let error = registration
        .complete(Ok(ready_info("selected-root", "other")?))
        .unwrap_err();
    assert!(matches!(error, ExecServerError::Protocol(_)));
    let readiness_error = environment.wait_until_ready().await.unwrap_err();
    assert!(
        readiness_error
            .to_string()
            .contains("belong to environment")
    );
    assert!(environment.selected_capability_roots().is_empty());
    assert_eq!(provider.calls(), 0);
    Ok(())
}

#[tokio::test]
async fn late_completion_is_isolated_from_replacement() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let old_provider = Arc::new(FailingNoiseConnectProvider::default());
    let old_registration =
        manager.register_deferred_noise_environment("tools".to_string(), old_provider.clone())?;
    let old_environment = manager.get_environment("tools").expect("old environment");
    let current_provider = Arc::new(FailingNoiseConnectProvider::default());
    let current_registration = manager
        .register_deferred_noise_environment("tools".to_string(), current_provider.clone())?;
    let current = manager.get_environment("tools").expect("current");

    let old_ready_info = ready_info("old-root", "tools")?;
    old_registration.complete(Ok(old_ready_info.clone()))?;
    assert_eq!(
        old_environment.selected_capability_roots(),
        old_ready_info.selected_capability_roots
    );
    assert!(current.selected_capability_roots().is_empty());
    let old_error = old_environment.wait_until_ready().await.unwrap_err();
    assert!(old_error.to_string().contains("test Noise provider called"));
    assert_eq!(old_provider.calls(), 1);
    let mut current_readiness = Box::pin(current.wait_until_ready());
    assert!(poll!(&mut current_readiness).is_pending());
    assert_eq!(current_provider.calls(), 0);

    let current_ready_info = ready_info("current-root", "tools")?;
    current_registration.complete(Ok(current_ready_info.clone()))?;
    assert_eq!(
        current.selected_capability_roots(),
        current_ready_info.selected_capability_roots
    );
    let current_error = current_readiness.await.unwrap_err();
    assert!(
        current_error
            .to_string()
            .contains("test Noise provider called")
    );
    assert_eq!(current_provider.calls(), 1);
    Ok(())
}

#[tokio::test]
async fn eager_noise_environment_connects_without_registration() -> anyhow::Result<()> {
    let manager = environment_manager_without_environments();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    manager.upsert_noise_environment("tools".to_string(), provider.clone())?;
    let environment = manager.get_environment("tools").expect("environment");

    let error = environment.wait_until_ready().await.unwrap_err();
    assert!(error.to_string().contains("test Noise provider called"));
    assert_eq!(provider.calls(), 1);
    Ok(())
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
            Ok(ready_info("selected-root", "other")?),
            provider.clone(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("first failure"));

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
