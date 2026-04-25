use std::collections::HashSet;
use std::sync::Arc;

use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::AppListUpdatedNotification;
use codex_app_server_protocol::AppsListResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ServerNotification;
use codex_chatgpt::connectors;

use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::outgoing_message::OutgoingMessageSender;

pub(super) fn merge_loaded_apps(
    all_connectors: Option<&[AppInfo]>,
    accessible_connectors: Option<&[AppInfo]>,
    plugin_declared_connector_ids: &HashSet<String>,
) -> Vec<AppInfo> {
    let all_connectors_loaded = all_connectors.is_some();
    let all = all_connectors.map_or_else(Vec::new, <[AppInfo]>::to_vec);
    let accessible = accessible_connectors.map_or_else(Vec::new, <[AppInfo]>::to_vec);
    connectors::merge_connectors_with_accessible(
        all,
        accessible,
        all_connectors_loaded,
        plugin_declared_connector_ids,
    )
}

pub(super) fn should_send_app_list_updated_notification(
    connectors: &[AppInfo],
    accessible_loaded: bool,
    all_loaded: bool,
) -> bool {
    connectors.iter().any(|connector| connector.is_accessible) || (accessible_loaded && all_loaded)
}

pub(super) fn paginate_apps(
    connectors: &[AppInfo],
    start: usize,
    limit: Option<u32>,
) -> Result<AppsListResponse, JSONRPCErrorError> {
    let total = connectors.len();
    if start > total {
        return Err(JSONRPCErrorError {
            code: INVALID_REQUEST_ERROR_CODE,
            message: format!("cursor {start} exceeds total apps {total}"),
            data: None,
        });
    }

    let effective_limit = limit.unwrap_or(total as u32).max(1) as usize;
    let end = start.saturating_add(effective_limit).min(total);
    let data = connectors[start..end].to_vec();
    let next_cursor = if end < total {
        Some(end.to_string())
    } else {
        None
    };

    Ok(AppsListResponse { data, next_cursor })
}

pub(super) async fn send_app_list_updated_notification(
    outgoing: &Arc<OutgoingMessageSender>,
    data: Vec<AppInfo>,
) {
    outgoing
        .send_server_notification(ServerNotification::AppListUpdated(
            AppListUpdatedNotification { data },
        ))
        .await;
}
