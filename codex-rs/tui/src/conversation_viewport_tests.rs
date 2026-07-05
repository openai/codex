use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;

use super::*;
use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::ReasoningSummaryCell;
use crate::history_cell::SelectionContribution;
use crate::history_cell::UserHistoryCell;
use crate::history_cell::selection_contribution_from_display_lines;
use crate::tui::MouseScrollDirection;

#[derive(Debug)]
struct TestCell {
    display: &'static str,
    raw: &'static str,
    transcript: &'static str,
    is_stream_continuation: bool,
}

impl HistoryCell for TestCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec![self.display.into()]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![self.raw.into()]
    }

    fn transcript_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec![self.transcript.into()]
    }

    fn is_stream_continuation(&self) -> bool {
        self.is_stream_continuation
    }

    fn selection_contribution(&self, width: u16, mode: HistoryRenderMode) -> SelectionContribution {
        selection_contribution_from_display_lines(self.display_lines_for_mode(width, mode), width)
    }
}

#[derive(Debug)]
struct BlockStyleCell;

impl HistoryCell for BlockStyleCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec![
            "".into(),
            "› one two three".into(),
            "  four".into(),
            "  five six".into(),
            "  seven eight".into(),
            "".into(),
        ]
    }

    fn rich_block_style(&self) -> Option<Style> {
        Some(Style::default().bg(Color::Red))
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec!["raw source".into()]
    }

    fn selection_contribution(&self, width: u16, mode: HistoryRenderMode) -> SelectionContribution {
        selection_contribution_from_display_lines(self.display_lines_for_mode(width, mode), width)
    }
}

#[derive(Debug)]
struct SourceFragmentCell {
    display: &'static str,
    source: &'static str,
}

impl HistoryCell for SourceFragmentCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec![self.display.into()]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![self.display.into()]
    }

    fn selection_contribution(
        &self,
        width: u16,
        _mode: HistoryRenderMode,
    ) -> SelectionContribution {
        match crate::history_cell::selection_contribution_from_semantic_text(
            self.source.to_string(),
            self.display_lines(width),
            width,
            /*first_row_prefix_columns*/ 0,
        ) {
            SelectionContribution::Selectable(projection) => {
                SelectionContribution::Selectable(projection.with_separator_before(""))
            }
            SelectionContribution::Transparent => SelectionContribution::Transparent,
        }
    }

    fn is_stream_continuation(&self) -> bool {
        true
    }
}

fn cell(display: &'static str) -> Arc<dyn HistoryCell> {
    Arc::new(TestCell {
        display,
        raw: display,
        transcript: display,
        is_stream_continuation: false,
    })
}

fn live_cell(
    width: u16,
    display: &'static str,
    is_stream_continuation: bool,
) -> ActiveCellDisplaySnapshot {
    let lines = vec![HyperlinkLine::from(display)];
    ActiveCellDisplaySnapshot {
        selection_projection: selection_contribution_from_display_lines(
            crate::terminal_hyperlinks::visible_lines(lines.clone()),
            width,
        )
        .into_projection(),
        lines,
        is_stream_continuation,
    }
}

fn viewport(cells: Vec<Arc<dyn HistoryCell>>) -> ConversationViewport {
    ConversationViewport::new(
        cells,
        HistoryRenderMode::Rich,
        crate::keymap::RuntimeKeymap::defaults().pager,
    )
}

fn select_entire_projection(projection: CellSelectionProjection) -> String {
    let (first_row, first_column) = projection
        .rows()
        .iter()
        .enumerate()
        .find_map(|(row, selection_row)| {
            selection_row
                .segments
                .first()
                .map(|segment| (row, segment.columns.start))
        })
        .expect("projection should have a first selectable column");
    let (last_row, last_column) = projection
        .rows()
        .iter()
        .enumerate()
        .rev()
        .find_map(|(row, selection_row)| {
            selection_row
                .segments
                .last()
                .map(|segment| (row, segment.columns.end.saturating_sub(/*rhs*/ 1)))
        })
        .expect("projection should have a last selectable column");
    let layout = [SelectionCellLayout {
        top: 0,
        height: projection.rows().len(),
    }];
    let mut selection = ConversationSelection::default();
    selection.start(SelectionPoint {
        row: first_row,
        column: first_column,
    });
    selection.update(SelectionPoint {
        row: last_row,
        column: last_column,
    });
    selection
        .finish(/*point*/ None, &[Some(projection)], &layout)
        .expect("dragging the full projection should select text")
}

#[test]
fn renders_main_display_and_live_tail_without_pager_chrome() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(TestCell {
        display: "compact display",
        raw: "raw source",
        transcript: "expanded transcript detail",
        is_stream_continuation: false,
    })];
    let mut viewport = viewport(cells);
    viewport.sync_live_tail(
        /*width*/ 32,
        Some(ActiveCellRenderKey {
            revision: 1,
            is_stream_continuation: false,
            animation_tick: None,
        }),
        |width| {
            Some(vec![live_cell(
                width,
                "live tail",
                /*is_stream_continuation*/ false,
            )])
        },
    );
    let mut terminal =
        Terminal::new(TestBackend::new(/*width*/ 32, /*height*/ 6)).expect("create terminal");

    terminal
        .draw(|frame| viewport.render(frame.area(), frame.buffer_mut()))
        .expect("render conversation viewport");

    assert_snapshot!(terminal.backend(), @r###"
"compact display                 "
"                                "
"live tail                       "
"                                "
"                                "
"                                "
"###);
}

