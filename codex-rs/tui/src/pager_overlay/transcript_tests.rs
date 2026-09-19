//! Transcript overlay navigation, pagination, and rendering tests.

use super::*;
use crate::diff_model::FileChange;
use crate::exec_cell::CommandOutput;
use crate::history_cell;
use crate::history_cell::HistoryCell;
use crate::history_cell::ReviewDecision;
use crate::history_cell::new_patch_event;
use codex_app_server_protocol::CommandExecutionSource as ExecCommandSource;
use codex_protocol::parse_command::ParsedCommand;
use crossterm::event::KeyModifiers;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[derive(Debug)]
struct TestCell {
    lines: Vec<Line<'static>>,
}

impl crate::history_cell::HistoryCell for TestCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.lines.clone()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        self.lines.clone()
    }

    fn transcript_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.lines.clone()
    }
}

#[derive(Debug)]
struct LayoutCountingCell {
    layout_calls: Arc<AtomicUsize>,
}

impl crate::history_cell::HistoryCell for LayoutCountingCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.layout_calls.fetch_add(/*val*/ 1, Ordering::Relaxed);
        vec![Line::from("counted")]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![Line::from("counted")]
    }
}

fn default_pager_keymap() -> crate::keymap::PagerKeymap {
    crate::keymap::RuntimeKeymap::defaults().pager
}

fn transcript_overlay(cells: Vec<Arc<dyn HistoryCell>>) -> TranscriptOverlay {
    TranscriptOverlay::new(cells, default_pager_keymap())
}

#[test]
fn jump_top_requests_older_history_from_the_bottom() {
    let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
        lines: vec![Line::from("recent")],
    })]);

    let home = KeyEvent::new(KeyCode::Home, crossterm::event::KeyModifiers::NONE);

    assert!(overlay.should_load_older(home));
    assert!(overlay.should_load_from_start(home));
    assert!(!overlay.should_load_from_start(KeyEvent::new(
        KeyCode::PageUp,
        crossterm::event::KeyModifiers::NONE,
    )));
}

#[test]
fn first_upward_key_requests_history_when_the_loaded_start_is_visible() {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 10,
    );
    for committed in [false, true] {
        for (line_count, near_start) in [(0, true), (3, true), (7, true), (30, false)] {
            let lines = (0..line_count)
                .map(|index| Line::from(format!("line {index}")))
                .collect::<Vec<_>>();
            let cells = if committed && !lines.is_empty() {
                vec![Arc::new(TestCell {
                    lines: lines.clone(),
                }) as Arc<dyn HistoryCell>]
            } else {
                Vec::new()
            };
            let mut overlay = transcript_overlay(cells);
            if !committed {
                overlay.sync_live_tail(area.width, /*key*/ None, |_| {
                    Some(lines.into_iter().map(HyperlinkLine::from).collect())
                });
            }
            overlay.set_history_state(TranscriptHistoryState::Partial);
            overlay.render(area, &mut Buffer::empty(area));
            assert!(overlay.view.is_following());

            // App checks pagination before forwarding the first navigation key.
            for key in [
                KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
                KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
            ] {
                assert_eq!(overlay.should_load_older(key), near_start);
            }
            assert!(!overlay.should_load_older(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE,)));
            if line_count == 30 {
                let up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
                overlay.scroll(/*rows*/ -1);
                assert!(
                    !overlay.should_load_older(up),
                    "one row must not prefetch a distant page"
                );
                overlay.scroll(/*rows*/ -18);
                assert!(
                    !overlay.should_load_older(up),
                    "row six is outside the five-row prefetch threshold"
                );
                overlay.scroll(/*rows*/ -1);
                assert!(
                    overlay.should_load_older(up),
                    "prefetch at row five even before a draw"
                );
            }
        }
    }
}

