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
struct HeightCountingCell {
    height_calls: Arc<AtomicUsize>,
}

impl crate::history_cell::HistoryCell for HeightCountingCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.height_calls.fetch_add(1, Ordering::Relaxed);
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
    let overlay = transcript_overlay(vec![Arc::new(TestCell {
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
fn edit_next_hint_is_visible_when_highlighted() {
    let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
        lines: vec![Line::from("hello")],
    })]);
    overlay.set_highlight_cell(Some(0));

    // Render into a wide buffer so the footer hints aren't truncated.
    let area = Rect::new(0, 0, 120, 10);
    let mut buf = Buffer::empty(area);
    overlay.render(area, &mut buf);

    let s = buffer_to_text(&buf, area);
    assert!(
        s.contains("edit next"),
        "expected 'edit next' hint in overlay footer, got: {s:?}"
    );
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
fn transcript_overlay_preserves_semantic_web_links() {
    let destination = "https://example.com/a/very/long/path";
    let mut overlay = transcript_overlay(vec![Arc::new(history_cell::AgentMarkdownCell::new(
        destination.to_string(),
        std::path::Path::new("/tmp"),
    ))]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 10,
    );
    let mut buf = Buffer::empty(area);

    overlay.render(area, &mut buf);

    assert!(area.positions().any(|position| {
        buf[position]
            .symbol()
            .contains(&format!("\x1b]8;;{destination}\x07"))
    }));
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
}

#[test]
fn transcript_overlay_preserves_live_tail_when_prepending_history() {
    let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
        lines: vec![Line::from("recent")],
    })]);
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
    overlay.prepend(
        vec![Arc::new(TestCell {
            lines: vec![Line::from("older")],
        })],
        /*width*/ 40,
    );

    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 10,
    );
    let mut buffer = Buffer::empty(area);
    overlay.render(area, &mut buffer);
    let rendered = buffer_to_text(&buffer, area);
    assert!(rendered.contains("older"));
    assert!(rendered.contains("recent"));
    assert!(rendered.contains("live tail"));
}

#[test]
fn transcript_overlay_live_tail_preserves_semantic_web_links() {
    let destination = "https://example.com/a/streamed/path";
    let cell =
        history_cell::AgentMarkdownCell::new(destination.to_string(), std::path::Path::new("/tmp"));
    let mut overlay = transcript_overlay(Vec::new());
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 10,
    );
    let mut buf = Buffer::empty(area);

    overlay.sync_live_tail(
        area.width,
        Some(ActiveCellTranscriptKey {
            cacheable: true,
            revision: 1,
            is_stream_continuation: false,
            animation_tick: None,
        }),
        |width| Some(cell.transcript_hyperlink_lines(width)),
    );
    overlay.render(area, &mut buf);

    assert!(area.positions().any(|position| {
        buf[position]
            .symbol()
            .contains(&format!("\x1b]8;;{destination}\x07"))
    }));
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
    overlay.view.scroll_offset = 0;
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
        overlay.view.is_scrolled_to_bottom(),
        "expected initial render to leave view at bottom"
    );

    for height in [9, 14] {
        term.backend_mut().resize(/*width*/ 40, height);
        term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
            .expect("draw after composer height change");
        assert!(overlay.is_scrolled_to_bottom());
    }

    overlay.insert_cell(Arc::new(TestCell {
        lines: vec!["tail".into()],
    }));

    assert_eq!(overlay.view.scroll_offset, usize::MAX);
    term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
        .expect("draw committed tail");
    assert_snapshot!("transcript_overlay_follows_resized_tail", term.backend());
}

#[test]
fn transcript_overlay_preserves_manual_scroll_position() {
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

    overlay.view.scroll_offset = 0;

    overlay.insert_cell(Arc::new(TestCell {
        lines: vec!["tail".into()],
    }));

    assert_eq!(overlay.view.scroll_offset, 0);
    overlay.view.scroll_offset = 3;
    term.draw(|frame| overlay.render(frame.area(), frame.buffer_mut()))
        .expect("draw");
    let content_area = Rect::new(
        /*x*/ 0, /*y*/ 1, /*width*/ 40, /*height*/ 4,
    );
    let visible_before = buffer_to_text(term.backend().buffer(), content_area);
    overlay.prepend(
        vec![Arc::new(TestCell {
            lines: (0..40).map(|i| Line::from(format!("older {i}"))).collect(),
        })],
        /*width*/ 40,
    );
    term.draw(|frame| overlay.render(frame.area(), frame.buffer_mut()))
        .expect("draw");
    assert_eq!(
        buffer_to_text(term.backend().buffer(), content_area),
        visible_before
    );
    assert_snapshot!(
        "transcript_overlay_prepended_history",
        visible_before.trim()
    );
}

