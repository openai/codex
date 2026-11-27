#![cfg(test)]
//! Exercise the MCP response envelope and the displayed footer’s polling lifecycle.

use super::*;
use crate::app_server_session::ThreadParamsMode;
use crate::bottom_pane::StatusLineFocus;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use crate::chatwidget::tests::render_bottom_popup;
use crate::statusline_metrics::tests::Collector;
use codex_app_server_client::AppServerClient;
use codex_app_server_client::CloudClient;
use codex_app_server_client::CloudCommand;
use codex_app_server_client::CloudOperation;
use codex_app_server_client::RequestResult;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::McpResourceContent;
use codex_config::types::BuiltinStatusLine;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::buffer::Cell;
use ratatui::layout::Rect;
use serde_json::json;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Instant;

#[test]
fn statusline_resource_requires_one_matching_json_text_item() {
    let uri = "ui://provider/footer";
    let content = |value| serde_json::from_value::<McpResourceContent>(value).unwrap();
    let text = |mime: &str, body: &str| content(json!({"uri":uri, "mimeType":mime, "text":body}));
    let item = text("APPLICATION/JSON; charset=utf-8", "null");
    assert_eq!(
        decode_tree(uri, std::slice::from_ref(&item)).unwrap(),
        UiTree(None)
    );
    let wrong_uri =
        content(json!({"uri":"ui://wrong","mimeType":"application/json","text":"null"}));
    let blob = content(json!({"uri":uri,"mimeType":"application/json","blob":"bnVsbA=="}));
    for (contents, expected) in [
        (vec![], ReadIssue::Envelope),
        (vec![item.clone(), item], ReadIssue::Envelope),
        (vec![wrong_uri], ReadIssue::Uri),
        (
            vec![content(json!({"uri":uri,"text":"null"}))],
            ReadIssue::Mime,
        ),
        (vec![blob], ReadIssue::Envelope),
        (vec![text("text/plain", "null")], ReadIssue::Mime),
        (vec![text("application/json", "{")], ReadIssue::Tree),
    ] {
        assert_eq!(decode_tree(uri, &contents).unwrap_err(), expected);
    }
}

fn read_events(rx: &mut UnboundedReceiver<AppEvent>) -> Vec<StatuslineReadEvent> {
    std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::StatuslineRead(read) => Some(read),
            _ => None,
        })
        .collect()
}

fn footer(chat: &ChatWidget) -> Option<String> {
    let content = chat.bottom_pane.status_line_content()?;
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 160, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    content.render(area, &mut buffer, StatusLineFocus::Focused);
    let row: String = buffer.content.iter().map(Cell::symbol).collect();
    Some(row.trim_end().to_owned())
}

#[tokio::test]
async fn custom_statusline_requests_headline_and_billing_with_no_native_items() {
    let (mut chat, _, mut rx, _) = make_chatwidget_manual_with_sender().await;
    let thread = ThreadId::new();
    chat.thread_id = Some(thread);
    chat.has_codex_backend_auth = true;
    chat.plan_type = Some(codex_protocol::account::PlanType::Business);
    chat.local_settings.tui.status_line = Some(Vec::new());
    chat.local_settings.tui.terminal_title = Some(Vec::new());
    chat.local_settings.tui.status_line_provider =
        Some(Box::new(StatusLineSelection::Mcp(StatusLineProviderId {
            server: "fixture".into(),
            tool: "line".into(),
        })));
    chat.statusline_feature_enabled = true;
    chat.refresh_status_line();

    let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AppEvent::RefreshStatusLineWorkspaceHeadline { .. }))
    );
    assert!(events.iter().any(|event| matches!(event, AppEvent::RefreshThreadUsage { thread_id, .. } if *thread_id == thread)));
}