#[test]
fn markdown_cells_use_single_blank_rows_inside_and_between_cells() {
    let cwd = std::env::temp_dir();
    let cells: Vec<Arc<dyn HistoryCell>> = vec![
        Arc::new(ReasoningSummaryCell::new(
            "reasoning".to_string(),
            "first thought\n\nsecond thought".to_string(),
            &cwd,
            /*transcript_only*/ false,
        )),
        Arc::new(AgentMarkdownCell::new(
            "first answer\n\nsecond answer".to_string(),
            &cwd,
        )),
    ];
    let mut viewport = viewport(cells);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 7,
    );
    let mut buffer = Buffer::empty(area);

    viewport.render(area, &mut buffer);

    let text = buffer_text(&buffer, area)
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    assert_snapshot!(text, @r###"
• first thought

  second thought

• first answer

  second answer
"###);
}

#[test]
fn switches_between_rich_and_raw_cell_representations() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(TestCell {
        display: "rich display",
        raw: "raw source",
        transcript: "expanded transcript detail",
        is_stream_continuation: false,
    })];
    let mut viewport = viewport(cells);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 3,
    );
    let mut rich = Buffer::empty(area);
    viewport.render(area, &mut rich);

    viewport.set_render_mode(HistoryRenderMode::Raw);
    let mut raw = Buffer::empty(area);
    viewport.render(area, &mut raw);

    assert!(buffer_text(&rich, area).contains("rich display"));
    assert!(buffer_text(&raw, area).contains("raw source"));
    assert!(!buffer_text(&raw, area).contains("expanded transcript detail"));
}

