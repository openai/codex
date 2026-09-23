//! Covers bounded context caching, shared initialization, cancellation retry, and cached warnings.

use super::*;
use crate::discover_v2::capability_locations::tests::request;
use crate::discover_v2::capability_locations::tests::write;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

async fn setup() -> anyhow::Result<(
    tempfile::TempDir,
    CapabilityLocationRequest,
    Arc<CapabilityManager>,
)> {
    let directory = tempfile::tempdir()?;
    write(
        directory.path(),
        "codex/skills/demo/SKILL.md",
        "---\nname: demo\ndescription: original\n---\nbody never cached",
    )?;
    let request = request(&std::fs::canonicalize(directory.path())?)?;
    let file_system = LocalFileSystem::unsandboxed();
    let manager = CapabilityManager::new(file_system);
    manager.prewarm_locations(request.clone()).await?;
    capability_watchers::tests::stop_listener(&manager).await?;
    Ok((directory, request, manager))
}

#[tokio::test]
async fn discovery_responses_are_cached_by_sandbox_context() -> anyhow::Result<()> {
    let (directory, request, manager) = setup().await?;
    let cwd = PathUri::from_host_native_path(directory.path().join("project"))?;
    let other_cwd = PathUri::from_host_native_path(directory.path().join("other"))?;
    let params = DiscoverV2CapabilitiesRequest {
        cwd: cwd.clone(),
        sandbox: None,
    };
    let first = manager
        .get_or_refresh_discovery(request.clone(), params.clone())
        .await?;
    assert_eq!(first.skills.len(), 1);
    let snapshot = manager
        .location_snapshot()
        .await
        .ok_or_else(|| anyhow::anyhow!("missing snapshot"))?;
    let other = manager
        .get_or_refresh_discovery(
            request.clone(),
            DiscoverV2CapabilitiesRequest {
                cwd: other_cwd.clone(),
                ..params.clone()
            },
        )
        .await?;
    assert!(Arc::ptr_eq(&first, &other));

    // The full sandbox context, including its own cwd, still identifies a separate cache.
    let mut previous = Arc::clone(&first);
    for index in 0..MAX_CACHED_SANDBOX_CONTEXTS {
        let sandbox_cwd = cwd.join(&format!("context-{index}"))?;
        let sandbox = DiscoverV2CapabilitiesRequest {
            sandbox: Some(FileSystemSandboxContext::from_legacy_sandbox_policy(
                SandboxPolicy::DangerFullAccess,
                sandbox_cwd,
            )?),
            ..params.clone()
        };
        let response = manager
            .get_or_refresh_discovery(request.clone(), sandbox.clone())
            .await?;
        assert_eq!(response, previous);
        assert!(!Arc::ptr_eq(&response, &previous));
        assert!(Arc::ptr_eq(
            &response,
            &manager
                .get_or_refresh_discovery(request.clone(), sandbox)
                .await?
        ));
        previous = response;
    }
    assert_eq!(
        snapshot.sandbox_discoveries.lock().await.len(),
        MAX_CACHED_SANDBOX_CONTEXTS
    );
    // The oldest context is evicted, but its previously returned response stays valid.
    let reloaded = manager.get_or_refresh_discovery(request, params).await?;
    assert_eq!(reloaded, first);
    assert!(!Arc::ptr_eq(&reloaded, &first));
    assert_eq!(
        snapshot.sandbox_discoveries.lock().await.len(),
        MAX_CACHED_SANDBOX_CONTEXTS
    );
    Ok(())
}

#[test]
fn cancelled_initialization_retries_and_warning_responses_are_cached() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()?
        .block_on(async {
            let (directory, request, manager) = setup().await?;
            let params = DiscoverV2CapabilitiesRequest {
                cwd: PathUri::from_host_native_path(directory.path().join("project"))?,
                sandbox: None,
            };
            let snapshot = manager
                .location_snapshot()
                .await
                .ok_or_else(|| anyhow::anyhow!("missing snapshot"))?;
            // Hold the only blocking worker so discovery's filesystem read cannot finish early.
            // Dropping the sender also releases the worker if an assertion unwinds.
            let (release, wait) = std::sync::mpsc::channel::<()>();
            let (started, ready) = tokio::sync::oneshot::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                let _ = started.send(());
                let _ = wait.recv();
            });
            ready.await?;
            let mut first =
                Box::pin(manager.get_or_refresh_discovery(request.clone(), params.clone()));
            assert!(futures::poll!(&mut first).is_pending());
            let discovery = snapshot
                .sandbox_discoveries
                .lock()
                .await
                .first()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("missing sandbox discovery"))?;
            let mut second =
                Box::pin(manager.get_or_refresh_discovery(request.clone(), params.clone()));
            assert!(futures::poll!(&mut second).is_pending());
            assert!(discovery.response.get().is_none());
            assert!(snapshot.sandbox_discoveries.try_lock().is_ok());
            // Cancel the initializer; the waiting request must be able to initialize instead.
            drop(first);
            drop(release);
            blocker.await?;
            let original = second.await?;
            assert_eq!(original.skills.len(), 1);
            assert!(Arc::ptr_eq(
                &original,
                &manager
                    .get_or_refresh_discovery(request.clone(), params.clone())
                    .await?,
            ));

            write(
                directory.path(),
                "codex/skills/demo/SKILL.md",
                "invalid skill metadata",
            )?;
            // Invalidation drops entries instead of overwriting their initialized responses.
            snapshot.sandbox_discoveries.lock().await.clear();
            let (partial, concurrent) = tokio::try_join!(
                manager.get_or_refresh_discovery(request.clone(), params.clone()),
                manager.get_or_refresh_discovery(request.clone(), params.clone()),
            )?;
            assert!(Arc::ptr_eq(&partial, &concurrent));
            assert!(partial.skills.is_empty());
            assert!(!partial.warnings.is_empty());
            assert!(Arc::ptr_eq(
                &partial,
                &manager
                    .get_or_refresh_discovery(request.clone(), params.clone())
                    .await?
            ));

            write(
                directory.path(),
                "codex/skills/demo/SKILL.md",
                "---\nname: demo\ndescription: recovered\n---\n",
            )?;
            assert!(Arc::ptr_eq(
                &partial,
                &manager
                    .get_or_refresh_discovery(request.clone(), params.clone())
                    .await?
            ));
            snapshot.sandbox_discoveries.lock().await.clear();
            let recovered = manager
                .get_or_refresh_discovery(request.clone(), params.clone())
                .await?;
            assert!(recovered.warnings.is_empty());
            assert_eq!(
                recovered
                    .skills
                    .iter()
                    .map(|skill| skill.description.as_str())
                    .collect::<Vec<_>>(),
                vec!["recovered"]
            );
            assert!(Arc::ptr_eq(
                &recovered,
                &manager
                    .get_or_refresh_discovery(request.clone(), params)
                    .await?
            ));
            Ok(())
        })
}
