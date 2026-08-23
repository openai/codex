//! Regression coverage for the live and replayed patch-approval pager.

use super::*;
use crate::app::app_server_requests::AppServerRequestResolution;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::PatchApplyStatus;
use pretty_assertions::assert_eq;
use ratatui::layout::Rect;
use tokio::sync::mpsc::UnboundedReceiver;

const TURN_ID: &str = "turn-patch-approval";
const ITEM_ID: &str = "exec-patch-approval";

fn changes() -> Vec<FileUpdateChange> {
    vec![
        FileUpdateChange {
            path: "a-added.txt".to_string(),
            kind: PatchChangeKind::Add,
            diff: "alpha\nbeta\n".to_string(),
        },
        FileUpdateChange {
            path: "m-updated.txt".to_string(),
            kind: PatchChangeKind::Update { move_path: None },
            diff: "@@ -1 +1 @@\n-old\n+new\n".to_string(),
        },
        FileUpdateChange {
            path: "z-deleted.txt".to_string(),
            kind: PatchChangeKind::Delete,
            diff: "removed\n".to_string(),
        },
    ]
}

fn expected_changes() -> HashMap<PathBuf, FileChange> {
    HashMap::from([
        (
            PathBuf::from("a-added.txt"),
            FileChange::Add {
                content: "alpha\nbeta\n".to_string(),
            },
        ),
        (
            PathBuf::from("m-updated.txt"),
            FileChange::Update {
                unified_diff: "@@ -1 +1 @@\n-old\n+new\n".to_string(),
                move_path: None,
            },
        ),
        (
            PathBuf::from("z-deleted.txt"),
            FileChange::Delete {
                content: "removed\n".to_string(),
            },
        ),
    ])
}

fn patch_item() -> ThreadItem {
    ThreadItem::FileChange {
        id: ITEM_ID.to_string(),
        changes: changes(),
        status: PatchApplyStatus::InProgress,
    }
}

fn request(thread_id: ThreadId) -> ServerRequest {
    ServerRequest::FileChangeRequestApproval {
        request_id: AppServerRequestId::Integer(17),
        params: FileChangeRequestApprovalParams {
            thread_id: thread_id.to_string(),
            turn_id: TURN_ID.to_string(),
            item_id: ITEM_ID.to_string(),
            started_at_ms: 0,
            reason: None,
            grant_root: None,
        },
    }
}

async fn enqueue_pending_patch(app: &mut App, thread_id: ThreadId) -> Result<()> {
    let cwd = app.chat_widget.config_ref().cwd.to_path_buf();
    app.enqueue_primary_thread_session(test_thread_session(thread_id, cwd), Vec::new())
        .await?;
    app.enqueue_thread_notification(
        thread_id,
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: thread_id.to_string(),
            turn_id: TURN_ID.to_string(),
            started_at_ms: 0,
            item: patch_item(),
        }),
    )
    .await?;
    let request = request(thread_id);
    assert_eq!(
        app.pending_app_server_requests
            .note_server_request(&request),
        None
    );
    app.enqueue_thread_request(thread_id, request).await
}

async fn drain_pending_patch(app: &mut App, tui: &mut crate::tui::Tui) -> Result<()> {
    app.drain_active_thread_events_until(tui, Instant::now() + Duration::from_secs(/*secs*/ 1))
        .await
}

fn open_patch(app: &mut App, rx: &mut UnboundedReceiver<AppEvent>) -> ApplyPatchApprovalRequest {
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
    let mut fullscreen = None;
    while let Ok(event) = rx.try_recv() {
        match event {
            AppEvent::FullScreenApprovalRequest(ApprovalRequest::ApplyPatch(request)) => {
                assert!(fullscreen.replace(request).is_none());
            }
            AppEvent::SubmitThreadOp { .. } => panic!("opening the pager must not decide approval"),
            _ => {}
        }
    }
    fullscreen.expect("Ctrl+A should open the pending patch")
}

fn render_pager(app: &mut App, width: u16, height: u16) -> Buffer {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
    let mut buffer = Buffer::empty(area);
    let Some(Overlay::Static(overlay)) = app.overlay.as_mut() else {
        panic!("expected the patch pager");
    };
    overlay.render(area, &mut buffer);
    buffer
}

fn assert_decision(
    app: &mut App,
    rx: &mut UnboundedReceiver<AppEvent>,
    thread_id: ThreadId,
    decision: FileChangeApprovalDecision,
) {
    let mut submitted = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let AppEvent::SubmitThreadOp { thread_id, op } = event {
            submitted.push((thread_id, op));
        }
    }
    let expected_op = AppCommand::patch_approval(ITEM_ID.to_string(), decision.clone());
    assert_eq!(submitted, vec![(thread_id, expected_op.clone())]);
    let thread_id = thread_id.to_string();
    assert_eq!(
        app.pending_app_server_requests
            .take_resolution(&thread_id, expected_op.clone()),
        Ok(Some(AppServerRequestResolution {
            request_id: AppServerRequestId::Integer(17),
            result: serde_json::json!({ "decision": decision }),
        }))
    );
    assert_eq!(
        app.pending_app_server_requests
            .take_resolution(&thread_id, expected_op),
        Ok(None)
    );
}

