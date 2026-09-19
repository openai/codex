//! Standalone transcript behavior exercised through the shared viewport.

use super::*;
use crate::history_cell;
use crossterm::event::KeyModifiers;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;

#[derive(Debug)]
struct TestCell {
    lines: Vec<Line<'static>>,
}

impl HistoryCell for TestCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.lines.clone()
    }
    fn raw_lines(&self) -> Vec<Line<'static>> {
        self.lines.clone()
    }
}

fn default_pager_keymap() -> PagerKeymap {
    crate::keymap::RuntimeKeymap::defaults().pager
}

fn transcript_overlay(cells: Vec<Arc<dyn HistoryCell>>) -> TranscriptOverlay {
    TranscriptOverlay::new(cells, default_pager_keymap())
}

#[test]
fn scrolling_prepend_and_resize_preserve_reading_content() {
    let cells = (0..30)
        .map(|index| {
            Arc::new(TestCell {
                lines: vec![format!("line {index}").into()],
            }) as Arc<dyn HistoryCell>
        })
        .collect();
    let mut overlay = transcript_overlay(cells);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 12,
    );
    overlay.render(area, &mut Buffer::empty(area));
    overlay.view.jump_to_entry(&overlay.cells, /*index*/ 12);
    let mut before = Buffer::empty(area);
    overlay.render(area, &mut before);
    overlay.prepend(vec![Arc::new(TestCell {
        lines: vec!["older".into()],
    })]);
    let mut after = Buffer::empty(area);
    overlay.render(area, &mut after);
    assert_eq!(after, before);
    let narrow = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 9,
    );
    let mut resized = Buffer::empty(narrow);
    overlay.render(narrow, &mut resized);
    assert!(buffer_to_text(&resized, narrow).contains("line 12"));
    assert!(!overlay.is_scrolled_to_bottom());
}

#[tokio::test]
async fn activity_hint_uses_the_configured_pager_jump_binding() -> Result<()> {
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut snapshots = Vec::new();
    for (label, binding) in [
        ("default", Some(key_hint::plain(KeyCode::End))),
        ("remapped", Some(key_hint::plain(KeyCode::F(8)))),
        ("unbound", None),
    ] {
        let mut keymap = default_pager_keymap();
        keymap.jump_bottom = binding.into_iter().collect();
        let mut overlay = TranscriptOverlay::new(
            vec![Arc::new(TestCell {
                lines: (0..20).map(|row| format!("row {row}").into()).collect(),
            })],
            keymap,
        );
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 64, /*height*/ 12,
        );
        overlay.render(area, &mut Buffer::empty(area));
        overlay.view.jump_to_entry(&overlay.cells, /*index*/ 0);
        overlay.insert_cell(Arc::new(TestCell {
            lines: vec!["new output".into()],
        }));
        let mut buffer = Buffer::empty(area);
        overlay.render(area, &mut buffer);
        let status = Rect::new(/*x*/ 0, /*y*/ 8, area.width, /*height*/ 1);
        snapshots.push(format!(
            "{label}: {}",
            buffer_to_text(&buffer, status).trim()
        ));

        let (code, modifiers) = binding
            .unwrap_or_else(|| key_hint::plain(KeyCode::End))
            .parts();
        overlay.handle_event(&mut tui, TuiEvent::Key(KeyEvent::new(code, modifiers)))?;
        assert_eq!(overlay.is_scrolled_to_bottom(), binding.is_some());
    }
    assert_snapshot!(snapshots.join("\n"), @"
    default: New activity · end latest
    remapped: New activity · f8 latest
    unbound: New activity
    ");
    Ok(())
}

#[tokio::test]
async fn pager_bindings_use_rendered_page_height_and_keep_control_f_navigation() -> Result<()> {
    let mut overlay = transcript_overlay(
        (0..40)
            .map(|index| {
                Arc::new(TestCell {
                    lines: vec![format!("line {index}").into()],
                }) as Arc<dyn HistoryCell>
            })
            .collect(),
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 11,
    );
    overlay.render(area, &mut Buffer::empty(area));
    overlay.view.jump_to_entry(&overlay.cells, /*index*/ 8);
    let mut initial = Buffer::empty(area);
    overlay.render(area, &mut initial);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    for key in [
        KeyEvent::new(KeyCode::Char('f'), crossterm::event::KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::PageUp, crossterm::event::KeyModifiers::NONE),
    ] {
        overlay.handle_event(&mut tui, TuiEvent::Key(key))?;
        overlay.render(area, &mut Buffer::empty(area));
    }
    let mut restored = Buffer::empty(area);
    overlay.render(area, &mut restored);
    assert_eq!(restored, initial);
    assert!(!overlay.view.has_active_interaction());
    Ok(())
}

