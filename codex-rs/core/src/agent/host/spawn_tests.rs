#![cfg(test)]

//! Tests same-manager child bindings and invocation lifetime.

use crate::CodexThread;
use crate::ConfigRefreshOutcome;
use crate::HostThreadSpawnReservation;
use crate::HostToolInvocationLookup;
use crate::ThreadManager;
use crate::agent::host::InvocationReservation;
use crate::config::ConfigBuilder;
use crate::thread_manager::StartThreadOptions;
use crate::thread_spawn::ThreadCreateRequest;
use crate::thread_spawn::ThreadSpawnError;
use crate::tools::context::ToolCallSource;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use codex_config::ScopedSkillsConfig;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_exec_server::ExecServerError;
use codex_exec_server::RemoteEnvironmentOptions;
use codex_execpolicy::Decision;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::ThreadId;
use codex_protocol::approvals::ExecPolicyAmendment;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_tools::ToolName;
use pretty_assertions::assert_eq;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

async fn spawn_call(parent: &CodexThread) -> anyhow::Result<ToolInvocation> {
    parent
        .thread_extension_data()
        .insert(HostToolInvocationLookup);
    let session = &parent.session;
    let step = session
        .capture_step_context(session.new_default_turn().await, &CancellationToken::new())
        .await?;
    Ok(ToolInvocation {
        session: Arc::clone(session),
        turn: Arc::clone(&step.turn),
        step_context: step,
        cancellation_token: CancellationToken::new(),
        tracker: Default::default(),
        call_id: "spawn-call".into(),
        tool_name: ToolName::plain("host_spawn"),
        source: ToolCallSource::Direct,
        payload: ToolPayload::Function {
            arguments: "{}".into(),
        },
        native_call: None,
        environments: Default::default(),
    })
}

#[tokio::test]
async fn prepare_child_retains_loaded_exec_policy_requirements_after_mcp_refresh()
-> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let builder =
        || ConfigBuilder::without_managed_config_for_tests().codex_home(home.path().to_path_buf());
    let mut config = builder()
        .cloud_config_bundle(CloudConfigBundleFixture::loader_with_enterprise_requirement(
            "[[rules.prefix_rules]]\npattern = [{ token = 'managed-command' }]\ndecision = 'forbidden'",
        ))
        .build()
        .await?;
    config.features.enable(Feature::MultiAgentV2)?;
    let manager = ThreadManager::with_models_provider_and_home_for_tests(
        CodexAuth::from_api_key("dummy"),
        config.model_provider.clone(),
        home.path().to_path_buf(),
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
    );
    let parent = manager
        .start_thread(StartThreadOptions::new(config))
        .await?
        .thread;
    let startup = parent.config().await;
    let loaded = startup
        .config_layer_stack
        .requirements()
        .exec_policy
        .as_deref()
        .expect("startup has a managed exec policy");
    assert_eq!(
        parent
            .refresh_mcp_config(Arc::clone(&startup), builder().build().await?)
            .await,
        ConfigRefreshOutcome::Published
    );
    let call = spawn_call(&parent).await?;
    assert_eq!(
        call.turn
            .config
            .config_layer_stack
            .requirements()
            .exec_policy,
        None
    );
    let reservation = InvocationReservation::default();
    reservation.reserve(&parent.session, &call.turn, &call.call_id)?;
    let _guard = reservation.activate(&call);
    let invocation = parent.tool_invocation(&call.turn.sub_id, &call.call_id)?;
    let prepared = parent.prepare_child(&invocation).await?;
    assert_eq!(
        prepared.exec_policy_source_requirements.as_deref(),
        Some(loaded)
    );
    assert_eq!(
        prepared
            .exec_policy
            .check(&["managed-command".into()], &|_| Decision::Allow)
            .decision,
        Decision::Forbidden
    );
    parent.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test]
