use super::*;
use crate::history_cell;
use crate::history_cell::SubagentPanelAgent;
use crate::history_cell::SubagentPanelState;
use crate::history_cell::SubagentStatusCell;
use ratatui::layout::Rect;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;
use std::time::Instant;

#[tokio::test]
async fn subagent_panel_is_not_flushed_into_transcript_history() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let state = Arc::new(StdMutex::new(SubagentPanelState {
        started_at: Instant::now(),
        total_agents: 1,
        running_count: 0,
        running_agents: vec![SubagentPanelAgent {
            ordinal: 1,
            name: "user-request-derisk-implement".to_string(),
            status: AgentStatus::PendingInit,
            is_watchdog: true,
            watchdog_countdown_duration: Duration::from_secs(60),
            watchdog_countdown_started_at: Some(Instant::now()),
            preview: "watchdog idle".to_string(),
            latest_update_at: Instant::now(),
        }],
    }));
    chat.on_subagent_panel_updated(Arc::new(SubagentStatusCell::new(
        Arc::clone(&state),
        /*animations_enabled*/ true,
    )));

    chat.add_to_history(history_cell::new_error_event("follow-up cell".to_string()));

    let inserted = drain_insert_history(&mut rx);
    assert_eq!(
        inserted.len(),
        1,
        "subagent panel should remain transient and not be inserted into transcript history"
    );
    let rendered = lines_to_single_string(&inserted[0]);
    assert!(rendered.contains("follow-up cell"));
    assert!(!rendered.contains("Subagents"));
    assert!(
        chat.subagent_panel
            .as_ref()
            .is_some_and(|panel| panel.matches_state(&state)),
        "subagent panel should stay mounted after other history cells are inserted"
    );
}

#[tokio::test]
async fn subagent_panel_mounts_while_placeholder_active_cell_exists_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.active_cell = Some(ChatWidget::placeholder_session_header_cell(
        chat.config_ref(),
    ));
    chat.bottom_pane.set_composer_text(
        "show current subagent state".to_string(),
        Vec::new(),
        Vec::new(),
    );

    let state = Arc::new(StdMutex::new(SubagentPanelState {
        started_at: Instant::now(),
        total_agents: 1,
        running_count: 0,
        running_agents: vec![SubagentPanelAgent {
            ordinal: 1,
            name: "watchdog-agent".to_string(),
            status: AgentStatus::PendingInit,
            is_watchdog: true,
            watchdog_countdown_duration: Duration::from_secs(60),
            watchdog_countdown_started_at: Some(Instant::now()),
            preview: "watchdog idle".to_string(),
            latest_update_at: Instant::now(),
        }],
    }));
    chat.on_subagent_panel_updated(Arc::new(SubagentStatusCell::new(
        Arc::clone(&state),
        /*animations_enabled*/ false,
    )));

    assert!(
        chat.active_cell
            .as_ref()
            .is_some_and(|cell| cell.as_any().is::<history_cell::SessionHeaderHistoryCell>()),
        "placeholder session header should remain the active cell"
    );
    assert!(
        chat.subagent_panel
            .as_ref()
            .is_some_and(|panel| panel.matches_state(&state)),
        "subagent panel should mount even when another active cell already exists"
    );

    let width = 80;
    let height = chat.desired_height(width);
    let mut terminal =
        ratatui::Terminal::new(VT100Backend::new(width, height)).expect("create terminal");
    terminal.set_viewport_area(Rect::new(0, 0, width, height));
    terminal
        .draw(|f| chat.render(f.area(), f.buffer_mut()))
        .expect("render chat with placeholder header and subagent panel");

    assert_chatwidget_snapshot!(
        "subagent_panel_mounts_while_placeholder_active_cell_exists",
        terminal.backend().vt100().screen().contents()
    );
}