#[tokio::test]
async fn statusline_reader_waits_for_the_displayed_thread_and_keeps_its_result() {
    let (mut chat, _, mut rx, _) = make_chatwidget_manual_with_sender().await;
    let server = crate::start_embedded_app_server_for_picker(&chat.config)
        .await
        .unwrap();
    let selected = Box::new(StatusLineSelection::Mcp(StatusLineProviderId {
        server: "fixture".into(),
        tool: "line".into(),
    }));
    chat.local_settings.tui.status_line = Some(Vec::new());
    chat.thread_id = Some(ThreadId::new());
    chat.local_settings.tui.status_line_provider = Some(selected.clone());
    chat.refresh_status_line();
    assert!(read_events(&mut rx).is_empty());

    chat.local_settings.tui.status_line_provider = Some(Box::new(StatusLineSelection::Builtin(
        BuiltinStatusLine::Default,
    )));
    chat.statusline_feature_enabled = true;
    chat.refresh_status_line();
    assert!(read_events(&mut rx).is_empty());
    chat.thread_id = None;
    chat.local_settings.tui.status_line_provider = Some(selected);
    chat.refresh_status_line();
    let initializing = footer(&chat).unwrap();
    assert!(read_events(&mut rx).is_empty());

    chat.thread_id = Some(ThreadId::new());
    chat.refresh_status_line();
    chat.refresh_status_line();
    let [start] = read_events(&mut rx).try_into().unwrap();
    let token = start.token;
    chat.set_active_agent_label(Some("native-agent-label".into()));
    let finish = |token, root| StatuslineReadEvent {
        token,
        step: ReadStep::Finished(server.backend_id(), Ok(UiTree(root))),
    };
    let collector = Collector::new();
    let mut app_config = chat.config.clone();
    let metrics = collector.metrics(&mut app_config);
    chat.handle_statusline_read(
        finish(token, Some(UiNode::String("Ready".into()))),
        &server,
        &metrics,
        &app_config,
    )
    .await;
    chat.refresh_status_line();
    let ready = footer(&chat).unwrap();
    insta::assert_snapshot!(format!("{initializing}\n{ready}"), @"
    Initializing…
    Ready
    ");
    chat.handle_statusline_read(finish(token, /*root*/ None), &server, &metrics, &app_config)
        .await;
    chat.refresh_status_line();
    assert_eq!(footer(&chat).as_deref(), Some(""));
    assert!(!render_bottom_popup(&chat, /*width*/ 80).contains("native-agent-label"));
    assert!(read_events(&mut rx).is_empty());

    chat.read_only_history = true;
    chat.refresh_status_line();
    assert!(footer(&chat).is_none());
    assert!(read_events(&mut rx).is_empty());
    chat.read_only_history = false;
    chat.refresh_status_line();
    let [resumed] = read_events(&mut rx).try_into().unwrap();
    chat.handle_statusline_read(finish(token, /*root*/ None), &server, &metrics, &app_config)
        .await;
    assert_eq!(footer(&chat).as_deref(), Some("Initializing…"));
    let token = resumed.token;
    chat.primary_environment_cwd = Some(crate::session_state::EnvironmentCwd {
        path: chat.config.cwd.join("executor-checkout").into(),
        is_server_local: false,
        cwd_is_only_root: true,
    });
    chat.refresh_status_line();
    assert!(read_events(&mut rx).is_empty());
    chat.current_cwd = chat.config.cwd.join("another-project").into();
    let (reply, snapshot) = oneshot::channel();
    chat.handle_statusline_read(
        StatuslineReadEvent {
            token,
            step: ReadStep::Snapshot(server.backend_id(), reply),
        },
        &server,
        &metrics,
        &app_config,
    )
    .await;
    assert!(snapshot.await.is_err());
    let [replacement] = read_events(&mut rx).try_into().unwrap();
    assert_eq!(footer(&chat).as_deref(), Some("Initializing…"));
    let token = replacement.token;
    chat.thread_id = Some(ThreadId::new());
    chat.refresh_status_line();
    chat.handle_statusline_read(
        finish(token, Some(UiNode::String("retired".into()))),
        &server,
        &metrics,
        &app_config,
    )
    .await;
    assert_eq!(footer(&chat).as_deref(), Some("Initializing…"));
    assert!(collector.snapshot().is_empty());
}

type Reply = oneshot::Sender<std::io::Result<RequestResult>>;
const URI: &str = "ui://fixture/status";

struct Reader {
    collector: Collector,
    app_config: Config,
    metrics: StatuslineMetrics,
    chat: ChatWidget,
    owner: AppServerSession,
    events: UnboundedReceiver<AppEvent>,
    requests: tokio::sync::mpsc::Receiver<CloudCommand>,
}

impl Reader {
    async fn new() -> Self {
        let (mut chat, _, events, _) = make_chatwidget_manual_with_sender().await;
        let (client, requests, _) = CloudClient::channel(/*capacity*/ 8);
        let owner = AppServerSession::new(
            BackendId::new(),
            AppServerClient::Cloud(client),
            ThreadParamsMode::Cloud,
        );
        chat.thread_id = Some(ThreadId::new());
        chat.statusline_feature_enabled = true;
        chat.local_settings.tui.status_line_provider =
            Some(Box::new(StatusLineSelection::Mcp(StatusLineProviderId {
                server: "fixture".into(),
                tool: "line".into(),
            })));
        chat.refresh_status_line();
        let collector = Collector::new();
        let mut app_config = chat.config.clone();
        let metrics = collector.metrics(&mut app_config);
        let mut reader = Self {
            collector,
            app_config,
            metrics,
            chat,
            owner,
            events,
            requests,
        };
        reader.dispatch().await;
        reader
    }

    async fn handle(&mut self, event: StatuslineReadEvent) {
        self.chat
            .handle_statusline_read(event, &self.owner, &self.metrics, &self.app_config)
            .await;
    }

    async fn dispatch(&mut self) {
        while let Some(event) = self.events.recv().await {
            if let AppEvent::StatuslineRead(event) = event {
                self.handle(event).await;
                return;
            }
        }
        panic!("reader event channel closed");
    }

    async fn request(&mut self) -> (ClientRequest, Reply) {
        let command = self.requests.recv().await.unwrap();
        let CloudOperation::Request(request) = command.operation else {
            panic!("expected an app-server request");
        };
        (*request, command.response)
    }

    async fn resolve(&mut self) {
        self.resolve_on(&[None, self.chat.thread_id]).await;
    }

    async fn resolve_on(&mut self, scopes: &[Option<ThreadId>]) {
        let (ClientRequest::ConfigRead { .. }, config) = self.request().await else {
            panic!("expected the selected thread's config");
        };
        let layer = json!({"mcp_servers":{"fixture":{"command":"fixture"}}});
        config
            .send(Ok(Ok(
                json!({"config":layer, "layers":[{"name":{"type":"user"}, "config":layer}]}),
            )))
            .unwrap();
        for thread in scopes {
            let (ClientRequest::McpServerStatusList { params, .. }, reply) = self.request().await
            else {
                panic!("expected a selected-server lookup");
            };
            assert_eq!(
                (params.server_name.as_deref(), params.thread_id),
                (Some("fixture"), thread.map(|id| id.to_string()))
            );
            let name = &self.chat.selected_statusline_provider().unwrap().tool;
            let tool = json!({"name":name,"title":"Calendar","inputSchema":{},"_meta":{
                "openai/ui":{"entrypoints":[{"type":"statusline","resourceUri":URI}]}
            }});
            let row = crate::statusline_provider::test_support::row("fixture", vec![tool]);
            reply
                .send(Ok(Ok(json!({"data":[row],"nextCursor":null}))))
                .unwrap();
        }
    }

    async fn read(&mut self, uri: &str) -> Reply {
        self.dispatch().await;
        let (ClientRequest::McpResourceRead { params, .. }, reply) = self.request().await else {
            panic!("expected the selected resource read");
        };
        assert_eq!(params.uri, uri);
        reply
    }

    fn assert_idle(&mut self) {
        assert!(
            read_events(&mut self.events).is_empty(),
            "unexpected reader event"
        );
        assert!(
            self.requests.try_recv().is_err(),
            "unexpected app-server request"
        );
    }
}

fn respond(reply: Reply, text: &str) {
    reply
        .send(Ok(Ok(
            json!({"contents":[{"uri":URI,"mimeType":"application/json","text":text}]}),
        )))
        .unwrap();
}

async fn advance(millis: u64) {
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(millis)).await;
    tokio::task::yield_now().await;
}

