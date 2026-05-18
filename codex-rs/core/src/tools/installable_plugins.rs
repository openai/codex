use std::sync::Weak;

use codex_plugins_extension::InstallablePluginsProvider;
use codex_tools::DiscoverableTool;
use codex_tools::RequestPluginInstallEntry;
use codex_tools::collect_request_plugin_install_entries;
use codex_tools::filter_request_plugin_install_discoverable_tools_for_client;

use crate::config::Config;
use crate::connectors;
use crate::session::session::Session;

pub(crate) struct SessionInstallablePluginsProvider {
    session: Weak<Session>,
}

impl SessionInstallablePluginsProvider {
    pub(crate) fn new(session: Weak<Session>) -> Self {
        Self { session }
    }
}

#[async_trait::async_trait]
impl InstallablePluginsProvider for SessionInstallablePluginsProvider {
    async fn list_installable_plugins(&self) -> Result<Vec<RequestPluginInstallEntry>, String> {
        let session = self
            .session
            .upgrade()
            .ok_or_else(|| "plugin install requests are unavailable right now".to_string())?;
        let config = session.get_config().await;
        let app_server_client_metadata = session.app_server_client_metadata().await;
        let discoverable_tools = discoverable_request_plugin_install_tools(
            session.as_ref(),
            config.as_ref(),
            app_server_client_metadata.client_name.as_deref(),
        )
        .await
        .map_err(|err| format!("plugin install requests are unavailable right now: {err}"))?;

        Ok(collect_request_plugin_install_entries(&discoverable_tools))
    }
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "plugin install discovery reads through the session-owned manager guard"
)]
pub(crate) async fn discoverable_request_plugin_install_tools(
    session: &Session,
    config: &Config,
    app_server_client_name: Option<&str>,
) -> anyhow::Result<Vec<DiscoverableTool>> {
    let auth = session.services.auth_manager.auth().await;
    let manager = session.services.mcp_connection_manager.read().await;
    let mcp_tools = manager.list_all_tools().await;
    drop(manager);

    let accessible_connectors = connectors::with_app_enabled_state(
        connectors::accessible_connectors_from_mcp_tools(&mcp_tools),
        config,
    );
    connectors::list_tool_suggest_discoverable_tools_with_auth(
        config,
        auth.as_ref(),
        &accessible_connectors,
        session.services.plugins_manager.as_ref(),
    )
    .await
    .map(|discoverable_tools| {
        filter_request_plugin_install_discoverable_tools_for_client(
            discoverable_tools,
            app_server_client_name,
        )
    })
}
