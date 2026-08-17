//! Installed connector metadata used by the TUI `$` mention picker.

use anyhow::Context;
use anyhow::Result;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::AppsInstalledParams;
use codex_app_server_protocol::AppsInstalledResponse;
use codex_app_server_protocol::AppsReadParams;
use codex_app_server_protocol::AppsReadResponse;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_connectors::AppInfo;
use std::collections::HashMap;
use uuid::Uuid;

use super::App;
use super::background_requests::fetch_connectors_list;
use crate::app_event::AppEvent;
use crate::app_event::ConnectorsSnapshot;
use crate::app_server_session::AppServerSession;
use crate::chatwidget::ConnectorScopeGeneration;

const APP_READ_BATCH_SIZE: usize = 100;

impl App {
    /// Fetch installed app mentions in the background and preserve their owning session scope.
    pub(super) fn fetch_installed_connector_mentions(
        &mut self,
        app_server: &AppServerSession,
        force_refresh: bool,
        generation: ConnectorScopeGeneration,
    ) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        let thread_id = self.current_displayed_thread_id();
        let cwd = self.chat_widget.config_ref().cwd.to_path_buf();
        tokio::spawn(async move {
            let result = fetch_installed_connector_mentions(
                request_handle,
                thread_id.map(|thread_id| thread_id.to_string()),
                force_refresh,
            )
            .await
            .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::InstalledConnectorMentionsLoaded {
                thread_id,
                cwd,
                generation,
                result,
            });
        });
    }
}

/// Load installed app identities and enrich callable apps with bounded metadata lookups.
///
/// Older app servers without `app/installed` fall back to their thread-scoped app directory.
pub(super) async fn fetch_installed_connector_mentions(
    request_handle: AppServerRequestHandle,
    thread_id: Option<String>,
    force_refresh: bool,
) -> Result<ConnectorsSnapshot> {
    let response: AppsInstalledResponse = match request_handle
        .request_typed(ClientRequest::AppsInstalled {
            request_id: RequestId::String(format!("apps-installed-{}", Uuid::new_v4())),
            params: AppsInstalledParams {
                thread_id: thread_id.clone(),
                force_refresh,
            },
        })
        .await
    {
        Ok(response) => response,
        Err(TypedRequestError::Server { source, .. })
            if source.code == -32601
                || source.code == -32600
                    && source.message.contains("app/installed")
                    && (source.message.contains("unknown variant")
                        || source.message.contains("unknown method")) =>
        {
            let mut snapshot = fetch_connectors_list(request_handle, force_refresh, thread_id)
                .await
                .map_err(|err| anyhow::anyhow!("{err:#}"))?;
            snapshot.connectors.retain(|app| app.is_accessible);
            return Ok(snapshot);
        }
        Err(err) => return Err(err).context("app/installed failed while loading app mentions"),
    };
    let callable_apps: Vec<_> = response.apps.iter().filter(|app| app.callable).collect();

    let mut metadata_by_id = HashMap::new();
    for apps in callable_apps.chunks(APP_READ_BATCH_SIZE) {
        match request_handle
            .request_typed::<AppsReadResponse>(ClientRequest::AppsRead {
                request_id: RequestId::String(format!("apps-read-{}", Uuid::new_v4())),
                params: AppsReadParams {
                    app_ids: apps.iter().map(|app| app.id.clone()).collect(),
                    thread_id: thread_id.clone(),
                    include_tools: false,
                },
            })
            .await
        {
            Ok(response) => {
                metadata_by_id.extend(response.apps.into_iter().map(|app| (app.id.clone(), app)))
            }
            Err(err) => tracing::warn!(
                error = %err,
                "app/read failed while loading TUI connector mentions; using runtime metadata"
            ),
        }
    }

    let mut connectors = response
        .apps
        .into_iter()
        .map(|app| {
            let mut connector = AppInfo {
                name: app.runtime_name.unwrap_or_else(|| app.id.clone()),
                id: app.id,
                description: None,
                logo_url: None,
                logo_url_dark: None,
                icon_assets: None,
                icon_dark_assets: None,
                distribution_channel: None,
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: None,
                is_accessible: true,
                is_enabled: app.callable,
                plugin_display_names: Vec::new(),
            };
            if let Some(metadata) = metadata_by_id.remove(connector.id.as_str()) {
                connector.name = metadata.name;
                connector.description = metadata.description;
                connector.logo_url = metadata.icon_url;
                connector.logo_url_dark = metadata.icon_url_dark;
                connector.distribution_channel = metadata.distribution_channel;
                connector.install_url = metadata.install_url;
                connector.plugin_display_names = metadata.plugin_display_names;
            }
            connector
        })
        .collect::<Vec<_>>();
    connectors.sort_by(|left, right| (&left.name, &left.id).cmp(&(&right.name, &right.id)));

    Ok(ConnectorsSnapshot { connectors })
}

#[cfg(test)]
#[path = "connector_mentions_tests.rs"]
mod tests;