#[tokio::test(start_paused = true)]
async fn statusline_polls_one_second_after_completion_including_empty_content() {
    let mut reader = Reader::new().await;
    reader.resolve().await;
    // A busy TUI can delay the first snapshot without consuming the provider’s read budget.
    advance(/*millis*/ 6000).await;
    let first = reader.read(URI).await;
    advance(/*millis*/ 800).await;
    reader.assert_idle();
    let completed = Instant::now();
    respond(first, r#""ready""#);
    reader.dispatch().await;
    assert_eq!(footer(&reader.chat).as_deref(), Some("ready"));

    advance(/*millis*/ 999).await;
    reader.assert_idle();
    advance(/*millis*/ 1).await;
    reader.chat.set_model("updated-model");
    reader.dispatch().await;
    let (ClientRequest::McpResourceRead { params, .. }, second) = reader.request().await else {
        panic!("expected second resource read");
    };
    let meta = params.meta.unwrap();
    assert_eq!(
        meta["com.openai/codex-statusline-state"]["thread"]["model"]["display_name"],
        "updated-model"
    );
    assert_eq!(completed.elapsed(), Duration::from_secs(1));
    let completed = Instant::now();
    respond(second, "null");
    reader.dispatch().await;
    assert_eq!(footer(&reader.chat).as_deref(), Some(""));
    advance(/*millis*/ 999).await;
    reader.assert_idle();
    advance(/*millis*/ 1).await;
    let _third = reader.read(URI).await;
    assert_eq!(completed.elapsed(), Duration::from_secs(1));
    assert_eq!(reader.collector.snapshot(), ["provider_started 1"]);
}

#[tokio::test(start_paused = true)]
async fn statusline_invalid_content_stops_polling_immediately() {
    let mut reader = Reader::new().await;
    reader.resolve().await;
    respond(reader.read(URI).await, r#""last good""#);
    reader.dispatch().await;
    advance(/*millis*/ 1000).await;

    let invalid = reader.read(URI).await;
    assert_eq!(footer(&reader.chat).as_deref(), Some("last good"));
    let failed = Instant::now();
    respond(invalid, "{");
    reader.dispatch().await;
    assert_eq!(failed.elapsed(), Duration::ZERO);
    insta::assert_snapshot!(footer(&reader.chat).unwrap(), @"Status line failed · /statusline");
    assert!(!reader.chat.provider_needs_snapshot());
    reader.chat.refresh_status_line();
    advance(/*millis*/ 60_000).await;
    reader.assert_idle();
    assert_eq!(
        reader.collector.snapshot(),
        ["provider_failed kind=error 1", "provider_started 1"]
    );
}

#[tokio::test(start_paused = true)]
async fn statusline_rpc_error_stops_polling_immediately() {
    let mut reader = Reader::new().await;
    reader.resolve().await;
    let resolved = Instant::now();
    let reply = reader.read(URI).await;
    assert_eq!(resolved.elapsed(), Duration::ZERO);
    let failed = Instant::now();
    reply
        .send(Err(std::io::Error::other(
            "PRIVATE provider error and account data",
        )))
        .unwrap();
    reader.dispatch().await;
    assert_eq!(failed.elapsed(), Duration::ZERO);
    assert_eq!(
        footer(&reader.chat).as_deref(),
        Some("Status line failed · /statusline")
    );
    advance(/*millis*/ 60_000).await;
    reader.assert_idle();
}

#[tokio::test(start_paused = true)]
async fn statusline_returning_to_a_thread_fetches_fresh_state_and_discards_its_old_reply() {
    let mut reader = Reader::new().await;
    let original = reader.chat.thread_id.unwrap();
    reader.resolve().await;
    let old = reader.read(URI).await;

    reader.chat.thread_id = Some(ThreadId::new());
    reader.chat.refresh_status_line();
    reader.dispatch().await;
    reader.resolve().await;
    let other = reader.read(URI).await;

    reader.chat.thread_id = Some(original);
    reader.chat.refresh_status_line();
    reader.dispatch().await;
    let returned = Instant::now();
    reader.resolve().await;
    let current = reader.read(URI).await;
    assert_eq!(returned.elapsed(), Duration::ZERO);
    respond(current, r#""fresh footer""#);
    reader.dispatch().await;
    assert_eq!(footer(&reader.chat).as_deref(), Some("fresh footer"));

    let late =
        json!({"contents":[{"uri":URI,"mimeType":"application/json","text":"\"old footer\""}]});
    assert!(old.send(Ok(Ok(late.clone()))).is_err());
    assert!(other.send(Ok(Ok(late))).is_err());
    advance(/*millis*/ 1000).await;
    let mut parked = reader.read(URI).await;
    reader.chat.park_voice();
    reader.chat.refresh_status_line();
    parked.closed().await;
    reader.assert_idle();
}

#[tokio::test(start_paused = true)]
async fn statusline_deadline_wins_when_a_late_reply_is_already_queued() {
    let mut reader = Reader::new().await;
    reader.resolve().await;
    let reply = reader.read(URI).await;
    // Queue the reply beyond the deadline before letting the reader poll either future.
    let mut advance = std::pin::pin!(tokio::time::advance(Duration::from_millis(1_001)));
    assert!(futures::poll!(&mut advance).is_pending());
    respond(reply, r#""too late""#);
    reader.dispatch().await;
    assert_eq!(
        footer(&reader.chat).as_deref(),
        Some("Status line timed out · /statusline")
    );
}

#[tokio::test(start_paused = true)]
async fn statusline_hung_read_stops_polling_after_one_second() {
    let mut reader = Reader::new().await;
    reader.resolve().await;
    let held = reader.read(URI).await;
    let started = Instant::now();
    advance(/*millis*/ 999).await;
    reader.assert_idle();
    advance(/*millis*/ 1).await;
    reader.dispatch().await;
    assert_eq!(started.elapsed(), Duration::from_secs(1));
    assert_eq!(
        footer(&reader.chat).as_deref(),
        Some("Status line timed out · /statusline")
    );
    assert!(held.is_closed());
    reader.chat.local_settings.tui.status_line_provider = None;
    reader.chat.refresh_status_line();
    advance(/*millis*/ 60_000).await;
    reader.assert_idle();
    assert_eq!(
        reader.collector.snapshot(),
        ["provider_failed kind=timeout 1", "provider_started 1"]
    );
}

#[tokio::test(start_paused = true)]
async fn statusline_startup_timeout_stops_before_the_first_read() {
    let mut reader = Reader::new().await;
    let (ClientRequest::ConfigRead { .. }, held) = reader.request().await else {
        panic!("expected startup lookup");
    };
    let started = Instant::now();
    advance(/*millis*/ 4999).await;
    reader.assert_idle();
    advance(/*millis*/ 1).await;
    reader.dispatch().await;
    assert_eq!(started.elapsed(), Duration::from_secs(5));
    assert_eq!(
        footer(&reader.chat).as_deref(),
        Some("Status line timed out · /statusline")
    );
    assert!(held.is_closed());
    reader.chat.refresh_status_line();
    advance(/*millis*/ 60_000).await;
    reader.assert_idle();
    assert_eq!(
        reader.collector.snapshot(),
        ["provider_failed kind=timeout 1", "provider_started 1"]
    );
}

#[tokio::test(start_paused = true)]
async fn statusline_hung_retry_reports_the_remaining_startup_deadline() {
    let mut reader = Reader::new().await;
    let started = Instant::now();
    let (_, first) = reader.request().await;
    first
        .send(Err(std::io::Error::other("private config error")))
        .unwrap();
    advance(/*millis*/ 2_000).await;
    reader
        .chat
        .statusline_read
        .as_ref()
        .unwrap()
        .server_ready("fixture");
    let (ClientRequest::ConfigRead { .. }, held) = reader.request().await else {
        panic!("expected a retry after the selected server became ready");
    };
    advance(/*millis*/ 3_000).await;
    reader.dispatch().await;
    assert!(held.is_closed());
    assert_eq!(started.elapsed(), LOOKUP_TIMEOUT);
    insta::assert_snapshot!(
        footer(&reader.chat).unwrap(), @"Status line timed out · /statusline"
    );
}

#[tokio::test(start_paused = true)]
async fn statusline_changing_provider_or_reconnecting_discards_the_previous_read() {
    enum Change {
        Provider,
        Connection,
    }
    let mut reader = Reader::new().await;
    reader.resolve().await;
    let mut held = reader.read(URI).await;
    for change in [Change::Provider, Change::Connection] {
        let old = reader.chat.statusline_read.as_ref().unwrap().token;
        match change {
            Change::Provider => {
                reader.chat.local_settings.tui.status_line_provider =
                    Some(Box::new(StatusLineSelection::Mcp(StatusLineProviderId {
                        server: "fixture".into(),
                        tool: "other".into(),
                    })));
            }
            Change::Connection => {
                reader.chat.local_settings.tui.status_line = Some(vec!["model".into()]);
                reader.chat.local_settings.tui.terminal_title = Some(vec!["run-state".into()]);
                reader.chat.pause_for_disconnect();
                assert_eq!(
                    footer(&reader.chat),
                    Some(reader.chat.model_display_name().into())
                );
                reader.assert_idle();
                reader.chat.statusline_disconnected = false;
                let (client, requests, _) = CloudClient::channel(/*capacity*/ 8);
                reader.requests = requests;
                reader.owner = AppServerSession::new(
                    reader.owner.backend_id(),
                    AppServerClient::Cloud(client),
                    ThreadParamsMode::Cloud,
                );
            }
        }
        reader.chat.refresh_status_line();
        reader.dispatch().await;
        reader.resolve().await;
        let next = reader.read(URI).await;
        held.closed().await;
        for result in [Ok(UiTree(None)), Err(Failure::Snapshot)] {
            let step = ReadStep::Finished(reader.owner.backend_id(), result);
            reader
                .handle(StatuslineReadEvent { token: old, step })
                .await;
        }
        assert_eq!(footer(&reader.chat).as_deref(), Some("Initializing…"));
        held = next;
    }
    assert_eq!(reader.collector.snapshot(), ["provider_started 3"]);
}

#[tokio::test(start_paused = true)]
async fn statusline_explicit_reselection_restarts_a_failed_reader() {
    let mut reader = Reader::new().await;
    reader.resolve().await;
    let _held = reader.read(URI).await;
    advance(/*millis*/ 1000).await;
    reader.dispatch().await;
    reader.chat.refresh_status_line();
    reader.assert_idle();
    assert_eq!(
        footer(&reader.chat).as_deref(),
        Some("Status line timed out · /statusline")
    );

    // An explicit choice can arrive while the displayed thread is starting.
    let thread = reader.chat.thread_id.take();
    let provider = crate::statusline_provider::test_support::advertised(
        "fixture",
        "line",
        "ui://global/footer",
    );
    reader.chat.reselect_statusline_provider(Some(provider));
    reader.chat.thread_id = thread;
    reader.chat.refresh_status_line();
    reader.dispatch().await;
    reader.resolve_on(&[reader.chat.thread_id]).await;
    respond(reader.read(URI).await, r#""restored""#);
    reader.dispatch().await;
    assert_eq!(footer(&reader.chat).as_deref(), Some("restored"));
    assert_eq!(
        reader.collector.snapshot(),
        ["provider_failed kind=timeout 1", "provider_started 2"]
    );
}
