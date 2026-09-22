//! Bounded discovery for permission pickers; fixed request IDs bound abandoned RPCs.
//! This never applies a profile; the app chooses local or server-owned selection.

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::ThreadParamsMode;
use crate::legacy_core::config::Config;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::ApprovalsReviewer;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ConfigRequirements;
use codex_app_server_protocol::ConfigRequirementsReadResponse;
use codex_app_server_protocol::PermissionProfileListParams;
use codex_app_server_protocol::PermissionProfileListResponse;
use codex_app_server_protocol::PermissionProfileSummary;
use codex_app_server_protocol::RequestId;
use codex_utils_approval_presets::builtin_approval_presets;
use std::collections::HashSet;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug)]
pub(crate) struct PermissionDiscovery {
    pub(crate) profiles: Vec<PermissionProfileSummary>,
    pub(crate) requirements: Option<ConfigRequirements>,
    pub(crate) explicit_profile_mode: bool,
}

impl PermissionDiscovery {
    pub(crate) fn local(config: &Config) -> Self {
        let mut profiles = builtin_approval_presets()
            .into_iter()
            .map(|preset| PermissionProfileSummary {
                id: preset.active_permission_profile.id,
                description: None,
                allowed: true,
            })
            .collect::<Vec<_>>();
        profiles.extend(config.custom_permission_profiles.iter().map(|profile| {
            PermissionProfileSummary {
                id: profile.id.clone(),
                description: profile.description.clone(),
                allowed: profile.allowed,
            }
        }));
        Self {
            profiles,
            requirements: None,
            explicit_profile_mode: true,
        }
    }

    pub(crate) fn disabled_reason(
        &self,
        id: &str,
        approval: Option<AskForApproval>,
        reviewer: Option<ApprovalsReviewer>,
    ) -> Option<String> {
        let Some(profile) = self.profiles.iter().find(|profile| profile.id == id) else {
            return Some("Not available on this server".to_string());
        };
        let requirements = self.requirements.as_ref();
        if !profile.allowed
            || requirements
                .and_then(|r| r.allowed_permission_profiles.as_ref())
                .is_some_and(|allowed| allowed.get(id) != Some(&true))
            || requirements
                .and_then(|r| r.allowed_approval_policies.as_ref())
                .is_some_and(|allowed| approval.is_some_and(|value| !allowed.contains(&value)))
            || requirements
                .and_then(|r| r.allowed_approvals_reviewers.as_ref())
                .is_some_and(|allowed| reviewer.is_some_and(|value| !allowed.contains(&value)))
        {
            return Some("Disabled by requirements".to_string());
        }
        None
    }
}

pub(crate) fn fetch(
    app_server: &AppServerSession,
    request_id: Uuid,
    config: &Config,
    thread_cwd: Option<&std::path::Path>,
    tx: AppEventSender,
) {
    let handle = app_server.request_handle();
    let mode = app_server.thread_params_mode();
    let cwd = thread_cwd
        .or_else(|| match mode {
            ThreadParamsMode::Embedded => Some(config.cwd.as_path()),
            ThreadParamsMode::Remote => app_server.remote_cwd_override(),
        })
        .map(|cwd| cwd.to_string_lossy().into_owned());
    // The daemon's catalog cannot see permission definitions from this invocation.
    let local_discovery = (!app_server.uses_embedded_app_server()
        && mode == ThreadParamsMode::Embedded
        && config.config_layer_stack.layers_low_to_high().any(|layer| {
            matches!(layer.name, codex_config::ConfigLayerSource::SessionFlags)
                && layer.config.get("permissions").is_some()
        }))
    .then(|| PermissionDiscovery::local(config));
    let active_custom_profile = config
        .permissions
        .active_permission_profile()
        .is_some_and(|profile| !profile.id.starts_with(':'));
    tokio::spawn(async move {
        let request = async {
            if let Some(discovery) = local_discovery {
                return Ok(discovery);
            }
            if mode == ThreadParamsMode::Remote && !active_custom_profile {
                let config: ConfigReadResponse = handle
                    .request_typed(ClientRequest::ConfigRead {
                        request_id: RequestId::String("tui-permission-config".to_string()),
                        params: ConfigReadParams {
                            include_layers: false,
                            cwd: cwd.clone(),
                        },
                    })
                    .await
                    .map_err(discovery_error)?;
                if !config
                    .config
                    .additional
                    .get("default_permissions")
                    .is_some_and(serde_json::Value::is_string)
                {
                    return Ok(PermissionDiscovery {
                        profiles: Vec::new(),
                        requirements: None,
                        explicit_profile_mode: false,
                    });
                }
            }
            let requirements: ConfigRequirementsReadResponse = handle
                .request_typed(ClientRequest::ConfigRequirementsRead {
                    request_id: RequestId::String("tui-permission-requirements".to_string()),
                    params: None,
                })
                .await
                .map_err(discovery_error)?;
            let mut profiles = Vec::new();
            let mut cursor = None;
            let mut cursors = HashSet::new();
            for _ in 0..10 {
                let response: PermissionProfileListResponse = handle
                    .request_typed(ClientRequest::PermissionProfileList {
                        request_id: RequestId::String("tui-permission-profiles".to_string()),
                        params: PermissionProfileListParams {
                            cursor,
                            limit: Some(100),
                            cwd: cwd.clone(),
                        },
                    })
                    .await
                    .map_err(discovery_error)?;
                if response.data.len() > 100 {
                    break;
                }
                profiles.extend(response.data);
                cursor = response.next_cursor;
                let Some(next) = cursor.as_ref() else {
                    let mut ids = HashSet::new();
                    if !profiles.iter().all(|profile| ids.insert(&profile.id)) {
                        return Err(
                            "The server returned duplicate permission profiles.".to_string()
                        );
                    }
                    return Ok(PermissionDiscovery {
                        profiles,
                        requirements: requirements.requirements,
                        explicit_profile_mode: true,
                    });
                };
                if !cursors.insert(next.clone()) {
                    break;
                }
            }
            Err(
                "Permission discovery exceeded its pagination limit. Try /permissions again."
                    .to_string(),
            )
        };
        let result = tokio::time::timeout(Duration::from_secs(10), request)
            .await
            .unwrap_or_else(|_| {
                Err("Permission discovery timed out. Try /permissions again.".to_string())
            });
        tx.send(AppEvent::PermissionProfilesLoaded { request_id, result });
    });
}

fn discovery_error(error: TypedRequestError) -> String {
    if let TypedRequestError::Server { source, .. } = &error
        && (source.code == -32601
            || (source.code == -32600
                && (source.message.contains("permissionProfile/list")
                    || source.message.contains("configRequirements/read")
                    || source.message.contains("config/read"))))
    {
        return "This server does not support permission discovery. Upgrade the Codex server to use this menu.".to_string();
    }
    format!("Failed to load permissions: {error}")
}

#[cfg(test)]
#[path = "permission_discovery_tests.rs"]
mod tests;