#[tokio::test]
async fn active_patch_approval_pager_preserves_changes_and_accepts_once() -> Result<()> {
    let (mut app, mut rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    enqueue_pending_patch(&mut app, thread_id).await?;
    drain_pending_patch(&mut app, &mut tui).await?;

    let request = open_patch(&mut app, &mut rx);
    assert_eq!(request.changes, expected_changes());
    assert_eq!(request.id, ITEM_ID);
    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::FullScreenApprovalRequest(ApprovalRequest::ApplyPatch(request)),
    )
    .await?;
    let first = render_pager(&mut app, /*width*/ 48, /*height*/ 11);
    insta::with_settings!({snapshot_path => "../../snapshots"}, {
        insta::assert_snapshot!("patch_approval_pager_top", format!("{first:?}"));
    });

    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
    )
    .await?;
    let last = render_pager(&mut app, /*width*/ 48, /*height*/ 11);
    insta::with_settings!({snapshot_path => "../../snapshots"}, {
        insta::assert_snapshot!("patch_approval_pager_bottom", format!("{last:?}"));
    });
    let _ = render_pager(&mut app, /*width*/ 36, /*height*/ 7);
    assert_eq!(render_pager(&mut app, /*width*/ 48, /*height*/ 11), last);

    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
    )
    .await?;
    assert!(app.overlay.is_none());
    assert!(
        app.pending_app_server_requests
            .contains_server_request(&self::request(thread_id))
    );
    assert_eq!(open_patch(&mut app, &mut rx).changes, expected_changes());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    assert_decision(
        &mut app,
        &mut rx,
        thread_id,
        FileChangeApprovalDecision::Accept,
    );
    Ok(())
}

#[tokio::test]
async fn active_patch_approval_cancel_and_resolved_replay() -> Result<()> {
    let (mut app, mut rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let mut tui = crate::tui::test_support::make_test_tui()?;
    enqueue_pending_patch(&mut app, thread_id).await?;
    drain_pending_patch(&mut app, &mut tui).await?;
    assert_eq!(open_patch(&mut app, &mut rx).changes, expected_changes());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_decision(
        &mut app,
        &mut rx,
        thread_id,
        FileChangeApprovalDecision::Cancel,
    );

    app.enqueue_thread_request(thread_id, request(thread_id))
        .await?;
    drain_pending_patch(&mut app, &mut tui).await?;
    assert!(!app.chat_widget.has_active_view());
    assert!(rx.try_recv().is_err());
    Ok(())
}

#[tokio::test]
async fn active_patch_approval_preserves_deferred_startup_protection() -> Result<()> {
    let (mut app, mut rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let mut tui = crate::tui::test_support::make_test_tui()?;
    enqueue_pending_patch(&mut app, thread_id).await?;
    app.startup_protected_input_boundary = true;
    app.chat_widget.handle_server_notification(
        agent_message_delta_notification(thread_id, TURN_ID, "agent-1", "streaming"),
        /*replay_kind*/ None,
    );
    drain_pending_patch(&mut app, &mut tui).await?;
    assert!(!app.chat_widget.has_active_view());
    assert!(app.startup_pending_protected_request);

    app.chat_widget.handle_server_notification(
        ServerNotification::ItemCompleted(codex_app_server_protocol::ItemCompletedNotification {
            thread_id: thread_id.to_string(),
            turn_id: TURN_ID.to_string(),
            completed_at_ms: 0,
            item: ThreadItem::AgentMessage {
                id: "agent-1".to_string(),
                text: "streaming".to_string(),
                phase: None,
                memory_citation: None,
                delivery: None,
            },
        }),
        /*replay_kind*/ None,
    );
    assert_eq!(open_patch(&mut app, &mut rx).changes, expected_changes());
    Ok(())
}

#[tokio::test]
async fn replayed_patch_approval_pager_recovers_stored_turn_changes() {
    let (mut app, mut rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let cwd = app.chat_widget.config_ref().cwd.to_path_buf();
    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: Some(test_thread_session(thread_id, cwd)),
            turns: vec![test_turn(
                TURN_ID,
                TurnStatus::InProgress,
                vec![patch_item()],
            )],
            events: vec![ThreadBufferedEvent::Request(Box::new(request(thread_id)))],
            input_state: None,
        },
        /*resume_restored_queue*/ false,
    );
    assert_eq!(open_patch(&mut app, &mut rx).changes, expected_changes());
}

#[test]
fn patch_approval_lookup_keeps_turn_and_item_identity() {
    let mut store = ThreadEventStore::new(/*capacity*/ 2);
    store.set_turns(vec![test_turn(
        TURN_ID,
        TurnStatus::InProgress,
        vec![patch_item()],
    )]);
    assert_eq!(store.file_change_changes(TURN_ID, ITEM_ID), Some(changes()));
    assert_eq!(store.file_change_changes("", ITEM_ID), Some(changes()));
    assert_eq!(store.file_change_changes("another-turn", ITEM_ID), None);
    assert_eq!(store.file_change_changes(TURN_ID, "another-item"), None);
}