#[test]
fn rich_block_style_fills_owned_cell_without_leaking_to_raw_or_following_cells() {
    let user_cell = UserHistoryCell {
        message: "prompt".to_string(),
        text_elements: Vec::new(),
        local_image_paths: Vec::new(),
        remote_image_urls: Vec::new(),
    };
    assert!(user_cell.rich_block_style().is_some());

    let mut viewport = viewport(vec![Arc::new(BlockStyleCell), cell("assistant")]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 16, /*height*/ 8,
    );
    let mut rich = Buffer::empty(area);
    viewport.render(area, &mut rich);

    viewport.set_render_mode(HistoryRenderMode::Raw);
    let mut raw = Buffer::empty(area);
    viewport.render(area, &mut raw);

    let trim_rows = |buffer: &Buffer| {
        buffer_text(buffer, area)
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
    };
    let background_mask = |buffer: &Buffer| {
        (area.y..area.bottom())
            .map(|y| {
                (area.x..area.right())
                    .map(|x| match buffer[(x, y)].style().bg {
                        Some(Color::Red) => '#',
                        _ => '.',
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    assert_snapshot!(format!(
        "rich text:\n{}\nrich background:\n{}\nraw text:\n{}\nraw background:\n{}",
        trim_rows(&rich),
        background_mask(&rich),
        trim_rows(&raw),
        background_mask(&raw),
    ), @r###"
rich text:

› one two three
  four
  five six
  seven eight


assistant
rich background:
################
################
################
################
################
################
................
................
raw text:
raw source

assistant





raw background:
................
................
................
................
................
................
................
................
"###);
}

#[test]
fn append_keeps_live_tail_after_committed_cells() {
    let mut viewport = viewport(Vec::new());
    viewport.sync_live_tail(
        /*width*/ 24,
        Some(ActiveCellRenderKey {
            revision: 1,
            is_stream_continuation: false,
            animation_tick: None,
        }),
        |width| {
            Some(vec![live_cell(
                width,
                "live tail",
                /*is_stream_continuation*/ false,
            )])
        },
    );

    viewport.push_cell(cell("committed"));

    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 5,
    );
    let mut buffer = Buffer::empty(area);
    viewport.render(area, &mut buffer);
    let text = buffer_text(&buffer, area);
    let committed = text
        .find("committed")
        .expect("committed cell should render");
    let live_tail = text.find("live tail").expect("live tail should render");
    assert!(committed < live_tail);
    assert_eq!(viewport.committed_cell_count(), 1);
}

#[test]
fn selection_uses_stream_continuation_separator_before_live_cells() {
    for (is_stream_continuation, live_row, expected) in [
        (false, 2, "committed\n\nlive"),
        (true, 1, "committed\nlive"),
    ] {
        let mut viewport = viewport(vec![cell("committed")]);
        viewport.sync_live_tail(
            /*width*/ 24,
            Some(ActiveCellRenderKey {
                revision: 1,
                is_stream_continuation,
                animation_tick: None,
            }),
            |width| Some(vec![live_cell(width, "live", is_stream_continuation)]),
        );
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 4,
        );
        viewport.render(area, &mut Buffer::empty(area));

        assert!(viewport.begin_selection(area, Position::new(/*x*/ 0, /*y*/ 0),));
        assert!(viewport.update_selection(area, Position::new(/*x*/ 3, live_row),));
        assert_eq!(
            viewport.finish_selection(area, Position::new(/*x*/ 3, live_row)),
            Some(expected.to_string())
        );
    }
}

#[test]
fn viewport_preserves_source_fragment_empty_separator() {
    for (source, expected) in [(" beta", "alpha beta"), ("\nbeta", "alpha\nbeta")] {
        let mut viewport = viewport(vec![
            cell("alpha"),
            Arc::new(SourceFragmentCell {
                display: "beta",
                source,
            }),
        ]);
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 2,
        );
        viewport.render(area, &mut Buffer::empty(area));

        assert!(viewport.begin_selection(area, Position::new(/*x*/ 0, /*y*/ 0),));
        assert!(viewport.update_selection(area, Position::new(/*x*/ 3, /*y*/ 1),));
        assert_eq!(
            viewport.finish_selection(area, Position::new(/*x*/ 3, /*y*/ 1)),
            Some(expected.to_string())
        );
    }
}

#[test]
fn multiple_live_cells_keep_individual_selection_projections() {
    let mut viewport = viewport(Vec::new());
    viewport.sync_live_tail(
        /*width*/ 24,
        Some(ActiveCellRenderKey {
            revision: 1,
            is_stream_continuation: false,
            animation_tick: None,
        }),
        |width| {
            Some(vec![
                live_cell(width, "tool output", /*is_stream_continuation*/ false),
                live_cell(width, "hook output", /*is_stream_continuation*/ false),
            ])
        },
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 4,
    );
    viewport.render(area, &mut Buffer::empty(area));

    assert!(viewport.begin_selection(area, Position::new(/*x*/ 0, /*y*/ 0),));
    assert!(viewport.update_selection(area, Position::new(/*x*/ 10, /*y*/ 2),));
    assert_eq!(
        viewport.finish_selection(area, Position::new(/*x*/ 10, /*y*/ 2)),
        Some("tool output\n\nhook output".to_string())
    );
}

#[test]
fn replacing_cells_invalidates_and_respaces_the_live_tail() {
    let key = ActiveCellRenderKey {
        revision: 1,
        is_stream_continuation: false,
        animation_tick: None,
    };
    let mut viewport = viewport(Vec::new());
    viewport.sync_live_tail(/*width*/ 24, Some(key), |width| {
        Some(vec![live_cell(
            width,
            "live tail",
            /*is_stream_continuation*/ false,
        )])
    });

    viewport.replace_cells(vec![cell("replacement")]);
    viewport.sync_live_tail(/*width*/ 24, Some(key), |width| {
        Some(vec![live_cell(
            width,
            "live tail",
            /*is_stream_continuation*/ false,
        )])
    });

    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 4,
    );
    let mut buffer = Buffer::empty(area);
    viewport.render(area, &mut buffer);
    let rows = buffer_text(&buffer, area);
    assert_eq!(
        rows.lines().map(str::trim_end).collect::<Vec<_>>(),
        vec!["replacement", "", "live tail", ""]
    );
}

#[test]
fn preserves_semantic_links_for_committed_and_live_content() {
    let committed_destination = "https://example.com/committed";
    let live_destination = "https://example.com/live";
    let committed: Arc<dyn HistoryCell> = Arc::new(AgentMarkdownCell::new(
        committed_destination.to_string(),
        std::path::Path::new("/tmp"),
    ));
    let live = AgentMarkdownCell::new(live_destination.to_string(), std::path::Path::new("/tmp"));
    let mut viewport = viewport(vec![committed]);
    viewport.sync_live_tail(
        /*width*/ 28,
        Some(ActiveCellRenderKey {
            revision: 1,
            is_stream_continuation: false,
            animation_tick: None,
        }),
        |width| {
            Some(vec![ActiveCellDisplaySnapshot {
                lines: live.display_hyperlink_lines(width),
                selection_projection: live
                    .selection_contribution(width, HistoryRenderMode::Rich)
                    .into_projection(),
                is_stream_continuation: live.is_stream_continuation(),
            }])
        },
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 28, /*height*/ 6,
    );
    let mut buffer = Buffer::empty(area);

    viewport.render(area, &mut buffer);

    let rendered = buffer_text(&buffer, area);
    assert!(rendered.contains(&format!("\x1b]8;;{committed_destination}\x07")));
    assert!(rendered.contains(&format!("\x1b]8;;{live_destination}\x07")));
}

#[test]
fn narrower_resize_stays_pinned_to_the_latest_cell() {
    let mut viewport = viewport(vec![
        cell("first long row that wraps"),
        cell("second long row that wraps"),
        cell("LATEST SENTINEL"),
    ]);
    let wide = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 4,
    );
    viewport.render(wide, &mut Buffer::empty(wide));

    let narrow = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 12, /*height*/ 4,
    );
    let mut buffer = Buffer::empty(narrow);
    viewport.render(narrow, &mut buffer);

    assert!(viewport.is_following_bottom());
    assert!(buffer_text(&buffer, narrow).contains("SENTINEL"));
}

