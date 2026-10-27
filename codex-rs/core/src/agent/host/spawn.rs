//! Capture child startup inputs on the source and start the reserved child at the destination.

use super::invocation::HostToolInvocationLookup;
use super::invocation::ToolInvocationContext;
use super::types::HostThreadSpawnReservation;
use super::types::PreparedChildInheritance;
use super::types::PreparedThreadSpawn;
use crate::agent::control::AgentControlInit;
use crate::agent::control::LocalAgentRuntime;
use crate::agent::control::PendingSpawn;
use crate::agent::control::SpawnOwner;
use crate::agent::control::effective_spawn_config;
use crate::agent::control::fresh_child_history_and_tools;
use crate::agent::control::inherited_exec_policy_for_parent;
use crate::agent::history::prepare_forked_history;
use crate::agent::types::AgentMetadata;
use crate::agent::types::LiveAgent;
use crate::codex_thread::CodexThread;
use crate::exec_policy::ExecPolicyManager;
use crate::thread_manager::StartThreadOptions;
use crate::thread_spawn::ChildSpawnSettings;
use crate::thread_spawn::ThreadSpawnError;
use crate::thread_spawn::ThreadSpawnResult;
use codex_config::McpServerAuth;
use codex_features::Feature;
use codex_history::InitialHistory;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::error::AgentErrorContext;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::ThreadSource;
use codex_thread_store::ListThreadAttachmentsParams;
use std::sync::Arc;

pub(super) enum SpawnDestination {
    SameManager,
    Detached,
}

pub(super) async fn prepare_spawn(
    parent: &CodexThread,
    request: ChildSpawnSettings,
    agent_path: AgentPath,
    destination: SpawnDestination,
) -> Result<PreparedThreadSpawn> {
    if matches!(destination, SpawnDestination::Detached)
        && (request.config.application_network_policy.is_managed()
            || request.config.application_auth_route_config.is_some()
            || request
                .config
                .config_layer_stack
                .cloud_config_binding()
                .is_some())
    {
        return Err(CodexErr::InvalidRequest(
            "prepared spawn requires destination binding for host-managed application authority"
                .into(),
        ));
    }
    let runtime = &parent.session.services.local_agent_runtime;
    let _membership = runtime.admit_start()?;
    let state = runtime.upgrade()?;
    let ChildSpawnSettings {
        config,
        source,
        options,
    } = request;
    if matches!(destination, SpawnDestination::Detached)
        && config.features.enabled(Feature::UseXaa)
        && config.mcp_enterprise_managed_auth.is_some()
        && config.mcp_servers.get().values().any(|server| {
            server.enabled
                && matches!(server.auth, McpServerAuth::EmaAuth)
                && server.validate_ema_auth_transport().is_ok()
        })
    {
        return Err(CodexErr::InvalidRequest(
            "prepared spawn requires destination binding for EMA-authenticated MCP servers".into(),
        ));
    }
    let caller = parent.session.thread_id;
    let instructions = parent.session.inherited_instructions().await;
    if matches!(destination, SpawnDestination::Detached)
        && (instructions.thread_provider.is_some() || instructions.user_provider.is_some())
    {
        return Err(CodexErr::InvalidRequest(
            "prepared spawn requires destination binding for inherited instruction providers"
                .into(),
        ));
    }
    let store = &parent.session.services.thread_store;
    if matches!(destination, SpawnDestination::Detached) && store.supports_thread_attachments() {
        let page = store
            .list_thread_attachments(ListThreadAttachmentsParams {
                thread_id: caller,
                cursor: None,
                limit: 1,
            })
            .await
            .map_err(|error| {
                CodexErr::InvalidRequest(format!("cannot prepare child attachments: {error}"))
            })?;
        if !page.attachments.is_empty() {
            return Err(CodexErr::InvalidRequest(
                "prepared spawn requires a host handoff for thread attachments".into(),
            ));
        }
    }
    let snapshot = options.environments.as_ref().ok_or_else(|| {
        CodexErr::InvalidRequest("prepared spawn requires captured environments".into())
    })?;
    let environments = snapshot.inheritable_requests();
    if matches!(destination, SpawnDestination::Detached)
        && environments
            .iter()
            .any(|environment| matches!(environment.config, EnvironmentConfigState::Pending))
    {
        return Err(CodexErr::InvalidRequest(
            "prepared spawn requires resolved environment configuration".into(),
        ));
    }
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
    let inherited_policy = inherited_exec_policy_for_parent(parent, &config).await;
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
    let exec_policy = match destination {
        SpawnDestination::SameManager => {
            thread_options.inherited_environments = options.environments;
            inherited_policy
        }
        SpawnDestination::Detached => {
            thread_options.environments = Some(environments);
            Some(Arc::new(match inherited_policy {
                Some(policy) => ExecPolicyManager::new(policy.current()),
                None => ExecPolicyManager::load(&thread_options.config.config_layer_stack)
                    .await
                    .map_err(|error| CodexErr::InvalidRequest(error.to_string()))?,
            }))
        }
    };
    Ok(PreparedThreadSpawn {
        caller,
        session_id: parent.session.session_id(),
        agent_path,
        client_metadata: parent.session.app_server_client_metadata().await,
        thread_options,
        inheritance: PreparedChildInheritance {
            instructions,
            exec_policy,
            originator: parent.config_snapshot().await.originator,
            attachment_source: (matches!(destination, SpawnDestination::SameManager)
                && options.fork_mode.is_some())
            .then_some(caller),
        },
    })
}

pub(crate) async fn start_prepared(
    runtime: &LocalAgentRuntime,
    prepared: PreparedThreadSpawn,
    reservation: HostThreadSpawnReservation,
    invocation: Option<&ToolInvocationContext>,
) -> std::result::Result<ThreadSpawnResult, ThreadSpawnError> {
    if reservation.control.identity() != prepared.session_id
        || reservation.thread_id == prepared.caller
        || SessionId::from(reservation.thread_id) == prepared.session_id
    {
        return Err(ThreadSpawnError::Preparation(
            "reservation does not match prepared child".into(),
        ));
    }
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
    let task_name = prepared.agent_path;
    let mut options = prepared.thread_options;
    options.reserved_thread_id = Some(reservation.thread_id);
    let child = state
        .spawn_prepared_child_thread(
            options,
            AgentControlInit::Provided {
                control: reservation.control,
                runtime: runtime.clone(),
            },
            prepared.caller,
            prepared.inheritance,
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
        .emit_subagent_session_started(prepared.client_metadata, /*resumed_created_at*/ None)
        .await;
    if child.session_configured.forked_from_id.is_some() {
        child.thread.ensure_rollout_materialized().await;
    }
    let config = effective_spawn_config(&child.thread).await?;
    let status = child.thread.io.agent_status.borrow().clone();
    let _active = invocation
        .map(ToolInvocationContext::lock_active)
        .transpose()?;
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
