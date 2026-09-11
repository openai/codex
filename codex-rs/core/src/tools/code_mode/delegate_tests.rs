//! Callback tracing identity and originating-step lifetime coverage.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use codex_code_mode::CellId;
use codex_code_mode::CodeModeNestedToolCall;
use codex_code_mode::CodeModeSessionDelegate;
use codex_code_mode::CodeModeToolKind;
use codex_tools::ToolName;
use pretty_assertions::assert_eq;
use tokio_util::sync::CancellationToken;
use tracing::field::Field;
use tracing::field::Visit;
use tracing::span::Attributes;
use tracing::span::Id;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::Registry;

use super::CodeModeCellDelegate;
use super::CodeModeDispatchBroker;
use super::DispatchMessage;
use crate::session::step_context::StepContext;
use crate::session::tests::make_session_and_context;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use crate::tools::registry::ToolRegistry;
use crate::tools::router::ToolRouter;
use crate::turn_diff_tracker::TurnDiffTracker;
use tokio::sync::oneshot;

#[derive(Default)]
struct DispatchFields(BTreeMap<String, String>);

impl Visit for DispatchFields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record_str(field, &format!("{value:?}"));
    }
}

type DispatchRecords = Arc<Mutex<Vec<(BTreeMap<String, String>, bool)>>>;

struct DispatchCapture(DispatchRecords);

impl Layer<Registry> for DispatchCapture {
    fn on_new_span(&self, attributes: &Attributes<'_>, id: &Id, context: Context<'_, Registry>) {
        if attributes.metadata().target() != "codex_core::tools::parallel"
            || attributes.metadata().name() != "dispatch_tool_call_with_code_mode_result"
        {
            return;
        }
        let mut fields = DispatchFields::default();
        attributes.record(&mut fields);
        let has_turn_ancestor = context.span(id).is_some_and(|span| {
            span.scope().any(|ancestor| {
                ancestor.metadata().target() == "codex_core::tasks"
                    && ancestor.metadata().name() == "turn"
            })
        });
        self.0.lock().unwrap().push((fields.0, has_turn_ancestor));
    }
}

struct TestHandler;

impl ToolExecutor<ToolInvocation> for TestHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("audit_probe").with_default_namespace()
    }

    fn spec(&self) -> codex_tools::ToolSpec {
        codex_tools::ToolSpec::Function(codex_tools::ResponsesApiTool {
            name: "audit_probe".to_string(),
            description: "Return a marker for the code-mode dispatch test.".to_string(),
            strict: false,
            defer_loading: None,
            parameters: codex_tools::JsonSchema::default(),
            output_schema: None,
        })
    }

    fn handle<'a>(&'a self, _invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(async {
            Ok(Box::new(FunctionToolOutput::from_text(
                "audit-probe-ok".to_string(),
                Some(true),
            )) as Box<dyn crate::tools::context::ToolOutput>)
        })
    }
}

impl CoreToolRuntime for TestHandler {}

