use super::*;
use crate::codex::make_session_and_context;
use crate::rollout_trace::RolloutTraceRecorder;
use crate::rollout_trace::ThreadStartedTraceMetadata;
use crate::tools::code_mode::CodeModeWaitHandler;
use crate::tools::code_mode::WAIT_TOOL_NAME;
use crate::turn_diff_tracker::TurnDiffTracker;
use codex_protocol::config_types::ModeKind;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TurnStartedEvent;
use codex_rollout_trace::ToolCallRequester;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;

#[derive(Default)]
struct TestHandler {
    first_class_trace_object: bool,
}

impl ToolHandler for TestHandler {
    type Output = crate::tools::context::FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn uses_first_class_trace_object(&self, _invocation: &ToolInvocation) -> bool {
        self.first_class_trace_object
    }

    async fn handle(&self, _invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        Ok(crate::tools::context::FunctionToolOutput::from_text(
            "ok".to_string(),
            Some(true),
        ))
    }
}

#[test]
fn handler_looks_up_namespaced_aliases_explicitly() {
    let plain_handler = Arc::new(TestHandler::default()) as Arc<dyn AnyToolHandler>;
    let namespaced_handler = Arc::new(TestHandler::default()) as Arc<dyn AnyToolHandler>;
    let namespace = "mcp__codex_apps__gmail";
    let tool_name = "gmail_get_recent_emails";
    let plain_name = codex_tools::ToolName::plain(tool_name);
    let namespaced_name = codex_tools::ToolName::namespaced(namespace, tool_name);
    let registry = ToolRegistry::new(HashMap::from([
        (plain_name.clone(), Arc::clone(&plain_handler)),
        (namespaced_name.clone(), Arc::clone(&namespaced_handler)),
    ]));

    let plain = registry.handler(&plain_name);
    let namespaced = registry.handler(&namespaced_name);
    let missing_namespaced = registry.handler(&codex_tools::ToolName::namespaced(
        "mcp__codex_apps__calendar",
        tool_name,
    ));

    assert_eq!(plain.is_some(), true);
    assert_eq!(namespaced.is_some(), true);
    assert_eq!(missing_namespaced.is_none(), true);
    assert!(
        plain
            .as_ref()
            .is_some_and(|handler| Arc::ptr_eq(handler, &plain_handler))
    );
    assert!(
        namespaced
            .as_ref()
            .is_some_and(|handler| Arc::ptr_eq(handler, &namespaced_handler))
    );
}

#[tokio::test]
async fn dispatch_lifecycle_trace_records_direct_and_code_mode_requesters() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let (mut session, turn) = make_session_and_context().await;
    attach_test_trace(&mut session, &turn, temp.path())?;
    session
        .services
        .rollout_trace
        .as_ref()
        .expect("trace recorder")
        .record_code_cell_started(
            session.conversation_id.to_string(),
            turn.sub_id.clone(),
            "cell-1",
            "call-code",
            "await tools.test_tool({})",
        );

    let registry = ToolRegistry::new(HashMap::from([(
        codex_tools::ToolName::plain("test_tool"),
        Arc::new(TestHandler::default()) as Arc<dyn AnyToolHandler>,
    )]));
    let session = Arc::new(session);
    let turn = Arc::new(turn);

    registry
        .dispatch_any(test_invocation(
            Arc::clone(&session),
            Arc::clone(&turn),
            "direct-call",
            "test_tool",
            ToolCallSource::Direct,
            "{}",
        ))
        .await?;
    registry
        .dispatch_any(test_invocation(
            session,
            turn,
            "code-mode-call",
            "test_tool",
            ToolCallSource::CodeMode {
                cell_id: "cell-1".to_string(),
                runtime_tool_call_id: "tool-1".to_string(),
            },
            "{}",
        ))
        .await?;

    let replayed = codex_rollout_trace::replay_bundle(single_bundle_dir(temp.path())?)?;
    assert_eq!(
        replayed.tool_calls["direct-call"].model_visible_call_id,
        Some("direct-call".to_string()),
    );
    assert_eq!(
        replayed.tool_calls["direct-call"].requester,
        ToolCallRequester::Model,
    );
    assert!(
        replayed.tool_calls["direct-call"]
            .raw_invocation_payload_id
            .is_some(),
        "dispatch tracing should keep the tool invocation payload",
    );
    assert!(
        replayed.tool_calls["direct-call"]
            .raw_result_payload_id
            .is_some(),
        "direct calls should keep the model-facing result payload",
    );
    assert_eq!(
        replayed.tool_calls["code-mode-call"].model_visible_call_id,
        None,
    );
    assert_eq!(
        replayed.tool_calls["code-mode-call"].code_mode_runtime_tool_id,
        Some("tool-1".to_string()),
    );
    assert_eq!(
        replayed.tool_calls["code-mode-call"].requester,
        ToolCallRequester::CodeCell {
            code_cell_id: "code_cell:call-code".to_string(),
        },
    );
    assert!(
        replayed.tool_calls["code-mode-call"]
            .raw_result_payload_id
            .is_some(),
        "code-mode calls should keep the result returned to JavaScript",
    );

    Ok(())
}

