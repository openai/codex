use super::*;
use codex_core_plugins::marketplace::MarketplaceListOutcome;
use std::path::PathBuf;
use std::sync::Arc;

fn hosted(revision: u64) -> Arc<HostedConnectorRuntimeFragment> {
    Arc::new(HostedConnectorRuntimeFragment {
        revision: HostedConnectorsRevision(revision),
        tools: Vec::new(),
        connectors: Vec::new(),
    })
}

fn plugin_revision(revision: &str) -> RevisionedPluginsRevision {
    RevisionedPluginsRevision(PathBuf::from("/plugin-catalog"), revision.to_string())
}

fn plugins(revision: &str) -> Arc<RevisionedPluginCatalogFragment> {
    Arc::new(RevisionedPluginCatalogFragment {
        revision: plugin_revision(revision),
        marketplaces: MarketplaceListOutcome::default(),
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
fn only_proven_unchanged_plugin_revisions_are_reused() {
    let fragment = plugins("one");
    let snapshot = catalog(None, Some(Arc::clone(&fragment)));

    assert!(Arc::ptr_eq(
        &fragment,
        &snapshot
            .revisioned_plugins_for(&plugin_revision("one"))
            .unwrap()
    ));
    assert!(
        snapshot
            .revisioned_plugins_for(&plugin_revision("two"))
            .is_none()
    );
}

#[test]
fn source_changes_are_independent_and_change_the_aggregate() {
    let manager = RuntimeToolCatalogManager::default();
    let empty = manager.snapshot();
    let plugins = plugins("one");
    let populated = manager
        .publish_if_current(
            &empty,
            Ok::<_, ()>(catalog(Some(hosted(1)), Some(Arc::clone(&plugins)))),
        )
        .unwrap();
    let hosted_changed = manager
        .publish_if_current(
            &populated,
            Ok::<_, ()>(catalog(
                Some(hosted(2)),
                populated.revisioned_plugins_for(&plugin_revision("one")),
            )),
        )
        .unwrap();
    let hosted_only = manager
        .publish_if_current(
            &hosted_changed,
            Ok::<_, ()>(catalog(hosted_changed.hosted_connectors.clone(), None)),
        )
        .unwrap();

    assert!(Arc::ptr_eq(
        &plugins,
        hosted_changed.revisioned_plugins.as_ref().unwrap()
    ));
    assert_eq!(
        (
            empty.hosted_connectors.is_some(),
            populated.revisioned_plugins.is_some(),
            hosted_only.revisioned_plugins.is_some(),
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
            Ok::<_, &str>(catalog(Some(hosted(1)), Some(plugins("one")))),
        )
        .unwrap();

    let result = manager.publish_if_current(
        &initial,
        Err::<RuntimeToolCatalogSnapshot, _>("plugin rebuild failed"),
    );

    assert!(matches!(result, Err("plugin rebuild failed")));
    assert!(Arc::ptr_eq(&initial, &manager.snapshot()));
}
