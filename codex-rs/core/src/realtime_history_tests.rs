use super::*;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::DynamicToolCallItem;
use codex_protocol::items::DynamicToolCallStatus;
use codex_protocol::items::ImageGenerationItem;
use codex_protocol::items::McpToolCallItem;
use codex_protocol::items::McpToolCallStatus;
use codex_protocol::items::SubAgentActivityItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::protocol::AgentMessageContentDeltaEvent;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ItemStartedEvent;
use codex_protocol::protocol::RealtimeConversationClosedEvent;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeConversationStartedEvent;
use codex_protocol::protocol::RealtimeConversationVersion;
use codex_protocol::protocol::SubAgentActivityKind;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnAbortedEvent;
use pretty_assertions::assert_eq;
use test_case::test_case;
use uuid::Uuid;

fn started_state() -> RealtimeHistoryState {
    let mut state = RealtimeHistoryState::default();
    state.observe(&EventMsg::TurnStarted(
        codex_protocol::protocol::TurnStartedEvent {
            turn_attribution: None,
            turn_id: "turn-1".to_string(),
            root_turn_id: None,
            trace_id: None,
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        },
    ));
    state.observe(&EventMsg::RealtimeConversationStarted(
        RealtimeConversationStartedEvent {
            realtime_session_id: Some("voice-1".to_string()),
            version: RealtimeConversationVersion::V2,
        },
    ));
    state
}

fn observe_realtime(
    state: &mut RealtimeHistoryState,
    payload: RealtimeEvent,
) -> RealtimeEventEffects {
    state.observe(&EventMsg::RealtimeConversationRealtime(
        RealtimeConversationRealtimeEvent { payload },
    ))
}

fn assistant_delta(item_id: &str, delta: &str) -> EventMsg {
    EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        item_id: item_id.to_string(),
        delta: delta.to_string(),
    })
}

fn completed_item(item: TurnItem) -> EventMsg {
    EventMsg::ItemCompleted(ItemCompletedEvent {
        content_token_count: None,
        thread_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        item,
        started_at_ms: None,
        completed_at_ms: 0,
    })
}

fn dynamic_tool_call(id: &str, status: DynamicToolCallStatus) -> TurnItem {
    TurnItem::DynamicToolCall(DynamicToolCallItem {
        id: id.to_string(),
        namespace: Some("another_client".to_string()),
        tool: "any_dynamic_tool".to_string(),
        arguments: serde_json::Value::Null,
        status,
        content_items: None,
        success: Some(status == DynamicToolCallStatus::Completed),
        error: None,
        duration: None,
    })
}

fn mcp_tool_call(id: &str, server: &str, status: McpToolCallStatus) -> TurnItem {
    TurnItem::McpToolCall(McpToolCallItem {
        id: id.to_string(),
        server: server.to_string(),
        environment_id: None,
        tool: "create_thread".to_string(),
        arguments: serde_json::Value::Null,
        connector_id: None,
        mcp_app_resource_uri: None,
        mcp_app_ui: None,
        link_id: None,
        app_name: None,
        action_name: None,
        plugin_id: None,
        read_only_hint: None,
        status,
        result: None,
        error: None,
        duration: None,
    })
}

fn contents(effects: RealtimeEventEffects) -> Vec<RealtimeItemContent> {
    effects
        .items
        .into_iter()
        .map(|item| {
            assert_eq!(Uuid::parse_str(&item.id).unwrap().get_version_num(), 7);
            item.content
        })
        .collect()
}

#[test_case(Some("turn-1"), None; "matching_turn")]
#[test_case(None, None; "without_turn_id")]
#[test_case(Some("another-turn"), Some("voice-2"); "unrelated_turn")]
fn interrupted_turn_is_not_associated_with_a_new_voice_session(
    aborted_turn_id: Option<&str>,
    expected_session_id: Option<&str>,
) {
    let mut state = RealtimeHistoryState::default();
    state.observe(&EventMsg::TurnStarted(
        codex_protocol::protocol::TurnStartedEvent {
            turn_attribution: None,
            turn_id: "turn-1".to_string(),
            root_turn_id: None,
            trace_id: None,
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        },
    ));
    let aborted = EventMsg::TurnAborted(TurnAbortedEvent {
        root_turn_id: None,
        turn_id: aborted_turn_id.map(str::to_string),
        reason: TurnAbortReason::Interrupted,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    });
    assert!(state.should_observe(&aborted));
    state.observe(&aborted);
    state.observe(&EventMsg::RealtimeConversationStarted(
        RealtimeConversationStartedEvent {
            realtime_session_id: Some("voice-2".to_string()),
            version: RealtimeConversationVersion::V2,
        },
    ));

    let promoted = state
        .observe(&completed_item(dynamic_tool_call(
            "late-tool",
            DynamicToolCallStatus::Completed,
        )))
        .items;
    assert_eq!(
        promoted
            .iter()
            .map(|item| item.realtime_session_id.as_str())
            .collect::<Vec<_>>(),
        expected_session_id.into_iter().collect::<Vec<_>>()
    );
}