#[tokio::test]
async fn loading_beginning_keeps_the_entry_until_history_is_complete() -> Result<()> {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 10,
    );
    let mut tui = crate::tui::test_support::make_test_tui()?;
    for already_rendered in [false, true] {
        let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
            lines: vec!["committed".into()],
        })]);
        let live = (0..20)
            .map(|index| Line::from(format!("live {index}")))
            .collect::<Vec<_>>();
        overlay.sync_live_tail(area.width, /*key*/ None, |_| {
            Some(live.iter().cloned().map(HyperlinkLine::from).collect())
        });
        overlay.set_history_state(TranscriptHistoryState::Partial);
        if already_rendered {
            overlay.render(area, &mut Buffer::empty(area));
        }

        // App marks loading before forwarding Home to the overlay.
        overlay.set_history_state(TranscriptHistoryState::LoadingBeginning);
        overlay.handle_event(
            &mut tui,
            TuiEvent::Key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
        )?;
        let mut before = Buffer::empty(area);
        overlay.render(area, &mut before);
        assert!(!overlay.view.is_following());
        assert_eq!(
            overlay.view.history,
            TranscriptHistoryState::LoadingBeginning
        );

        overlay.insert_cell(Arc::new(TestCell { lines: live }));
        overlay.sync_live_tail(area.width, /*key*/ None, |_| {
            Some(
                (20..40)
                    .map(|index| HyperlinkLine::from(format!("live {index}")))
                    .collect(),
            )
        });
        overlay.prepend(vec![Arc::new(TestCell {
            lines: vec!["oldest".into()],
        })]);
        let mut actual = Buffer::empty(area);
        overlay.render(area, &mut actual);
        assert_eq!(actual, before);

        overlay.set_history_state(TranscriptHistoryState::Complete);
        overlay.render(area, &mut actual);
        assert!(buffer_to_text(&actual, overlay.content_area).starts_with("oldest\n"));
    }
    Ok(())
}

#[test]
fn navigation_supersedes_home_before_and_after_the_first_draw() {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 10,
    );
    for rendered in [false, true] {
        for key in [
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
        ] {
            let cells = vec![Arc::new(TestCell {
                lines: (0..30)
                    .map(|index| Line::from(format!("line {index}")))
                    .collect(),
            }) as Arc<dyn HistoryCell>];
            let mut expected = transcript_overlay(cells.clone());
            let mut actual = transcript_overlay(cells);
            actual.set_history_state(TranscriptHistoryState::Partial);
            if rendered {
                expected.render(area, &mut Buffer::empty(area));
                actual.render(area, &mut Buffer::empty(area));
            }
            expected.navigate(key);
            actual.navigate(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
            actual.navigate(key);
            assert_eq!(actual.view.history, TranscriptHistoryState::LoadingOlder);
            actual.prepend(vec![Arc::new(TestCell {
                lines: vec!["received older history".into()],
            })]);
            actual.set_history_state(TranscriptHistoryState::Complete);
            let mut expected_buffer = Buffer::empty(area);
            let mut actual_buffer = Buffer::empty(area);
            expected.render(area, &mut expected_buffer);
            actual.render(area, &mut actual_buffer);
            assert_eq!(
                buffer_to_text(&actual_buffer, actual.content_area),
                buffer_to_text(&expected_buffer, expected.content_area)
            );
        }
    }
}

#[test]
fn transcript_overlay_snapshots_paginated_history_states() {
    let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
        lines: vec![Line::from("recent transcript")],
    })]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 72, /*height*/ 10,
    );
    let mut snapshots = String::new();

    for (name, state) in [
        ("loading", TranscriptHistoryState::LoadingOlder),
        ("partial", TranscriptHistoryState::Partial),
        ("failed", TranscriptHistoryState::Failed),
        ("complete", TranscriptHistoryState::Complete),
    ] {
        overlay.set_history_state(TranscriptHistoryState::Partial);
        overlay.navigate(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        overlay.navigate(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        overlay.set_history_state(state);
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);
        snapshots.push_str(&format!("--- {name} ---\n{}", buffer_to_text(&buf, area)));
    }

    assert_snapshot!("transcript_overlay_paginated_history_states", snapshots);
}

