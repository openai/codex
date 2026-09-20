//! Exercise compact streaming and source-backed completion without list-specific holdback.

use super::*;
use crate::local_settings::LocalSettings;
use codex_config::types::AltScreenMode;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn list_spacing_streams_compact_then_reflows_on_completion_or_interruption() {
    let source = "1. One\n2. This item has enough words to wrap\n3. Three\n4. Four";
    let mut stages = Vec::new();
    for (enabled, alternate_screen) in [
        (true, AltScreenMode::Always),
        (false, AltScreenMode::Always),
        (true, AltScreenMode::Never),
    ] {
        for plan in [false, true] {
            for interrupted in [false, true] {
                let (mut chat, mut rx, _ops) =
                    make_chatwidget_manual(/*model_override*/ None).await;
                chat.config.tui_fullscreen_transcript = enabled;
                chat.config.tui_alternate_screen = alternate_screen;
                chat.local_settings = LocalSettings::from(&chat.config);
                let owned = chat.local_settings.transcript_mode.is_owned();
                let width = if plan { 28 } else { 26 };
                chat.note_rendered_width(width);
                if plan {
                    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
                    chat.set_collaboration_mask(CollaborationModeMask {
                        name: "Plan".into(),
                        mode: Some(ModeKind::Plan),
                        model: None,
                        reasoning_effort: None,
                        developer_instructions: None,
                    });
                }
                drain_insert_history(&mut rx);
                let mut live = Vec::new();
                for chunk in source.split_inclusive('\n') {
                    let (cell, tail) = if plan {
                        chat.on_plan_delta(chunk.into());
                        let controller = chat.plan_stream_controller.as_mut().unwrap();
                        (
                            controller.on_commit_tick_batch(usize::MAX).0,
                            controller.current_tail_display_lines(),
                        )
                    } else {
                        chat.on_agent_message_delta(chunk.into());
                        let controller = chat.stream_controller.as_mut().unwrap();
                        (
                            controller.on_commit_tick_batch(usize::MAX).0,
                            controller.current_tail_lines(),
                        )
                    };
                    live.extend(
                        drain_insert_history_with(&mut rx, |cell| cell.display_lines(width))
                            .into_iter()
                            .flatten(),
                    );
                    if let Some(cell) = cell {
                        live.extend(cell.display_lines(width));
                    }
                    if chunk.ends_with('\n') {
                        assert!(tail.is_empty(), "completed list rows must not be held back");
                    }
                }
                if interrupted {
                    chat.flush_answer_and_plan_streams();
                } else if plan {
                    chat.on_plan_item_completed(source.into());
                } else {
                    chat.finalize_completed_assistant_message(Some(source));
                }
                let mut completed = None;
                while let Ok(event) = rx.try_recv() {
                    match event {
                        AppEvent::InsertHistoryCell(cell) => live.extend(cell.display_lines(width)),
                        AppEvent::ConsolidateAgentMessage {
                            source: final_source,
                            cwd,
                            deferred_history_cell,
                            ..
                        } => {
                            if let Some(cell) = deferred_history_cell {
                                live.extend(cell.display_lines(width));
                            }
                            assert_eq!(final_source.trim_end(), source);
                            completed = Some(Box::new(history_cell::AgentMarkdownCell::new(
                                final_source,
                                &cwd,
                            ))
                                as Box<dyn HistoryCell>);
                        }
                        AppEvent::ConsolidateProposedPlan(final_source) => {
                            assert_eq!(final_source.trim_end(), source);
                            completed = Some(Box::new(history_cell::new_proposed_plan(
                                final_source,
                                &chat.config.cwd,
                            ))
                                as Box<dyn HistoryCell>);
                        }
                        _ => {}
                    }
                }
                let completed = completed.expect("termination must retain source for reflow");
                let text = |lines: Vec<Line<'static>>| {
                    lines
                        .iter()
                        .map(|line| line.to_string().trim_end().to_owned())
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                let render = |width| {
                    if owned {
                        completed.retained_hyperlink_lines(width, /*detailed*/ false)
                    } else {
                        completed.display_hyperlink_lines(width)
                    }
                };
                let narrow = render(width);
                let native = completed.display_hyperlink_lines(width);
                if owned {
                    assert_ne!(native, narrow);
                }
                let wide = render(/*width*/ 100);
                assert_eq!(
                    render(width),
                    narrow,
                    "width and spacing must both key the render cache"
                );
                let live = text(live);
                assert_eq!(live.contains("wrap\n\n"), !owned);
                let stage = format!(
                    "owned={owned}, plan={plan}\nStreaming:\n{live}\nCompleted:\n{}\nWide:\n{}",
                    text(crate::terminal_hyperlinks::visible_lines(narrow)),
                    text(crate::terminal_hyperlinks::visible_lines(wide))
                );
                if enabled && alternate_screen == AltScreenMode::Always {
                    if interrupted {
                        assert_eq!(stages.last(), Some(&stage));
                    } else {
                        stages.push(stage);
                    }
                }
            }
        }
    }
    insta::assert_snapshot!(stages.join("\n\n---\n\n"));
}
