//! Watch events invalidate discovery; no subscription keeps requests uncached.

use super::*;
use crate::DiscoverV2CapabilitiesRequest;
use crate::LocalFileSystem;
use crate::discover_v2::capability_locations::tests::request;
use crate::discover_v2::capability_locations::tests::write;
use anyhow::Context;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::time::Duration;

// Keep native events from racing deterministic cache assertions.
pub(in crate::discover_v2::capability_manager) async fn stop_listener(
    manager: &CapabilityManager,
) -> anyhow::Result<()> {
    manager
        .watchers
        .subscription
        .get()
        .context("watcher unavailable")?
        ._listener
        .abort();
    drop(manager.location_refresh_lock.acquire().await?);
    Ok(())
}

#[tokio::test]
async fn watch_events_invalidate_discovery_and_no_subscription_disables_caching()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let root = std::fs::canonicalize(directory.path())?;
    write(
        &root,
        "codex/skills/demo/SKILL.md",
        "---\nname: demo\ndescription: original\n---\n",
    )?;
    let request = request(&root)?;
    let demo_skill = request.codex_home.join("skills/demo/SKILL.md")?;
    let manager = CapabilityManager::new(LocalFileSystem::unsandboxed());
    let params = DiscoverV2CapabilitiesRequest {
        cwd: PathUri::from_host_native_path(&root)?,
        sandbox: None,
    };
    let original = manager
        .get_or_refresh_discovery(request.clone(), params.clone())
        .await?;
    assert!(
        original
            .skills
            .iter()
            .any(|skill| skill.path == demo_skill && skill.description == "original")
    );
    let initial = manager
        .location_snapshot()
        .await
        .context("missing initial snapshot")?;

    // Wait for native delivery before testing one-shot creation of missing roots.
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut interval = tokio::time::interval(Duration::from_millis(50));
        loop {
            interval.tick().await;
            write(
                &root,
                "codex/skills/demo/SKILL.md",
                "---\nname: demo\ndescription: ready\n---\n",
            )?;
            let snapshot = manager
                .location_snapshot()
                .await
                .context("missing snapshot")?;
            if !Arc::ptr_eq(&initial, &snapshot) {
                assert!(snapshot.sandbox_discoveries.lock().await.is_empty());
                break anyhow::Ok(());
            }
        }
    })
    .await
    .context("native watcher did not deliver its first content event")??;

    // Neither the plugin cache nor the user skills directory existed at startup.
    write(
        &root,
        "codex/plugins/cache/acme/gmail/1.0.0/.codex-plugin/plugin.json",
        r#"{"name":"gmail"}"#,
    )?;
    write(
        &root,
        "home/.agents/skills/added/SKILL.md",
        "---\nname: added\ndescription: new global root\n---\n",
    )?;
    let home_skills = PathUri::from_host_native_path(root.join("home/.agents/skills"))?;
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut interval = tokio::time::interval(Duration::from_millis(50));
        loop {
            interval.tick().await;
            // Inspect only locations: a request-time rescan must not hide a missed event.
            let snapshot = manager
                .location_snapshot()
                .await
                .context("missing snapshot")?;
            if snapshot
                .locations
                .locations
                .iter()
                .any(|location| location.root == home_skills)
                && snapshot.locations.locations.iter().any(|location| {
                    location
                        .plugin
                        .as_ref()
                        .is_some_and(|plugin| plugin.id == "gmail@acme")
                })
            {
                assert!(snapshot.sandbox_discoveries.lock().await.is_empty());
                break anyhow::Ok(());
            }
        }
    })
    .await
    .context("native watcher missed creation of a global root")??;
    // Drive subsequent refreshes explicitly so native delivery cannot race cache assertions.
    stop_listener(&manager).await?;
    let refreshed = manager
        .get_or_refresh_discovery(request.clone(), params.clone())
        .await?;
    let added_skill = home_skills.join("added/SKILL.md")?;
    assert!(
        refreshed
            .skills
            .iter()
            .any(|skill| skill.path == added_skill)
    );
    assert_eq!(refreshed.plugins.len(), 1);
    assert!(
        refreshed
            .skills
            .iter()
            .any(|skill| skill.path == demo_skill && skill.description == "ready")
    );
    let sandbox_params = DiscoverV2CapabilitiesRequest {
        sandbox: Some(crate::FileSystemSandboxContext::from_legacy_sandbox_policy(
            codex_protocol::protocol::SandboxPolicy::DangerFullAccess,
            params.cwd.clone(),
        )?),
        ..params.clone()
    };
    manager
        .get_or_refresh_discovery(request.clone(), sandbox_params)
        .await?;
    let retained = manager
        .location_snapshot()
        .await
        .context("missing populated snapshot")?;
    assert_eq!(retained.sandbox_discoveries.lock().await.len(), 2);
    write(
        &root,
        "codex/plugins/cache/acme/gmail/2.0.0/.codex-plugin/plugin.json",
        r#"{"name":"gmail"}"#,
    )?;
    write(
        &root,
        "codex/skills/demo/SKILL.md",
        "---\nname: demo\ndescription: refreshed\n---\n",
    )?;
    manager.refresh_locations().await;
    let next = manager
        .location_snapshot()
        .await
        .context("missing refreshed snapshot")?;
    assert_eq!(
        next.locations
            .locations
            .iter()
            .filter_map(|location| location.plugin.as_ref())
            .find(|plugin| plugin.id == "gmail@acme")
            .map(|plugin| plugin.version.as_str()),
        Some("2.0.0")
    );
    assert!(next.sandbox_discoveries.lock().await.is_empty());
    assert_eq!(retained.sandbox_discoveries.lock().await.len(), 2);
    assert_eq!(
        retained
            .locations
            .locations
            .iter()
            .filter_map(|location| location.plugin.as_ref())
            .find(|plugin| plugin.id == "gmail@acme")
            .map(|plugin| plugin.version.as_str()),
        Some("1.0.0")
    );
    let after = manager
        .get_or_refresh_discovery(request.clone(), params.clone())
        .await?;
    assert!(
        after
            .skills
            .iter()
            .any(|skill| skill.path == demo_skill && skill.description == "refreshed")
    );
    // Only the requested context is rebuilt; subsequent requests reuse it.
    assert_eq!(next.sandbox_discoveries.lock().await.len(), 1);
    assert!(Arc::ptr_eq(
        &after,
        &manager
            .get_or_refresh_discovery(request.clone(), params.clone())
            .await?
    ));

    #[cfg(unix)]
    {
        // A failed event refresh retains the response but retries without another event.
        let cache = root.join("codex/plugins/cache");
        let saved_cache = root.join("codex/plugins/saved-cache");
        std::fs::rename(&cache, &saved_cache)?;
        std::os::unix::fs::symlink(&cache, &cache)?;
        manager.refresh_locations().await;
        let fallback = manager
            .get_or_refresh_discovery(request.clone(), params.clone())
            .await?;
        assert!(Arc::ptr_eq(&after, &fallback));
        std::fs::remove_file(&cache)?;
        std::fs::rename(&saved_cache, &cache)?;
        write(
            &root,
            "codex/skills/recovered/SKILL.md",
            "---\nname: recovered\ndescription: recovered after failed refresh\n---\n",
        )?;
        let recovered = manager
            .get_or_refresh_discovery(request.clone(), params.clone())
            .await?;
        assert!(
            recovered
                .skills
                .iter()
                .any(|skill| skill.name == "recovered")
        );
        assert!(!Arc::ptr_eq(&fallback, &recovered));
        assert!(Arc::ptr_eq(
            &recovered,
            &manager
                .get_or_refresh_discovery(request.clone(), params.clone())
                .await?
        ));
    }

    // Without a subscription, requests reload both membership and content.
    let mut uncached = CapabilityManager::new(LocalFileSystem::unsandboxed());
    Arc::get_mut(&mut uncached)
        .context("new manager unexpectedly shared")?
        .watchers
        .file_watcher = None;
    let before = uncached
        .get_or_refresh_discovery(request.clone(), params.clone())
        .await?;
    assert!(uncached.watchers.subscription.get().is_none());
    write(
        &root,
        "codex/skills/demo/SKILL.md",
        "---\nname: demo\ndescription: uncached edit\n---\n",
    )?;
    write(
        &root,
        "codex/plugins/cache/acme/calendar/1.0.0/.codex-plugin/plugin.json",
        r#"{"name":"calendar"}"#,
    )?;
    let after = uncached
        .get_or_refresh_discovery(request.clone(), params.clone())
        .await?;
    assert_eq!(after.plugins.len(), before.plugins.len() + 1);
    assert!(
        after
            .plugins
            .iter()
            .any(|plugin| plugin.id == "calendar@acme")
    );
    assert!(
        after
            .skills
            .iter()
            .any(|skill| skill.path == demo_skill && skill.description == "uncached edit")
    );
    let repeated = uncached.get_or_refresh_discovery(request, params).await?;
    assert_eq!(after, repeated);
    assert!(!Arc::ptr_eq(&after, &repeated));
    Ok(())
}