#[test]
fn transcript_overlay_snapshot_basic() {
    let mut overlay = transcript_overlay(vec![
        Arc::new(TestCell {
            lines: vec![
                Line::from(vec!["    indented ".green(), "styled 漢字 ｶﾞ text".bold()]),
                Line::from("abc-def ghi    continuation spaces and a-long-hyphenated-token"),
                Line::from("+ added text").on_green(),
            ],
        }),
        Arc::new(history_cell::AgentMarkdownCell::new(
            "A **styled** [link](https://example.com/transcript)".to_string(),
            std::path::Path::new("/tmp"),
        )),
    ]);
    let mut term = Terminal::new(TestBackend::new(24, 16)).expect("term");
    term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
        .expect("draw");
    assert_snapshot!(format!("{:?}", term.backend().buffer()));
}

#[test]
fn transcript_overlay_preserves_complete_wrapped_links_in_history_and_live_output() {
    let destination = "https://example.com/a/very/long/path";
    let cell: Arc<dyn HistoryCell> = Arc::new(history_cell::AgentMarkdownCell::new(
        destination.to_string(),
        std::path::Path::new("/tmp"),
    ));
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 10,
    );
    for live in [false, true] {
        let mut overlay = transcript_overlay(if live { Vec::new() } else { vec![cell.clone()] });
        if live {
            overlay.sync_live_tail(area.width, /*key*/ None, |width| {
                Some(cell.transcript_hyperlink_lines(width))
            });
        }
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);
        let linked_text = area
            .positions()
            .filter_map(|position| {
                let symbol = buf[position].symbol();
                symbol
                    .contains(&format!("\x1b]8;;{destination}\x07"))
                    .then(|| crate::terminal_hyperlinks::strip_osc8(symbol))
            })
            .collect::<String>();
        assert_eq!(linked_text, destination);
    }
}

#[test]
fn transcript_overlay_renders_live_tail() {
    let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
        lines: vec![Line::from("alpha")],
    })]);
    overlay.sync_live_tail(
        /*width*/ 40,
        Some(ActiveCellTranscriptKey {
            cacheable: true,
            revision: 1,
            is_stream_continuation: false,
            animation_tick: None,
        }),
        |_| Some(vec![HyperlinkLine::from("tail")]),
    );

    let mut term = Terminal::new(TestBackend::new(40, 10)).expect("term");
    term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
        .expect("draw");
    assert_snapshot!(term.backend());
    overlay.navigate(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    term.draw(|frame| overlay.render(frame.area(), frame.buffer_mut()))
        .expect("draw");
    assert!(overlay.live_tail_visible());
}

#[test]
fn transcript_overlay_prepend_preserves_reading_highlight_and_live_tail() {
    for follow in [false, true] {
        let mut overlay = transcript_overlay(vec![
            Arc::new(TestCell {
                lines: vec!["newer one".into(), "newer two".into()],
            }),
            Arc::new(TestCell {
                lines: (0..20)
                    .map(|i| Line::from(format!("reading {i}")))
                    .collect(),
            }),
        ]);
        overlay.highlight_cell = Some(1);
        overlay.sync_live_tail(
            /*width*/ 40,
            Some(ActiveCellTranscriptKey {
                cacheable: true,
                revision: 1,
                is_stream_continuation: false,
                animation_tick: None,
            }),
            |_| Some(vec![HyperlinkLine::from("live tail")]),
        );
        let mut term = Terminal::new(TestBackend::new(/*width*/ 40, /*height*/ 8)).expect("term");
        term.draw(|frame| overlay.render(frame.area(), frame.buffer_mut()))
            .expect("draw");
        if !follow {
            overlay.view.jump_to_entry(&overlay.cells, /*index*/ 1);
        }
        term.draw(|frame| overlay.render(frame.area(), frame.buffer_mut()))
            .expect("draw");
        let content_area = overlay.content_area;
        let before = buffer_to_text(term.backend().buffer(), content_area);
        overlay.reveal_highlight = true;
        overlay.prepend(vec![Arc::new(TestCell {
            lines: vec!["older one".into(), "older two".into(), "older three".into()],
        })]);
        assert_eq!(
            (overlay.highlight_cell, overlay.reveal_highlight),
            (Some(2), true)
        );
        overlay.reveal_highlight = false;
        term.draw(|frame| overlay.render(frame.area(), frame.buffer_mut()))
            .expect("draw prepended");
        assert_eq!(
            buffer_to_text(term.backend().buffer(), content_area),
            before
        );
        assert_eq!(overlay.view.is_following(), follow);
        overlay.view.jump_to_latest();
        term.draw(|frame| overlay.render(frame.area(), frame.buffer_mut()))
            .expect("draw tail");
        assert!(buffer_to_text(term.backend().buffer(), content_area).contains("live tail"));
    }
}

