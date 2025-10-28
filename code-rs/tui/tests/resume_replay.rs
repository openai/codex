#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use code_core::history::{
    ExploreEntry, ExploreEntryStatus, ExploreRecord, ExploreSummary, HistoryId, HistoryRecord,
    HistorySnapshot, InlineSpan, OrderKeySnapshot, PlanIcon, PlanProgress, PlanStep, PlanUpdateState,
    ReasoningBlock, ReasoningSection, ReasoningState, TextEmphasis, TextTone,
};
use code_core::plan_tool::StepStatus;
use code_core::protocol::{Event, EventMsg, ReplayHistoryEvent};
use code_protocol::models::{ContentItem, ResponseItem};
use code_tui::test_helpers::{render_chat_widget_to_vt100, ChatWidgetHarness};
use serde_json::to_value;

fn assistant_cell_count(screen: &str) -> usize {
    screen
        .lines()
        .filter(|line| line.trim_start().starts_with("• "))
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.contains("How can I help you today?")
                || trimmed.contains("I can help with various tasks"))
        })
        .count()
}

fn message(role: &str, text: &str) -> ResponseItem {
    let content = match role {
        "assistant" => ContentItem::OutputText { text: text.to_string() },
        _ => ContentItem::InputText { text: text.to_string() },
    };

    ResponseItem::Message {
        id: None,
        role: role.to_string(),
        content: vec![content],
    }
}

fn inline_span(text: &str) -> InlineSpan {
    InlineSpan {
        text: text.to_string(),
        tone: TextTone::Default,
        emphasis: TextEmphasis::default(),
        entity: None,
    }
}

fn reasoning_state(id: u64, heading: &str) -> HistoryRecord {
    let summary = vec![inline_span(heading)];
    let section = ReasoningSection {
        heading: Some(heading.to_string()),
        summary: Some(summary.clone()),
        blocks: vec![ReasoningBlock::Paragraph(summary)],
    };

    HistoryRecord::Reasoning(ReasoningState {
        id: HistoryId(id),
        sections: vec![section],
        effort: None,
        in_progress: false,
    })
}

fn plan_update_state(id: u64, description: &str) -> HistoryRecord {
    HistoryRecord::PlanUpdate(PlanUpdateState {
        id: HistoryId(id),
        name: "Plan".to_string(),
        icon: PlanIcon::Clipboard,
        progress: PlanProgress {
            completed: 0,
            total: 1,
        },
        steps: vec![PlanStep {
            description: description.to_string(),
            status: StepStatus::Pending,
        }],
    })
}

fn explore_record(id: u64) -> HistoryRecord {
    HistoryRecord::Explore(ExploreRecord {
        id: HistoryId(id),
        entries: vec![ExploreEntry {
            action: code_core::history::ExecAction::List,
            summary: ExploreSummary::Command {
                display: "ls workspace".to_string(),
                annotation: None,
            },
            status: ExploreEntryStatus::Running,
        }],
    })
}

fn interleaved_reasoning_snapshot() -> HistorySnapshot {
    let records = vec![
        explore_record(1),
        reasoning_state(2, "Inspecting directory structure"),
        plan_update_state(3, "Queue follow-up command"),
        reasoning_state(4, "Summarizing findings"),
    ];

    HistorySnapshot {
        records,
        next_id: 5,
        exec_call_lookup: Default::default(),
        tool_call_lookup: Default::default(),
        stream_lookup: Default::default(),
        order: vec![
            OrderKeySnapshot { req: 1, out: 0, seq: 1 },
            OrderKeySnapshot { req: 2, out: 0, seq: 2 },
            OrderKeySnapshot { req: 3, out: 0, seq: 3 },
            OrderKeySnapshot { req: 4, out: 0, seq: 4 },
        ],
        order_debug: Vec::new(),
    }
}