#[test]
fn interrupted_turn_keeps_its_existing_voice_session_for_late_artifacts() {
    let mut state = started_state();
    state.observe(&EventMsg::TurnAborted(TurnAbortedEvent {
        root_turn_id: None,
        turn_id: Some("turn-1".to_string()),
        reason: TurnAbortReason::Interrupted,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    }));
    state.observe(&EventMsg::RealtimeConversationStarted(
        RealtimeConversationStartedEvent {
            realtime_session_id: Some("voice-2".to_string()),
            version: RealtimeConversationVersion::V2,
        },
    ));
    let promoted = state
        .observe(&completed_item(dynamic_tool_call(
            "late-tool",
            DynamicToolCallStatus::Completed,
        )))
        .items;
    assert_eq!(
        promoted
            .iter()
            .map(|item| item.realtime_session_id.as_str())
            .collect::<Vec<_>>(),
        vec!["voice-1"]
    );
}

#[test]
fn promotes_backing_agent_artifacts_once_without_a_client_request() {
    let mut state = started_state();
    let first_delta = assistant_delta("message-1", "[analysis] ::codex-realtime-inline{}");
    assert!(state.observe(&first_delta).items.is_empty());

    let second_delta = assistant_delta("message-1", "\nVisible explanation");
    let effects = state.observe(&second_delta);
    assert_eq!(effects.order, RealtimeEventOrder::AfterEvent);
    assert_eq!(
        contents(effects),
        vec![RealtimeItemContent::BemItemPromoted {
            turn_id: "turn-1".to_string(),
            item_id: "message-1".to_string(),
            presentation: BemItemPresentation::InlineMarkdown,
        }]
    );

    let completed = completed_item(TurnItem::AgentMessage(AgentMessageItem {
        moderation: None,
        id: "message-1".to_string(),
        content: vec![AgentMessageContent::Text {
            text: "[analysis] ::codex-realtime-inline{}\nVisible explanation".to_string(),
        }],
        phase: None,
        memory_citation: None,
        delivery: None,
        questions: None,
    }));
    assert!(state.observe(&completed).items.is_empty());

    let next_message = assistant_delta("message-2", "::codex-realtime-inline{}\nNext message");
    assert_eq!(
        contents(state.observe(&next_message)),
        vec![RealtimeItemContent::BemItemPromoted {
            turn_id: "turn-1".to_string(),
            item_id: "message-2".to_string(),
            presentation: BemItemPresentation::InlineMarkdown,
        }]
    );

    let image = EventMsg::ItemStarted(ItemStartedEvent {
        thread_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        item: TurnItem::ImageGeneration(ImageGenerationItem {
            id: "image-1".to_string(),
            status: "in_progress".to_string(),
            revised_prompt: None,
            result: String::new(),
            saved_path: None,
        }),
        started_at_ms: 0,
    });
    let subagent = completed_item(TurnItem::SubAgentActivity(SubAgentActivityItem {
        model: None,
        reasoning_effort: None,
        id: "subagent-1".to_string(),
        kind: SubAgentActivityKind::Started,
        agent_thread_id: ThreadId::new(),
        agent_path: AgentPath::root(),
    }));
    for (event, item_id) in [(image, "image-1"), (subagent, "subagent-1")] {
        assert_eq!(
            contents(state.observe(&event)),
            vec![RealtimeItemContent::BemItemPromoted {
                turn_id: "turn-1".to_string(),
                item_id: item_id.to_string(),
                presentation: BemItemPresentation::WholeItem,
            }]
        );
    }
    state.observe(&EventMsg::RealtimeConversationClosed(
        RealtimeConversationClosedEvent { reason: None },
    ));
    let late = state.observe(&assistant_delta(
        "late-artifact",
        "::codex-realtime-inline{}\npresent later",
    ));
    assert_eq!(late.items.len(), 1);
    assert_eq!(late.items[0].realtime_session_id, "voice-1");
}