async fn same_manager_creation_retains_step_executor_and_shares_exec_policy() -> anyhow::Result<()>
{
    let home = tempfile::tempdir()?;
    let mut config = ConfigBuilder::without_managed_config_for_tests()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    let manager = ThreadManager::with_models_provider_and_home_for_tests(
        CodexAuth::from_api_key("dummy"),
        config.model_provider.clone(),
        home.path().to_path_buf(),
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
    );
    config.features.enable(Feature::MultiAgentV2)?;
    config.features.enable(Feature::DeferredExecutor)?;
    let environments = manager.environment_manager();
    // Leave the handshake pending so the registration stays attached throughout the spawn.
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let url = format!("ws://{}", listener.local_addr()?);
    environments.upsert_environment_with_options(
        "workspace".into(),
        RemoteEnvironmentOptions {
            exec_server_url: url.clone(),
            connect_timeout: Some(Duration::from_secs(300)),
            http_headers: Default::default(),
        },
        ScopedSkillsConfig {
            required: vec!["original-skill".into()],
        },
    )?;
    let mut requests = manager.default_environment_requests(&config.cwd, &config.workspace_roots);
    requests
        .iter_mut()
        .find(|request| request.environment_id == "workspace")
        .expect("registered workspace")
        .config = EnvironmentConfigState::Pending;
    let parent = manager
        .start_thread(StartThreadOptions {
            environments: Some(requests),
            ..StartThreadOptions::new(config)
        })
        .await?
        .thread;
    let session = &parent.session;
    let call = spawn_call(&parent).await?;
    let reservation = InvocationReservation::default();
    reservation.reserve(session, &call.turn, &call.call_id)?;
    assert!(
        parent
            .tool_invocation(&call.turn.sub_id, &call.call_id)
            .is_err()
    );
    let guard = reservation.activate(&call);
    let invocation = parent.tool_invocation(&call.turn.sub_id, &call.call_id)?;
    let before = parent.prepare_child(&invocation).await?.exec_policy;
    let amendment = ExecPolicyAmendment::new(vec!["snapshot-tool".into(), "check".into()]);
    session.persist_execpolicy_amendment(&amendment).await?;
    let after = parent.prepare_child(&invocation).await?.exec_policy;
    assert!(!before.get_allowed_prefixes().contains(&amendment.command));
    assert!(after.get_allowed_prefixes().contains(&amendment.command));
    environments.upsert_environment("workspace".into(), url, /*connect_timeout*/ None)?;
    let child_id = ThreadId::new();
    parent
        .create_child(
            &invocation,
            ThreadCreateRequest {
                task_name: "worker".into(),
                fork_mode: None,
                agent_type: None,
                model: None,
                reasoning_effort: None,
            },
            HostThreadSpawnReservation {
                thread_id: child_id,
                control: Arc::clone(&session.services.agent_control),
            },
        )
        .await?;
    let child = manager.get_thread(child_id).await?;
    assert!(
        child
            .thread_extension_data()
            .get::<HostToolInvocationLookup>()
            .is_some()
    );
    let snapshot = child.session.services.turn_environments.snapshot_now();
    assert_eq!(
        snapshot
            .required_skills()
            .find(|required| required.environment_id == "workspace")
            .expect("child retained the original executor registration")
            .skill_names,
        ["original-skill"]
    );
    let pending = snapshot
        .starting()
        .find(|starting| starting.selection.environment_id == "workspace")
        .expect("child awaits the workspace");
    let failure = "owner failed to configure workspace";
    parent
        .environment_failed(&pending.selection, failure.into())
        .await?;
    let error = tokio::time::timeout(Duration::from_secs(5), pending.wait_until_ready())
        .await?
        .expect_err("child should receive the owner's failure");
    assert!(matches!(error.as_ref(), ExecServerError::Protocol(message) if message == failure));
    assert!(Arc::ptr_eq(
        &session.services.exec_policy,
        &child.session.services.exec_policy,
    ));
    drop(guard);
    assert!(
        parent
            .tool_invocation(&call.turn.sub_id, &call.call_id)
            .is_err()
    );
    drop(reservation);
    let next = InvocationReservation::default();
    next.reserve(session, &call.turn, &call.call_id)?;
    let _next_guard = next.activate(&call);
    parent.tool_invocation(&call.turn.sub_id, &call.call_id)?;
    assert!(matches!(
        parent.prepare_child(&invocation).await,
        Err(ThreadSpawnError::Preparation(message)) if message == "spawning tool call finished"
    ));
    child.shutdown_and_wait().await?;
    parent.shutdown_and_wait().await?;
    Ok(())
}
