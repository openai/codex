use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_core::ConversationManager;
use codex_core::CrossSessionSpawnParams;
use codex_core::config::Config;
use codex_core::cross_session::AssistantMessage;
use codex_core::cross_session::CrossSessionError;
use codex_core::cross_session::CrossSessionHub;
use codex_core::cross_session::PostUserTurnRequest;
use codex_core::cross_session::RoleOrId;
use codex_core::cross_session::TurnHandle;
use serde_json::Value;
use tokio::time::Instant;
use tokio_stream::StreamExt as _;

use crate::progress::ProgressReporter;
use crate::types::RoleConfig;
use crate::types::RoleSession;

pub async fn spawn_role(
    hub: Arc<CrossSessionHub>,
    manager: &ConversationManager,
    run_id: &str,
    run_path: &Path,
    role_config: RoleConfig,
    ensure_instructions: impl FnOnce(&str, &mut Config),
) -> Result<RoleSession> {
    let RoleConfig {
        role, mut config, ..
    } = role_config;
    config.cwd = run_path.to_path_buf();
    ensure_instructions(&role, &mut config);
    let cfg_for_session = config.clone();
    let session = manager
        .new_conversation_with_cross_session(
            cfg_for_session,
            CrossSessionSpawnParams {
                hub: Arc::clone(&hub),
                run_id: Some(run_id.to_string()),
                role: Some(role.clone()),
            },
        )
        .await?;
    // Note: include the final config used to spawn the session
    Ok(RoleSession::from_new(role, session, config))
}

// resumable runs are disabled for now; resume_role removed

pub async fn post_turn(
    hub: &CrossSessionHub,
    run_id: &str,
    role: &str,
    text: impl Into<String>,
    final_output_json_schema: Option<Value>,
) -> Result<TurnHandle, CrossSessionError> {
    hub.post_user_turn(PostUserTurnRequest {
        target: RoleOrId::RunRole {
            run_id: run_id.to_string(),
            role: role.to_string(),
        },
        text: text.into(),
        final_output_json_schema,
    })
    .await
}

pub async fn await_first_idle(
    hub: &CrossSessionHub,
    handle: &TurnHandle,
    idle_timeout: Duration,
    progress: Option<(&dyn ProgressReporter, &str)>,
) -> Result<AssistantMessage> {
    let mut events = hub.stream_events(handle.conversation_id())?;
    let wait_first = hub.await_first_assistant(handle, idle_timeout);
    tokio::pin!(wait_first);

    let idle = tokio::time::sleep(idle_timeout);
    tokio::pin!(idle);

    let submission_id = handle.submission_id().to_string();

    loop {
        tokio::select! {
            result = &mut wait_first => {
                return result.map_err(|err| anyhow!(err));
            }
            maybe_event = events.next() => {
                let Some(event) = maybe_event else {
                    bail!(CrossSessionError::SessionClosed);
                };
                if event.event.id == submission_id {
                    if let Some((reporter, role)) = progress {
                        reporter.role_event(role, &event.event.msg);
                    }
                    if let codex_core::protocol::EventMsg::Error(err) = &event.event.msg {
                        bail!(anyhow!(err.message.clone()));
                    }
                    idle.as_mut().reset(Instant::now() + idle_timeout);
                }
            }
            _ = &mut idle => {
                bail!(CrossSessionError::AwaitTimeout(idle_timeout));
            }
        }
    }
}