#[test]
fn promotes_distinct_visualizations_once_and_ignores_markdown_fences() {
    let mut state = started_state();
    let text = "```\n::codex-inline-vis{file=hidden}\n```\n~~~\nvisualize{file=also-hidden}\n~~~\n::codex-inline-vis{file=first}\nvisualize{file=second}";
    let message = assistant_delta("message-1", text);
    assert_eq!(
        contents(state.observe(&message)),
        (0..2)
            .map(|index| RealtimeItemContent::BemItemPromoted {
                turn_id: "turn-1".to_string(),
                item_id: "message-1".to_string(),
                presentation: BemItemPresentation::InlineVisualization { index },
            })
            .collect::<Vec<_>>()
    );

    let completed = completed_item(TurnItem::AgentMessage(AgentMessageItem {
        moderation: None,
        id: "message-1".to_string(),
        content: vec![AgentMessageContent::Text {
            text: text.to_string(),
        }],
        phase: None,
        memory_citation: None,
        delivery: None,
        questions: None,
    }));
    assert!(state.observe(&completed).items.is_empty());
}

#[test]
fn streams_stable_segment_items_and_ignores_upstream_transcript_revisions() {
    let mut state = started_state();
    let effects = observe_realtime(
        &mut state,
        RealtimeEvent::OutputTranscriptDelta(RealtimeTranscriptDelta {
            delta: "hello".to_string(),
        }),
    );
    assert!(effects.items.is_empty());
    let stream = effects.transcript_stream.expect("streaming delta");
    let started = stream.started_item.expect("segment start");
    assert_eq!(stream.item_id, started.id);
    assert_eq!(Uuid::parse_str(&started.id).unwrap().get_version_num(), 7);
    assert_eq!(stream.delta, "hello");

    let done = observe_realtime(
        &mut state,
        RealtimeEvent::OutputTranscriptDone(RealtimeTranscriptDone {
            text: "a substantially different upstream revision".to_string(),
        }),
    );
    assert_eq!(
        done.items,
        vec![RealtimeItem {
            id: started.id,
            observed_at_ms: started.observed_at_ms,
            realtime_session_id: "voice-1".to_string(),
            content: RealtimeItemContent::TranscriptSegment {
                role: RealtimeTranscriptRole::Assistant,
                text: "hello".to_string(),
            },
        }],
    );
    assert!(done.transcript_stream.is_none());
}

#[test]
fn emits_the_complete_item_lifecycle_when_only_a_final_transcript_arrives() {
    let mut state = started_state();
    let effects = observe_realtime(
        &mut state,
        RealtimeEvent::OutputTranscriptDone(RealtimeTranscriptDone {
            text: "final transcript".to_string(),
        }),
    );
    let stream = effects.transcript_stream.expect("final transcript stream");
    let started = stream.started_item.expect("transcript item started");
    assert_eq!(stream.item_id, started.id);
    assert_eq!(stream.delta, "final transcript");
    assert_eq!(
        effects.items,
        vec![RealtimeItem {
            id: started.id,
            observed_at_ms: started.observed_at_ms,
            realtime_session_id: "voice-1".to_string(),
            content: RealtimeItemContent::TranscriptSegment {
                role: RealtimeTranscriptRole::Assistant,
                text: "final transcript".to_string(),
            },
        }]
    );
}

