use std::path::Path;
use std::sync::Arc;

use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionDataInit;
use codex_extension_api::McpServerContributionContext;
use codex_plugin::AppConnectorId;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tokio::sync::Barrier;

use super::CachedSelectedRoot;
use super::SelectedExecutorPluginMcpState;
use super::SelectedPluginMetadata;
use super::selected_plugin_connector_snapshot;

#[tokio::test]
async fn concurrent_step_projections_keep_connector_attribution_disjoint() {
    let plugin_root = tempfile::tempdir().expect("plugin root");
    let alpha = selected_root(
        "alpha",
        "environment-alpha",
        &plugin_root.path().join("alpha"),
    );
    let beta = selected_root("beta", "environment-beta", &plugin_root.path().join("beta"));
    let mut thread_init = ExtensionDataInit::new();
    thread_init.insert(vec![alpha.clone(), beta.clone()]);
    let thread_store = Arc::new(ExtensionData::new_with_init(
        "test-thread",
        thread_init.clone(),
    ));
    let state = thread_store.get_or_init(SelectedExecutorPluginMcpState::default);
    state
        .cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .extend([
            cached_root(alpha, "Alpha Plugin", "connector-alpha"),
            cached_root(beta, "Beta Plugin", "connector-beta"),
        ]);

    let barrier = Arc::new(Barrier::new(3));
    let alpha_projection = spawn_projection(
        thread_init.clone(),
        Arc::clone(&thread_store),
        Arc::clone(&barrier),
        "environment-alpha",
    );
    let beta_projection = spawn_projection(
        thread_init,
        thread_store,
        Arc::clone(&barrier),
        "environment-beta",
    );
    barrier.wait().await;
    let (alpha_projection, beta_projection) = tokio::join!(alpha_projection, beta_projection);
    let alpha_projection = alpha_projection.expect("alpha projection task");
    let beta_projection = beta_projection.expect("beta projection task");

    assert_eq!(
        alpha_projection
            .connector_ids()
            .iter()
            .map(|connector_id| connector_id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["connector-alpha"]
    );
    assert_eq!(
        alpha_projection.plugin_display_names_for_connector_id("connector-alpha"),
        &["Alpha Plugin".to_string()]
    );
    assert_eq!(
        beta_projection
            .connector_ids()
            .iter()
            .map(|connector_id| connector_id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["connector-beta"]
    );
    assert_eq!(
        beta_projection.plugin_display_names_for_connector_id("connector-beta"),
        &["Beta Plugin".to_string()]
    );
}

fn spawn_projection(
    thread_init: ExtensionDataInit,
    thread_store: Arc<ExtensionData>,
    barrier: Arc<Barrier>,
    available_environment_id: &str,
) -> tokio::task::JoinHandle<codex_connectors::ConnectorSnapshot> {
    let available_environment_id = available_environment_id.to_string();
    tokio::spawn(async move {
        let config = ();
        let available_environment_ids = [available_environment_id];
        barrier.wait().await;
        selected_plugin_connector_snapshot(McpServerContributionContext::for_step(
            &config,
            &thread_init,
            thread_store.as_ref(),
            &available_environment_ids,
        ))
    })
}

fn selected_root(id: &str, environment_id: &str, path: &Path) -> SelectedCapabilityRoot {
    SelectedCapabilityRoot {
        id: id.to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: environment_id.to_string(),
            path: PathUri::from_host_native_path(path).expect("plugin root path URI"),
        },
    }
}

fn cached_root(
    root: SelectedCapabilityRoot,
    plugin_display_name: &str,
    connector_id: &str,
) -> CachedSelectedRoot {
    CachedSelectedRoot {
        metadata: Some(SelectedPluginMetadata {
            plugin_id: root.id.clone(),
            plugin_display_name: plugin_display_name.to_string(),
            servers: Vec::new(),
            connector_ids: vec![AppConnectorId(connector_id.to_string())],
        }),
        root,
    }
}