#[test]
fn page_navigation_leaves_and_restores_bottom_follow() {
    let mut viewport = viewport(vec![cell("oldest"), cell("middle"), cell("LATEST")]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 2,
    );
    let mut bottom = Buffer::empty(area);
    viewport.render(area, &mut bottom);
    assert!(buffer_text(&bottom, area).contains("LATEST"));

    assert!(
        viewport.handle_navigation_key(area, KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),)
    );
    let mut scrolled = Buffer::empty(area);
    viewport.render(area, &mut scrolled);
    assert!(!viewport.is_following_bottom());
    assert!(buffer_text(&scrolled, area).contains("middle"));

    assert!(
        viewport.handle_navigation_key(area, KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),)
    );
    let mut restored = Buffer::empty(area);
    viewport.render(area, &mut restored);
    assert!(viewport.is_following_bottom());
    let trim_rows = |buffer: &Buffer| {
        buffer_text(buffer, area)
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_snapshot!(format!(
        "bottom:\n{}\nscrolled:\n{}\nrestored:\n{}",
        trim_rows(&bottom),
        trim_rows(&scrolled),
        trim_rows(&restored),
    ), @r###"
bottom:

LATEST
scrolled:

middle
restored:

LATEST
"###);
}

#[test]
fn mouse_wheel_leaves_and_restores_bottom_follow() {
    let mut viewport = viewport(vec![
        cell("oldest"),
        cell("older"),
        cell("middle"),
        cell("newer"),
        cell("LATEST"),
    ]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 3,
    );
    let mut bottom = Buffer::empty(area);
    viewport.render(area, &mut bottom);

    viewport.handle_mouse_scroll(MouseScrollDirection::Up);
    let mut scrolled = Buffer::empty(area);
    viewport.render(area, &mut scrolled);
    assert!(!viewport.is_following_bottom());

    viewport.handle_mouse_scroll(MouseScrollDirection::Down);
    let mut restored = Buffer::empty(area);
    viewport.render(area, &mut restored);
    assert!(viewport.is_following_bottom());

    let trim_rows = |buffer: &Buffer| {
        buffer_text(buffer, area)
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_snapshot!(format!(
        "bottom:\n{}\nscrolled:\n{}\nrestored:\n{}",
        trim_rows(&bottom),
        trim_rows(&scrolled),
        trim_rows(&restored),
    ), @r###"
bottom:
newer

LATEST
scrolled:

middle

restored:
newer

LATEST
"###);
}

#[test]
fn mouse_wheel_extends_an_active_selection_into_offscreen_history() {
    let mut viewport = viewport(vec![
        cell("oldest"),
        cell("older"),
        cell("middle"),
        cell("newer"),
        cell("LATEST"),
    ]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 3,
    );
    viewport.render(area, &mut Buffer::empty(area));

    let anchor = Position::new(/*x*/ 5, /*y*/ 2);
    let focus = Position::new(/*x*/ 0, /*y*/ 0);
    assert!(viewport.begin_selection(area, anchor));
    assert!(viewport.update_selection(area, focus));

    viewport.handle_selection_mouse_scroll(area, MouseScrollDirection::Up, focus);

    assert!(viewport.selection_is_active());
    assert!(!viewport.is_following_bottom());
    let mut highlighted = Buffer::empty(area);
    viewport.render(area, &mut highlighted);
    for column in 0.."middle".len() as u16 {
        assert!(
            highlighted[(column, 1)]
                .modifier
                .contains(Modifier::REVERSED),
            "middle column {column} should be highlighted after scrolling"
        );
    }
    assert_eq!(
        viewport.finish_selection(area, focus),
        Some("middle\n\nnewer\n\nLATEST".to_string())
    );
}

#[test]
fn selection_hit_testing_accounts_for_scroll_offset_and_cell_spacing() {
    let mut viewport = viewport(vec![cell("oldest"), cell("LATEST")]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 2,
    );
    viewport.render(area, &mut Buffer::empty(area));

    assert!(viewport.begin_selection(area, Position::new(/*x*/ 0, /*y*/ 0)));
    viewport.cancel_selection();
    assert!(viewport.begin_selection(area, Position::new(/*x*/ 0, /*y*/ 1),));
    assert!(viewport.update_selection(area, Position::new(/*x*/ 5, /*y*/ 1),));

    let mut highlighted = Buffer::empty(area);
    viewport.render(area, &mut highlighted);
    for column in 0..=5 {
        assert!(
            highlighted[(column, 1)]
                .modifier
                .contains(ratatui::style::Modifier::REVERSED)
        );
    }
    assert_eq!(
        viewport.finish_selection(area, Position::new(/*x*/ 5, /*y*/ 1)),
        Some("LATEST".to_string())
    );
}

#[test]
fn selection_can_start_in_right_padding_and_drag_backward() {
    let mut viewport = viewport(vec![cell("hello")]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 1,
    );
    viewport.render(area, &mut Buffer::empty(area));

    assert!(viewport.begin_selection(area, Position::new(/*x*/ 19, /*y*/ 0)));
    assert!(viewport.update_selection(area, Position::new(/*x*/ 1, /*y*/ 0)));
    let mut highlighted = Buffer::empty(area);
    viewport.render(area, &mut highlighted);
    let mask = (area.x..area.right())
        .map(|x| {
            if highlighted[(x, area.y)]
                .modifier
                .contains(Modifier::REVERSED)
            {
                '#'
            } else {
                '.'
            }
        })
        .collect::<String>();
    assert_snapshot!(mask, @".####...............");
    assert_eq!(
        viewport.finish_selection(area, Position::new(/*x*/ 1, /*y*/ 0)),
        Some("ello".to_string())
    );
}

#[test]
fn selection_can_start_in_inter_cell_spacing() {
    let mut viewport = viewport(vec![cell("first"), cell("second")]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 3,
    );
    viewport.render(area, &mut Buffer::empty(area));

    assert!(viewport.begin_selection(area, Position::new(/*x*/ 10, /*y*/ 1)));
    assert!(viewport.update_selection(area, Position::new(/*x*/ 5, /*y*/ 2)));
    assert_eq!(
        viewport.finish_selection(area, Position::new(/*x*/ 5, /*y*/ 2)),
        Some("second".to_string())
    );
}

#[test]
fn backward_selection_can_start_below_the_last_cell() {
    let mut viewport = viewport(vec![cell("hello")]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 3,
    );
    viewport.render(area, &mut Buffer::empty(area));

    assert!(viewport.begin_selection(area, Position::new(/*x*/ 19, /*y*/ 2)));
    assert!(viewport.update_selection(area, Position::new(/*x*/ 0, /*y*/ 0)));
    assert_eq!(
        viewport.finish_selection(area, Position::new(/*x*/ 0, /*y*/ 0)),
        Some("hello".to_string())
    );
}

#[test]
fn press_outside_conversation_area_does_not_start_selection() {
    let mut viewport = viewport(vec![cell("hello")]);
    let area = Rect::new(
        /*x*/ 2, /*y*/ 2, /*width*/ 20, /*height*/ 3,
    );
    viewport.render(area, &mut Buffer::empty(area));

    assert!(!viewport.begin_selection(area, Position::new(/*x*/ 1, /*y*/ 2)));
}

#[test]
fn release_beyond_the_last_drag_lazily_projects_the_full_cell_span() {
    let mut viewport = viewport(vec![cell("first"), cell("second"), cell("third")]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 5,
    );
    viewport.render(area, &mut Buffer::empty(area));
    assert!(viewport.begin_selection(area, Position::new(/*x*/ 0, /*y*/ 0),));
    assert!(viewport.update_selection(area, Position::new(/*x*/ 2, /*y*/ 0),));

    assert_eq!(
        viewport.finish_selection(area, Position::new(/*x*/ 4, /*y*/ 4)),
        Some("first\n\nsecond\n\nthird".to_string())
    );
}

#[test]
fn replacing_cells_is_deferred_until_active_selection_finishes() {
    let mut viewport = viewport(vec![cell("old")]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 2,
    );
    viewport.render(area, &mut Buffer::empty(area));
    assert!(viewport.begin_selection(area, Position::new(/*x*/ 0, /*y*/ 0),));
    assert!(viewport.update_selection(area, Position::new(/*x*/ 2, /*y*/ 0),));

    viewport.replace_cells(vec![cell("replacement")]);

    assert!(viewport.selection_is_active());
    assert_eq!(
        viewport.finish_selection(area, Position::new(/*x*/ 2, /*y*/ 0)),
        Some("old".to_string())
    );
    let mut updated = Buffer::empty(area);
    viewport.render(area, &mut updated);
    assert!(buffer_text(&updated, area).starts_with("replacement"));
}

#[test]
fn width_change_safely_applies_a_deferred_shrinking_replacement() {
    let mut viewport = viewport(vec![cell("first"), cell("second")]);
    let wide = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 3,
    );
    viewport.render(wide, &mut Buffer::empty(wide));
    assert!(viewport.begin_selection(wide, Position::new(/*x*/ 0, /*y*/ 0),));
    assert!(viewport.update_selection(wide, Position::new(/*x*/ 4, /*y*/ 0),));
    viewport.replace_cells(vec![cell("new")]);

    let narrow = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 10, /*height*/ 3,
    );
    viewport.render(narrow, &mut Buffer::empty(narrow));

    assert!(!viewport.selection_is_active());
    assert_eq!(viewport.committed_cell_count(), 1);
    let mut updated = Buffer::empty(narrow);
    viewport.render(narrow, &mut updated);
    assert!(buffer_text(&updated, narrow).starts_with("new"));
}

#[test]
fn committed_stream_chunks_are_deferred_until_active_selection_finishes() {
    let mut viewport = viewport(vec![cell("stable")]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 3,
    );
    viewport.render(area, &mut Buffer::empty(area));
    assert!(viewport.begin_selection(area, Position::new(/*x*/ 0, /*y*/ 0),));
    assert!(viewport.update_selection(area, Position::new(/*x*/ 5, /*y*/ 0),));

    viewport.push_cell(cell("new stream chunk"));
    let mut frozen = Buffer::empty(area);
    viewport.render(area, &mut frozen);

    assert!(viewport.selection_is_active());
    assert!(!buffer_text(&frozen, area).contains("new stream chunk"));
    assert_eq!(
        viewport.finish_selection(area, Position::new(/*x*/ 5, /*y*/ 0)),
        Some("stable".to_string())
    );
    let mut updated = Buffer::empty(area);
    viewport.render(area, &mut updated);
    assert!(buffer_text(&updated, area).contains("new stream chunk"));
}

#[test]
fn live_tail_updates_are_frozen_while_selecting_live_text() {
    let mut viewport = viewport(vec![cell("committed")]);
    viewport.sync_live_tail(
        /*width*/ 20,
        Some(ActiveCellRenderKey {
            revision: 1,
            is_stream_continuation: false,
            animation_tick: Some(1),
        }),
        |width| {
            Some(vec![live_cell(
                width, "live one", /*is_stream_continuation*/ false,
            )])
        },
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 4,
    );
    viewport.render(area, &mut Buffer::empty(area));
    assert!(viewport.begin_selection(area, Position::new(/*x*/ 0, /*y*/ 2),));

    viewport.sync_live_tail(
        /*width*/ 20,
        Some(ActiveCellRenderKey {
            revision: 1,
            is_stream_continuation: false,
            animation_tick: Some(2),
        }),
        |width| {
            Some(vec![live_cell(
                width,
                "live two now wraps across several terminal rows",
                /*is_stream_continuation*/ false,
            )])
        },
    );
    let mut frozen = Buffer::empty(area);
    viewport.render(area, &mut frozen);

    assert!(viewport.selection_is_active());
    assert!(buffer_text(&frozen, area).contains("live one"));
    assert!(!buffer_text(&frozen, area).contains("live two"));
    assert!(viewport.update_selection(area, Position::new(/*x*/ 7, /*y*/ 2),));
    let mut highlighted = Buffer::empty(area);
    viewport.render(area, &mut highlighted);
    let mask = (area.x..area.right())
        .map(|x| {
            if highlighted[(x, area.y.saturating_add(/*rhs*/ 2))]
                .modifier
                .contains(Modifier::REVERSED)
            {
                '#'
            } else {
                '.'
            }
        })
        .collect::<String>();
    assert_snapshot!(mask, @"########............");
    assert_eq!(
        viewport.finish_selection(area, Position::new(/*x*/ 7, /*y*/ 2)),
        Some("live one".to_string())
    );
}

#[test]
fn user_selection_is_source_exact_at_narrow_and_wide_widths() {
    let source = "界 x — visit https://example.com/a/very/long/path and keep going";
    let cell = UserHistoryCell {
        message: source.to_string(),
        text_elements: Vec::new(),
        local_image_paths: Vec::new(),
        remote_image_urls: Vec::new(),
    };

    for width in [18, 80] {
        let projection = cell
            .selection_contribution(width, HistoryRenderMode::Rich)
            .into_projection()
            .expect("user message should be selectable");
        assert_eq!(select_entire_projection(projection), source);
    }
}

#[test]
fn agent_markdown_selection_is_semantic_and_width_invariant() {
    let destination = "https://example.com/a/very/long/path";
    let markdown = format!("**界 x** — [site]({destination})\n\n`code` stays");
    let expected = format!("界 x — site ({destination})\n\ncode stays");
    let cell = AgentMarkdownCell::new(markdown, std::path::Path::new("/tmp"));

    for width in [18, 80] {
        let projection = cell
            .selection_contribution(width, HistoryRenderMode::Rich)
            .into_projection()
            .expect("agent Markdown should be selectable");
        assert_eq!(select_entire_projection(projection), expected);
    }
}

#[test]
fn agent_markdown_preserves_whitespace_only_code_line() {
    let markdown = "```text\none\n \t \ntwo\n```";
    let cell = AgentMarkdownCell::new(markdown.to_string(), &std::env::temp_dir());
    let lines = cell.display_lines(/*width*/ 80);
    let authored_line = lines[1]
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    let projection = cell
        .selection_contribution(/*width*/ 80, HistoryRenderMode::Rich)
        .into_projection()
        .expect("agent Markdown should be selectable");

    assert_eq!(authored_line, "   \t ");
    assert_eq!(select_entire_projection(projection), "one\n \t \ntwo");
}

#[test]
fn agent_markdown_partial_selection_uses_screen_coordinates_and_semantic_link_text() {
    let destination = "https://example.com/z";
    let markdown = format!("before **bold** [Ω]({destination}) after");
    let cell: Arc<dyn HistoryCell> = Arc::new(AgentMarkdownCell::new(
        markdown,
        std::path::Path::new("/tmp"),
    ));
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 18, /*height*/ 6,
    );
    let mut viewport = viewport(vec![cell]);
    let mut buffer = Buffer::empty(area);
    viewport.render(area, &mut buffer);
    let find_symbol = |needle: char| {
        (area.y..area.bottom())
            .flat_map(|y| (area.x..area.right()).map(move |x| Position::new(x, y)))
            .find(|position| {
                crate::terminal_hyperlinks::strip_osc8(buffer[(position.x, position.y)].symbol())
                    == needle.to_string()
            })
            .unwrap_or_else(|| panic!("rendered Markdown should contain {needle}"))
    };
    let start = find_symbol('Ω');
    let end = find_symbol(')');

    assert!(viewport.begin_selection(area, start));
    assert!(viewport.update_selection(area, end));
    assert_eq!(
        viewport.finish_selection(area, end),
        Some(format!("Ω ({destination})"))
    );
}