#[test]
fn preserves_first_transcript_activity_order_when_both_speakers_are_active() {
    for roles in [
        [
            RealtimeTranscriptRole::Assistant,
            RealtimeTranscriptRole::User,
        ],
        [
            RealtimeTranscriptRole::User,
            RealtimeTranscriptRole::Assistant,
        ],
    ] {
        let mut state = started_state();
        for role in roles {
            let delta = RealtimeTranscriptDelta {
                delta: format!("{role:?} speech"),
            };
            observe_realtime(
                &mut state,
                match role {
                    RealtimeTranscriptRole::User => RealtimeEvent::InputTranscriptDelta(delta),
                    RealtimeTranscriptRole::Assistant => {
                        RealtimeEvent::OutputTranscriptDelta(delta)
                    }
                },
            );
        }
        let items = state
            .observe(&EventMsg::RealtimeConversationClosed(
                RealtimeConversationClosedEvent { reason: None },
            ))
            .items;
        let observed_roles = items
            .iter()
            .filter_map(|item| match item.content {
                RealtimeItemContent::TranscriptSegment { role, .. } => Some(role),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(observed_roles, roles);
    }
}

#[test]
fn does_not_replay_a_final_transcript_after_a_promotion_split() {
    let mut state = started_state();
    observe_realtime(
        &mut state,
        RealtimeEvent::OutputTranscriptDelta(RealtimeTranscriptDelta {
            delta: "Already spoken".to_string(),
        }),
    );
    let promotion = completed_item(dynamic_tool_call(
        "successful-tool",
        DynamicToolCallStatus::Completed,
    ));
    assert_eq!(state.observe(&promotion).items.len(), 2);

    let done = observe_realtime(
        &mut state,
        RealtimeEvent::OutputTranscriptDone(RealtimeTranscriptDone {
            text: "Already spoken".to_string(),
        }),
    );
    assert!(done.items.is_empty());
    assert!(done.transcript_stream.is_none());
}

#[test]
fn generates_distinct_boundary_ids_for_reused_realtime_sessions() {
    let mut state = RealtimeHistoryState::default();
    let mut boundary_ids = Vec::new();
    for _ in 0..2 {
        let started = state.observe(&EventMsg::RealtimeConversationStarted(
            RealtimeConversationStartedEvent {
                realtime_session_id: Some("reused-session".to_string()),
                version: RealtimeConversationVersion::V2,
            },
        ));
        let closed = state.observe(&EventMsg::RealtimeConversationClosed(
            RealtimeConversationClosedEvent { reason: None },
        ));
        for item in started.items.into_iter().chain(closed.items) {
            assert_eq!(item.realtime_session_id, "reused-session");
            assert_eq!(Uuid::parse_str(&item.id).unwrap().get_version_num(), 7);
            boundary_ids.push(item.id);
        }
    }
    let unique_ids = boundary_ids
        .iter()
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(unique_ids.len(), 4);

    let fallback = state.observe(&EventMsg::RealtimeConversationStarted(
        RealtimeConversationStartedEvent {
            realtime_session_id: None,
            version: RealtimeConversationVersion::V2,
        },
    ));
    assert_eq!(fallback.items.len(), 1);
    assert_eq!(
        Uuid::parse_str(&fallback.items[0].realtime_session_id)
            .unwrap()
            .get_version_num(),
        7
    );
}

#[test]
fn promotes_successful_codex_app_mcp_calls_and_splits_active_transcripts() {
    let mut state = started_state();
    observe_realtime(
        &mut state,
        RealtimeEvent::OutputTranscriptDelta(RealtimeTranscriptDelta {
            delta: "Before tool.".to_string(),
        }),
    );
    for (server, status) in [
        ("other_server", McpToolCallStatus::Completed),
        ("codex_app", McpToolCallStatus::InProgress),
        ("codex_app", McpToolCallStatus::Failed),
    ] {
        assert!(
            state
                .observe(&completed_item(mcp_tool_call(
                    "not-promoted",
                    server,
                    status
                )),)
                .items
                .is_empty()
        );
    }
    let completed = completed_item(mcp_tool_call(
        "app-tool",
        "codex_app",
        McpToolCallStatus::Completed,
    ));
    assert_eq!(
        contents(state.observe(&completed)),
        vec![
            RealtimeItemContent::TranscriptSegment {
                role: RealtimeTranscriptRole::Assistant,
                text: "Before tool.".to_string(),
            },
            RealtimeItemContent::BemItemPromoted {
                turn_id: "turn-1".to_string(),
                item_id: "app-tool".to_string(),
                presentation: BemItemPresentation::WholeItem,
            },
        ]
    );
    assert!(state.observe(&completed).items.is_empty());
}

#[test]
fn promotes_successful_dynamic_tools_and_splits_active_transcripts() {
    let mut state = started_state();
    let first = observe_realtime(
        &mut state,
        RealtimeEvent::OutputTranscriptDelta(RealtimeTranscriptDelta {
            delta: "Before tool. ".to_string(),
        }),
    )
    .transcript_stream
    .expect("first transcript stream");

    let failed = completed_item(dynamic_tool_call(
        "failed-tool",
        DynamicToolCallStatus::Failed,
    ));
    assert!(state.observe(&failed).items.is_empty());

    let completed = completed_item(dynamic_tool_call(
        "successful-tool",
        DynamicToolCallStatus::Completed,
    ));
    let effects = state.observe(&completed);
    assert_eq!(effects.order, RealtimeEventOrder::AfterEvent);
    let items = effects.items;
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0],
        RealtimeItem {
            id: first.item_id.clone(),
            observed_at_ms: first.started_item.as_ref().unwrap().observed_at_ms,
            realtime_session_id: "voice-1".to_string(),
            content: RealtimeItemContent::TranscriptSegment {
                role: RealtimeTranscriptRole::Assistant,
                text: "Before tool. ".to_string(),
            },
        }
    );
    assert_eq!(
        items[1].content,
        RealtimeItemContent::BemItemPromoted {
            turn_id: "turn-1".to_string(),
            item_id: "successful-tool".to_string(),
            presentation: BemItemPresentation::WholeItem,
        }
    );
    assert_eq!(Uuid::parse_str(&items[1].id).unwrap().get_version_num(), 7);

    let second = observe_realtime(
        &mut state,
        RealtimeEvent::OutputTranscriptDelta(RealtimeTranscriptDelta {
            delta: "After tool.".to_string(),
        }),
    )
    .transcript_stream
    .expect("second transcript stream");
    assert_ne!(first.item_id, second.item_id);
    assert!(second.started_item.is_some());
    assert!(state.observe(&completed).items.is_empty());

    let closed = state.observe(&EventMsg::RealtimeConversationClosed(
        RealtimeConversationClosedEvent { reason: None },
    ));
    assert_eq!(closed.items[0].id, second.item_id);
    let late = completed_item(dynamic_tool_call(
        "late-tool",
        DynamicToolCallStatus::Completed,
    ));
    assert!(state.observe(&late).items.is_empty());

    state.observe(&EventMsg::RealtimeConversationStarted(
        RealtimeConversationStartedEvent {
            realtime_session_id: Some("voice-2".to_string()),
            version: RealtimeConversationVersion::V2,
        },
    ));
    let resumed = state.observe(&late).items;
    assert_eq!(resumed.len(), 1);
    assert_eq!(resumed[0].realtime_session_id, "voice-2");
}

