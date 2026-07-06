use super::*;

#[test]
fn resume_cut_routes_snapshot_and_stream_events_without_duplicates_or_loss() {
    let existing = ConnectionId(1);
    let joining = ConnectionId(2);

    for (label, subscribers, resumed, represented, expected) in [
        (
            "persisted event stays in snapshot",
            vec![existing],
            Some(joining),
            true,
            vec![existing],
        ),
        (
            "stream-only event replays to joiner",
            vec![existing],
            Some(joining),
            false,
            vec![existing, joining],
        ),
        (
            "re-resume does not duplicate snapshot event",
            vec![existing, joining],
            Some(joining),
            true,
            vec![existing],
        ),
        (
            "failed resume preserves original stream",
            vec![existing, joining],
            None,
            true,
            vec![existing, joining],
        ),
    ] {
        assert_eq!(
            buffered_event_recipients(
                &subscribers,
                resumed,
                ResumeEventCoverage {
                    represented_in_resume_snapshot: represented,
                    request_live_for_resumed_connection: true,
                },
            ),
            expected,
            "{label}"
        );
    }
}

fn buffered_user_input_request(turn_id: &str) -> BufferedThreadEvent {
    buffered_event(
        turn_id,
        EventMsg::RequestUserInput(RequestUserInputEvent {
            call_id: format!("request-{turn_id}"),
            turn_id: turn_id.to_string(),
            questions: Vec::new(),
            auto_resolution_ms: None,
        }),
    )
}

fn buffered_turn_completion(turn_id: &str) -> BufferedThreadEvent {
    represented_buffered_event(turn_id, turn_complete_event(turn_id))
}

#[test]
fn resume_cut_projects_request_liveness_before_joiner_delivery() {
    let existing = ConnectionId(1);
    let joining = ConnectionId(2);

    let mut terminal_only = vec![buffered_turn_completion("turn-a")];
    assert!(
        !project_buffered_request_liveness(&mut terminal_only),
        "a buffered terminal transition invalidates pre-cut pending requests"
    );

    let mut dead_buffered_request = vec![
        buffered_user_input_request("turn-a"),
        buffered_turn_completion("turn-a"),
    ];
    assert!(!project_buffered_request_liveness(
        &mut dead_buffered_request
    ));
    assert!(!dead_buffered_request[0].request_live_for_resumed_connection);
    assert_eq!(
        buffered_event_recipients(
            &[existing],
            Some(joining),
            ResumeEventCoverage {
                represented_in_resume_snapshot: false,
                request_live_for_resumed_connection: dead_buffered_request[0]
                    .request_live_for_resumed_connection,
            },
        ),
        vec![existing],
        "a request already canceled by the final snapshot must not reach the joiner"
    );

    let mut live_buffered_request = vec![
        buffered_turn_completion("turn-a"),
        buffered_user_input_request("turn-b"),
    ];
    assert!(!project_buffered_request_liveness(
        &mut live_buffered_request
    ));
    assert!(live_buffered_request[1].request_live_for_resumed_connection);
    assert_eq!(
        buffered_event_recipients(
            &[existing],
            Some(joining),
            ResumeEventCoverage {
                represented_in_resume_snapshot: false,
                request_live_for_resumed_connection: live_buffered_request[1]
                    .request_live_for_resumed_connection,
            },
        ),
        vec![existing, joining],
        "a request created after the last canceling transition must reach the joiner once"
    );

    let mut no_transitions = vec![buffered_user_input_request("turn-a")];
    assert!(
        project_buffered_request_liveness(&mut no_transitions),
        "without a buffered transition, pre-cut pending requests remain replayable"
    );
}