#[tokio::test(flavor = "current_thread")]
async fn detached_code_mode_callback_keeps_thread_id_on_dispatch_span() -> anyhow::Result<()> {
    let (session, turn) = make_session_and_context().await;
    let thread_id = session.thread_id.to_string();
    let turn = Arc::new(turn);
    let registry = ToolRegistry::with_handler_for_test(Arc::new(TestHandler));
    let router = Arc::new(ToolRouter::from_registry(
        &turn,
        turn.model_info(),
        registry,
        /*hosted_specs*/ Vec::new(),
        &Default::default(),
    ));
    let step = StepContext::for_test(Arc::clone(&turn)).with_tool_router_for_test(router);
    let broker = Arc::new(CodeModeDispatchBroker::new(
        session.thread_id,
        /*executed_tool_calls*/ Default::default(),
    ));
    let cell_id = CellId::new("audit-cell".to_string());
    broker.mark_cell_ready_for_dispatch(&cell_id, /*originating_item_id*/ None);
    let records = DispatchRecords::default();
    let subscriber = tracing_subscriber::registry().with(DispatchCapture(Arc::clone(&records)));
    let _untraced = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let _subscriber = tracing::subscriber::set_default(subscriber);
    let delegate = CodeModeCellDelegate {
        broker: Arc::clone(&broker),
        step_context: Arc::clone(&step),
    };
    let _worker = broker.start_turn_worker(
        Arc::new(session),
        step,
        Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new())),
    );
    // Process-owned code-mode callbacks run on a fresh task without a turn span.
    let result = tokio::spawn(async move {
        delegate
            .invoke_tool(
                CodeModeNestedToolCall {
                    cell_id,
                    runtime_tool_call_id: "runtime-audit-call".to_string(),
                    tool_name: ToolName::plain("audit_probe"),
                    tool_kind: CodeModeToolKind::Function,
                    input: Some(serde_json::json!({})),
                },
                CancellationToken::new(),
            )
            .await
    })
    .await?
    .map_err(anyhow::Error::msg)?;
    assert_eq!(result, serde_json::json!("audit-probe-ok"));
    let records = records.lock().unwrap();
    let [(fields, has_turn_ancestor)] = records.as_slice() else {
        panic!("expected one nested dispatch span, got {records:?}");
    };
    assert!(!has_turn_ancestor);
    assert_eq!(fields.get("thread.id"), Some(&thread_id));
    assert_eq!(
        fields.get("tool_name"),
        Some(&TestHandler.tool_name().to_string())
    );
    let call_id = fields
        .get("call_id")
        .expect("dispatch must identify the tool call");
    let uuid = call_id
        .strip_prefix("exec-")
        .expect("nested call ID must belong to code mode");
    uuid::Uuid::parse_str(uuid)?;
    Ok(())
}

#[tokio::test]
async fn dropped_tool_callbacks_release_the_origin_before_dispatch() {
    let (session, turn) = make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let broker = Arc::new(CodeModeDispatchBroker::new(
        session.thread_id,
        /*executed_tool_calls*/ Default::default(),
    ));
    let delegate = CodeModeCellDelegate {
        broker: Arc::clone(&broker),
        step_context: StepContext::for_test(Arc::clone(&turn)),
    };
    let origin = Arc::downgrade(&delegate.step_context);
    let cell_id = CellId::new("queued-cell".to_string());
    let cancellation_token = CancellationToken::new();
    let mut callback = delegate.invoke_tool(
        CodeModeNestedToolCall {
            cell_id: cell_id.clone(),
            runtime_tool_call_id: "call-a".to_string(),
            tool_name: codex_tools::ToolName::plain("test-tool"),
            tool_kind: codex_code_mode::CodeModeToolKind::Function,
            input: None,
        },
        cancellation_token.clone(),
    );
    assert!(futures::poll!(callback.as_mut()).is_pending());
    assert_eq!(broker.dispatch_rx.len(), 1);
    drop(callback);
    drop(delegate);
    assert!(
        origin.upgrade().is_none(),
        "queued work must not own the step"
    );

    // Observe the queued message's result after its original receiver was dropped.
    let mut message = broker.dispatch_rx.try_recv().unwrap();
    let (response_tx, response_rx) = oneshot::channel();
    let DispatchMessage::InvokeTool {
        response_tx: queued_response,
        ..
    } = &mut message
    else {
        panic!("expected tool invocation");
    };
    *queued_response = response_tx;
    broker.dispatch_tx.send(message).await.unwrap();
    broker.mark_cell_ready_for_dispatch(&cell_id, /*originating_item_id*/ None);
    let _worker = broker.start_turn_worker(
        Arc::clone(&session),
        StepContext::for_test(Arc::clone(&turn)),
        Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new())),
    );
    let response = response_rx.await;
    assert_eq!(
        response.unwrap(),
        Err("code mode nested tool call cancelled".to_string()),
    );
}