#[test]
fn overlapping_committed_handoffs_seal_each_frame_once_and_rejection_releases_it() {
    use codex_protocol::protocol::RealtimeHandoffRequested;

    let mut state = started_state();
    state.frame_enrollment = Some(true);
    let request = |id: &str| RealtimeHandoffRequested {
        handoff_id: id.to_owned(),
        item_id: id.to_owned(),
        input_transcript: String::new(),
        active_transcript: Vec::new(),
    };
    let first_frame = observe_realtime(
        &mut state,
        RealtimeEvent::InputTranscriptDelta(RealtimeTranscriptDelta {
            delta: "first".to_owned(),
        }),
    )
    .transcript_stream
    .unwrap()
    .item_id;
    let first = state.prepare_handoff(&request("first")).unwrap();
    assert!(state.reserve_handoff(first.id));
    let second_frame = observe_realtime(
        &mut state,
        RealtimeEvent::InputTranscriptDelta(RealtimeTranscriptDelta {
            delta: " second".to_owned(),
        }),
    )
    .transcript_stream
    .unwrap()
    .item_id;
    let second = state.prepare_handoff(&request("second")).unwrap();
    assert!(state.reserve_handoff(second.id));

    assert_eq!(
        first
            .new_items
            .iter()
            .map(|item| &item.id)
            .collect::<Vec<_>>(),
        vec![&first_frame]
    );
    assert_eq!(
        second
            .new_items
            .iter()
            .map(|item| &item.id)
            .collect::<Vec<_>>(),
        vec![&second_frame]
    );
    state.finish_handoff(first.id, Some("turn-first"));
    state.finish_handoff(second.id, Some("turn-second"));

    observe_realtime(
        &mut state,
        RealtimeEvent::InputTranscriptDelta(RealtimeTranscriptDelta {
            delta: " third".to_owned(),
        }),
    );
    let rejected = state.prepare_handoff(&request("rejected")).unwrap();
    assert!(state.reserve_handoff(rejected.id));
    state.reject_handoff(rejected.id);
    let retry = state.prepare_handoff(&request("retry")).unwrap();
    assert_eq!(retry.new_items, rejected.new_items);
}

#[test_case(|item| EventMsg::ItemStarted(ItemStartedEvent {
    thread_id: ThreadId::new(),
    turn_id: "turn-1".to_string(),
    item,
    started_at_ms: 0,
}); "started")]
#[test_case(completed_item; "completed")]
fn typed_input_seals_both_roles_but_realtime_delegation_does_not(
    user_event: fn(TurnItem) -> EventMsg,
) {
    let mut state = started_state();
    for event in [
        RealtimeEvent::OutputTranscriptDelta(RealtimeTranscriptDelta {
            delta: "assistant".to_string(),
        }),
        RealtimeEvent::InputTranscriptDelta(RealtimeTranscriptDelta {
            delta: "user".to_string(),
        }),
    ] {
        observe_realtime(&mut state, event);
    }
    assert!(
        state
            .observe(&user_event(TurnItem::UserMessage(UserMessageItem::new(&[
                UserInput::Text {
                    text: "<realtime_delegation>delegated</realtime_delegation>".to_string(),
                    text_elements: Vec::new(),
                }
            ]))))
            .items
            .is_empty()
    );
    let effects = state.observe(&user_event(TurnItem::UserMessage(UserMessageItem::new(&[
        UserInput::Text {
            text: "typed steering".to_string(),
            text_elements: Vec::new(),
        },
    ]))));
    assert_eq!(effects.order, RealtimeEventOrder::BeforeEvent);
    assert_eq!(
        contents(effects),
        vec![
            RealtimeItemContent::TranscriptSegment {
                role: RealtimeTranscriptRole::Assistant,
                text: "assistant".to_string()
            },
            RealtimeItemContent::TranscriptSegment {
                role: RealtimeTranscriptRole::User,
                text: "user".to_string()
            },
        ]
    );
    for (role, event) in [
        (
            RealtimeTranscriptRole::Assistant,
            RealtimeEvent::OutputTranscriptDone(RealtimeTranscriptDone {
                text: "assistant revised".to_string(),
            }),
        ),
        (
            RealtimeTranscriptRole::User,
            RealtimeEvent::InputTranscriptDone(RealtimeTranscriptDone {
                text: "user revised".to_string(),
            }),
        ),
    ] {
        assert_eq!(
            contents(observe_realtime(&mut state, event)),
            vec![RealtimeItemContent::TranscriptSegment {
                role,
                text: " revised".to_string(),
            }],
        );
    }
}

