use super::*;
use crate::history_cell;
use crate::history_cell::SubagentPanelAgent;
use crate::history_cell::SubagentPanelState;
use crate::history_cell::SubagentStatusCell;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
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
            preview: "watchdog idle".to_string(),
            latest_update_at: Instant::now(),
        }],
    }));
    chat.on_subagent_panel_updated(Arc::new(SubagentStatusCell::new(
        state,
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
        chat.active_cell
            .as_ref()
            .is_some_and(|cell| cell.as_any().is::<SubagentStatusCell>()),
        "subagent panel should stay mounted after other history cells are inserted"
    );
}
