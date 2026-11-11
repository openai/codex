//! Run isolated structured requests through existing app-server methods.

use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxMode;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItemPayload;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_READ_ONLY;
use codex_protocol::openai_models::ReasoningEffort;
use color_eyre::eyre::eyre;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const STRUCTURED_TURN_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 30);
const STRUCTURED_RESPONSE_MAX_BYTES: usize = 8 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TemporaryThreadKind {
    Title,
    Recap,
}

/// Settings for an isolated structured request using the visible thread's provider.
pub(crate) struct TemporaryStructuredThreadOptions {
    pub(crate) kind: TemporaryThreadKind,
    pub(crate) model: String,
    pub(crate) model_provider: String,
    pub(crate) cwd: String,
    pub(crate) active_permission_profile: Option<String>,
}

/// Start an ephemeral thread without widening permissions or exposing tools and environment access.
///
/// Structured prompts can contain untrusted transcript text, so the effective app-server config is
/// read first and every MCP server is explicitly disabled alongside built-in and extension tools.
/// As on desktop, title threads request read-only permissions and the `never` approval policy.
pub(crate) async fn start_temporary_thread(
    request_handle: &AppServerRequestHandle,
    options: TemporaryStructuredThreadOptions,
) -> color_eyre::Result<ThreadStartResponse> {
    let TemporaryStructuredThreadOptions {
        kind,
        model,
        model_provider,
        cwd,
        active_permission_profile,
    } = options;
    let (source, permissions, approval_policy) = match kind {
        TemporaryThreadKind::Title => (
            "thread_title",
            Some(BUILT_IN_PERMISSION_PROFILE_READ_ONLY.to_string()),
            Some(AskForApproval::Never),
        ),
        TemporaryThreadKind::Recap => (
            "system",
            active_permission_profile.filter(|profile| !profile.starts_with(':')),
            None,
        ),
    };
    let mut config = std::collections::HashMap::from([
        ("features.apps".to_string(), false.into()),
        ("features.code_mode".to_string(), false.into()),
        ("features.code_mode_only".to_string(), false.into()),
        ("features.context_management".to_string(), false.into()),
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
        ("features.token_budget".to_string(), false.into()),
        ("features.tool_suggest".to_string(), false.into()),
        ("features.unified_exec".to_string(), false.into()),
        ("features.view_image".to_string(), false.into()),
        ("cloud.skills.enabled".to_string(), false.into()),
        ("skills.include_instructions".to_string(), false.into()),
        (
            "tools.experimental_request_user_input.enabled".to_string(),
            false.into(),
        ),
        ("tools.update_plan.enabled".to_string(), false.into()),
        ("web_search".to_string(), "disabled".into()),
    ]);
    if permissions.is_none() {
        // Managed profiles take precedence over the legacy sandbox override below.
        config.insert(
            "default_permissions".to_string(),
            BUILT_IN_PERMISSION_PROFILE_READ_ONLY.into(),
        );
    }
    let response: ThreadStartResponse = tokio::time::timeout(STRUCTURED_TURN_TIMEOUT, async {
        // Fail closed if the remote-effective MCP configuration cannot be read.
        let mut servers =
            crate::config_update::read_mcp_servers(request_handle, cwd.clone()).await?;
        for server in servers.values_mut() {
            *server = serde_json::json!({ "enabled": false });
        }
        config.insert("mcp_servers".to_string(), Value::Object(servers));

        request_handle
            .request_typed(ClientRequest::ThreadStart {
                request_id: RequestId::String(format!("temporary-structured-{}", Uuid::new_v4())),
                params: ThreadStartParams {
                    model: Some(model),
                    model_provider: Some(model_provider),
                    cwd: Some(cwd),
                    approval_policy,
                    sandbox: permissions.is_none().then_some(SandboxMode::ReadOnly),
                    permissions: permissions.clone(),
                    runtime_workspace_roots: Some(Vec::new()),
                    ephemeral: Some(true),
                    thread_source: Some(ThreadSource::Feature(source.to_string())),
                    environments: Some(Vec::new()),
                    dynamic_tools: Some(Vec::new()),
                    selected_capability_roots: Some(Vec::new()),
                    config: Some(config),
                    ..ThreadStartParams::default()
                },
            })
            .await
            .map_err(color_eyre::Report::from)
    })
    .await
    .map_err(|_| eyre!("temporary structured thread start timed out"))??;

    if kind == TemporaryThreadKind::Recap
        && let Some(expected_profile) = permissions
    {
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
        unsubscribe_temporary_thread(request_handle, response.thread.id).await;
        return Err(eyre!(
            "temporary structured thread did not start with read-only permissions"
        ));
    }

    Ok(response)
}

