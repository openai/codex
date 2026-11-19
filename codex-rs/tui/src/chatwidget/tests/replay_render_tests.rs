//! Completed replay messages preserve order, formatting, and composer state.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn completed_replay_preserves_messages_and_draft_across_reconstruction() {
    let mut outputs = Vec::new();
    for _ in 0..2 {
        let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.note_rendered_width(/*width*/ 80);
        chat.handle_paste("Unsent draft".to_string());
        replay_user_message_text(
            &mut chat,
            "user",
            "**Question**",
            ReplayKind::ThreadSnapshot,
        );
        replay_agent_message(
            &mut chat,
            "first",
            "First **answer**",
            ReplayKind::ThreadSnapshot,
        );
        replay_agent_message(
            &mut chat,
            "second",
            "Second `answer`",
            ReplayKind::ThreadSnapshot,
        );
        let cells = drain_insert_history(&mut rx);
        outputs.push(cells.into_iter().flatten().collect::<Vec<_>>());
        assert_eq!(chat.composer_text_with_pending(), "Unsent draft");
    }
    assert_eq!(outputs[0], outputs[1]);
    insta::assert_snapshot!(outputs[0].iter().map(ToString::to_string).collect::<Vec<_>>().join("\n").trim(), @"
    › Question


    • First answer

    • Second answer
    ");
}

