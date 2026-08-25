use super::*;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::DynamicToolCallItem;
use codex_protocol::items::ImageGenerationItem;
use codex_protocol::items::McpToolCallItem;
use codex_protocol::items::SubAgentActivityItem;
use codex_protocol::protocol::AgentMessageContentDeltaEvent;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ItemStartedEvent;
use codex_protocol::protocol::RealtimeConversationClosedEvent;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeConversationStartedEvent;
use codex_protocol::protocol::RealtimeConversationVersion;
use pretty_assertions::assert_eq;
use uuid::Uuid;

fn started_state() -> RealtimeHistoryState {
    let mut state = RealtimeHistoryState::default();
    state.observe(
        &EventMsg::RealtimeConversationStarted(RealtimeConversationStartedEvent {
            realtime_session_id: Some("voice-1".to_string()),
            version: RealtimeConversationVersion::V2,
        }),
        Some("turn-1"),
    );
    state
}

fn observe_realtime(
    state: &mut RealtimeHistoryState,
    payload: RealtimeEvent,
) -> RealtimeEventEffects {
    state.observe(
        &EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent { payload }),
        /*active_turn_id*/ None,
    )
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
        tool: "create_thread".to_string(),
        arguments: serde_json::Value::Null,
        connector_id: None,
        mcp_app_resource_uri: None,
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

#[test]
fn promotes_backing_agent_artifacts_once_without_a_client_request() {
    let mut state = started_state();
    let first_delta = assistant_delta("message-1", "[analysis] ::codex-realtime-inline{}");
    assert!(
        state
            .observe(&first_delta, /*active_turn_id*/ None)
            .items
            .is_empty()
    );

    let second_delta = assistant_delta("message-1", "\nVisible explanation");
    assert_eq!(
        contents(state.observe(&second_delta, /*active_turn_id*/ None)),
        vec![RealtimeItemContent::BemItemPromoted {
            turn_id: "turn-1".to_string(),
            item_id: "message-1".to_string(),
            presentation: BemItemPresentation::InlineMarkdown,
        }]
    );

    let completed = completed_item(TurnItem::AgentMessage(AgentMessageItem {
        id: "message-1".to_string(),
        content: vec![AgentMessageContent::Text {
            text: "[analysis] ::codex-realtime-inline{}\nVisible explanation".to_string(),
        }],
        phase: None,
        memory_citation: None,
        delivery: None,
    }));
    assert!(
        state
            .observe(&completed, /*active_turn_id*/ None)
            .items
            .is_empty()
    );

    let next_message = assistant_delta("message-2", "::codex-realtime-inline{}\nNext message");
    assert_eq!(
        contents(state.observe(&next_message, /*active_turn_id*/ None)),
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
        id: "subagent-1".to_string(),
        kind: SubAgentActivityKind::Started,
        agent_thread_id: ThreadId::new(),
        agent_path: AgentPath::root(),
    }));
    for (event, item_id) in [(image, "image-1"), (subagent, "subagent-1")] {
        assert_eq!(
            contents(state.observe(&event, /*active_turn_id*/ None)),
            vec![RealtimeItemContent::BemItemPromoted {
                turn_id: "turn-1".to_string(),
                item_id: item_id.to_string(),
                presentation: BemItemPresentation::WholeItem,
            }]
        );
    }
}

#[test]
fn promotes_distinct_visualizations_once_and_ignores_markdown_fences() {
    let mut state = started_state();
    let text = "```\n::codex-inline-vis{file=hidden}\n```\n~~~\nvisualize{file=also-hidden}\n~~~\n::codex-inline-vis{file=first}\nvisualize{file=second}";
    let message = assistant_delta("message-1", text);
    assert_eq!(
        contents(state.observe(&message, /*active_turn_id*/ None)),
        (0..2)
            .map(|index| RealtimeItemContent::BemItemPromoted {
                turn_id: "turn-1".to_string(),
                item_id: "message-1".to_string(),
                presentation: BemItemPresentation::InlineVisualization { index },
            })
            .collect::<Vec<_>>()
    );

    let completed = completed_item(TurnItem::AgentMessage(AgentMessageItem {
        id: "message-1".to_string(),
        content: vec![AgentMessageContent::Text {
            text: text.to_string(),
        }],
        phase: None,
        memory_citation: None,
        delivery: None,
    }));
    assert!(
        state
            .observe(&completed, /*active_turn_id*/ None)
            .items
            .is_empty()
    );
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
            .observe(
                &EventMsg::RealtimeConversationClosed(RealtimeConversationClosedEvent {
                    reason: None,
                }),
                /*active_turn_id*/ None,
            )
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
    assert_eq!(
        state
            .observe(&promotion, /*active_turn_id*/ None)
            .items
            .len(),
        2
    );

    let done = observe_realtime(
        &mut state,
        RealtimeEvent::OutputTranscriptDone(RealtimeTranscriptDone {
            text: "Already spoken, with an upstream revision".to_string(),
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
        let started = state.observe(
            &EventMsg::RealtimeConversationStarted(RealtimeConversationStartedEvent {
                realtime_session_id: Some("reused-session".to_string()),
                version: RealtimeConversationVersion::V2,
            }),
            /*active_turn_id*/ None,
        );
        let closed = state.observe(
            &EventMsg::RealtimeConversationClosed(RealtimeConversationClosedEvent { reason: None }),
            /*active_turn_id*/ None,
        );
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

    let fallback = state.observe(
        &EventMsg::RealtimeConversationStarted(RealtimeConversationStartedEvent {
            realtime_session_id: None,
            version: RealtimeConversationVersion::V2,
        }),
        /*active_turn_id*/ None,
    );
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
                .observe(
                    &completed_item(mcp_tool_call("not-promoted", server, status)),
                    /*active_turn_id*/ None,
                )
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
        contents(state.observe(&completed, /*active_turn_id*/ None)),
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
    assert!(
        state
            .observe(&completed, /*active_turn_id*/ None)
            .items
            .is_empty()
    );
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
    assert!(
        state
            .observe(&failed, /*active_turn_id*/ None)
            .items
            .is_empty()
    );

    let completed = completed_item(dynamic_tool_call(
        "successful-tool",
        DynamicToolCallStatus::Completed,
    ));
    let items = state.observe(&completed, /*active_turn_id*/ None).items;
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0],
        RealtimeItem {
            id: first.item_id.clone(),
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
    assert!(
        state
            .observe(&completed, /*active_turn_id*/ None)
            .items
            .is_empty()
    );

    let closed = state.observe(
        &EventMsg::RealtimeConversationClosed(RealtimeConversationClosedEvent { reason: None }),
        /*active_turn_id*/ None,
    );
    assert_eq!(closed.items[0].id, second.item_id);
    let late = completed_item(dynamic_tool_call(
        "late-tool",
        DynamicToolCallStatus::Completed,
    ));
    assert!(
        state
            .observe(&late, /*active_turn_id*/ None)
            .items
            .is_empty()
    );

    state.observe(
        &EventMsg::RealtimeConversationStarted(RealtimeConversationStartedEvent {
            realtime_session_id: Some("voice-2".to_string()),
            version: RealtimeConversationVersion::V2,
        }),
        Some("turn-1"),
    );
    let resumed = state.observe(&late, /*active_turn_id*/ None).items;
    assert_eq!(resumed.len(), 1);
    assert_eq!(resumed[0].realtime_session_id, "voice-2");
}