#[test]
fn transcript_overlay_sync_live_tail_is_noop_for_identical_key() {
    let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
        lines: vec![Line::from("alpha")],
    })]);

    let calls = std::cell::Cell::new(0usize);
    let key = ActiveCellTranscriptKey {
        cacheable: true,
        revision: 1,
        is_stream_continuation: false,
        animation_tick: None,
    };

    overlay.sync_live_tail(/*width*/ 40, Some(key), |_| {
        calls.set(calls.get() + 1);
        Some(vec![HyperlinkLine::from("tail")])
    });
    overlay.sync_live_tail(/*width*/ 40, Some(key), |_| {
        calls.set(calls.get() + 1);
        Some(vec![HyperlinkLine::from("tail2")])
    });

    assert_eq!(calls.get(), 1);
}

fn buffer_to_text(buf: &Buffer, area: Rect) -> String {
    let mut out = String::new();
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let symbol = buf[(x, y)].symbol();
            if symbol.is_empty() {
                out.push(' ');
            } else {
                out.push(symbol.chars().next().unwrap_or(' '));
            }
        }
        // Trim trailing spaces for stability.
        while out.ends_with(' ') {
            out.pop();
        }
        out.push('\n');
    }
    out
}

#[test]
fn transcript_overlay_apply_patch_scroll_vt100_clears_previous_page() {
    let cwd = PathBuf::from("/repo");
    let mut cells: Vec<Arc<dyn HistoryCell>> = Vec::new();

    let mut approval_changes = HashMap::new();
    approval_changes.insert(
        PathBuf::from("foo.txt"),
        FileChange::Add {
            content: "hello\nworld\n".to_string(),
        },
    );
    let approval_cell: Arc<dyn HistoryCell> = Arc::new(new_patch_event(approval_changes, &cwd));
    cells.push(approval_cell);

    let mut apply_changes = HashMap::new();
    apply_changes.insert(
        PathBuf::from("foo.txt"),
        FileChange::Add {
            content: "hello\nworld\n".to_string(),
        },
    );
    let apply_begin_cell: Arc<dyn HistoryCell> = Arc::new(new_patch_event(apply_changes, &cwd));
    cells.push(apply_begin_cell);

    let apply_end_cell: Arc<dyn HistoryCell> = history_cell::new_approval_decision_cell(
        history_cell::ApprovalDecisionSubject::Command(vec!["ls".into()]),
        ReviewDecision::Approved,
        history_cell::ApprovalDecisionActor::User,
    )
    .into();
    cells.push(apply_end_cell);

    let mut exec_cell = crate::exec_cell::new_active_exec_command(
        "exec-1".into(),
        vec!["bash".into(), "-lc".into(), "ls".into()],
        vec![ParsedCommand::Unknown { cmd: "ls".into() }],
        ExecCommandSource::Agent,
        /*interaction_input*/ None,
        /*animations_enabled*/ true,
    );
    exec_cell.complete_call(
        "exec-1",
        CommandOutput::new(/*exit_code*/ 0, "src\nREADME.md\n".into()),
        Duration::from_millis(420),
    );
    let exec_cell: Arc<dyn HistoryCell> = Arc::new(exec_cell);
    cells.push(exec_cell);

    let mut overlay = transcript_overlay(cells);
    let area = Rect::new(0, 0, 80, 12);
    let mut buf = Buffer::empty(area);

    overlay.render(area, &mut buf);
    overlay.view.jump_to_entry(&overlay.cells, /*index*/ 0);
    overlay.render(area, &mut buf);

    let snapshot = buffer_to_text(&buf, area);
    assert_snapshot!("transcript_overlay_apply_patch_scroll_vt100", snapshot);
}

