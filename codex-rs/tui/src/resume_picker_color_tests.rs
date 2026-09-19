use super::*;
use crate::terminal_palette::rgb_color;
use crate::terminal_palette::with_test_default_colors;
use crate::terminal_probe::DefaultColors;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::style::Modifier;

#[test]
fn thread_colors_and_selection_contrast_in_both_picker_layouts() {
    let original_theme = crate::render::highlight::current_syntax_theme();
    crate::render::highlight::set_syntax_theme(
        two_face::theme::extra()
            .get(two_face::theme::EmbeddedThemeName::CatppuccinMocha)
            .clone(),
    );
    let row = Row {
        path: None,
        thread_id: Some(ThreadId::from_u128(/*value*/ 42)),
        thread_name: Some("Named task".into()),
        preview: "Original prompt".into(),
        created_at: None,
        updated_at: None,
        cwd: None,
        git_branch: None,
    };
    let mut state = PickerState::new(
        FrameRequester::test_dummy(),
        Arc::new(|_| {}),
        ProviderFilter::Any,
        /*show_all*/ true,
        /*filter_cwd*/ None,
        SessionPickerAction::Resume,
    );
    let (fg, bg) = ((0, 0, 0), (255, 255, 255));
    with_test_default_colors(DefaultColors { fg, bg }, || {
        for density in [SessionListDensity::Comfortable, SessionListDensity::Dense] {
            state.density = density;
            for use_colors in [true, false] {
                state.use_theme_colors = use_colors;
                for (selected, zebra) in [(true, false), (false, false), (false, true)] {
                    let lines = render_session_lines(
                        &row, &state, selected, /*is_expanded*/ false, zebra,
                        /*width*/ 40,
                    );
                    let area = Rect::new(
                        /*x*/ 0,
                        /*y*/ 0,
                        /*width*/ 40,
                        lines.len() as u16,
                    );
                    let mut buffer = Buffer::empty(area);
                    buffer.set_style(area, Style::default().fg(rgb_color(fg)).bg(rgb_color(bg)));
                    for (y, line) in lines.into_iter().enumerate() {
                        line.render(
                            Rect::new(/*x*/ 0, y as u16, /*width*/ 40, /*height*/ 1),
                            &mut buffer,
                        );
                    }
                    let title_x = match density {
                        SessionListDensity::Comfortable => 2,
                        SessionListDensity::Dense => 14,
                    };
                    let title_cell = &buffer[(title_x, 0)];
                    if use_colors {
                        assert_eq!(
                            title_cell.fg,
                            readable_color_on(
                                crate::thread_color::thread_color(row.thread_id.unwrap()),
                                Some(title_cell.bg),
                            )
                        );
                    } else {
                        let expected = if selected {
                            selected_session_style().fg.unwrap()
                        } else {
                            rgb_color(fg)
                        };
                        assert_eq!(title_cell.fg, expected);
                    }
                    if selected {
                        let style = selected_session_style();
                        let metadata_y = match density {
                            SessionListDensity::Comfortable => 1,
                            SessionListDensity::Dense => 0,
                        };
                        let metadata = &buffer[(2, metadata_y)];
                        assert_eq!(
                            (
                                buffer
                                    .content
                                    .iter()
                                    .map(|cell| cell.bg)
                                    .collect::<Vec<_>>(),
                                title_cell.modifier.contains(Modifier::BOLD),
                                metadata.modifier.contains(Modifier::BOLD),
                            ),
                            (vec![style.bg.unwrap(); buffer.content.len()], true, false)
                        );
                    }
                }
            }
        }
    });
    crate::render::highlight::set_syntax_theme(original_theme);
}

#[test]
fn loading_overlay_clears_inherited_selection_colors_and_reversal() {
    use crate::custom_terminal::Terminal;
    use crate::test_backend::VT100Backend;

    let width = UnicodeWidthStr::width("Loading transcript…") as u16;
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 1);
    let backend = VT100Backend::new(width, /*height*/ 1);
    let mut terminal = Terminal::with_options(backend).expect("terminal");
    terminal.set_viewport_area(area);
    let mut frame = terminal.get_frame();
    frame.buffer.set_style(
        area,
        Style::default().red().bg(Color::Yellow).dim().reversed(),
    );

    render_transcript_loading_overlay(&mut frame, area);

    let style = transcript_loading_overlay_style();
    assert_eq!(
        frame
            .buffer
            .content
            .iter()
            .map(|cell| (cell.fg, cell.bg, cell.modifier))
            .collect::<Vec<_>>(),
        vec![
            (
                style.fg.unwrap_or(Color::Reset),
                style.bg.unwrap_or(Color::Reset),
                Modifier::BOLD
            );
            usize::from(width)
        ]
    );
}