fn final_reasoning_snapshot() -> HistorySnapshot {
    let records = vec![explore_record(1), reasoning_state(2, "Summarizing findings")];

    HistorySnapshot {
        records,
        next_id: 3,
        exec_call_lookup: Default::default(),
        tool_call_lookup: Default::default(),
        stream_lookup: Default::default(),
        order: vec![
            OrderKeySnapshot { req: 1, out: 0, seq: 1 },
            OrderKeySnapshot { req: 2, out: 0, seq: 2 },
        ],
        order_debug: Vec::new(),
    }
}

#[test]
fn replay_history_duplicates_short_assistant_messages() {
    let mut harness = ChatWidgetHarness::new();

    let items = vec![
        message("user", "Please summarize the plan."),
        message("assistant", "Working."),
        message("assistant", "Working. Done."),
    ];

    harness.handle_event(Event {
        id: "resume-replay".to_string(),
        event_seq: 0,
        msg: EventMsg::ReplayHistory(ReplayHistoryEvent {
            items,
            history_snapshot: None,
        }),
        order: None,
    });

    let screen = render_chat_widget_to_vt100(&mut harness, 80, 24);

    assert_eq!(
        1,
        assistant_cell_count(&screen),
        "expected a single restored assistant message but saw: {screen}"
    );
    assert!(screen.contains("Working. Done."));
}

#[test]
fn replay_history_handles_prefixed_revisions() {
    let mut harness = ChatWidgetHarness::new();

    let items = vec![
        message("user", "Please summarize the plan."),
        message("assistant", "Working."),
        message("assistant", "Update:\nWorking."),
    ];

    harness.handle_event(Event {
        id: "resume-replay".to_string(),
        event_seq: 0,
        msg: EventMsg::ReplayHistory(ReplayHistoryEvent {
            items,
            history_snapshot: None,
        }),
        order: None,
    });

    let screen = render_chat_widget_to_vt100(&mut harness, 80, 24);

    assert_eq!(
        1,
        assistant_cell_count(&screen),
        "expected a single restored assistant message but saw: {screen}"
    );
    assert!(screen.contains("Update:"));
}

#[test]
fn replay_history_hides_interleaved_reasoning_after_exploring() {
    let mut harness = ChatWidgetHarness::new();

    let snapshot_json = to_value(&interleaved_reasoning_snapshot()).expect("snapshot to json");

    harness.handle_event(Event {
        id: "resume-replay".to_string(),
        event_seq: 0,
        msg: EventMsg::ReplayHistory(ReplayHistoryEvent {
            items: Vec::new(),
            history_snapshot: Some(snapshot_json),
        }),
        order: None,
    });

    let screen = render_chat_widget_to_vt100(&mut harness, 80, 24);

    assert!(screen.contains("Exploring..."), "screen: {screen}");
    assert!(
        !screen.contains("Inspecting directory structure"),
        "screen: {screen}"
    );
    assert!(
        screen.contains("Summarizing findings"),
        "screen: {screen}"
    );
}

#[test]
fn replay_history_keeps_spacing_before_final_reasoning() {
    let mut harness = ChatWidgetHarness::new();

    let snapshot_json = to_value(&final_reasoning_snapshot()).expect("snapshot to json");

    harness.handle_event(Event {
        id: "resume-replay".to_string(),
        event_seq: 0,
        msg: EventMsg::ReplayHistory(ReplayHistoryEvent {
            items: Vec::new(),
            history_snapshot: Some(snapshot_json),
        }),
        order: None,
    });

    let screen = render_chat_widget_to_vt100(&mut harness, 80, 24);
    let lines: Vec<&str> = screen.lines().collect();

    let exploring_idx = lines
        .iter()
        .position(|line| line.contains("Exploring..."))
        .expect("exploring line present");
    let reasoning_idx = lines
        .iter()
        .position(|line| line.contains("Summarizing findings"))
        .expect("reasoning line present");

    assert!(reasoning_idx > exploring_idx + 1, "screen: {screen}");
    let in_between = &lines[exploring_idx + 1..reasoning_idx];
    assert!(
        in_between.iter().any(|line| line.trim().is_empty()),
        "expected blank line between exploring and reasoning. screen: {screen}"
    );
}
