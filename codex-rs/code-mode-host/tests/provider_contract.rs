#![allow(clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use codex_code_mode_client::CodeModeHostCommand;
use codex_code_mode_client::StdioCodeModeSessionProvider;
use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CellOutcome;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeSession;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::CodeModeSessionProvider;
use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::CreateCellRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::NotificationFuture;
use codex_code_mode_protocol::ObserveRequest;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::ToolDefinition;
use codex_code_mode_protocol::ToolInvocationFuture;
use codex_protocol::ToolName;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

struct ProviderCase {
    name: &'static str,
    provider: Arc<dyn CodeModeSessionProvider>,
}

fn provider_cases() -> Vec<ProviderCase> {
    let host_program =
        codex_utils_cargo_bin::cargo_bin("codex-code-mode-host").expect("resolve host binary");
    vec![ProviderCase {
        name: "stdio",
        provider: Arc::new(StdioCodeModeSessionProvider::new(CodeModeHostCommand {
            program: host_program,
            args: Vec::new(),
        })),
    }]
}

#[derive(Debug, PartialEq)]
enum DelegateEvent {
    NotificationStarted,
    ToolStarted,
    CellClosed(CellId),
}

struct BlockingDelegate {
    events_tx: mpsc::UnboundedSender<DelegateEvent>,
}

impl BlockingDelegate {
    fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<DelegateEvent>) {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        (Arc::new(Self { events_tx }), events_rx)
    }
}

impl CodeModeSessionDelegate for BlockingDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            let _ = self.events_tx.send(DelegateEvent::ToolStarted);
            let _ = cancellation_token;
            std::future::pending().await
        })
    }

    fn notify<'a>(
        &'a self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async move {
            let _ = self.events_tx.send(DelegateEvent::NotificationStarted);
            let _ = cancellation_token;
            std::future::pending().await
        })
    }

    fn cell_closed(&self, cell_id: &CellId) {
        let _ = self
            .events_tx
            .send(DelegateEvent::CellClosed(cell_id.clone()));
    }
}

fn create_cell_request(source: &str) -> CreateCellRequest {
    CreateCellRequest {
        tool_call_id: "call-1".to_string(),
        enabled_tools: Vec::new(),
        source: source.to_string(),
    }
}