#[tokio::test]
async fn hidden_cloud_reply_preserves_only_this_turns_committed_notification() {
    let deliver = |chat: &mut ChatWidget, turn: &str, id: &str, text: &str, from_replay| {
        let item = AgentMessageItem {
            id: id.into(),
            content: vec![AgentMessageContent::Text { text: text.into() }],
            moderation: None,
            phase: None,
            memory_citation: None,
            delivery: None,
            questions: None,
        };
        chat.on_agent_message_item_completed(item, turn, from_replay);
    };
    let notification = |chat: &mut ChatWidget| {
        chat.on_task_complete(
            /*last_agent_message*/ None, /*completion*/ None, /*from_replay*/ false,
        );
        let Some(Notification::AgentTurnComplete { response }) = chat.pending_notification.take()
        else {
            panic!("missing turn completion notification")
        };
        response
    };
    let (mut local, _, _, _) = make_chatwidget_manual_with_sender().await;
    let mut turn = app_server_turn(
        "current",
        AppServerTurnStatus::InProgress,
        /*duration_ms*/ None,
        /*error*/ None,
    );
    turn.items
        .push(codex_app_server_protocol::ThreadItem::untimed(
            AppServerThreadItemPayload::AgentMessage {
                id: "legacy".into(),
                text: "Legacy answer".into(),
                moderation: None,
                phase: Some(MessagePhase::FinalAnswer),
                memory_citation: None,
                delivery: None,
                questions: None,
            },
        ));
    local.replay_backend_thread_turns(
        vec![turn],
        ReplayKind::ThreadSnapshot,
        &crate::AppServerTarget::Embedded,
    );
    assert_eq!(notification(&mut local), "Legacy answer");

    let (mut chat, _, _, _) = make_chatwidget_manual_with_sender().await;
    chat.transcript.replayed_cloud_text = Some(Default::default());
    deliver(&mut chat, "old", "reply", "Partial", true);
    handle_turn_started(&mut chat, "new");
    deliver(&mut chat, "old", "reply", "Final", false);
    insta::assert_snapshot!(chat.bottom_pane.status_widget().unwrap().header(), @"Working");

    let (mut ordered, _, mut ordered_events, _) = make_chatwidget_manual_with_sender().await;
    ordered.transcript.replayed_cloud_text = Some(Default::default());
    ordered.on_task_started();
    ordered.turn_lifecycle.last_turn_id = Some("current".into());
    ordered.transcript.active_cell = Some(Box::new(history_cell::PlainHistoryCell::new(vec![
        "Tool activity".into(),
    ])));
    deliver(&mut ordered, "current", "blank", "", true);
    let inserted = std::iter::from_fn(|| ordered_events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(cell),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(inserted.len(), 2);
    assert!(inserted[0].as_any().is::<history_cell::PlainHistoryCell>());
    assert!(
        inserted[1]
            .as_any()
            .is::<history_cell::PendingAgentMessageCell>()
    );
    let older_entry = |id: &str, text: &str| {
        let payload = serde_json::json!({"type":"agentMessage", "id":id, "text":text});
        codex_app_server_protocol::ThreadItem::untimed(serde_json::from_value(payload).unwrap())
    };
    let in_progress = AppServerTurnStatus::InProgress;
    let mut later = older_entry("later", "Later answer");
    later.completed_at_ms = Some(2);
    ordered.remember_older_cloud_reply("current", &later, &in_progress);
    ordered.remember_older_cloud_reply("current", &older_entry("blank", ""), &in_progress);
    deliver(&mut ordered, "current", "blank", "Earlier answer", false);
    assert_eq!(ordered.last_agent_markdown_text(), Some("Later answer"));
    assert_eq!(notification(&mut ordered), "Later answer");

    let (mut page_only, _, _, _) = make_chatwidget_manual_with_sender().await;
    page_only.transcript.replayed_cloud_text = Some(Default::default());
    page_only.on_task_started();
    page_only.turn_lifecycle.last_turn_id = Some("current".into());
    page_only.remember_older_cloud_reply("current", &older_entry("newer", ""), &in_progress);
    page_only.remember_older_cloud_reply("current", &older_entry("first", ""), &in_progress);
    assert_eq!(page_only.last_agent_markdown_text(), None);
    deliver(&mut page_only, "current", "first", "Page final", false);
    assert_eq!(page_only.last_agent_markdown_text(), Some("Page final"));
    deliver(&mut page_only, "current", "newer", "Newer final", false);
    assert_eq!(page_only.last_agent_markdown_text(), Some("Newer final"));
    assert_eq!(notification(&mut page_only), "Newer final");

    for same_turn in [false, true] {
        for (delayed_correction, reverse) in [(false, false), (true, false), (false, true)] {
            let (mut chat, _, _, _) = make_chatwidget_manual_with_sender().await;
            chat.transcript
                .record_agent_markdown("Earlier reply".into(), "Earlier reply".into());
            chat.on_task_started();
            chat.turn_lifecycle.last_turn_id = Some("current".into());
            chat.transcript.replayed_cloud_text = Some(Default::default());
            let earlier_turn = if same_turn { "current" } else { "older" };
            deliver(&mut chat, earlier_turn, "earlier", "Earlier par", true);
            deliver(&mut chat, "current", "hidden", "::git-st", true);
            assert_eq!(chat.last_agent_markdown_text(), Some("::git-st"));
            if reverse {
                deliver(&mut chat, "current", "hidden", "Later final", false);
            }
            chat.transcript.recording_finished_cloud_text = reverse;
            deliver(&mut chat, earlier_turn, "earlier", "Earlier reply", reverse);
            let expected_copy = if reverse { "Later final" } else { "::git-st" };
            assert_eq!(chat.last_agent_markdown_text(), Some(expected_copy));
            if reverse {
                assert_eq!(notification(&mut chat), "Later final");
                continue;
            }
            let directive = r#"::git-stage{cwd="/repo"}"#;
            deliver(&mut chat, "current", "hidden", directive, false);
            let key = ("current".to_owned(), "hidden".to_owned());
            let previous = history_cell::AgentMarkdownCell::new(
                "Earlier reply".into(),
                chat.config.cwd.as_path(),
            );
            if !delayed_correction {
                chat.reconcile_corrected_cloud_copy(&key, Some(&previous));
            }
            let expected = if same_turn { "Earlier reply" } else { "" };
            assert_eq!(notification(&mut chat), expected);
            if delayed_correction {
                chat.reconcile_corrected_cloud_copy(&key, Some(&previous));
            }
            assert_eq!(chat.last_agent_markdown_text(), Some("Earlier reply"));
            chat.transcript.active_streamed_agent_message = Some(("next".into(), "live".into()));
            chat.on_agent_message_delta("Still streaming".into());
            deliver(&mut chat, "older", "untracked", directive, false);
            assert!(chat.stream_controller.is_some());
            let transcript = &chat.transcript;
            assert!(transcript.has_replayed_cloud_text("older", "untracked"));
        }
    }
    for plan_first in [false, true] {
        let (mut chat, _, _, _) = make_chatwidget_manual_with_sender().await;
        chat.transcript.replayed_cloud_text = Some(Default::default());
        deliver(&mut chat, "current", "blank", "", true);
        chat.transcript.last_flushed_agent_message = Some(("current".into(), "blank".into()));
        deliver(&mut chat, "current", "blank", "Visible", false);
        assert_eq!(chat.last_agent_markdown_text(), Some("Visible"));
        if plan_first {
            chat.on_plan_item_completed("Plan".into());
        }
        deliver(&mut chat, "current", "agent", "Part", true);
        if !plan_first {
            chat.on_plan_item_completed("Plan".into());
        }
        deliver(&mut chat, "current", "agent", "Final", false);
        let expected = if plan_first { "Final" } else { "Plan" };
        assert_eq!(chat.last_agent_markdown_text(), Some(expected));
        assert_eq!(notification(&mut chat), expected);
    }
}