#[test]
fn compact_picker_keeps_metadata_and_labeled_primary_actions() {
    use crate::custom_terminal::Terminal;
    use crate::test_backend::VT100Backend;

    let timestamp = parse_timestamp_str("2026-09-17T12:00:00Z");
    let mut state = PickerState::new(
        FrameRequester::test_dummy(),
        Arc::new(|_| {}),
        ProviderFilter::Any,
        /*show_all*/ false,
        Some(PathBuf::from("/tmp/codex")),
        SessionPickerAction::Resume,
    );
    state.relative_time_reference = timestamp;
    state.filtered_rows = (1..=6)
        .map(|id| Row {
            path: None,
            thread_id: Some(ThreadId::from_u128(id)),
            thread_name: Some(format!("Session {id}")),
            preview: String::new(),
            created_at: timestamp,
            updated_at: timestamp,
            cwd: Some(PathBuf::from("/tmp/codex")),
            git_branch: Some("fcoury/contrast".into()),
        })
        .collect();
    let mut snapshots = Vec::new();
    with_test_default_colors(
        DefaultColors {
            fg: (32, 32, 32),
            bg: (255, 255, 255),
        },
        || {
            for (action, width, height) in [
                (SessionPickerAction::Resume, 118, 30),
                (SessionPickerAction::Resume, 70, 12),
                (SessionPickerAction::Resume, 38, 12),
                (SessionPickerAction::Fork, 38, 12),
            ] {
                state.action = action;
                let backend = VT100Backend::new(width, height);
                let mut terminal = Terminal::with_options(backend).expect("terminal");
                let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
                terminal.set_viewport_area(area);
                let list = layout::areas(area).list;
                state.update_viewport(usize::from(list.height), list_viewport_width(width));
                {
                    let mut frame = terminal.get_frame();
                    layout::render(&mut frame, &state);
                    let progress = frame
                        .buffer
                        .content
                        .iter()
                        .filter(|cell| cell.symbol() == "%")
                        .collect::<Vec<_>>();
                    assert_eq!(progress.len(), 1);
                    assert!(!progress[0].modifier.contains(Modifier::DIM));
                }
                terminal.flush().expect("flush");
                let text = terminal.backend().to_string();
                assert!(
                    text.contains(" fcoury/contrast"),
                    "{width}x{height}: {text}"
                );
                assert!(text.contains("now"), "{width}x{height}: {text}");
                assert!(
                    text.contains(&format!("enter {}", action.action_label())),
                    "{action:?} {width}x{height}: {text}"
                );
                assert!(
                    text.contains(if width >= FOOTER_COMPACT_BREAKPOINT {
                        "esc start new"
                    } else {
                        "esc new"
                    }),
                    "{width}x{height}: {text}"
                );
                snapshots.push(format!("{action:?} {width}x{height}\n{text}"));
            }
        },
    );
    insta::assert_snapshot!(snapshots.join("\n"));
}

#[test]
fn reversed_selection_overrides_thread_color() {
    let line = apply_line_background(
        Line::from("Title".fg(rgb_color((150, 50, 170)))),
        Style::default()
            .fg(Color::Reset)
            .bg(Color::Reset)
            .reversed(),
        /*width*/ 12,
    );
    assert_eq!(
        (
            line.spans[0].style.fg,
            line.spans[0]
                .style
                .add_modifier
                .contains(Modifier::REVERSED)
        ),
        (Some(Color::Reset), true)
    );
}

#[tokio::test]
async fn narrow_toolbar_keeps_keyboard_focused_control_visible() {
    use crate::custom_terminal::Terminal;
    use crate::test_backend::VT100Backend;

    let mut snapshots = Vec::new();
    for action in [SessionPickerAction::Resume, SessionPickerAction::Fork] {
        for (width, height) in [(38, 12), (24, 9)] {
            let mut state = PickerState::new(
                FrameRequester::test_dummy(),
                Arc::new(|_| {}),
                ProviderFilter::Any,
                /*show_all*/ false,
                Some(PathBuf::from("/tmp/codex")),
                action,
            );
            if matches!(action, SessionPickerAction::Resume) {
                state.status = SessionStatus::Archived;
            }
            let controls: &[(&str, &str)] = match action {
                SessionPickerAction::Resume => &[
                    ("Filter: Cwd", "Cwd"),
                    ("Status: Archived", "Archived"),
                    ("Sort: Updated", "Updated"),
                ],
                SessionPickerAction::Fork => {
                    &[("Filter: Cwd", "Cwd"), ("Sort: Updated", "Updated")]
                }
            };
            for &(label, value) in controls {
                with_test_default_colors(
                    DefaultColors {
                        fg: (32, 32, 32),
                        bg: (130, 130, 130),
                    },
                    || {
                        let backend = VT100Backend::new(width, height);
                        let mut terminal = Terminal::with_options(backend).expect("terminal");
                        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
                        terminal.set_viewport_area(area);
                        let list = layout::areas(area).list;
                        state.update_viewport(usize::from(list.height), list_viewport_width(width));
                        let mut frame = terminal.get_frame();
                        layout::render(&mut frame, &state);
                        let rows = frame
                            .buffer
                            .content
                            .chunks(usize::from(width))
                            .map(|row| {
                                row.iter()
                                    .map(ratatui::buffer::Cell::symbol)
                                    .collect::<String>()
                            })
                            .collect::<Vec<_>>();
                        let y = rows
                            .iter()
                            .position(|row| row.contains(label))
                            .expect("keyboard-focused toolbar control stays visible");
                        let x = rows[y].find(value).unwrap();
                        let focused_style = crate::bottom_pane::active_tab_style();
                        let cells = (x..x + value.len())
                            .map(|x| &frame.buffer[(x as u16, y as u16)])
                            .collect::<Vec<_>>();
                        assert_eq!(
                            cells.iter().map(|cell| cell.bg).collect::<Vec<_>>(),
                            vec![focused_style.bg.unwrap(); value.len()]
                        );
                        assert!(
                            cells
                                .iter()
                                .all(|cell| !cell.modifier.contains(Modifier::DIM))
                        );
                        snapshots.push(format!(
                            "{action:?} {width}x{height} {label}\n{}",
                            rows[y].trim_end()
                        ));
                    },
                );
                state
                    .handle_key(KeyEvent::from(KeyCode::Tab))
                    .await
                    .unwrap();
            }
        }
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
}