#[test]
fn agent_tables_copy_logical_cells_without_layout_chrome() {
    let markdown = "| Name | Description |\n| --- | --- |\n| alpha | a value that wraps over several words |\n| beta | short |";
    let cell = AgentMarkdownCell::new(markdown.to_string(), std::path::Path::new("/tmp"));
    let expected = "Name\tDescription\nalpha\ta value that wraps over several words\nbeta\tshort";

    for width in [12, 16, 20, 24, 32, 40, 80] {
        let projection = cell
            .selection_contribution(width, HistoryRenderMode::Rich)
            .into_projection()
            .expect("rich table selection should expose a projection");
        assert_eq!(projection.text(), expected);
        assert_eq!(
            select_entire_projection(projection),
            expected,
            "rich table selection should stay semantic at width {width}",
        );
    }
    let projection = cell
        .selection_contribution(/*width*/ 24, HistoryRenderMode::Raw)
        .into_projection()
        .expect("raw Markdown should remain selectable");
    assert_eq!(select_entire_projection(projection), markdown);
}

#[test]
fn agent_table_projection_resumes_after_table_and_preceding_overlong_line() {
    let markdown = "before\n\n```text\nthis-is-an-intentionally-overlong-code-line\n```\n\n| Name | Value |\n| --- | --- |\n| alpha | a value that wraps |\n\nafter";
    let cell = AgentMarkdownCell::new(markdown.to_string(), std::path::Path::new("/tmp"));
    let expected = crate::markdown_render::render_markdown_selection_text(
        markdown,
        Some(std::path::Path::new("/tmp")),
    );

    let projection = cell
        .selection_contribution(/*width*/ 16, HistoryRenderMode::Rich)
        .into_projection()
        .expect("mixed Markdown table output should be selectable");
    assert_eq!(select_entire_projection(projection), expected);
}