#[test]
fn recovered_v3_speech_excludes_exact_consumed_frames_and_precedes_new_chunks() {
    use super::handoff::RecoveredSessionStatus;
    use codex_protocol::protocol::RealtimeTranscriptEntry;
    use codex_protocol::realtime::SessionInputReason;
    let item = |id: &str, content| RealtimeItem {
        id: id.to_owned(),
        realtime_session_id: "call".to_owned(),
        observed_at_ms: Some(1),
        content,
    };
    let frame = |id: &str, role, text: &str| {
        item(
            id,
            RealtimeItemContent::Frame {
                channels: RealtimeFrameChannels {
                    voice_transcript: RealtimeVoiceTranscript {
                        role,
                        text: text.to_owned(),
                    },
                },
            },
        )
    };
    let items = vec![
        item("start", RealtimeItemContent::RealtimeSessionStarted),
        frame("a1", RealtimeTranscriptRole::User, "first"),
        frame("b1", RealtimeTranscriptRole::Assistant, "pending"),
        frame("a2", RealtimeTranscriptRole::User, "last"),
        item(
            "chunk",
            RealtimeItemContent::SessionInputSaved {
                reason: SessionInputReason::Chunk,
                source_event_ids: vec!["a1".to_owned(), "a2".to_owned()],
                input_item_id: "input".to_owned(),
                input: "first last".to_owned(),
            },
        ),
    ];
    let mut state = RealtimeHistoryState::default();
    assert_eq!(
        state.recover_session("call", &items),
        RecoveredSessionStatus::Open
    );
    let new = vec![RealtimeTranscriptEntry {
        role: "user".to_owned(),
        text: "new".to_owned(),
    }];
    assert_eq!(
        state.prepare_recorded_chunk(&new),
        (
            vec![
                RealtimeTranscriptEntry {
                    role: "assistant".to_owned(),
                    text: "pending".to_owned()
                },
                new[0].clone()
            ],
            vec!["b1".to_owned()]
        )
    );
    let mut ending = items;
    ending.push(item(
        "end",
        RealtimeItemContent::SessionInputSaved {
            reason: SessionInputReason::SessionEnd,
            source_event_ids: vec!["b1".to_owned()],
            input_item_id: "tail".to_owned(),
            input: "pending".to_owned(),
        },
    ));
    assert_eq!(
        state.recover_session("call", &ending),
        RecoveredSessionStatus::Ending
    );
}
#[tokio::test]
async fn handoff_settlement_releases_rejected_sources_but_preserves_uncertain_seals() {
    use codex_protocol::models::ResponseItem;
    use codex_protocol::protocol::RealtimeHandoffRequested;
    let mut state = started_state();
    state.frame_enrollment = Some(true);
    let source = observe_realtime(
        &mut state,
        RealtimeEvent::InputTranscriptDone(RealtimeTranscriptDone {
            text: "speech".into(),
        }),
    )
    .items
    .remove(0)
    .id;
    let request = RealtimeHandoffRequested {
        handoff_id: "handoff".into(),
        item_id: "item".into(),
        input_transcript: "speech".into(),
        active_transcript: Vec::new(),
    };
    let prepared = state.prepare_handoff(&request).unwrap();
    assert!(state.reserve_handoff(prepared.id));
    let handoff::HandoffSettlement::Wait(settlement) = state.pending_handoff_settlement().unwrap()
    else {
        panic!("reserved handoff should wait for settlement");
    };
    state.reject_handoff(prepared.id);
    settlement.notified().await;
    assert!(matches!(
        state.pending_handoff_settlement().unwrap(),
        handoff::HandoffSettlement::Settled
    ));

    let retry = state.prepare_handoff(&request).unwrap();
    assert_eq!(retry.sources, vec![source]);
    assert!(state.reserve_handoff(retry.id));
    state
        .cache_handoff_input(retry.id, ResponseItem::CompactionTrigger {}.into())
        .unwrap();
    state.reject_handoff(retry.id);
    assert!(state.pending_handoff_settlement().is_err());
    assert!(
        state.prepare_carryover("hangup").is_none(),
        "an uncertain seal must not release sources to carryover"
    );
    let same = state.prepare_handoff(&request).unwrap();
    assert_eq!(same.id, retry.id);
    assert!(state.reserve_handoff(same.id));
    state.finish_handoff(same.id, /*turn_id*/ None);
    assert!(matches!(
        state.pending_handoff_settlement().unwrap(),
        handoff::HandoffSettlement::Settled
    ));
}