#[tokio::test]
async fn dispatch_lifecycle_trace_skips_noncanonical_boundaries() -> anyhow::Result<()> {
    assert_dispatch_trace_skips(
        Arc::new(TestHandler::default()) as Arc<dyn AnyToolHandler>,
        ToolCallSource::JsRepl,
    )
    .await?;
    assert_dispatch_trace_skips(
        Arc::new(TestHandler {
            first_class_trace_object: true,
        }) as Arc<dyn AnyToolHandler>,
        ToolCallSource::Direct,
    )
    .await
}

async fn assert_dispatch_trace_skips(
    handler: Arc<dyn AnyToolHandler>,
    source: ToolCallSource,
) -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let (mut session, turn) = make_session_and_context().await;
    attach_test_trace(&mut session, &turn, temp.path())?;

    let registry = ToolRegistry::new(HashMap::from([(
        codex_tools::ToolName::plain("test_tool"),
        handler,
    )]));
    let session = Arc::new(session);
    let turn = Arc::new(turn);

    registry
        .dispatch_any(test_invocation(
            session,
            turn,
            "skipped-call",
            "test_tool",
            source,
            "{}",
        ))
        .await?;

    let replayed = codex_rollout_trace::replay_bundle(single_bundle_dir(temp.path())?)?;
    assert_eq!(replayed.tool_calls, Default::default());

    Ok(())
}

#[tokio::test]
async fn missing_code_mode_wait_traces_only_the_wait_tool_call() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let (mut session, turn) = make_session_and_context().await;
    attach_test_trace(&mut session, &turn, temp.path())?;

    let registry = ToolRegistry::new(HashMap::from([(
        codex_tools::ToolName::plain(WAIT_TOOL_NAME),
        Arc::new(CodeModeWaitHandler) as Arc<dyn AnyToolHandler>,
    )]));
    let session = Arc::new(session);
    let turn = Arc::new(turn);

    registry
        .dispatch_any(test_invocation(
            session,
            turn,
            "wait-call",
            WAIT_TOOL_NAME,
            ToolCallSource::Direct,
            r#"{"cell_id":"noop","terminate":true}"#,
        ))
        .await?;

    let replayed = codex_rollout_trace::replay_bundle(single_bundle_dir(temp.path())?)?;
    assert_eq!(replayed.code_cells.len(), 0);
    assert!(
        replayed.tool_calls["wait-call"]
            .raw_result_payload_id
            .is_some()
    );

    Ok(())
}

fn test_invocation(
    session: Arc<crate::codex::Session>,
    turn: Arc<crate::codex::TurnContext>,
    call_id: &str,
    tool_name: &str,
    source: ToolCallSource,
    arguments: &str,
) -> ToolInvocation {
    ToolInvocation {
        session,
        turn,
        tracker: Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new())),
        call_id: call_id.to_string(),
        tool_name: codex_tools::ToolName::plain(tool_name),
        source,
        payload: ToolPayload::Function {
            arguments: arguments.to_string(),
        },
    }
}

fn attach_test_trace(
    session: &mut crate::codex::Session,
    turn: &crate::codex::TurnContext,
    root: &Path,
) -> anyhow::Result<()> {
    let thread_id = session.conversation_id;
    let recorder = RolloutTraceRecorder::create_in_root_for_test(
        root,
        thread_id,
        ThreadStartedTraceMetadata {
            thread_id: thread_id.to_string(),
            agent_path: "/root".to_string(),
            task_name: None,
            nickname: None,
            agent_role: None,
            session_source: SessionSource::Exec,
            cwd: PathBuf::from("/workspace"),
            rollout_path: None,
            model: "gpt-test".to_string(),
            provider_name: "test-provider".to_string(),
            approval_policy: "never".to_string(),
            sandbox_policy: "danger-full-access".to_string(),
        },
    )?;
    recorder.record_codex_turn_event(
        thread_id.to_string(),
        &turn.sub_id,
        &EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: turn.sub_id.clone(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::default(),
        }),
    );
    session.services.rollout_trace = Some(recorder);
    Ok(())
}

fn single_bundle_dir(root: &Path) -> anyhow::Result<PathBuf> {
    let mut entries = fs::read_dir(root)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    assert_eq!(entries.len(), 1);
    Ok(entries.remove(0))
}
