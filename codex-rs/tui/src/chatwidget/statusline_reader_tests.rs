#![cfg(test)]
//! Exercise the MCP response envelope and the displayed footer’s polling lifecycle.

use super::*;
use crate::app_server_session::ThreadParamsMode;
use crate::bottom_pane::StatusLineFocus;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use crate::chatwidget::tests::render_bottom_popup;
use codex_app_server_client::AppServerClient;
use codex_app_server_client::CloudClient;
use codex_app_server_client::CloudCommand;
use codex_app_server_client::CloudOperation;
use codex_app_server_client::RequestResult;
use codex_app_server_protocol::ClientRequest;
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
    for contents in [
        vec![],
        vec![item.clone(), item],
        vec![content(
            json!({"uri":"ui://wrong","mimeType":"application/json","text":"null"}),
        )],
        vec![content(json!({"uri":uri,"text":"null"}))],
        vec![content(
            json!({"uri":uri,"mimeType":"application/json","blob":"bnVsbA=="}),
        )],
        vec![text("text/plain", "null")],
        vec![text("application/json", "{")],
    ] {
        assert!(decode_tree(uri, &contents).is_err());
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
    chat.set_feature_enabled(Feature::TuiExtensionsStatusLine, /*enabled*/ true);
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
    chat.set_feature_enabled(Feature::TuiExtensionsStatusLine, /*enabled*/ false);
    chat.refresh_status_line();
    assert!(read_events(&mut rx).is_empty());

    chat.local_settings.tui.status_line_provider = Some(Box::new(StatusLineSelection::Builtin(
        BuiltinStatusLine::Default,
    )));
    chat.set_feature_enabled(Feature::TuiExtensionsStatusLine, /*enabled*/ true);
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
    chat.handle_statusline_read(finish(token, Some(UiNode::String("Ready".into()))), &server);
    chat.refresh_status_line();
    let ready = footer(&chat).unwrap();
    insta::assert_snapshot!(format!("{initializing}\n{ready}"), @"
    Initializing…
    Ready
    ");
    chat.handle_statusline_read(finish(token, /*root*/ None), &server);
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
    chat.handle_statusline_read(finish(token, None), &server);
    assert_eq!(footer(&chat).as_deref(), Some("Initializing…"));
    let token = resumed.token;
    chat.primary_environment_cwd = Some(crate::session_state::EnvironmentCwd {
        path: LegacyAppPathString::from_path(chat.config.cwd.join("executor-checkout").as_path()),
        is_server_local: false,
    });
    chat.refresh_status_line();
    assert!(read_events(&mut rx).is_empty());
    chat.current_cwd = Some(chat.config.cwd.join("another-project").to_path_buf());
    let (reply, snapshot) = oneshot::channel();
    chat.handle_statusline_read(
        StatuslineReadEvent {
            token,
            step: ReadStep::Snapshot(server.backend_id(), reply),
        },
        &server,
    );
    assert!(snapshot.await.is_err());
    let [replacement] = read_events(&mut rx).try_into().unwrap();
    assert_eq!(footer(&chat).as_deref(), Some("Initializing…"));
    let token = replacement.token;
    chat.thread_id = Some(ThreadId::new());
    chat.refresh_status_line();
    chat.handle_statusline_read(
        finish(token, Some(UiNode::String("retired".into()))),
        &server,
    );
    assert_eq!(footer(&chat).as_deref(), Some("Initializing…"));
}

type Reply = oneshot::Sender<std::io::Result<RequestResult>>;
const URI: &str = "ui://fixture/status";

struct Reader {
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
        chat.set_feature_enabled(Feature::TuiExtensionsStatusLine, /*enabled*/ true);
        chat.local_settings.tui.status_line_provider =
            Some(Box::new(StatusLineSelection::Mcp(StatusLineProviderId {
                server: "fixture".into(),
                tool: "line".into(),
            })));
        chat.refresh_status_line();
        let mut reader = Self {
            chat,
            owner,
            events,
            requests,
        };
        reader.dispatch().await;
        reader
    }

    async fn dispatch(&mut self) {
        while let Some(event) = self.events.recv().await {
            if let AppEvent::StatuslineRead(event) = event {
                self.chat.handle_statusline_read(event, &self.owner);
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
        let (ClientRequest::ConfigRead { .. }, config) = self.request().await else {
            panic!("expected the selected thread's config");
        };
        let layer = json!({"mcp_servers":{"fixture":{"command":"fixture"}}});
        config
            .send(Ok(Ok(
                json!({"config":layer, "layers":[{"name":{"type":"user"}, "config":layer}]}),
            )))
            .unwrap();
        for thread in [None, self.chat.thread_id.map(|id| id.to_string())] {
            let (ClientRequest::McpServerStatusList { params, .. }, reply) = self.request().await
            else {
                panic!("expected a selected-server lookup");
            };
            assert_eq!(
                (params.server_name.as_deref(), params.thread_id),
                (Some("fixture"), thread)
            );
            let tool = json!({"name":"line","title":"Calendar","inputSchema":{},"_meta":{
                "openai/ui":{"entrypoints":[{"type":"statusline","resourceUri":URI}]}
            }});
            reply.send(Ok(Ok(json!({"data":[{"name":"fixture","httpOrigin":null,"runtimeStatus":"connected",
                "tools":{"line":tool},"resources":[],"resourceTemplates":[],"authStatus":"unsupported"}],"nextCursor":null})))).unwrap();
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
    insta::assert_snapshot!(footer(&reader.chat).unwrap(), @"Error loading statusline contents from Calendar. Use /statusline to (re)select your provider and try again.");
    reader.chat.refresh_status_line();
    advance(/*millis*/ 60_000).await;
    reader.assert_idle();
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
        .send(Err(std::io::Error::other("provider disconnected")))
        .unwrap();
    reader.dispatch().await;
    assert_eq!(failed.elapsed(), Duration::ZERO);
    assert!(
        footer(&reader.chat)
            .unwrap()
            .contains("Error loading statusline contents from Calendar")
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
    assert!(
        footer(&reader.chat)
            .unwrap()
            .contains("Error loading statusline contents from Calendar")
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
    assert!(
        footer(&reader.chat)
            .unwrap()
            .contains("Error loading statusline contents from Calendar")
    );
    assert!(held.is_closed());
    advance(/*millis*/ 60_000).await;
    reader.assert_idle();
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
    assert!(
        footer(&reader.chat)
            .unwrap()
            .contains("Error loading statusline contents from line")
    );
    assert!(held.is_closed());
    reader.chat.refresh_status_line();
    advance(/*millis*/ 60_000).await;
    reader.assert_idle();
}