#[tokio::test]
async fn focus_loss_stops_edge_drag_and_preserves_selection_after_focus_returns() -> Result<()> {
    use crossterm::event::KeyModifiers;
    use crossterm::event::MouseButton;
    use crossterm::event::MouseEvent;
    use crossterm::event::MouseEventKind;
    use ratatui::layout::Size;

    let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
        lines: (0..30).map(|row| format!("row {row:02}").into()).collect(),
    })]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 12,
    );
    overlay.render(area, &mut Buffer::empty(area));
    overlay.view.jump_to_entry(&overlay.cells, /*index*/ 0);
    overlay.render(area, &mut Buffer::empty(area));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.terminal.resize(Size::new(area.width, area.height))?;
    for (kind, column, row) in [
        (MouseEventKind::Down(MouseButton::Left), 0, 2),
        (MouseEventKind::Drag(MouseButton::Left), 6, 7),
    ] {
        overlay.handle_event(
            &mut tui,
            TuiEvent::Mouse(MouseEvent {
                kind,
                column,
                row,
                modifiers: KeyModifiers::NONE,
            }),
        )?;
    }
    assert!(overlay.view.tick_selection(&overlay.cells));
    let mut before = Buffer::empty(area);
    overlay.render(area, &mut before);
    let selected = overlay.view.selected_text(&overlay.cells);
    assert_eq!(
        selected.as_deref(),
        Some("row 01\nrow 02\nrow 03\nrow 04\nrow 05\nrow 06\nrow 07")
    );
    tui.screen_size_for_event(&TuiEvent::Draw)?;
    overlay.draw(&mut tui)?;
    let mut repainted = Buffer::empty(area);
    overlay.render(area, &mut repainted);
    assert_eq!(repainted, before);

    for event in [
        TuiEvent::Resume,
        TuiEvent::FocusLost,
        TuiEvent::Draw,
        TuiEvent::FocusGained,
        TuiEvent::Draw,
    ] {
        let resumed = matches!(event, TuiEvent::Resume);
        tui.screen_size_for_event(&event)?;
        overlay.handle_event(&mut tui, event)?;
        if resumed {
            let rendered =
                crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
            assert!(buffer_to_text(rendered, rendered.area).contains("row 01"));
        }
        assert!(!overlay.view.tick_selection(&overlay.cells));
        let mut after = Buffer::empty(area);
        overlay.render(area, &mut after);
        assert_eq!(after, before);
        assert_eq!(overlay.view.selected_text(&overlay.cells), selected);
    }
    let content_and_status = Rect::new(
        /*x*/ 0, /*y*/ 1, /*width*/ 40, /*height*/ 8,
    );
    assert_snapshot!(buffer_to_text(&before, content_and_status), @"
    row 01
    row 02
    row 03
    row 04
    row 05
    row 06
    row 07
    enter copy & follow · esc clear
    ");
    overlay.view.jump_to_latest();
    overlay.render(area, &mut Buffer::empty(area));
    overlay.handle_event(
        &mut tui,
        TuiEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 1,
            row: 2,
            modifiers: KeyModifiers::NONE,
        }),
    )?;
    assert_eq!(overlay.view.selected_text(&overlay.cells), None);
    overlay.handle_event(&mut tui, TuiEvent::FocusLost)?;
    overlay.handle_event(&mut tui, TuiEvent::FocusGained)?;
    overlay.insert_cell(Arc::new(TestCell {
        lines: vec!["new output".into()],
    }));
    let mut after = Buffer::empty(area);
    overlay.render(area, &mut after);
    assert!(overlay.is_scrolled_to_bottom());
    assert_eq!(overlay.view.selected_text(&overlay.cells), None);
    assert!(buffer_to_text(&after, area).contains("new output"));
    Ok(())
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

