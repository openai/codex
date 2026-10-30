//! Create a host-reserved child on the invoking thread's manager.

use super::invocation::HostToolInvocationLookup;
use super::invocation::ToolInvocationContext;
use super::types::HostThreadSpawnReservation;
use super::types::PreparedChildInheritance;
use crate::agent::control::AgentControlInit;
use crate::agent::control::PendingSpawn;
use crate::agent::control::SpawnOwner;
use crate::agent::control::effective_spawn_config;
use crate::agent::control::fresh_child_history_and_tools;
use crate::agent::control::inherited_exec_policy_for_parent;
use crate::agent::history::prepare_forked_history;
use crate::agent::types::AgentMetadata;
use crate::agent::types::LiveAgent;
use crate::codex_thread::CodexThread;
use crate::session::session::AppServerClientMetadata;
use crate::thread_manager::StartThreadOptions;
use crate::thread_spawn::ChildSpawnSettings;
use crate::thread_spawn::ThreadSpawnError;
use crate::thread_spawn::ThreadSpawnResult;
use codex_history::InitialHistory;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::error::AgentErrorContext;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::ThreadSource;
use std::sync::Arc;

async fn prepare_spawn_inputs(
    parent: &CodexThread,
    request: ChildSpawnSettings,
) -> Result<(
    StartThreadOptions,
    PreparedChildInheritance,
    AppServerClientMetadata,
)> {
    let runtime = &parent.session.services.local_agent_runtime;
    let _membership = runtime.admit_start()?;
    let state = runtime.upgrade()?;
    let ChildSpawnSettings {
        config,
        source,
        options,
    } = request;
    let caller = parent.session.thread_id;
    let instructions = parent.session.inherited_instructions().await;
    let snapshot = options.environments.as_ref().ok_or_else(|| {
        CodexErr::InvalidRequest("prepared spawn requires captured environments".into())
    })?;
    let (initial_history, history_mode, dynamic_tools) = if options.fork_mode.is_some() {
        let (history, mode) =
            prepare_forked_history(&state, &source, &options, &config, MultiAgentVersion::V2)
                .await
                .map_err(|error| error.with_agent_context(AgentErrorContext::ForkHistory))?;
        (history, mode, Vec::new())
    } else {
        let (mode, tools) =
            fresh_child_history_and_tools(parent, &config, MultiAgentVersion::V2).await;
        (InitialHistory::New, mode, tools)
    };
    if !std::ptr::eq(state.get_thread(caller).await?.as_ref(), parent) {
        return Err(CodexErr::ThreadNotFound(caller));
    }
    let exec_policy = inherited_exec_policy_for_parent(parent, &config).await;
    let mut thread_options = StartThreadOptions {
        session_source: Some(source),
        thread_source: Some(ThreadSource::Subagent),
        initial_history,
        history_mode,
        dynamic_tools,
        disabled_plugin_ids: options.disabled_plugin_ids,
        client_mcp_extensions: parent.session.services.client_mcp_extensions.clone(),
        prompt_cache_key: parent
            .session
            .services
            .model_client
            .prompt_cache_key_policy(),
        ..StartThreadOptions::new(config)
    };
    thread_options
        .thread_extension_init
        .insert(snapshot.selected_capability_roots());
    thread_options
        .thread_extension_init
        .insert(HostToolInvocationLookup);
    thread_options.inherited_environments = options.environments;
    let client_metadata = parent.session.app_server_client_metadata().await;
    let inheritance = PreparedChildInheritance {
        instructions,
        exec_policy,
        originator: parent.session.originator().await,
        attachment_source: options.fork_mode.is_some().then_some(caller),
    };
    Ok((thread_options, inheritance, client_metadata))
}

pub(super) async fn create_spawn(
    parent: &CodexThread,
    invocation: &ToolInvocationContext,
    request: ChildSpawnSettings,
    task_name: AgentPath,
    reservation: HostThreadSpawnReservation,
) -> std::result::Result<ThreadSpawnResult, ThreadSpawnError> {
    let (mut options, inheritance, client_metadata) =
        Box::pin(prepare_spawn_inputs(parent, request)).await?;
    let caller = parent.session.thread_id;
    let session_id = parent.session.session_id();
    if reservation.control.identity() != session_id
        || reservation.thread_id == caller
        || SessionId::from(reservation.thread_id) == session_id
    {
        return Err(ThreadSpawnError::Preparation(
            "reservation does not match prepared child".into(),
        ));
    }
    let runtime = &parent.session.services.local_agent_runtime;
    let membership = runtime.admit_start()?;
    let state = runtime.upgrade()?;
    if state
        .threads
        .read()
        .await
        .contains_key(&reservation.thread_id)
    {
        return Err(CodexErr::InvalidRequest(format!(
            "thread {} is already running",
            reservation.thread_id,
        ))
        .into());
    }
    options.reserved_thread_id = Some(reservation.thread_id);
    let child = state
        .spawn_host_child_thread(
            options,
            AgentControlInit::Provided {
                control: reservation.control,
                runtime: runtime.clone(),
            },
            caller,
            inheritance,
        )
        .await
        .map_err(|error| error.with_agent_context(AgentErrorContext::ChildStartup))?;
    let pending = PendingSpawn::new(
        Arc::clone(&state),
        child.thread_id,
        membership,
        SpawnOwner::Host,
    );
    child.thread.emit_thread_ready_lifecycle().await;
    child
        .thread
        .emit_subagent_session_started(client_metadata, /*resumed_created_at*/ None)
        .await;
    if child.session_configured.forked_from_id.is_some() {
        child.thread.ensure_rollout_materialized().await;
    }
    let config = effective_spawn_config(&child.thread).await?;
    let status = child.thread.io.agent_status.borrow().clone();
    let _active = invocation.lock_active()?;
    let _membership = pending.disarm();
    state.notify_thread_created(child.thread_id);
    Ok(ThreadSpawnResult {
        agent: LiveAgent {
            thread_id: child.thread_id,
            metadata: AgentMetadata {
                agent_id: Some(child.thread_id),
                agent_path: Some(task_name.clone()),
                agent_nickname: child.thread.session_source.get_nickname(),
                agent_role: child.thread.session_source.get_agent_role(),
            },
            status,
        },
        config,
        task_name,
    })
}

#[cfg(test)]
#[path = "spawn_tests.rs"]
mod tests;
