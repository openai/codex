use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_protocol::protocol::SessionSource;
use codex_rollout_trace::ExecutionStatus;
use codex_rollout_trace::RolloutTraceRecorder;
use codex_rollout_trace::ThreadStartedTraceMetadata;
use codex_rollout_trace::ToolCallRequester;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

use crate::function_tool::FunctionCallError;
use crate::session::session::Session;
use crate::session::tests::make_session_and_context;
use crate::session::turn_context::TurnContext;
use crate::tools::code_mode::CodeModeWaitHandler;
use crate::tools::code_mode::WAIT_TOOL_NAME;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolCallSource;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::registry::ToolRegistry;
use crate::turn_diff_tracker::TurnDiffTracker;

#[derive(Default)]
struct TestHandler;

impl ToolHandler for TestHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, _invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        Ok(FunctionToolOutput::from_text("ok".to_string(), Some(true)))
    }
}

#[tokio::test]
async fn dispatch_lifecycle_trace_records_direct_and_code_mode_requesters() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let (mut session, turn) = make_session_and_context().await;
    attach_test_trace(&mut session, &turn, temp.path())?;
    session.services.rollout_trace.start_code_cell_trace(
        session.conversation_id,
        turn.sub_id.as_str(),
        "cell-1",
        "call-code",
        "await tools.test_tool({})",
    );

    let registry = ToolRegistry::with_handler_for_test(
        codex_tools::ToolName::plain("test_tool"),
        Arc::new(TestHandler),
    );
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
    assert_dispatch_trace_skips(ToolCallSource::JsRepl).await
}

#[tokio::test]
async fn dispatch_lifecycle_trace_records_unsupported_tool_failures() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let (mut session, turn) = make_session_and_context().await;
    attach_test_trace(&mut session, &turn, temp.path())?;

    let registry = ToolRegistry::empty_for_test();
    let session = Arc::new(session);
    let turn = Arc::new(turn);

    let result = registry
        .dispatch_any(test_invocation(
            session,
            turn,
            "unsupported-call",
            "missing_tool",
            ToolCallSource::Direct,
            "{}",
        ))
        .await;

    assert!(matches!(result, Err(FunctionCallError::RespondToModel(_))));
    let replayed = codex_rollout_trace::replay_bundle(single_bundle_dir(temp.path())?)?;
    let tool_call = &replayed.tool_calls["unsupported-call"];
    assert_eq!(tool_call.execution.status, ExecutionStatus::Failed);
    assert!(tool_call.raw_result_payload_id.is_some());

    Ok(())
}

#[tokio::test]
async fn dispatch_lifecycle_trace_records_incompatible_payload_failures() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let (mut session, turn) = make_session_and_context().await;
    attach_test_trace(&mut session, &turn, temp.path())?;

    let registry = ToolRegistry::with_handler_for_test(
        codex_tools::ToolName::plain("test_tool"),
        Arc::new(TestHandler),
    );
    let session = Arc::new(session);
    let turn = Arc::new(turn);

    let result = registry
        .dispatch_any(test_invocation_with_payload(
            session,
            turn,
            "incompatible-call",
            codex_tools::ToolName::plain("test_tool"),
            ToolCallSource::Direct,
            ToolPayload::Custom {
                input: "{}".to_string(),
            },
        ))
        .await;

    assert!(matches!(result, Err(FunctionCallError::Fatal(_))));
    let replayed = codex_rollout_trace::replay_bundle(single_bundle_dir(temp.path())?)?;
    let tool_call = &replayed.tool_calls["incompatible-call"];
    assert_eq!(tool_call.execution.status, ExecutionStatus::Failed);
    assert!(tool_call.raw_result_payload_id.is_some());

    Ok(())
}

#[tokio::test]
async fn missing_code_mode_wait_traces_only_the_wait_tool_call() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let (mut session, turn) = make_session_and_context().await;
    attach_test_trace(&mut session, &turn, temp.path())?;

    let registry = ToolRegistry::with_handler_for_test(
        codex_tools::ToolName::plain(WAIT_TOOL_NAME),
        Arc::new(CodeModeWaitHandler),
    );
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

async fn assert_dispatch_trace_skips(source: ToolCallSource) -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let (mut session, turn) = make_session_and_context().await;
    attach_test_trace(&mut session, &turn, temp.path())?;

    let registry = ToolRegistry::with_handler_for_test(
        codex_tools::ToolName::plain("test_tool"),
        Arc::new(TestHandler),
    );
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

fn test_invocation(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: &str,
    tool_name: &str,
    source: ToolCallSource,
    arguments: &str,
) -> ToolInvocation {
    test_invocation_with_payload(
        session,
        turn,
        call_id,
        codex_tools::ToolName::plain(tool_name),
        source,
        ToolPayload::Function {
            arguments: arguments.to_string(),
        },
    )
}

fn test_invocation_with_payload(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: &str,
    tool_name: codex_tools::ToolName,
    source: ToolCallSource,
    payload: ToolPayload,
) -> ToolInvocation {
    ToolInvocation {
        session,
        turn,
        cancellation_token: CancellationToken::new(),
        tracker: Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new())),
        call_id: call_id.to_string(),
        tool_name,
        source,
        payload,
    }
}

fn attach_test_trace(session: &mut Session, turn: &TurnContext, root: &Path) -> anyhow::Result<()> {
    let thread_id = session.conversation_id;
    let recorder = RolloutTraceRecorder::create_in_root_for_test(root, thread_id)?;
    recorder.record_thread_started(ThreadStartedTraceMetadata {
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
    });
    recorder.record_codex_turn_started(thread_id, turn.sub_id.as_str());
    session.services.rollout_trace = recorder;
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