#[test]
fn handoff_seals_stable_prefix_and_rejection_recomputes_sources() {
    use codex_protocol::protocol::RealtimeHandoffRequested;
    let mut state = started_state();
    state.frame_enrollment = Some(true);
    let stream = observe_realtime(
        &mut state,
        RealtimeEvent::InputTranscriptDelta(RealtimeTranscriptDelta {
            delta: "partial speech".into(),
        }),
    )
    .transcript_stream
    .unwrap();
    let request = RealtimeHandoffRequested {
        handoff_id: "handoff".into(),
        item_id: "upstream".into(),
        input_transcript: "".into(),
        active_transcript: Vec::new(),
    };
    let prepared = state.prepare_handoff(&request).unwrap();
    assert_eq!(prepared.sources, vec![stream.item_id.clone()]);
    assert_eq!(prepared.new_items[0].id, stream.item_id);
    assert!(prepared.input.contains("user: partial speech"));
    assert!(state.reserve_handoff(prepared.id));
    assert!(!state.reserve_handoff(prepared.id));
    // An in-flight input reserves its source; later deltas form a new event.
    let continued = observe_realtime(
        &mut state,
        RealtimeEvent::InputTranscriptDelta(RealtimeTranscriptDelta {
            delta: " more".into(),
        }),
    )
    .transcript_stream
    .unwrap();
    assert_ne!(continued.item_id, stream.item_id);
    let overlapping = state
        .prepare_handoff(&RealtimeHandoffRequested {
            handoff_id: "overlapping".into(),
            ..request.clone()
        })
        .unwrap();
    assert_eq!(overlapping.sources, vec![continued.item_id]);
    // A definite rejection frees the sources; a retry builds a fresh snapshot.
    state.reject_handoff(prepared.id);
    state.finish_handoff(overlapping.id, /*turn_id*/ None);
    let retry = state.prepare_handoff(&request).unwrap();
    assert_ne!(retry.id, prepared.id);
    assert_eq!(retry.input, prepared.input);
    assert_eq!(retry.sources, prepared.sources);
    let next = state
        .prepare_handoff(&RealtimeHandoffRequested {
            handoff_id: "next".into(),
            ..request
        })
        .unwrap();
    assert_eq!(next.sources, vec![stream.item_id]);
}

#[test]
fn handoff_fallback_is_retained_without_inventing_user_speech() {
    let mut state = started_state();
    state.frame_enrollment = Some(true);
    let prepared = state
        .prepare_handoff(&codex_protocol::protocol::RealtimeHandoffRequested {
            handoff_id: "fallback".into(),
            item_id: "upstream".into(),
            input_transcript: "only available in the handoff".into(),
            active_transcript: Vec::new(),
        })
        .unwrap();
    assert!(prepared.sources.is_empty());
    assert!(prepared.new_items.is_empty());
    assert!(
        prepared
            .input
            .contains("<input>only available in the handoff</input>")
    );
    assert!(
        prepared
            .input
            .contains("<transcript_delta>user: only available in the handoff</transcript_delta>")
    );
}

/// A final's added words stay on the original unsealed event and enter handoff context.
#[test]
fn final_transcript_extends_unsealed_event_without_replacing_its_identity() {
    let mut state = started_state();
    state.frame_enrollment = Some(true);
    let start = observe_realtime(
        &mut state,
        RealtimeEvent::OutputTranscriptDelta(RealtimeTranscriptDelta {
            delta: "A bunch of stuff".into(),
        }),
    )
    .transcript_stream
    .unwrap()
    .started_item
    .unwrap();
    let done = observe_realtime(
        &mut state,
        RealtimeEvent::OutputTranscriptDone(RealtimeTranscriptDone {
            text: "A bunch of stuff, including coding.".into(),
        }),
    );
    let stream = done.transcript_stream.unwrap();
    assert_eq!(stream.item_id, start.id);
    assert!(stream.started_item.is_none());
    assert_eq!(stream.delta, ", including coding.");
    assert_eq!(done.items[0].id, start.id);
    assert_eq!(done.items[0].observed_at_ms, start.observed_at_ms);
    assert_eq!(
        done.items[0].content,
        super::speech(
            /*frames_enabled*/ true,
            RealtimeTranscriptRole::Assistant,
            "A bunch of stuff, including coding.".into()
        )
    );
}

