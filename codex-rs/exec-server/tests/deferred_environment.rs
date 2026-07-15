use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerError;
use codex_exec_server::NoiseChannelPublicKey;
use codex_exec_server::NoiseRendezvousConnectBundle;
use codex_exec_server::NoiseRendezvousConnectProvider;
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

#[tokio::test]
async fn deferred_environment_waits_before_connecting() -> anyhow::Result<()> {
    let manager = EnvironmentManager::without_environments();
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

    registration.complete(Ok(()))?;
    let error = readiness.await.unwrap_err();
    assert!(error.to_string().contains("test Noise provider called"));
    assert_eq!(provider.calls(), 1);
    assert!(!connection_state.has_changed()?);
    Ok(())
}

#[tokio::test]
async fn failure_and_dropped_registration_are_terminal() -> anyhow::Result<()> {
    let manager = EnvironmentManager::without_environments();
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
async fn late_completion_is_isolated_from_replacement() -> anyhow::Result<()> {
    let manager = EnvironmentManager::without_environments();
    let old_provider = Arc::new(FailingNoiseConnectProvider::default());
    let old_registration =
        manager.register_deferred_noise_environment("tools".to_string(), old_provider.clone())?;
    let old_environment = manager.get_environment("tools").expect("old environment");
    let current_provider = Arc::new(FailingNoiseConnectProvider::default());
    let current_registration = manager
        .register_deferred_noise_environment("tools".to_string(), current_provider.clone())?;
    let current = manager.get_environment("tools").expect("current");

    old_registration.complete(Ok(()))?;
    let old_error = old_environment.wait_until_ready().await.unwrap_err();
    assert!(old_error.to_string().contains("test Noise provider called"));
    assert_eq!(old_provider.calls(), 1);
    let mut current_readiness = Box::pin(current.wait_until_ready());
    assert!(poll!(&mut current_readiness).is_pending());
    assert_eq!(current_provider.calls(), 0);

    current_registration.complete(Ok(()))?;
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
    let manager = EnvironmentManager::without_environments();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    manager.upsert_noise_environment("tools".to_string(), provider.clone())?;
    let environment = manager.get_environment("tools").expect("environment");

    let error = environment.wait_until_ready().await.unwrap_err();
    assert!(error.to_string().contains("test Noise provider called"));
    assert_eq!(provider.calls(), 1);
    Ok(())
}
