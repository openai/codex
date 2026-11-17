#![cfg(test)]
//! Exercise the MCP response envelope and the displayed footer's one-shot lifecycle.

use super::*;
use crate::bottom_pane::StatusLineFocus;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use crate::chatwidget::tests::render_bottom_popup;
use codex_config::types::BuiltinStatusLine;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::buffer::Cell;
use ratatui::layout::Rect;
use serde_json::json;
use tokio::sync::mpsc::UnboundedReceiver;

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