#[test]
fn narrow_wrapped_table_value_selection_omits_wrap_newlines_and_padding() {
    let markdown =
        "| Name | Description |\n| --- | --- |\n| alpha | a value that wraps over several words |";
    let cell = AgentMarkdownCell::new(markdown.to_string(), std::path::Path::new("/tmp"));
    let projection = cell
        .selection_contribution(/*width*/ 16, HistoryRenderMode::Rich)
        .into_projection()
        .expect("wrapped table should expose source-backed rows");
    let expected = "value that wraps over several words";
    let start_byte = projection
        .text()
        .find(expected)
        .expect("canonical table text should contain the wrapped value");
    let end_byte = start_byte.saturating_add(expected.len());
    let start = projection
        .rows()
        .iter()
        .enumerate()
        .find_map(|(row, selection_row)| {
            selection_row
                .segments
                .iter()
                .find(|segment| segment.bytes.start == start_byte)
                .map(|segment| SelectionPoint {
                    row,
                    column: segment.columns.start,
                })
        })
        .expect("wrapped value start should map to a screen column");
    let end = projection
        .rows()
        .iter()
        .enumerate()
        .find_map(|(row, selection_row)| {
            selection_row
                .segments
                .iter()
                .find(|segment| segment.bytes.end == end_byte)
                .map(|segment| SelectionPoint {
                    row,
                    column: segment.columns.end.saturating_sub(/*rhs*/ 1),
                })
        })
        .expect("wrapped value end should map to a screen column");
    assert!(
        start.row < end.row,
        "fixture should wrap across screen rows"
    );

    let layout = [SelectionCellLayout {
        top: 0,
        height: projection.rows().len(),
    }];
    let mut selection = ConversationSelection::default();
    selection.start(start);
    assert!(selection.update(end));
    assert_eq!(
        selection.finish(Some(end), &[Some(projection)], &layout),
        Some(expected.to_string())
    );
}