#[test]
fn transcript_overlay_insert_preserves_cached_cell_heights() {
    let height_calls = Arc::new(AtomicUsize::new(0));
    let mut overlay = transcript_overlay(vec![Arc::new(HeightCountingCell {
        height_calls: height_calls.clone(),
    })]);
    let area = Rect::new(0, 0, 40, 12);
    let mut buf = Buffer::empty(area);

    overlay.render(area, &mut buf);
    assert_eq!(height_calls.load(Ordering::Relaxed), 1);

    overlay.insert_cell(Arc::new(TestCell {
        lines: vec![Line::from("inserted")],
    }));
    overlay.render(area, &mut buf);

    assert_eq!(height_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn transcript_overlay_history_rebuild_preserves_only_the_live_tail() {
    for replace in [false, true] {
        for tail in [
            None,
            Some(Vec::new()),
            Some(vec![HyperlinkLine::from("live")]),
        ] {
            let mut overlay = transcript_overlay(
                ["first", "last"]
                    .map(|line| {
                        Arc::new(TestCell {
                            lines: vec![line.into()],
                        }) as Arc<dyn HistoryCell>
                    })
                    .to_vec(),
            );
            let key = tail.as_ref().map(|_| ActiveCellTranscriptKey {
                cacheable: true,
                revision: 1,
                is_stream_continuation: false,
                animation_tick: None,
            });
            overlay.sync_live_tail(/*width*/ 40, key, |_| tail.clone());
            let consolidated = Arc::new(TestCell {
                lines: vec!["first".into(), "last".into()],
            });
            if replace {
                overlay.replace_cells(vec![consolidated]);
            } else {
                overlay.consolidate_cells(0..2, consolidated);
            }
            // A draw may arrive after the active tail has already been cleared.
            overlay.sync_live_tail(/*width*/ 40, key, |_| tail.clone());
            let mut reopened = transcript_overlay(overlay.cells.clone());
            reopened.sync_live_tail(/*width*/ 40, key, |_| tail.clone());
            let area = Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 10,
            );
            let mut actual = Buffer::empty(area);
            let mut expected = Buffer::empty(area);
            overlay.render(area, &mut actual);
            reopened.render(area, &mut expected);
            assert_eq!(actual, expected);
            if tail.is_none() {
                assert_snapshot!(
                    "transcript_overlay_completed_stream",
                    buffer_to_text(&actual, area)
                );
            }
        }
    }
}

#[test]
fn transcript_overlay_consolidation_remaps_highlight_inside_range() {
    let mut overlay = transcript_overlay(
        (0..6)
            .map(|i| {
                Arc::new(TestCell {
                    lines: vec![Line::from(format!("line{i}"))],
                }) as Arc<dyn HistoryCell>
            })
            .collect(),
    );
    overlay.set_highlight_cell(Some(3));

    overlay.consolidate_cells(
        2..5,
        Arc::new(TestCell {
            lines: vec![Line::from("consolidated")],
        }),
    );

    assert_eq!(
        overlay.highlight_cell,
        Some(2),
        "highlight inside consolidated range should point to replacement cell",
    );
}

#[test]
fn transcript_overlay_consolidation_remaps_highlight_after_range() {
    let mut overlay = transcript_overlay(
        (0..7)
            .map(|i| {
                Arc::new(TestCell {
                    lines: vec![Line::from(format!("line{i}"))],
                }) as Arc<dyn HistoryCell>
            })
            .collect(),
    );
    overlay.set_highlight_cell(Some(6));

    overlay.consolidate_cells(
        2..5,
        Arc::new(TestCell {
            lines: vec![Line::from("consolidated")],
        }),
    );

    assert_eq!(
        overlay.highlight_cell,
        Some(4),
        "highlight after consolidated range should shift left by removed cells",
    );
}

fn transcript_line_numbers(overlay: &mut TranscriptOverlay, area: Rect) -> Vec<usize> {
    let mut buf = Buffer::empty(area);
    overlay.render(area, &mut buf);

    let top_h = area.height.saturating_sub(3);
    let top = Rect::new(area.x, area.y, area.width, top_h);
    let content_area = overlay.view.content_area(top);

    let mut nums = Vec::new();
    for y in content_area.y..content_area.bottom() {
        let mut line = String::new();
        for x in content_area.x..content_area.right() {
            line.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
        }
        if let Some(n) = line
            .split_whitespace()
            .find_map(|w| w.strip_prefix("line-"))
            .and_then(|s| s.parse().ok())
        {
            nums.push(n);
        }
    }
    nums
}

#[test]
fn transcript_overlay_paging_is_continuous_and_round_trips() {
    let mut overlay = transcript_overlay(
        (0..50)
            .map(|i| {
                Arc::new(TestCell {
                    lines: vec![Line::from(format!("line-{i:02}"))],
                }) as Arc<dyn HistoryCell>
            })
            .collect(),
    );
    let area = Rect::new(0, 0, 40, 15);

    // Prime layout so last_content_height is populated and paging uses the real content height.
    let mut buf = Buffer::empty(area);
    overlay.view.scroll_offset = 0;
    overlay.render(area, &mut buf);
    let page_height = overlay.view.page_height(area);

    // Scenario 1: starting from the top, PageDown should show the next page of content.
    overlay.view.scroll_offset = 0;
    let page1 = transcript_line_numbers(&mut overlay, area);
    let page1_len = page1.len();
    let expected_page1: Vec<usize> = (0..page1_len).collect();
    assert_eq!(
        page1, expected_page1,
        "first page should start at line-00 and show a full page of content"
    );

    overlay.view.scroll_offset = overlay.view.scroll_offset.saturating_add(page_height);
    let page2 = transcript_line_numbers(&mut overlay, area);
    assert_eq!(
        page2.len(),
        page1_len,
        "second page should have the same number of visible lines as the first page"
    );
    let expected_page2_first = *page1.last().unwrap() + 1;
    assert_eq!(
        page2[0], expected_page2_first,
        "second page after PageDown should immediately follow the first page"
    );

    // Scenario 2: from an interior offset (start=3), PageDown then PageUp should round-trip.
    let interior_offset = 3usize;
    overlay.view.scroll_offset = interior_offset;
    let before = transcript_line_numbers(&mut overlay, area);
    overlay.view.scroll_offset = overlay.view.scroll_offset.saturating_add(page_height);
    let _ = transcript_line_numbers(&mut overlay, area);
    overlay.view.scroll_offset = overlay.view.scroll_offset.saturating_sub(page_height);
    let after = transcript_line_numbers(&mut overlay, area);
    assert_eq!(
        before, after,
        "PageDown+PageUp from interior offset ({interior_offset}) should round-trip"
    );

    // Scenario 3: from the top of the second page, PageUp then PageDown should round-trip.
    overlay.view.scroll_offset = page_height;
    let before2 = transcript_line_numbers(&mut overlay, area);
    overlay.view.scroll_offset = overlay.view.scroll_offset.saturating_sub(page_height);
    let _ = transcript_line_numbers(&mut overlay, area);
    overlay.view.scroll_offset = overlay.view.scroll_offset.saturating_add(page_height);
    let after2 = transcript_line_numbers(&mut overlay, area);
    assert_eq!(
        before2, after2,
        "PageUp+PageDown from the top of the second page should round-trip"
    );
}

#[tokio::test]
async fn half_page_uses_the_last_rendered_content_height() -> Result<()> {
    let mut overlay = transcript_overlay(
        (0..50)
            .map(|i| {
                Arc::new(TestCell {
                    lines: vec![Line::from(format!("line-{i:02}"))],
                }) as Arc<dyn HistoryCell>
            })
            .collect(),
    );
    let transcript_area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 10,
    );
    let mut buf = Buffer::empty(transcript_area);
    overlay.render(transcript_area, &mut buf);
    let page_height = overlay.view.page_height(transcript_area);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.terminal.set_viewport_area(Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 24,
    ));
    overlay.view.scroll_offset = 10;

    overlay.view.handle_key_event(
        &mut tui,
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
    )?;

    assert_eq!(
        overlay.view.scroll_offset,
        10 + page_height.saturating_add(1) / 2
    );
    Ok(())
}