fn buffered_exec_lifecycle(turn_id: &str) -> (BufferedThreadEvent, BufferedThreadEvent) {
    let command = vec!["printf".to_string(), "done".to_string()];
    let begin = ExecCommandBeginEvent {
        call_id: "exec-1".to_string(),
        process_id: Some("process-1".to_string()),
        turn_id: turn_id.to_string(),
        started_at_ms: 1_000,
        command: command.clone(),
        cwd: "file:///tmp".parse().expect("path uri"),
        parsed_cmd: Vec::new(),
        source: ExecCommandSource::Agent,
        interaction_input: None,
    };
    let end = ExecCommandEndEvent {
        call_id: "exec-1".to_string(),
        process_id: Some("process-1".to_string()),
        turn_id: turn_id.to_string(),
        completed_at_ms: 2_000,
        command,
        cwd: "file:///tmp".parse().expect("path uri"),
        parsed_cmd: Vec::new(),
        source: ExecCommandSource::Agent,
        interaction_input: None,
        stdout: "done".to_string(),
        stderr: String::new(),
        aggregated_output: "done".to_string(),
        exit_code: 0,
        duration: Duration::from_millis(250),
        formatted_output: "done".to_string(),
        status: ExecCommandStatus::Completed,
    };
    (
        buffered_event(turn_id, EventMsg::ExecCommandBegin(begin)),
        buffered_event(turn_id, EventMsg::ExecCommandEnd(end)),
    )
}

#[test]
fn canonical_item_coverage_matches_generated_history_ids_and_respects_redaction() {
    let buffered_agent = buffered_completed_item(
        "latest-turn",
        TurnItem::AgentMessage(agent_message_item("canonical-agent-id", "durable answer")),
    );
    let mut full_turn = turn_with_view("latest-turn", TurnItemsView::Full, TurnStatus::Completed);
    full_turn
        .items
        .push(thread_agent_message("item-2", "durable answer"));
    let full_turns = vec![full_turn];
    assert!(
        full_turns_cover_event(&buffered_agent, &full_turns),
        "canonical and rebuilt agent items with different ids must not be delivered twice"
    );

    let second_identical_agent = buffered_completed_item(
        "latest-turn",
        TurnItem::AgentMessage(agent_message_item(
            "second-canonical-agent-id",
            "durable answer",
        )),
    );
    let mut occurrence_coverage =
        ResumePayloadItemCoverage::new(&full_turns, /*initial_turns_page*/ None);
    assert!(consume_full_turn_coverage(
        &buffered_agent,
        &full_turns,
        &mut occurrence_coverage,
    ));
    assert!(
        !consume_full_turn_coverage(
            &second_identical_agent,
            &full_turns,
            &mut occurrence_coverage,
        ),
        "one durable item cannot cover two identical canonical occurrences"
    );

    let same_canonical_id_item = agent_message_item("shared-canonical-agent-id", "durable answer");
    let mut lifecycle_pair = [
        buffered_started_item(
            "latest-turn",
            TurnItem::AgentMessage(same_canonical_id_item.clone()),
        ),
        buffered_completed_item(
            "latest-turn",
            TurnItem::AgentMessage(same_canonical_id_item),
        ),
    ];
    let mut lifecycle_coverage =
        ResumePayloadItemCoverage::new(&full_turns, /*initial_turns_page*/ None);
    for buffered in lifecycle_pair.iter_mut().rev() {
        buffered.represented_in_resume_snapshot =
            consume_full_turn_coverage(buffered, &full_turns, &mut lifecycle_coverage);
    }
    assert!(
        lifecycle_pair
            .iter()
            .all(|buffered| buffered.represented_in_resume_snapshot),
        "one generated-id final item must cover start and completion for the same canonical id"
    );

    let redacted_items = [
        (
            "image generation",
            TurnItem::ImageGeneration(ImageGenerationItem {
                id: "image-1".to_string(),
                status: "completed".to_string(),
                revised_prompt: Some("secret prompt".to_string()),
                result: "secret image payload".to_string(),
                saved_path: None,
            }),
        ),
        (
            "MCP tool call",
            mcp_tool_item("mcp-1", codex_protocol::items::McpToolCallStatus::Completed),
        ),
    ];
    for (label, item) in redacted_items {
        let buffered = buffered_completed_item("omitted-turn", item);
        assert!(
            event_is_represented(
                &buffered,
                &[],
                /*initial_turns_page*/ None,
                ResumePayloadMode::Redacted,
            ),
            "redacted {label} must be covered"
        );
        assert!(
            !full_turns_cover_event(&buffered, &[]),
            "omitted {label} remains replayable for an unredacted client"
        );
    }

    let buffered_raw_image = buffered_event(
        "omitted-turn",
        EventMsg::RawResponseItem(RawResponseItemEvent {
            item: ResponseItem::ImageGenerationCall {
                id: Some("raw-image".to_string()),
                status: "completed".to_string(),
                revised_prompt: Some("secret prompt".to_string()),
                result: "secret base64".to_string(),
                internal_chat_message_metadata_passthrough: None,
            },
        }),
    );
    assert!(event_is_represented(
        &buffered_raw_image,
        &[],
        /*initial_turns_page*/ None,
        ResumePayloadMode::Redacted,
    ));
    assert!(full_turns_cover_event(&buffered_raw_image, &[]));
}