fn blocking_tool() -> ToolDefinition {
    ToolDefinition {
        name: "block".to_string(),
        tool_name: ToolName::plain("block"),
        description: String::new(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        output_schema: None,
    }
}

async fn create_session(
    provider: &ProviderCase,
    delegate: Arc<dyn CodeModeSessionDelegate>,
) -> Arc<dyn CodeModeSession> {
    provider
        .provider
        .create_session(delegate)
        .await
        .unwrap_or_else(|err| panic!("{} session creation failed: {err}", provider.name))
}

async fn observe(
    session: &Arc<dyn CodeModeSession>,
    cell_id: &CellId,
    yield_time_ms: u64,
) -> Result<CellOutcome, String> {
    session
        .observe(ObserveRequest {
            cell_id: cell_id.clone(),
            yield_time_ms,
        })
        .await
}

async fn next_event(
    provider_name: &str,
    events_rx: &mut mpsc::UnboundedReceiver<DelegateEvent>,
) -> DelegateEvent {
    tokio::time::timeout(Duration::from_secs(2), events_rx.recv())
        .await
        .unwrap_or_else(|_| panic!("{provider_name} delegate event timeout"))
        .unwrap_or_else(|| panic!("{provider_name} delegate event channel closed"))
}

#[tokio::test]
async fn providers_yield_and_resume() {
    for provider in provider_cases() {
        let (delegate, _) = BlockingDelegate::new();
        let session = create_session(&provider, delegate).await;
        let cell_id = session
            .create_cell(create_cell_request(
                r#"text("before"); yield_control(); text("after");"#,
            ))
            .await
            .unwrap_or_else(|err| panic!("{} create failed: {err}", provider.name));

        assert_eq!(
            observe(&session, &cell_id, 60_000).await.unwrap(),
            CellOutcome::LiveCell(RuntimeResponse::Yielded {
                cell_id: cell_id.clone(),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "before".to_string(),
                }],
            }),
            "{}",
            provider.name
        );
        assert_eq!(
            observe(&session, &cell_id, 1).await.unwrap(),
            CellOutcome::LiveCell(RuntimeResponse::Result {
                cell_id: cell_id.clone(),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "after".to_string(),
                }],
                error_text: None,
            }),
            "{}",
            provider.name
        );
        session.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn providers_preserve_natural_completion_when_termination_arrives_late() {
    for provider in provider_cases() {
        let (delegate, _) = BlockingDelegate::new();
        let session = create_session(&provider, delegate).await;
        let cell_id = session
            .create_cell(create_cell_request(
                r#"yield_control(); store("finished", true); text("done");"#,
            ))
            .await
            .unwrap();
        assert!(matches!(
            observe(&session, &cell_id, 60_000).await.unwrap(),
            CellOutcome::LiveCell(RuntimeResponse::Yielded { .. })
        ));

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let probe_id = session
                    .create_cell(create_cell_request(r#"text(String(load("finished")));"#))
                    .await
                    .unwrap();
                let response = observe(&session, &probe_id, 60_000).await.unwrap();
                let CellOutcome::LiveCell(RuntimeResponse::Result { content_items, .. }) = response
                else {
                    panic!("{} stored-value probe did not complete", provider.name);
                };
                if content_items
                    == vec![FunctionCallOutputContentItem::InputText {
                        text: "true".to_string(),
                    }]
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("{} completion probe timed out", provider.name));

        assert_eq!(
            session.terminate(cell_id.clone()).await.unwrap(),
            CellOutcome::LiveCell(RuntimeResponse::Result {
                cell_id,
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "done".to_string(),
                }],
                error_text: None,
            }),
            "{}",
            provider.name
        );
        session.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn providers_cancel_callbacks_before_termination_returns() {
    for provider in provider_cases() {
        let (delegate, mut events_rx) = BlockingDelegate::new();
        let session = create_session(&provider, delegate).await;
        let cell_id = session
            .create_cell(create_cell_request(
                r#"notify("pending"); await new Promise(() => {});"#,
            ))
            .await
            .unwrap();
        let observing_session = Arc::clone(&session);
        let observed_cell_id = cell_id.clone();
        let observer =
            tokio::spawn(
                async move { observe(&observing_session, &observed_cell_id, 60_000).await },
            );

        assert_eq!(
            next_event(provider.name, &mut events_rx).await,
            DelegateEvent::NotificationStarted
        );
        let terminated = CellOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: cell_id.clone(),
            content_items: Vec::new(),
        });
        assert_eq!(
            session.terminate(cell_id).await.unwrap(),
            terminated,
            "{}",
            provider.name
        );
        assert_eq!(
            observer.await.unwrap().unwrap(),
            terminated,
            "{}",
            provider.name
        );
        session.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn providers_reject_a_second_observer_without_displacing_the_first() {
    for provider in provider_cases() {
        let (delegate, _) = BlockingDelegate::new();
        let session = create_session(&provider, delegate).await;
        let cell_id = session
            .create_cell(create_cell_request("await new Promise(() => {});"))
            .await
            .unwrap();
        assert!(matches!(
            observe(&session, &cell_id, 1).await.unwrap(),
            CellOutcome::LiveCell(RuntimeResponse::Yielded { .. })
        ));

        let mut first_observer = session.observe(ObserveRequest {
            cell_id: cell_id.clone(),
            yield_time_ms: 60_000,
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut first_observer)
                .await
                .is_err(),
            "{} first observer returned unexpectedly",
            provider.name
        );
        assert_eq!(
            observe(&session, &cell_id, 60_000).await.unwrap_err(),
            format!("exec cell {cell_id} already has an active observer"),
            "{}",
            provider.name
        );

        let terminated = CellOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: cell_id.clone(),
            content_items: Vec::new(),
        });
        assert_eq!(
            session.terminate(cell_id).await.unwrap(),
            terminated,
            "{}",
            provider.name
        );
        assert_eq!(
            first_observer.await.unwrap(),
            terminated,
            "{}",
            provider.name
        );
        session.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn providers_cleanup_callbacks_before_natural_completion_returns() {
    for provider in provider_cases() {
        let (delegate, mut events_rx) = BlockingDelegate::new();
        let session = create_session(&provider, delegate).await;
        let mut request = create_cell_request(
            r#"
tools.block({});
await new Promise((resolve) => setTimeout(resolve, 0));
text("done");
"#,
        );
        request.enabled_tools.push(blocking_tool());
        let cell_id = session.create_cell(request).await.unwrap();

        assert_eq!(
            next_event(provider.name, &mut events_rx).await,
            DelegateEvent::ToolStarted
        );
        assert_eq!(
            observe(&session, &cell_id, 60_000).await.unwrap(),
            CellOutcome::LiveCell(RuntimeResponse::Result {
                cell_id,
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "done".to_string(),
                }],
                error_text: None,
            }),
            "{}",
            provider.name
        );
        session.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn providers_report_background_completion_to_the_delegate() {
    for provider in provider_cases() {
        let (delegate, mut events_rx) = BlockingDelegate::new();
        let session = create_session(&provider, delegate).await;
        let cell_id = session
            .create_cell(create_cell_request(concat!(
                "await new Promise(resolve => setTimeout(resolve, 100));",
                "text('done');",
            )))
            .await
            .unwrap();

        assert_eq!(
            observe(&session, &cell_id, 1).await.unwrap(),
            CellOutcome::LiveCell(RuntimeResponse::Yielded {
                cell_id: cell_id.clone(),
                content_items: Vec::new(),
            }),
            "{}",
            provider.name
        );
        assert_eq!(
            next_event(provider.name, &mut events_rx).await,
            DelegateEvent::CellClosed(cell_id.clone()),
            "{}",
            provider.name
        );
        assert_eq!(
            observe(&session, &cell_id, 60_000).await.unwrap(),
            CellOutcome::LiveCell(RuntimeResponse::Result {
                cell_id,
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "done".to_string(),
                }],
                error_text: None,
            }),
            "{}",
            provider.name
        );
        session.shutdown().await.unwrap();
    }
}