/// A final received after a handoff contributes only new words and keeps sealed evidence unchanged.
#[test]
fn final_transcript_after_handoff_seals_only_the_added_suffix() {
    let mut state = started_state();
    state.frame_enrollment = Some(true);
    observe_realtime(
        &mut state,
        RealtimeEvent::OutputTranscriptDelta(RealtimeTranscriptDelta {
            delta: "Already spoken".into(),
        }),
    );
    let first = state
        .prepare_handoff(&codex_protocol::protocol::RealtimeHandoffRequested {
            handoff_id: "first".into(),
            item_id: "first".into(),
            input_transcript: "first task".into(),
            active_transcript: Vec::new(),
        })
        .unwrap();
    let done = observe_realtime(
        &mut state,
        RealtimeEvent::OutputTranscriptDone(RealtimeTranscriptDone {
            text: "Already spoken, with more detail".into(),
        }),
    );
    let stream = done.transcript_stream.unwrap();
    let started = stream.started_item.unwrap();
    assert_ne!(started.id, first.new_items[0].id);
    assert_eq!(stream.delta, ", with more detail");
    assert_eq!(done.items[0].id, started.id);
    assert_eq!(done.items[0].observed_at_ms, started.observed_at_ms);
    assert_eq!(
        done.items[0].content,
        super::speech(
            /*frames_enabled*/ true,
            RealtimeTranscriptRole::Assistant,
            ", with more detail".into()
        )
    );
    assert_eq!(
        first.new_items[0].content,
        super::speech(
            /*frames_enabled*/ true,
            RealtimeTranscriptRole::Assistant,
            "Already spoken".into()
        )
    );
}

#[test]
fn v3_completed_chunk_retires_only_its_own_frames() {
    use codex_protocol::protocol::RealtimeHandoffRequested;
    use codex_protocol::protocol::RealtimeTranscriptEntry;
    let mut state = RealtimeHistoryState {
        frame_enrollment: Some(true),
        ..Default::default()
    };
    state.observe(&EventMsg::RealtimeConversationStarted(
        RealtimeConversationStartedEvent {
            realtime_session_id: Some("v3-call".into()),
            version: RealtimeConversationVersion::V3,
        },
    ));
    let user = |text: &str| {
        RealtimeEvent::InputTranscriptDelta(RealtimeTranscriptDelta { delta: text.into() })
    };
    let user_done = |text: &str| {
        RealtimeEvent::InputTranscriptDone(RealtimeTranscriptDone { text: text.into() })
    };
    let ask = |id: &str| RealtimeHandoffRequested {
        handoff_id: id.into(),
        item_id: id.into(),
        input_transcript: "task".into(),
        active_transcript: Vec::new(),
    };
    let chunk = |text: &str| RealtimeTranscriptEntry {
        role: "user".into(),
        text: text.into(),
    };

    // A rejected request already took this utterance from the socket but did
    // not commit its frame to model history. An identical later chunk must keep it.
    let old = observe_realtime(&mut state, user_done("repeat"))
        .items
        .remove(0)
        .id;
    let request = ask("rejected");
    let rejected = state.prepare_handoff(&request).unwrap();
    state.reject_handoff(rejected.id);
    observe_realtime(&mut state, RealtimeEvent::HandoffRequested(request));
    observe_realtime(&mut state, user_done("repeat"));
    state.consume_recorded_chunk(&[chunk("repeat")]);

    // User starts first; assistant finishes first. Handoff sources must follow
    // the socket's capture order, not the order in which the frames sealed.
    let user_id = observe_realtime(&mut state, user("prefix"))
        .transcript_stream
        .unwrap()
        .item_id;
    let assistant_id = observe_realtime(
        &mut state,
        RealtimeEvent::OutputTranscriptDone(RealtimeTranscriptDone {
            text: "reply".into(),
        }),
    )
    .items
    .remove(0)
    .id;
    let request = ask("accepted");
    let accepted = state.prepare_handoff(&request).unwrap();
    assert_eq!(accepted.sources, vec![old, user_id, assistant_id]);
    state.finish_handoff(accepted.id, /*turn_id*/ None);
    observe_realtime(&mut state, RealtimeEvent::HandoffRequested(request));

    // The transport final repeats the accepted prefix; Core seals only its
    // suffix. Recording the chunk must still retire that exact suffix.
    let final_text = "prefix plus more";
    assert_eq!(
        observe_realtime(&mut state, user_done(final_text))
            .items
            .len(),
        1
    );
    state.consume_recorded_chunk(&[chunk(final_text)]);
    assert!(state.prepare_carryover("ended").is_none());
}