#[test]
fn full_busy_snapshot_covers_projected_exec_lifecycle_but_omitted_items_do_not() {
    let (begin, end) = buffered_exec_lifecycle("busy-turn");
    let EventMsg::ExecCommandEnd(end_event) = &end.event.msg else {
        unreachable!();
    };
    let mut full_turn = turn_with_view("busy-turn", TurnItemsView::Full, TurnStatus::InProgress);
    full_turn
        .items
        .push(build_command_execution_end_item(end_event));
    let full_turns = vec![full_turn];
    let mut buffered = [begin, end];
    let mut coverage =
        ResumePayloadItemCoverage::new(&full_turns, /*initial_turns_page*/ None);
    for event in buffered.iter_mut().rev() {
        event.represented_in_resume_snapshot =
            consume_full_turn_coverage(event, &full_turns, &mut coverage);
    }
    assert!(
        buffered
            .iter()
            .all(|event| event.represented_in_resume_snapshot),
        "the final command in a busy snapshot dominates both buffered lifecycle events"
    );

    let omitted_turns = vec![turn_with_view(
        "busy-turn",
        TurnItemsView::NotLoaded,
        TurnStatus::InProgress,
    )];
    assert!(
        buffered
            .iter()
            .all(|event| !full_turns_cover_event(event, &omitted_turns)),
        "omitted lifecycle items must still be replayed after resume"
    );
}

#[test]
fn raw_hook_prompt_routes_typed_and_raw_channels_independently() {
    let fragments = vec![
        HookPromptFragment::from_single_hook("Retry with tests.", "hook-run-1"),
        HookPromptFragment::from_single_hook("Then summarize.", "hook-run-2"),
    ];
    let mut raw_item = build_hook_prompt_message(&fragments).expect("hook prompt message");
    let ResponseItem::Message { id, .. } = &mut raw_item else {
        unreachable!();
    };
    *id = None;
    let buffered = buffered_event(
        "busy-turn",
        EventMsg::RawResponseItem(RawResponseItemEvent {
            item: raw_item.clone(),
        }),
    );
    let mut full_turn = turn_with_view("busy-turn", TurnItemsView::Full, TurnStatus::InProgress);
    full_turn.items.push(ThreadItem::HookPrompt {
        id: "history-generated-id".to_string(),
        fragments: fragments
            .iter()
            .cloned()
            .map(codex_app_server_protocol::HookPromptFragment::from)
            .collect(),
    });
    let full_turns = vec![full_turn];

    let mut typed_coverage =
        ResumePayloadItemCoverage::new(&full_turns, /*initial_turns_page*/ None);
    assert!(consume_full_turn_coverage(
        &buffered,
        &full_turns,
        &mut typed_coverage,
    ));
    let second_identical = buffered_event(
        "busy-turn",
        EventMsg::RawResponseItem(RawResponseItemEvent { item: raw_item }),
    );
    assert!(
        !consume_full_turn_coverage(&second_identical, &full_turns, &mut typed_coverage,),
        "one durable hook occurrence cannot cover two buffered hook prompts"
    );

    assert!(!full_turns_cover_event(&buffered, &[]));
    assert!(
        !event_is_represented(
            &buffered,
            &[],
            /*initial_turns_page*/ None,
            ResumePayloadMode::Redacted,
        ),
        "an omitted hook remains safe to replay as a typed item on redacted resume"
    );

    let existing = ConnectionId(1);
    let joining = ConnectionId(2);
    let (typed_recipients, raw_recipients) = buffered_raw_response_recipients(
        &[existing],
        Some(joining),
        BufferedRawResponseRouting {
            event_coverage: ResumeEventCoverage {
                represented_in_resume_snapshot: true,
                request_live_for_resumed_connection: true,
            },
            raw_events_enabled: true,
            resume_payload_mode: ResumePayloadMode::Full,
        },
    );
    assert_eq!(typed_recipients, vec![existing]);
    assert_eq!(raw_recipients, vec![existing, joining]);

    let (typed_recipients, raw_recipients) = buffered_raw_response_recipients(
        &[existing],
        Some(joining),
        BufferedRawResponseRouting {
            event_coverage: ResumeEventCoverage {
                represented_in_resume_snapshot: false,
                request_live_for_resumed_connection: true,
            },
            raw_events_enabled: true,
            resume_payload_mode: ResumePayloadMode::Redacted,
        },
    );
    assert_eq!(typed_recipients, vec![existing, joining]);
    assert_eq!(raw_recipients, vec![existing]);
}

