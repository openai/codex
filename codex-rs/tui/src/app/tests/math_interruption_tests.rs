use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn unicode_math_termination_preserves_source_through_mode_and_resize() -> Result<()> {
    for (kind, status) in [
        ("answer", TurnStatus::Interrupted),
        ("plan", TurnStatus::Interrupted),
        ("plan_error", TurnStatus::Failed),
        ("plan_policy", TurnStatus::Failed),
    ] {
        for ending in ["\n", ""] {
            let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
            let mut app_server = start_config_write_test_app_server(&app).await?;
            let mut tui = crate::tui::test_support::make_test_tui()?;
            let thread_id = ThreadId::new();
            let plan = kind != "answer";
            if plan {
                app.chat_widget
                    .set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
                app.chat_widget
                    .set_collaboration_mask(CollaborationModeMask {
                        name: "Plan".into(),
                        mode: Some(ModeKind::Plan),
                        model: None,
                        reasoning_effort: None,
                        developer_instructions: None,
                    });
            }
            app.chat_widget.handle_server_notification(
                turn_started_notification(thread_id, "math-turn"),
                /*replay_kind*/ None,
            );
            let source = format!("Intro.\n\n$$\n\\frac{{a+b+c+d+e+f}}{{g+h}}{ending}");
            for delta in ["Intro.\n\n", &source[8..]] {
                let notification = if plan {
                    ServerNotification::PlanDelta(
                        codex_app_server_protocol::PlanDeltaNotification {
                            thread_id: thread_id.to_string(),
                            turn_id: "math-turn".into(),
                            item_id: "math-message".into(),
                            delta: delta.into(),
                        },
                    )
                } else {
                    agent_message_delta_notification(thread_id, "math-turn", "math-message", delta)
                };
                app.chat_widget
                    .handle_server_notification(notification, /*replay_kind*/ None);
                for _ in 0..10 {
                    app.chat_widget.on_commit_tick();
                }
            }
            let mut completed = turn_completed_notification(thread_id, "math-turn", status.clone());
            if matches!(kind, "plan_error" | "plan_policy")
                && let ServerNotification::TurnCompleted(notification) = &mut completed
            {
                notification.turn.error = Some(AppServerTurnError {
                    message: if kind == "plan_policy" {
                        r#"{"error":{"code":"bio_policy"}}"#.into()
                    } else {
                        "Server error".into()
                    },
                    codex_error_info: None,
                    additional_details: None,
                    misalignment: None,
                });
            }
            app.chat_widget
                .handle_server_notification(completed, /*replay_kind*/ None);
            let mut saved = Vec::new();
            while let Ok(event) = events.try_recv() {
                if let AppEvent::ConsolidateAgentMessage { source, .. }
                | AppEvent::ConsolidateProposedPlan(source) = &event
                {
                    saved.push(source.trim_end_matches('\n').to_owned());
                }
                if matches!(
                    event,
                    AppEvent::InsertHistoryCell(_)
                        | AppEvent::ConsolidateAgentMessage { .. }
                        | AppEvent::ConsolidateProposedPlan(_)
                ) {
                    app.handle_event(&mut tui, &mut app_server, event).await?;
                }
            }
            assert_eq!(saved, vec![source.trim_end_matches('\n').to_owned()]);
            let mut expected = [None, None];
            for raw in [false, true, false] {
                app.chat_widget.set_raw_output_mode(raw);
                for width in [80, 20, 80] {
                    let rendered = app
                        .render_transcript_lines_for_reflow(width)
                        .lines
                        .iter()
                        .map(rendered_line_text)
                        .collect::<Vec<_>>()
                        .join("\n");
                    let content = rendered.split_whitespace().collect::<String>();
                    assert_eq!(
                        expected[usize::from(raw)].get_or_insert_with(|| content.clone()),
                        &content
                    );
                    if !raw && width == 20 && !ending.is_empty() && kind != "plan_policy" {
                        insta::assert_snapshot!(format!("{kind}_math_narrow"), rendered);
                    }
                }
            }
        }
    }
    Ok(())
}