#[test]
fn transcript_overlay_keeps_scroll_pinned_at_bottom() {
    let mut overlay = transcript_overlay(
        (0..20)
            .map(|i| {
                Arc::new(TestCell {
                    lines: vec![Line::from(format!("line{i}"))],
                }) as Arc<dyn HistoryCell>
            })
            .collect(),
    );
    let mut term = Terminal::new(TestBackend::new(40, 12)).expect("term");
    term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
        .expect("draw");

    assert!(
        overlay.view.is_following(),
        "expected initial render to leave view at bottom"
    );

    for height in [9, 14] {
        term.backend_mut().resize(/*width*/ 40, height);
        term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
            .expect("draw after composer height change");
        assert!(overlay.view.is_following());
    }

    overlay.insert_cell(Arc::new(TestCell {
        lines: vec!["tail".into()],
    }));

    assert!(overlay.view.is_following());
    term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
        .expect("draw committed tail");
    assert_snapshot!("transcript_overlay_follows_resized_tail", term.backend());
}

#[test]
fn transcript_overlay_consolidation_remaps_highlight() {
    for (selected, expected) in [(3, 2), (6, 4)] {
        let mut overlay = transcript_overlay(
            (0..7)
                .map(|i| {
                    Arc::new(TestCell {
                        lines: vec![Line::from(format!("line{i}"))],
                    }) as Arc<dyn HistoryCell>
                })
                .collect(),
        );
        overlay.set_highlight_cell(Some(selected));
        overlay.consolidate_cells(
            2..5,
            Arc::new(TestCell {
                lines: vec![Line::from("consolidated")],
            }),
        );
        assert_eq!(overlay.highlight_cell, Some(expected));
    }
}

#[test]
fn transcript_overlay_paging_is_continuous_and_round_trips() {
    let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
        lines: (0..50)
            .map(|i| Line::from(format!("line-{i:02}")))
            .collect(),
    })]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 15,
    );
    let mut buffer = Buffer::empty(area);
    overlay.render(area, &mut buffer);
    overlay.view.jump_to_entry(&overlay.cells, /*index*/ 0);
    for start in [0, 10] {
        overlay.render(area, &mut buffer);
        assert_eq!(
            buffer_to_text(&buffer, overlay.content_area),
            (start..start + 10)
                .map(|i| format!("line-{i:02}\n"))
                .collect::<String>()
        );
        overlay.navigate(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    }
    overlay.scroll(/*rows*/ -17);
    overlay.render(area, &mut buffer);
    let before = buffer.clone();
    for key in [KeyCode::PageDown, KeyCode::PageUp] {
        overlay.navigate(KeyEvent::new(key, KeyModifiers::NONE));
        overlay.render(area, &mut buffer);
    }
    assert_eq!(buffer, before);
}

#[tokio::test]
async fn half_page_uses_the_last_rendered_content_height() -> Result<()> {
    let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
        lines: (0..50)
            .map(|i| Line::from(format!("line-{i:02}")))
            .collect(),
    })]);
    let transcript_area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 10,
    );
    let mut buf = Buffer::empty(transcript_area);
    overlay.render(transcript_area, &mut buf);
    let page_height = usize::from(overlay.content_area.height);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.terminal.set_viewport_area(Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 24,
    ));
    overlay.view.jump_to_entry(&overlay.cells, /*index*/ 0);
    overlay.scroll(/*rows*/ 10);

    overlay.handle_event(
        &mut tui,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
    )?;

    overlay.render(transcript_area, &mut buf);
    let first = 10 + page_height.div_ceil(/*rhs*/ 2);
    assert!(buffer_to_text(&buf, overlay.content_area).starts_with(&format!("line-{first:02}\n")));
    Ok(())
}