#[test]
fn canonical_completion_requires_final_state_and_stable_call_ids_never_normalize() {
    use codex_protocol::items::McpToolCallStatus;

    let mut in_progress_turn =
        turn_with_view("latest-turn", TurnItemsView::Full, TurnStatus::InProgress);
    let in_progress_api_item: ThreadItem =
        mcp_tool_item("mcp-a", McpToolCallStatus::InProgress).into();
    in_progress_turn.items.push(in_progress_api_item);
    let completed_a = buffered_completed_item(
        "latest-turn",
        mcp_tool_item("mcp-a", McpToolCallStatus::Completed),
    );
    assert!(
        !full_turns_cover_event(&completed_a, &[in_progress_turn]),
        "same-id in-progress state cannot cover a buffered completion"
    );

    let mut completed_turn =
        turn_with_view("latest-turn", TurnItemsView::Full, TurnStatus::Completed);
    let completed_api_item: ThreadItem =
        mcp_tool_item("mcp-a", McpToolCallStatus::Completed).into();
    completed_turn.items.push(completed_api_item);
    assert!(full_turns_cover_event(
        &completed_a,
        &[completed_turn.clone()]
    ));

    let completed_b = buffered_completed_item(
        "latest-turn",
        mcp_tool_item("mcp-b", McpToolCallStatus::Completed),
    );
    assert!(
        !full_turns_cover_event(&completed_b, &[completed_turn]),
        "a different stable call id must not match by normalized payload"
    );
}

#[test]
fn final_item_coverage_suppresses_folded_deltas_but_omitted_items_replay_them() {
    let item = agent_message_item("canonical-agent-id", "final answer");
    let mut buffered = [
        buffered_started_item("busy-turn", TurnItem::AgentMessage(item.clone())),
        buffered_event(
            "busy-turn",
            EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent {
                thread_id: ThreadId::new().to_string(),
                turn_id: "busy-turn".to_string(),
                item_id: "canonical-agent-id".to_string(),
                delta: "final answer".to_string(),
            }),
        ),
        buffered_completed_item("busy-turn", TurnItem::AgentMessage(item)),
    ];
    let mut full_turn = turn_with_view("busy-turn", TurnItemsView::Full, TurnStatus::InProgress);
    full_turn
        .items
        .push(thread_agent_message("history-generated-id", "final answer"));
    let full_turns = vec![full_turn];
    let mut coverage =
        ResumePayloadItemCoverage::new(&full_turns, /*initial_turns_page*/ None);
    for event in buffered.iter_mut().rev() {
        event.represented_in_resume_snapshot =
            consume_full_turn_coverage(event, &full_turns, &mut coverage);
    }
    assert!(
        buffered
            .iter()
            .all(|event| event.represented_in_resume_snapshot),
        "a full final item must own its start, folded delta, and completion"
    );

    assert!(
        buffered
            .iter()
            .all(|event| !full_turns_cover_event(event, &[]))
    );
}
