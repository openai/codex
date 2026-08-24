//! Run isolated structured requests through existing app-server methods.

use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxMode;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_protocol::openai_models::ReasoningEffort;
use color_eyre::eyre::eyre;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

const STRUCTURED_TURN_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 30);
const STRUCTURED_RESPONSE_MAX_BYTES: usize = 8 * 1024;

/// Preserve the visible thread's provider, permissions, and external-tool isolation.
pub(crate) struct TemporaryStructuredThreadOptions {
    pub(crate) model: String,
    pub(crate) model_provider: String,
    pub(crate) cwd: String,
    pub(crate) active_permission_profile: Option<String>,
    pub(crate) mcp_server_names: Vec<String>,
}

/// Start an ephemeral, non-interactive thread without widening its parent's permissions.
pub(crate) async fn start_temporary_thread(
    request_handle: &AppServerRequestHandle,
    options: TemporaryStructuredThreadOptions,
) -> color_eyre::Result<ThreadStartResponse> {
    let TemporaryStructuredThreadOptions {
        model,
        model_provider,
        cwd,
        active_permission_profile,
        mcp_server_names,
    } = options;
    let custom_permission_profile =
        active_permission_profile.filter(|profile| !profile.starts_with(':'));
    let mut config = std::collections::HashMap::from([
        ("features.apps".to_string(), false.into()),
        ("features.code_mode".to_string(), false.into()),
        ("features.code_mode_only".to_string(), false.into()),
        ("features.current_time_reminder".to_string(), false.into()),
        ("features.deferred_executor".to_string(), false.into()),
        ("features.enable_fanout".to_string(), false.into()),
        ("features.goals".to_string(), false.into()),
        ("features.hooks".to_string(), false.into()),
        ("features.image_generation".to_string(), false.into()),
        ("features.memories".to_string(), false.into()),
        ("features.multi_agent".to_string(), false.into()),
        ("features.multi_agent_v2".to_string(), false.into()),
        ("features.plugins".to_string(), false.into()),
        (
            "features.request_permissions_tool".to_string(),
            false.into(),
        ),
        ("features.shell_snapshot".to_string(), false.into()),
        ("features.shell_tool".to_string(), false.into()),
        ("features.standalone_web_search".to_string(), false.into()),
        ("features.tool_suggest".to_string(), false.into()),
        ("features.unified_exec".to_string(), false.into()),
        ("features.view_image".to_string(), false.into()),
        (
            "tools.experimental_request_user_input.enabled".to_string(),
            false.into(),
        ),
        ("tools.update_plan.enabled".to_string(), false.into()),
        ("web_search".to_string(), "disabled".into()),
    ]);
    for server_name in mcp_server_names {
        config.insert(format!("mcp_servers.{server_name}.enabled"), false.into());
    }

    let response: ThreadStartResponse = tokio::time::timeout(
        STRUCTURED_TURN_TIMEOUT,
        request_handle.request_typed(ClientRequest::ThreadStart {
            request_id: RequestId::String(format!("temporary-structured-{}", Uuid::new_v4())),
            params: ThreadStartParams {
                model: Some(model),
                model_provider: Some(model_provider),
                cwd: Some(cwd),
                approval_policy: Some(AskForApproval::Never),
                sandbox: custom_permission_profile
                    .is_none()
                    .then_some(SandboxMode::ReadOnly),
                permissions: custom_permission_profile.clone(),
                runtime_workspace_roots: Some(Vec::new()),
                ephemeral: Some(true),
                thread_source: Some(ThreadSource::Feature("system".to_string())),
                environments: Some(Vec::new()),
                dynamic_tools: Some(Vec::new()),
                selected_capability_roots: Some(Vec::new()),
                config: Some(config),
                ..ThreadStartParams::default()
            },
        }),
    )
    .await
    .map_err(|_| eyre!("temporary structured thread start timed out"))??;

    if let Some(expected_profile) = custom_permission_profile {
        if response
            .active_permission_profile
            .as_ref()
            .is_none_or(|profile| profile.id != expected_profile)
        {
            return Err(eyre!(
                "temporary structured thread did not preserve permission profile {expected_profile}"
            ));
        }
    } else if !matches!(response.sandbox, SandboxPolicy::ReadOnly { .. }) {
        return Err(eyre!(
            "temporary structured thread did not start with read-only permissions"
        ));
    }

    Ok(response)
}