#[test]
fn narrow_table_adjacent_field_selection_is_stable_in_both_directions() {
    let markdown =
        "| Name | Description |\n| --- | --- |\n| alpha | a value that wraps over several words |";
    let cell = AgentMarkdownCell::new(markdown.to_string(), std::path::Path::new("/tmp"));
    let projection = cell
        .selection_contribution(/*width*/ 12, HistoryRenderMode::Rich)
        .into_projection()
        .expect("narrow table should expose source-backed rows");
    let expected = "Description\nalpha\ta value that wraps over several words";
    let start_byte = projection
        .text()
        .find("alpha")
        .expect("canonical table text should contain the first body field");
    let value = "a value that wraps over several words";
    let end_byte = projection
        .text()
        .find(value)
        .expect("canonical table text should contain the second body field")
        .saturating_add(value.len());
    let point_for_byte = |byte: usize, use_end: bool| {
        projection
            .rows()
            .iter()
            .enumerate()
            .find_map(|(row, selection_row)| {
                selection_row.segments.iter().find_map(|segment| {
                    let matches = if use_end {
                        segment.bytes.end == byte
                    } else {
                        segment.bytes.start == byte
                    };
                    matches.then(|| SelectionPoint {
                        row,
                        column: if use_end {
                            segment.columns.end.saturating_sub(/*rhs*/ 1)
                        } else {
                            segment.columns.start
                        },
                    })
                })
            })
            .expect("body field endpoint should map to a screen column")
    };
    let start = point_for_byte(start_byte, /*use_end*/ false);
    let end = point_for_byte(end_byte, /*use_end*/ true);
    assert!(
        start.row < end.row,
        "fixture should use stacked record fields"
    );
    let layout = [SelectionCellLayout {
        top: 0,
        height: projection.rows().len(),
    }];

    for (anchor, focus) in [(start, end), (end, start)] {
        let mut selection = ConversationSelection::default();
        selection.start(anchor);
        assert!(selection.update(focus));
        assert_eq!(
            selection.finish(Some(focus), &[Some(projection.clone())], &layout),
            Some(expected.to_string())
        );
    }

    let label = "Description";
    let label_start = projection
        .text()
        .find(label)
        .expect("visible record label should be in canonical text");
    let label_end = label_start.saturating_add(label.len());
    let label_start = point_for_byte(label_start, /*use_end*/ false);
    let label_end = point_for_byte(label_end, /*use_end*/ true);
    let mut selection = ConversationSelection::default();
    selection.start(label_start);
    assert!(selection.update(label_end));
    assert_eq!(
        selection.finish(Some(label_end), &[Some(projection.clone())], &layout,),
        Some(label.to_string())
    );
}

