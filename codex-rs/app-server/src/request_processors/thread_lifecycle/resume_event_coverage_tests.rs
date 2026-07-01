use super::*;

#[test]
fn resume_cut_routes_snapshot_and_stream_events_without_duplicates_or_loss() {
    let existing = ConnectionId(1);
    let joining = ConnectionId(2);

    assert_eq!(
        buffered_event_recipients(
            &[existing],
            Some(joining),
            ResumeEventCoverage {
                represented_in_resume_snapshot: true,
                request_live_for_resumed_connection: true,
            },
        ),
        vec![existing],
        "a pre-cut persisted event belongs in the snapshot, not a joiner notification"
    );
    assert_eq!(
        buffered_event_recipients(
            &[existing],
            Some(joining),
            ResumeEventCoverage {
                represented_in_resume_snapshot: false,
                request_live_for_resumed_connection: true,
            },
        ),
        vec![existing, joining],
        "a pre-cut stream-only event must be replayed to the joiner after the response"
    );
    assert_eq!(
        buffered_event_recipients(
            &[existing, joining],
            Some(joining),
            ResumeEventCoverage {
                represented_in_resume_snapshot: true,
                request_live_for_resumed_connection: true,
            },
        ),
        vec![existing],
        "re-resume must not duplicate a snapshot event on an existing connection"
    );
    assert_eq!(
        buffered_event_recipients(
            &[existing, joining],
            None,
            ResumeEventCoverage {
                represented_in_resume_snapshot: true,
                request_live_for_resumed_connection: true,
            },
        ),
        vec![existing, joining],
        "a failed resume must leave the original event stream untouched"
    );
}

fn buffered_user_input_request(turn_id: &str) -> BufferedThreadEvent {
    BufferedThreadEvent {
        event: Event {
            id: turn_id.to_string(),
            msg: EventMsg::RequestUserInput(RequestUserInputEvent {
                call_id: format!("request-{turn_id}"),
                turn_id: turn_id.to_string(),
                questions: Vec::new(),
                auto_resolution_ms: None,
            }),
        },
        represented_in_resume_snapshot: false,
        request_live_for_resumed_connection: true,
    }
}

fn buffered_turn_completion(turn_id: &str) -> BufferedThreadEvent {
    BufferedThreadEvent {
        event: Event {
            id: turn_id.to_string(),
            msg: EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: turn_id.to_string(),
                last_agent_message: None,
                completed_at: Some(2),
                duration_ms: Some(1_000),
                time_to_first_token_ms: None,
            }),
        },
        represented_in_resume_snapshot: true,
        request_live_for_resumed_connection: true,
    }
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

fn buffered_completed_item(turn_id: &str, item: TurnItem) -> BufferedThreadEvent {
    BufferedThreadEvent {
        event: Event {
            id: turn_id.to_string(),
            msg: EventMsg::ItemCompleted(ItemCompletedEvent {
                thread_id: ThreadId::new(),
                turn_id: turn_id.to_string(),
                item,
                completed_at_ms: 2_000,
            }),
        },
        represented_in_resume_snapshot: false,
        request_live_for_resumed_connection: true,
    }
}

fn buffered_started_item(turn_id: &str, item: TurnItem) -> BufferedThreadEvent {
    BufferedThreadEvent {
        event: Event {
            id: turn_id.to_string(),
            msg: EventMsg::ItemStarted(ItemStartedEvent {
                thread_id: ThreadId::new(),
                turn_id: turn_id.to_string(),
                item,
                started_at_ms: 1_000,
            }),
        },
        represented_in_resume_snapshot: false,
        request_live_for_resumed_connection: true,
    }
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
        BufferedThreadEvent {
            event: Event {
                id: turn_id.to_string(),
                msg: EventMsg::ExecCommandBegin(begin),
            },
            represented_in_resume_snapshot: false,
            request_live_for_resumed_connection: true,
        },
        BufferedThreadEvent {
            event: Event {
                id: turn_id.to_string(),
                msg: EventMsg::ExecCommandEnd(end),
            },
            represented_in_resume_snapshot: false,
            request_live_for_resumed_connection: true,
        },
    )
}

fn mcp_tool_item(id: &str, status: codex_protocol::items::McpToolCallStatus) -> TurnItem {
    TurnItem::McpToolCall(McpToolCallItem {
        id: id.to_string(),
        server: "private".to_string(),
        tool: "lookup".to_string(),
        arguments: serde_json::json!({"secret": true}),
        connector_id: None,
        mcp_app_resource_uri: None,
        link_id: None,
        app_name: None,
        template_id: None,
        action_name: None,
        plugin_id: None,
        status,
        result: None,
        error: None,
        duration: None,
    })
}