/// Submit a structured turn using the temporary thread's existing settings.
pub(crate) async fn start_structured_turn(
    request_handle: &AppServerRequestHandle,
    thread_id: String,
    prompt: String,
    output_schema: Value,
    effort: Option<ReasoningEffort>,
) -> Result<TurnStartResponse, TypedRequestError> {
    request_handle
        .request_typed(ClientRequest::TurnStart {
            request_id: RequestId::String(format!("temporary-structured-turn-{}", Uuid::new_v4())),
            params: TurnStartParams {
                thread_id,
                input: vec![UserInput::Text {
                    text: prompt,
                    text_elements: Vec::new(),
                }],
                output_schema: Some(output_schema),
                effort,
                ..TurnStartParams::default()
            },
        })
        .await
}

/// Return the latest assistant message when the requested turn completes.
pub(crate) async fn collect_structured_response(
    mut notifications: UnboundedReceiver<ServerNotification>,
    turn_id: &str,
) -> color_eyre::Result<String> {
    let mut response = None;

    while let Some(notification) = notifications.recv().await {
        match notification {
            ServerNotification::ItemCompleted(completed) if completed.turn_id == turn_id => {
                if let ThreadItem::AgentMessage { text, .. } = completed.item {
                    if text.len() > STRUCTURED_RESPONSE_MAX_BYTES {
                        return Err(eyre!(
                            "temporary structured response exceeds {STRUCTURED_RESPONSE_MAX_BYTES} bytes"
                        ));
                    }
                    response = Some(text);
                }
            }
            ServerNotification::TurnCompleted(completed) if completed.turn.id == turn_id => {
                if completed.turn.status != TurnStatus::Completed {
                    return Err(eyre!(
                        "temporary structured turn ended with status {:?}",
                        completed.turn.status,
                    ));
                }

                return response.ok_or_else(|| {
                    eyre!("temporary structured turn completed without a response")
                });
            }
            _ => {}
        }
    }

    Err(eyre!(
        "temporary structured turn notification channel closed"
    ))
}

/// Run a bounded structured turn and make a bounded temporary-thread cleanup attempt.
pub(crate) async fn run_temporary_structured_turn(
    request_handle: AppServerRequestHandle,
    thread_id: String,
    prompt: String,
    output_schema: Value,
    effort: Option<ReasoningEffort>,
    notifications: UnboundedReceiver<ServerNotification>,
) -> color_eyre::Result<String> {
    let result = tokio::time::timeout(STRUCTURED_TURN_TIMEOUT, async {
        let turn = start_structured_turn(
            &request_handle,
            thread_id.clone(),
            prompt,
            output_schema,
            effort,
        )
        .await?;

        collect_structured_response(notifications, &turn.turn.id).await
    })
    .await
    .unwrap_or_else(|_| Err(eyre!("temporary structured turn timed out")));

    match tokio::time::timeout(
        STRUCTURED_TURN_TIMEOUT,
        request_handle.request_typed::<ThreadUnsubscribeResponse>(
            ClientRequest::ThreadUnsubscribe {
                request_id: RequestId::String(format!(
                    "temporary-structured-unsubscribe-{}",
                    Uuid::new_v4()
                )),
                params: ThreadUnsubscribeParams { thread_id },
            },
        ),
    )
    .await
    {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => {
            tracing::debug!(%error, "failed to unsubscribe from temporary structured thread");
        }
        Err(_) => {
            tracing::debug!("temporary structured thread unsubscribe timed out");
        }
    }

    result
}

#[cfg(test)]
#[path = "temporary_structured_request_tests.rs"]
mod tests;