#[test]
fn wrapped_grid_continuations_remain_selectable_in_visual_order() {
    let markdown =
        "| Left | Right |\n| --- | --- |\n| alpha beta gamma delta | one two three four |";
    let cell = AgentMarkdownCell::new(markdown.to_string(), std::path::Path::new("/tmp"));
    let width = 28;
    assert!(
        cell.display_lines(width)
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content.contains('━'))),
        "fixture should use the aligned grid presentation"
    );
    let projection = cell
        .selection_contribution(width, HistoryRenderMode::Rich)
        .into_projection()
        .expect("wrapped grid should expose source-backed rows");
    let mapped_ranges = projection
        .rows()
        .iter()
        .flat_map(|row| &row.segments)
        .map(|segment| segment.bytes.clone())
        .collect::<Vec<_>>();
    for (start, character) in projection.text().char_indices() {
        if character.is_whitespace() {
            continue;
        }
        let range = start..start.saturating_add(character.len_utf8());
        assert!(
            mapped_ranges.contains(&range),
            "visible semantic character {character:?} at {range:?} had no grid segment"
        );
    }

    let continuation = "gamma delta";
    let start_byte = projection
        .text()
        .find(continuation)
        .expect("first-column continuation should remain in canonical text");
    let end_byte = start_byte.saturating_add(continuation.len());
    let point_for_byte = |byte: usize, use_end: bool| {
        projection
            .rows()
            .iter()
            .enumerate()
            .find_map(|(row, selection_row)| {
                selection_row.segments.iter().find_map(|segment| {
                    (if use_end {
                        segment.bytes.end == byte
                    } else {
                        segment.bytes.start == byte
                    })
                    .then(|| SelectionPoint {
                        row,
                        column: if use_end {
                            segment.columns.end.saturating_sub(/*rhs*/ 1)
                        } else {
                            segment.columns.start
                        },
                    })
                })
            })
            .expect("grid continuation endpoint should map to a screen column")
    };
    let start = point_for_byte(start_byte, /*use_end*/ false);
    let end = point_for_byte(end_byte, /*use_end*/ true);
    let layout = [SelectionCellLayout {
        top: 0,
        height: projection.rows().len(),
    }];
    let mut selection = ConversationSelection::default();
    selection.start(start);
    assert!(selection.update(end));
    assert_eq!(
        selection.finish(Some(end), &[Some(projection)], &layout),
        Some("gamma delta\nthree four".to_string())
    );
}

#[test]
fn selected_user_prompt_highlights_source_glyphs_without_chrome_or_padding() {
    let width = 18;
    let source = "alpha beta gamma delta";
    let cell: Arc<dyn HistoryCell> = Arc::new(UserHistoryCell {
        message: source.to_string(),
        text_elements: Vec::new(),
        local_image_paths: Vec::new(),
        remote_image_urls: Vec::new(),
    });
    let height = cell.desired_height_for_mode(width, HistoryRenderMode::Rich);
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
    let mut viewport = viewport(vec![cell]);
    viewport.render(area, &mut Buffer::empty(area));
    assert!(viewport.begin_selection(area, Position::new(/*x*/ 2, /*y*/ 1),));
    assert!(viewport.update_selection(area, Position::new(/*x*/ 12, /*y*/ 2),));

    let mut highlighted = Buffer::empty(area);
    viewport.render(area, &mut highlighted);
    let text = buffer_text(&highlighted, area)
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    let mask = (area.y..area.bottom())
        .map(|y| {
            (area.x..area.right())
                .map(|x| {
                    if highlighted[(x, y)].modifier.contains(Modifier::REVERSED) {
                        '#'
                    } else {
                        '.'
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert_snapshot!(format!("text:\n{text}\nselection:\n{mask}"));
    assert_eq!(
        viewport.finish_selection(area, Position::new(/*x*/ 12, /*y*/ 2)),
        Some(source.to_string())
    );
}

fn buffer_text(buffer: &Buffer, area: Rect) -> String {
    let mut rows = Vec::new();
    for y in area.y..area.bottom() {
        let mut row = String::new();
        for x in area.x..area.right() {
            row.push_str(buffer[(x, y)].symbol());
        }
        rows.push(row);
    }
    rows.join("\n")
}