#[test]
fn canonical_item_coverage_matches_generated_history_ids_and_respects_redaction() {
    let buffered_agent = buffered_completed_item(
        "latest-turn",
        TurnItem::AgentMessage(AgentMessageItem {
            id: "canonical-agent-id".to_string(),
            content: vec![AgentMessageContent::Text {
                text: "durable answer".to_string(),
            }],
            phase: None,
            memory_citation: None,
        }),
    );
    let mut full_turn = turn_with_view("latest-turn", TurnItemsView::Full, TurnStatus::Completed);
    full_turn.items.push(ThreadItem::AgentMessage {
        id: "item-2".to_string(),
        text: "durable answer".to_string(),
        phase: None,
        memory_citation: None,
    });
    let full_turns = vec![full_turn];
    assert!(
        event_is_represented(&buffered_agent, &full_turns, None, ResumePayloadMode::Full,),
        "canonical and rebuilt agent items with different ids must not be delivered twice"
    );

    let second_identical_agent = buffered_completed_item(
        "latest-turn",
        TurnItem::AgentMessage(AgentMessageItem {
            id: "second-canonical-agent-id".to_string(),
            content: vec![AgentMessageContent::Text {
                text: "durable answer".to_string(),
            }],
            phase: None,
            memory_citation: None,
        }),
    );
    let mut occurrence_coverage = ResumePayloadItemCoverage::new(&full_turns, None);
    assert!(buffered_event_is_represented_in_resume_payload(
        &buffered_agent,
        &full_turns,
        None,
        &mut occurrence_coverage,
        ResumePayloadMode::Full,
    ));
    assert!(
        !buffered_event_is_represented_in_resume_payload(
            &second_identical_agent,
            &full_turns,
            None,
            &mut occurrence_coverage,
            ResumePayloadMode::Full,
        ),
        "one durable item cannot cover two identical canonical occurrences"
    );

    let same_canonical_id_item = AgentMessageItem {
        id: "shared-canonical-agent-id".to_string(),
        content: vec![AgentMessageContent::Text {
            text: "durable answer".to_string(),
        }],
        phase: None,
        memory_citation: None,
    };
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
    let mut lifecycle_coverage = ResumePayloadItemCoverage::new(&full_turns, None);
    for buffered in lifecycle_pair.iter_mut().rev() {
        buffered.represented_in_resume_snapshot = buffered_event_is_represented_in_resume_payload(
            buffered,
            &full_turns,
            None,
            &mut lifecycle_coverage,
            ResumePayloadMode::Full,
        );
    }
    assert!(
        lifecycle_pair
            .iter()
            .all(|buffered| buffered.represented_in_resume_snapshot),
        "one generated-id final item must cover start and completion for the same canonical id"
    );

    let redacted_items = [
        TurnItem::ImageGeneration(ImageGenerationItem {
            id: "image-1".to_string(),
            status: "completed".to_string(),
            revised_prompt: Some("secret prompt".to_string()),
            result: "secret image payload".to_string(),
            saved_path: None,
        }),
        TurnItem::McpToolCall(McpToolCallItem {
            id: "mcp-1".to_string(),
            server: "private".to_string(),
            tool: "lookup".to_string(),
            arguments: serde_json::json!({"secret": true}),
            connector_id: None,
            mcp_app_resource_uri: None,
            link_id: None,
            app_name: None,
            template_id: None,
            action_name: None,
            plugin_id: None,
            status: codex_protocol::items::McpToolCallStatus::Completed,
            result: None,
            error: None,
            duration: None,
        }),
    ];
    for item in redacted_items {
        let buffered = buffered_completed_item("omitted-turn", item);
        assert!(event_is_represented(
            &buffered,
            &[],
            None,
            ResumePayloadMode::Redacted,
        ));
        assert!(
            !event_is_represented(&buffered, &[], None, ResumePayloadMode::Full,),
            "an omitted item remains replayable for an unredacted client"
        );
    }

    let buffered_raw_image = BufferedThreadEvent {
        event: Event {
            id: "omitted-turn".to_string(),
            msg: EventMsg::RawResponseItem(RawResponseItemEvent {
                item: ResponseItem::ImageGenerationCall {
                    id: Some("raw-image".to_string()),
                    status: "completed".to_string(),
                    revised_prompt: Some("secret prompt".to_string()),
                    result: "secret base64".to_string(),
                    internal_chat_message_metadata_passthrough: None,
                },
            }),
        },
        represented_in_resume_snapshot: false,
        request_live_for_resumed_connection: true,
    };
    assert!(event_is_represented(
        &buffered_raw_image,
        &[],
        None,
        ResumePayloadMode::Redacted,
    ));
    assert!(event_is_represented(
        &buffered_raw_image,
        &[],
        None,
        ResumePayloadMode::Full,
    ));
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
    let mut coverage = ResumePayloadItemCoverage::new(&full_turns, None);
    for event in buffered.iter_mut().rev() {
        event.represented_in_resume_snapshot = buffered_event_is_represented_in_resume_payload(
            event,
            &full_turns,
            None,
            &mut coverage,
            ResumePayloadMode::Full,
        );
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
        buffered.iter().all(|event| !event_is_represented(
            event,
            &omitted_turns,
            None,
            ResumePayloadMode::Full,
        )),
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
    let buffered = BufferedThreadEvent {
        event: Event {
            id: "busy-turn".to_string(),
            msg: EventMsg::RawResponseItem(RawResponseItemEvent {
                item: raw_item.clone(),
            }),
        },
        represented_in_resume_snapshot: false,
        request_live_for_resumed_connection: true,
    };
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

    let mut typed_coverage = ResumePayloadItemCoverage::new(&full_turns, None);
    assert!(buffered_event_is_represented_in_resume_payload(
        &buffered,
        &full_turns,
        None,
        &mut typed_coverage,
        ResumePayloadMode::Full,
    ));
    let second_identical = BufferedThreadEvent {
        event: Event {
            id: "busy-turn".to_string(),
            msg: EventMsg::RawResponseItem(RawResponseItemEvent { item: raw_item }),
        },
        represented_in_resume_snapshot: false,
        request_live_for_resumed_connection: true,
    };
    assert!(
        !buffered_event_is_represented_in_resume_payload(
            &second_identical,
            &full_turns,
            None,
            &mut typed_coverage,
            ResumePayloadMode::Full,
        ),
        "one durable hook occurrence cannot cover two buffered hook prompts"
    );

    assert!(!event_is_represented(
        &buffered,
        &[],
        None,
        ResumePayloadMode::Full,
    ));
    assert!(
        !event_is_represented(&buffered, &[], None, ResumePayloadMode::Redacted),
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
        !event_is_represented(
            &completed_a,
            &[in_progress_turn],
            None,
            ResumePayloadMode::Full,
        ),
        "same-id in-progress state cannot cover a buffered completion"
    );

    let mut completed_turn =
        turn_with_view("latest-turn", TurnItemsView::Full, TurnStatus::Completed);
    let completed_api_item: ThreadItem =
        mcp_tool_item("mcp-a", McpToolCallStatus::Completed).into();
    completed_turn.items.push(completed_api_item);
    assert!(event_is_represented(
        &completed_a,
        &[completed_turn.clone()],
        None,
        ResumePayloadMode::Full,
    ));

    let completed_b = buffered_completed_item(
        "latest-turn",
        mcp_tool_item("mcp-b", McpToolCallStatus::Completed),
    );
    assert!(
        !event_is_represented(
            &completed_b,
            &[completed_turn],
            None,
            ResumePayloadMode::Full,
        ),
        "a different stable call id must not match by normalized payload"
    );
}

#[test]
fn final_item_coverage_suppresses_folded_deltas_but_omitted_items_replay_them() {
    let item = AgentMessageItem {
        id: "canonical-agent-id".to_string(),
        content: vec![AgentMessageContent::Text {
            text: "final answer".to_string(),
        }],
        phase: None,
        memory_citation: None,
    };
    let mut buffered = [
        buffered_started_item("busy-turn", TurnItem::AgentMessage(item.clone())),
        BufferedThreadEvent {
            event: Event {
                id: "busy-turn".to_string(),
                msg: EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent {
                    thread_id: ThreadId::new().to_string(),
                    turn_id: "busy-turn".to_string(),
                    item_id: "canonical-agent-id".to_string(),
                    delta: "final answer".to_string(),
                }),
            },
            represented_in_resume_snapshot: false,
            request_live_for_resumed_connection: true,
        },
        buffered_completed_item("busy-turn", TurnItem::AgentMessage(item)),
    ];
    let mut full_turn = turn_with_view("busy-turn", TurnItemsView::Full, TurnStatus::InProgress);
    full_turn.items.push(ThreadItem::AgentMessage {
        id: "history-generated-id".to_string(),
        text: "final answer".to_string(),
        phase: None,
        memory_citation: None,
    });
    let full_turns = vec![full_turn];
    let mut coverage = ResumePayloadItemCoverage::new(&full_turns, None);
    for event in buffered.iter_mut().rev() {
        event.represented_in_resume_snapshot = buffered_event_is_represented_in_resume_payload(
            event,
            &full_turns,
            None,
            &mut coverage,
            ResumePayloadMode::Full,
        );
    }
    assert!(
        buffered
            .iter()
            .all(|event| event.represented_in_resume_snapshot),
        "a full final item must own its start, folded delta, and completion"
    );

    assert!(buffered.iter().all(|event| !event_is_represented(
        event,
        &[],
        None,
        ResumePayloadMode::Full,
    )));
}