#[test]
fn transcript_highlight_scrolls_offscreen_entries_in_both_directions() {
    let mut overlay = transcript_overlay(
        (0..20)
            .map(|index| {
                Arc::new(TestCell {
                    lines: vec![Line::from(format!("entry {index}"))],
                }) as Arc<dyn HistoryCell>
            })
            .collect(),
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 12,
    );
    let mut buffer = Buffer::empty(area);
    overlay.render(area, &mut buffer);
    for index in [0, 19, 0] {
        overlay.set_highlight_cell(Some(index));
        overlay.render(area, &mut buffer);
        assert!(buffer_to_text(&buffer, overlay.content_area).contains(&format!("entry {index}")));
    }
}

#[test]
fn transcript_footer_does_not_format_offscreen_history_for_a_percentage() {
    let counts = (0..200)
        .map(|_| Arc::new(AtomicUsize::new(/*v*/ 0)))
        .collect::<Vec<_>>();
    let mut overlay = transcript_overlay(
        counts
            .iter()
            .map(|count| {
                Arc::new(LayoutCountingCell {
                    layout_calls: Arc::clone(count),
                }) as Arc<dyn HistoryCell>
            })
            .collect(),
    );
    let area = Rect::new(
        /*x*/ 2, /*y*/ 1, /*width*/ 72, /*height*/ 12,
    );
    let mut buffer = Buffer::empty(area);
    overlay.set_history_state(TranscriptHistoryState::Partial);
    overlay.render(area, &mut buffer);
    assert_eq!(
        overlay.content_area,
        Rect::new(
            /*x*/ 2, /*y*/ 2, /*width*/ 72, /*height*/ 7
        )
    );
    assert!(!buffer_to_text(&buffer, area).contains('%'));
    let before = counts
        .iter()
        .map(|count| count.load(Ordering::Relaxed))
        .collect::<Vec<_>>();
    assert!(before[..190].iter().all(|count| *count == 0));
    assert!((1..10).contains(&before.iter().sum::<usize>()));
    overlay.set_history_state(TranscriptHistoryState::Complete);
    overlay.render(area, &mut buffer);
    assert!(buffer_to_text(&buffer, area).contains("100%"));
    assert_eq!(
        counts
            .iter()
            .map(|count| count.load(Ordering::Relaxed))
            .collect::<Vec<_>>(),
        before
    );
    for selection in [Some(199), Some(198), None] {
        overlay.set_highlight_cell(selection);
        overlay.render(area, &mut buffer);
        assert_eq!(
            counts
                .iter()
                .map(|count| count.load(Ordering::Relaxed))
                .collect::<Vec<_>>(),
            before
        );
        if selection.is_some() {
            assert!(buffer_to_text(&buffer, area).contains("edit next"));
        }
    }
    overlay.view.jump_to_latest();
    overlay.insert_cell(Arc::new(TestCell {
        lines: vec!["new output".into()],
    }));
    overlay.render(area, &mut buffer);
    assert_eq!(
        counts
            .iter()
            .map(|count| count.load(Ordering::Relaxed))
            .collect::<Vec<_>>(),
        before
    );
    overlay.scroll(/*rows*/ -1);
    overlay.render(area, &mut buffer);
    assert!(!buffer_to_text(&buffer, area).contains('%'));
}

#[test]
fn transcript_overlay_clips_zero_and_short_viewports_to_their_area() {
    let canvas = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 48, /*height*/ 16,
    );
    for width in [0, 1, 40] {
        for height in 0..=6 {
            let area = Rect::new(/*x*/ 2, /*y*/ 3, width, height);
            let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
                lines: vec!["content".into()],
            })]);
            let mut buffer = Buffer::filled(canvas, Cell::from('.'));
            overlay.render(area, &mut buffer);
            for position in canvas
                .positions()
                .filter(|position| !area.contains(*position))
            {
                assert_eq!(
                    buffer[position],
                    Cell::from('.'),
                    "area={area:?}, position={position:?}"
                );
            }
        }
    }
}