#[tokio::test]
async fn jump_top_holds_the_visible_page_until_all_history_arrives() -> Result<()> {
    let mut overlay = transcript_overlay(vec![Arc::new(TestCell {
        lines: (0..8).map(|index| format!("line {index}").into()).collect(),
    })]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 27, /*height*/ 8,
    );
    let content = Rect::new(
        /*x*/ 0, /*y*/ 1, /*width*/ 27, /*height*/ 3,
    );
    let content_and_status = Rect::new(
        /*x*/ 0, /*y*/ 1, /*width*/ 27, /*height*/ 4,
    );
    overlay.set_history_state(TranscriptHistoryState::Partial);
    let mut initial = Buffer::empty(area);
    overlay.render(area, &mut initial);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let home = KeyEvent::new(KeyCode::Home, crossterm::event::KeyModifiers::NONE);
    overlay.handle_event(&mut tui, TuiEvent::Key(home))?;
    let mut pending = Buffer::empty(area);
    overlay.render(area, &mut pending);
    assert_snapshot!(buffer_to_text(&pending, content_and_status), @"
    line 5
    line 6
    line 7
    ↑ Loading… · end latest
    ");
    for label in ["earlier", "oldest"] {
        overlay.prepend(vec![Arc::new(TestCell {
            lines: ["zero", "one", "two"]
                .map(|row| format!("{label} {row}").into())
                .into(),
        })]);
        overlay.handle_event(&mut tui, TuiEvent::Key(home))?;
        overlay.render(area, &mut pending);
        assert_eq!(
            buffer_to_text(&pending, content),
            buffer_to_text(&initial, content)
        );
        assert_eq!(
            overlay.history_state(),
            TranscriptHistoryState::LoadingBeginning
        );
    }
    overlay.set_history_state(TranscriptHistoryState::Complete);
    let mut complete = Buffer::empty(area);
    overlay.render(area, &mut complete);
    assert_snapshot!(buffer_to_text(&complete, content_and_status), @"
    oldest zero
    oldest one
    oldest two
    end latest
    ");
    overlay.handle_event(
        &mut tui,
        TuiEvent::Key(KeyEvent::new(
            KeyCode::End,
            crossterm::event::KeyModifiers::NONE,
        )),
    )?;
    overlay.handle_event(&mut tui, TuiEvent::Key(home))?;
    overlay.render(area, &mut pending);
    assert_eq!(pending, complete);
    Ok(())
}

#[test]
fn highlighting_reveals_prompt_body_when_only_padding_is_visible() {
    let mut overlay = transcript_overlay(vec![
        Arc::new(history_cell::new_user_prompt(
            "first prompt".into(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )),
        Arc::new(history_cell::PlainHistoryCell::new(vec![
            "tail one".into(),
            "tail two".into(),
            "tail three".into(),
        ])),
    ]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 120, /*height*/ 10,
    );
    let mut buf = Buffer::empty(area);
    overlay.render(area, &mut buf);
    assert!(!buffer_to_text(&buf, area).contains("first prompt"));
    overlay.set_highlight_cell(Some(0));
    let mut buf = Buffer::empty(area);
    overlay.render(area, &mut buf);
    let text = buffer_to_text(&buf, area);
    assert!(text.contains("first prompt"), "{text}");
    let highlighted = buf
        .content()
        .iter()
        .filter(|cell| cell.modifier.contains(ratatui::style::Modifier::REVERSED))
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert_eq!(highlighted, "first prompt");
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

    let snapshot_name = if cfg!(target_os = "macos") {
        "transcript_overlay_paginated_history_states"
    } else {
        "transcript_overlay_paginated_history_states_non_macos"
    };
    assert_snapshot!(snapshot_name, snapshots);
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
    overlay.prepend(vec![Arc::new(TestCell {
        lines: vec![Line::from("older")],
    })]);

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
fn transcript_overlay_absorbs_older_tail_into_live_without_duplicate_cells() {
    let header: Arc<dyn HistoryCell> = Arc::new(history_cell::SessionHeaderHistoryCell::new(
        "test model".to_owned(),
        /*reasoning_effort*/ None,
        /*show_fast_status*/ false,
        std::path::PathBuf::from("/project"),
        "test",
    ));
    let mut overlay = transcript_overlay(vec![Arc::clone(&header)]);
    let key = ActiveCellTranscriptKey {
        cacheable: true,
        revision: 1,
        is_stream_continuation: false,
        animation_tick: None,
    };
    overlay.sync_live_tail(/*width*/ 40, Some(key), |_| {
        Some(vec![HyperlinkLine::from("current group")])
    });
    overlay.prepend(vec![Arc::new(TestCell {
        lines: vec!["older group".into()],
    })]);
    overlay.set_highlight_cell(Some(1));
    overlay.absorb_tail_into_live(/*previous_revision*/ 1, /*hydrated_revision*/ 2);
    overlay.sync_live_tail(
        /*width*/ 40,
        Some(ActiveCellTranscriptKey { revision: 2, ..key }),
        |_| Some(vec![HyperlinkLine::from("hydrated group")]),
    );

    assert_eq!(overlay.cells.len(), 1);
    assert!(Arc::ptr_eq(&overlay.cells[0], &header));
    assert_eq!(overlay.highlight_cell, Some(1));
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 10,
    );
    let mut buffer = Buffer::empty(area);
    overlay.render(area, &mut buffer);
    let rendered = buffer_to_text(&buffer, area);
    assert!(rendered.contains("hydrated group"));
    assert!(!rendered.contains("older group"));
    assert!(!rendered.contains("New activity"));
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

    let calls = std::cell::Cell::new(/*value*/ 0usize);
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

    overlay.prepend(vec![Arc::new(TestCell {
        lines: vec!["older".into()],
    })]);
    overlay.regroup_cells(
        3..6,
        Arc::new(TestCell {
            lines: vec![Line::from("consolidated")],
        }),
    );

    assert_eq!(
        overlay.highlight_cell,
        Some(5),
        "highlight after consolidated range should shift left by removed cells",
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 7,
    );
    let mut buffer = Buffer::empty(area);
    overlay.render(area, &mut buffer);
    let selected = buffer
        .content()
        .iter()
        .filter(|cell| cell.modifier.contains(ratatui::style::Modifier::REVERSED))
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert_eq!(selected, "line6");
}

#[test]
fn completing_agent_and_plan_streams_preserves_reading_in_later_fragments() {
    use crate::history_cell::HistoryRenderMode;
    use crate::streaming::controller::PlanStreamController;
    use crate::streaming::controller::StreamController;

    let cwd = std::path::Path::new("/workspace");
    let source = (0..12)
        .map(|index| {
            format!("Passage {index:02} alpha   beta   gamma   delta   epsilon   zeta   theta.\n\n")
        })
        .collect::<String>();
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 9,
    );
    let render = |overlay: &mut TranscriptOverlay, area: Rect| {
        let mut buffer = Buffer::empty(area);
        overlay.render(area, &mut buffer);
        buffer
    };
    for plan in [false, true] {
        let mut cells = vec![Arc::new(history_cell::PlainHistoryCell::new(vec![
            "preceding prompt".into(),
        ])) as Arc<dyn HistoryCell>];
        let replacement: Arc<dyn HistoryCell> = if plan {
            let mut controller = PlanStreamController::new(
                Some(usize::from(area.width - 4)),
                cwd,
                HistoryRenderMode::Rich,
            );
            controller.push(&source);
            while let (Some(cell), _) = controller.on_commit_tick_batch(/*max_lines*/ 1) {
                cells.push(cell.into());
            }
            let (tail, _) = controller.finalize();
            if let Some(tail) = tail {
                cells.push(tail.into());
            }
            Arc::new(crate::history_cell::new_proposed_plan(source.clone(), cwd))
        } else {
            let mut controller = StreamController::new(
                Some(usize::from(area.width - 2)),
                cwd,
                HistoryRenderMode::Rich,
            );
            controller.push(&source);
            while let (Some(cell), _) = controller.on_commit_tick_batch(/*max_lines*/ 1) {
                cells.push(cell.into());
            }
            let (tail, _) = controller.finalize();
            if let Some(tail) = tail {
                cells.push(tail.into());
            }
            Arc::new(crate::history_cell::AgentMarkdownCell::new(
                source.clone(),
                cwd,
            ))
        };
        assert!(
            cells.len() > 3,
            "the stream must span multiple committed fragments"
        );
        for resize_before_draw in [false, true] {
            let mut overlay = transcript_overlay(cells.clone());
            render(&mut overlay, area);
            overlay.view.jump_to_entry(&overlay.cells, /*index*/ 2);
            overlay.view.scroll(&overlay.cells, /*rows*/ 1);
            let before = render(&mut overlay, area);
            assert!(buffer_to_text(&before, area).contains("Passage"));
            let end = overlay.cells.len();
            overlay.consolidate_cells(1..end, Arc::clone(&replacement));
            if resize_before_draw {
                render(&mut overlay, Rect { width: 80, ..area });
            }
            let completed = render(&mut overlay, area);
            // Stream completion changes the activity footer, but not the passage being read.
            let content = Rect {
                y: 1,
                height: area.height - 5,
                ..area
            };
            assert_eq!(
                buffer_to_text(&completed, content),
                buffer_to_text(&before, content),
            );
            assert!(!overlay.is_scrolled_to_bottom());
        }
    }
}

#[tokio::test]
async fn navigation_supersedes_home_before_and_after_the_first_draw() {
    let mut tui = crate::tui::test_support::make_test_tui().expect("tui");
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
            expected
                .handle_event(&mut tui, TuiEvent::Key(key))
                .expect("navigate");
            actual
                .handle_event(
                    &mut tui,
                    TuiEvent::Key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
                )
                .expect("Home");
            actual
                .handle_event(&mut tui, TuiEvent::Key(key))
                .expect("navigate");
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
                buffer_to_text(
                    &actual_buffer,
                    Rect::new(
                        /*x*/ 0, /*y*/ 1, /*width*/ 40, /*height*/ 5
                    )
                ),
                buffer_to_text(
                    &expected_buffer,
                    Rect::new(
                        /*x*/ 0, /*y*/ 1, /*width*/ 40, /*height*/ 5
                    )
                )
            );
        }
    }
}
