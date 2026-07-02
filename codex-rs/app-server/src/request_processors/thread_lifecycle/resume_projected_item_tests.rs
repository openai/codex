use super::*;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::DynamicToolCallStatus;
use codex_app_server_protocol::build_item_from_guardian_event;
use codex_protocol::dynamic_tools::DynamicToolCallRequest;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::GuardianAssessmentStatus;

fn guardian_assessment(status: GuardianAssessmentStatus) -> GuardianAssessmentEvent {
    GuardianAssessmentEvent {
        id: "guardian-review".to_string(),
        target_item_id: Some("guardian-command".to_string()),
        turn_id: "busy-turn".to_string(),
        started_at_ms: 1_000,
        completed_at_ms: (!matches!(status, GuardianAssessmentStatus::InProgress)).then_some(1_042),
        status,
        risk_level: None,
        user_authorization: None,
        rationale: None,
        decision_source: None,
        action: serde_json::from_value(serde_json::json!({
            "type": "command",
            "source": "shell",
            "command": "printf guardian",
            "cwd": "/tmp",
        }))
        .expect("guardian command action"),
    }
}

fn dynamic_tool_request() -> DynamicToolCallRequest {
    DynamicToolCallRequest {
        call_id: "dynamic-call".to_string(),
        turn_id: "busy-turn".to_string(),
        started_at_ms: 1_000,
        namespace: Some("workspace".to_string()),
        tool: "lookup".to_string(),
        arguments: serde_json::json!({"query": "active"}),
    }
}

#[test]
fn busy_snapshot_covers_guardian_and_dynamic_tool_projected_items() {
    let guardian_started = guardian_assessment(GuardianAssessmentStatus::InProgress);
    let guardian_denied = guardian_assessment(GuardianAssessmentStatus::Denied);
    let dynamic_request = dynamic_tool_request();
    let mut turn = turn_with_view("busy-turn", TurnItemsView::Full, TurnStatus::InProgress);
    turn.items.push(
        build_item_from_guardian_event(&guardian_denied, CommandExecutionStatus::Declined)
            .expect("guardian command item"),
    );
    turn.items.push(ThreadItem::DynamicToolCall {
        id: dynamic_request.call_id.clone(),
        namespace: dynamic_request.namespace.clone(),
        tool: dynamic_request.tool.clone(),
        arguments: dynamic_request.arguments.clone(),
        status: DynamicToolCallStatus::InProgress,
        content_items: None,
        success: None,
        duration_ms: None,
    });
    let turns = vec![turn];

    for (label, msg) in [
        (
            "guardian start",
            EventMsg::GuardianAssessment(guardian_started),
        ),
        (
            "guardian denial",
            EventMsg::GuardianAssessment(guardian_denied),
        ),
        (
            "dynamic tool request",
            EventMsg::DynamicToolCallRequest(dynamic_request),
        ),
    ] {
        assert!(
            full_turns_cover_event(&represented_buffered_event("busy-turn", msg), &turns),
            "{label} projected lifecycle must be covered by the busy snapshot"
        );
    }
}

#[test]
fn represented_guardian_and_dynamic_events_split_companion_and_item_recipients() {
    let existing = ConnectionId(1);
    let joining = ConnectionId(2);
    let coverage = ResumeEventCoverage {
        represented_in_resume_snapshot: true,
        request_live_for_resumed_connection: true,
    };

    for (label, msg) in [
        (
            "guardian assessment",
            EventMsg::GuardianAssessment(guardian_assessment(GuardianAssessmentStatus::InProgress)),
        ),
        (
            "dynamic tool request",
            EventMsg::DynamicToolCallRequest(dynamic_tool_request()),
        ),
    ] {
        let (companion_recipients, item_recipients) =
            buffered_event_delivery_recipients(&[existing, joining], Some(joining), &msg, coverage);
        assert_eq!(companion_recipients, vec![existing, joining], "{label}");
        assert_eq!(item_recipients, Some(vec![existing]), "{label}");
    }

    let (companion_recipients, item_recipients) = buffered_event_delivery_recipients(
        &[existing],
        Some(joining),
        &EventMsg::DynamicToolCallRequest(dynamic_tool_request()),
        ResumeEventCoverage {
            represented_in_resume_snapshot: true,
            request_live_for_resumed_connection: false,
        },
    );
    assert_eq!(companion_recipients, vec![existing]);
    assert_eq!(item_recipients, Some(vec![existing]));
}

#[test]
fn duplicate_summary_and_full_turn_coverage_prefers_the_full_item_view() {
    let mut summary_turn = turn_with_view(
        "duplicate-turn",
        TurnItemsView::Summary,
        TurnStatus::InProgress,
    );
    summary_turn.items.push(
        mcp_tool_item(
            "mcp-duplicate",
            codex_protocol::items::McpToolCallStatus::InProgress,
        )
        .into(),
    );
    let mut full_turn =
        turn_with_view("duplicate-turn", TurnItemsView::Full, TurnStatus::Completed);
    full_turn.items.push(
        mcp_tool_item(
            "mcp-duplicate",
            codex_protocol::items::McpToolCallStatus::Completed,
        )
        .into(),
    );
    let page = TurnsPage {
        data: vec![full_turn],
        next_cursor: None,
        backwards_cursor: None,
    };
    let buffered = buffered_completed_item(
        "duplicate-turn",
        mcp_tool_item(
            "mcp-duplicate",
            codex_protocol::items::McpToolCallStatus::Completed,
        ),
    );

    assert!(event_is_represented(
        &buffered,
        &[summary_turn],
        Some(&page),
        ResumePayloadMode::Full,
    ));
}