impl TemporaryThreadKind {
    pub(crate) fn turn_params(
        self,
        thread_id: String,
        prompt: String,
        output_schema: Value,
        effort: Option<ReasoningEffort>,
    ) -> TurnStartParams {
        let is_title = self == Self::Title;
        TurnStartParams {
            thread_id,
            input: vec![UserInput::Text {
                text: prompt,
                text_elements: Vec::new(),
            }],
            permissions: is_title.then(|| BUILT_IN_PERMISSION_PROFILE_READ_ONLY.to_string()),
            runtime_workspace_roots: is_title.then(Vec::new),
            output_schema: Some(output_schema),
            effort,
            ..Default::default()
        }
    }
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
                if let ThreadItemPayload::AgentMessage { text, .. } = completed.item.payload {
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

/// Make a bounded best-effort attempt to detach an ephemeral thread.
pub(crate) async fn unsubscribe_temporary_thread(
    request_handle: &AppServerRequestHandle,
    thread_id: String,
) {
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
}

/// Run a bounded structured turn, interrupt it on cancellation, and detach the temporary thread.
pub(crate) async fn run_temporary_structured_turn(
    request_handle: AppServerRequestHandle,
    params: TurnStartParams,
    notifications: UnboundedReceiver<ServerNotification>,
    cancellation: CancellationToken,
) -> color_eyre::Result<String> {
    let thread_id = params.thread_id.clone();
    let mut turn_id = None;
    let result = tokio::time::timeout(STRUCTURED_TURN_TIMEOUT, async {
        if cancellation.is_cancelled() {
            return Err(eyre!("temporary structured turn cancelled"));
        }
        // Wait for turn/start to return its ID before interrupting, even if cancellation
        // arrives during startup. Dropping that request could leave an orphaned turn.
        let turn: TurnStartResponse = request_handle
            .request_typed(ClientRequest::TurnStart {
                request_id: RequestId::String(format!(
                    "temporary-structured-turn-{}",
                    Uuid::new_v4()
                )),
                params,
            })
            .await?;

        turn_id = Some(turn.turn.id.clone());
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(eyre!("temporary structured turn cancelled")),
            result = collect_structured_response(notifications, &turn.turn.id) => result,
        }
    })
    .await;
    let timed_out = result.is_err();
    let result = result.unwrap_or_else(|_| Err(eyre!("temporary structured turn timed out")));

    // Give interruption its own deadline so cancellation near the response timeout
    // still gets a chance to stop the hidden turn.
    if (timed_out || cancellation.is_cancelled())
        && let Some(turn_id) = turn_id
    {
        let interrupt =
            request_handle.request_typed::<TurnInterruptResponse>(ClientRequest::TurnInterrupt {
                request_id: RequestId::String(format!(
                    "temporary-structured-interrupt-{}",
                    Uuid::new_v4()
                )),
                params: TurnInterruptParams {
                    thread_id: thread_id.clone(),
                    turn_id,
                },
            });
        match tokio::time::timeout(STRUCTURED_TURN_TIMEOUT, interrupt).await {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => {
                tracing::debug!(%error, "failed to interrupt temporary structured turn")
            }
            Err(_) => tracing::debug!("temporary structured turn interrupt timed out"),
        }
    }
    unsubscribe_temporary_thread(&request_handle, thread_id).await;

    result
}

#[cfg(test)]
#[path = "temporary_structured_request_tests.rs"]
mod tests;
