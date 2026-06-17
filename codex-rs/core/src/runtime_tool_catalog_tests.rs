use super::*;
use codex_core_plugins::marketplace::MarketplaceListOutcome;
use codex_core_plugins::remote::RemoteDiscoverablePlugin;
use std::sync::Arc;

fn hosted(revision: u64) -> Arc<HostedConnectorRuntimeFragment> {
    Arc::new(HostedConnectorRuntimeFragment {
        revision: HostedConnectorsRevision(revision),
        tools: Vec::new(),
        connectors: Vec::new(),
    })
}

fn plugin_revision(revision: u64) -> RevisionedPluginsRevision {
    RevisionedPluginsRevision(revision)
}

fn curated(revision: u64) -> Arc<CuratedPluginCatalogFragment> {
    Arc::new(CuratedPluginCatalogFragment {
        revision: plugin_revision(revision),
        marketplaces: Arc::new(MarketplaceListOutcome::default()),
    })
}

fn remote_curated(revision: u64) -> Arc<RemoteCuratedPluginCatalogFragment> {
    Arc::new(RemoteCuratedPluginCatalogFragment {
        revision: plugin_revision(revision),
        plugins: Arc::new(Vec::<RemoteDiscoverablePlugin>::new()),
    })
}

fn plugins(
    curated: Option<Arc<CuratedPluginCatalogFragment>>,
    remote_curated: Option<Arc<RemoteCuratedPluginCatalogFragment>>,
) -> Arc<RevisionedPluginCatalogFragment> {
    Arc::new(RevisionedPluginCatalogFragment {
        curated,
        remote_curated,
    })
}

fn catalog(
    hosted_connectors: Option<Arc<HostedConnectorRuntimeFragment>>,
    revisioned_plugins: Option<Arc<RevisionedPluginCatalogFragment>>,
) -> RuntimeToolCatalogSnapshot {
    RuntimeToolCatalogSnapshot {
        hosted_connectors,
        revisioned_plugins,
    }
}

#[test]
fn hosted_revision_controls_fragment_reuse() {
    let fragment = hosted(1);
    let snapshot = catalog(Some(Arc::clone(&fragment)), None);

    assert!(Arc::ptr_eq(
        &fragment,
        &snapshot
            .hosted_connectors_for(HostedConnectorsRevision(1))
            .unwrap()
    ));
    assert!(
        snapshot
            .hosted_connectors_for(HostedConnectorsRevision(2))
            .is_none()
    );
}

#[test]
fn plugin_source_revisions_control_fragment_reuse_independently() {
    let curated = curated(1);
    let remote_curated = remote_curated(2);
    let fragment = plugins(
        Some(Arc::clone(&curated)),
        Some(Arc::clone(&remote_curated)),
    );
    let snapshot = catalog(None, Some(Arc::clone(&fragment)));

    assert!(Arc::ptr_eq(
        &curated,
        &snapshot.curated_plugins_for(plugin_revision(1)).unwrap()
    ));
    assert!(snapshot.curated_plugins_for(plugin_revision(3)).is_none());
    assert!(Arc::ptr_eq(
        &remote_curated,
        &snapshot
            .remote_curated_plugins_for(plugin_revision(2))
            .unwrap()
    ));
    assert!(
        snapshot
            .remote_curated_plugins_for(plugin_revision(4))
            .is_none()
    );
}

#[test]
fn source_changes_are_independent_and_change_the_aggregate() {
    let manager = RuntimeToolCatalogManager::default();
    let empty = manager.snapshot();
    let curated_fragment = curated(1);
    let remote_curated = remote_curated(1);
    let plugin_fragment = plugins(
        Some(Arc::clone(&curated_fragment)),
        Some(Arc::clone(&remote_curated)),
    );
    let populated = manager
        .publish_if_current(
            &empty,
            Ok::<_, ()>(catalog(Some(hosted(1)), Some(Arc::clone(&plugin_fragment)))),
        )
        .unwrap();
    let hosted_changed = manager
        .publish_if_current(
            &populated,
            Ok::<_, ()>(catalog(
                Some(hosted(2)),
                populated.revisioned_plugins.clone(),
            )),
        )
        .unwrap();
    let curated_changed = manager
        .publish_if_current(
            &hosted_changed,
            Ok::<_, ()>(catalog(
                hosted_changed.hosted_connectors.clone(),
                Some(plugins(
                    Some(curated(2)),
                    hosted_changed.remote_curated_plugins_for(plugin_revision(1)),
                )),
            )),
        )
        .unwrap();
    let curated_only = manager
        .publish_if_current(
            &curated_changed,
            Ok::<_, ()>(catalog(
                curated_changed.hosted_connectors.clone(),
                Some(plugins(
                    curated_changed.curated_plugins_for(plugin_revision(2)),
                    None,
                )),
            )),
        )
        .unwrap();

    assert!(Arc::ptr_eq(
        &plugin_fragment,
        hosted_changed.revisioned_plugins.as_ref().unwrap()
    ));
    assert!(Arc::ptr_eq(
        &remote_curated,
        curated_changed
            .revisioned_plugins
            .as_ref()
            .unwrap()
            .remote_curated
            .as_ref()
            .unwrap()
    ));
    assert_eq!(
        (
            empty.hosted_connectors.is_some(),
            populated.revisioned_plugins.is_some(),
            curated_only
                .revisioned_plugins
                .as_ref()
                .unwrap()
                .remote_curated
                .is_some(),
        ),
        (false, true, false)
    );
}

#[test]
fn failed_rebuild_does_not_publish_partial_state() {
    let manager = RuntimeToolCatalogManager::default();
    let empty = manager.snapshot();
    let initial = manager
        .publish_if_current(
            &empty,
            Ok::<_, &str>(catalog(
                Some(hosted(1)),
                Some(plugins(Some(curated(1)), None)),
            )),
        )
        .unwrap();

    let result = manager.publish_if_current(
        &initial,
        Err::<RuntimeToolCatalogSnapshot, _>("plugin rebuild failed"),
    );

    assert!(matches!(result, Err("plugin rebuild failed")));
    assert!(Arc::ptr_eq(&initial, &manager.snapshot()));
}
