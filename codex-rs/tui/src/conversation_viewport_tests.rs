use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;

use super::*;
use crate::history_cell::AgentMarkdownCell;

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
}

fn cell(display: &'static str) -> Arc<dyn HistoryCell> {
    Arc::new(TestCell {
        display,
        raw: display,
        transcript: display,
        is_stream_continuation: false,
    })
}

fn viewport(cells: Vec<Arc<dyn HistoryCell>>) -> ConversationViewport {
    ConversationViewport::new(
        cells,
        HistoryRenderMode::Rich,
        crate::keymap::RuntimeKeymap::defaults().pager,
    )
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
        |_| Some(vec![HyperlinkLine::from("live tail")]),
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
fn append_keeps_live_tail_after_committed_cells() {
    let mut viewport = viewport(Vec::new());
    viewport.sync_live_tail(
        /*width*/ 24,
        Some(ActiveCellRenderKey {
            revision: 1,
            is_stream_continuation: false,
            animation_tick: None,
        }),
        |_| Some(vec![HyperlinkLine::from("live tail")]),
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
fn replacing_cells_invalidates_and_respaces_the_live_tail() {
    let key = ActiveCellRenderKey {
        revision: 1,
        is_stream_continuation: false,
        animation_tick: None,
    };
    let mut viewport = viewport(Vec::new());
    viewport.sync_live_tail(/*width*/ 24, Some(key), |_| {
        Some(vec![HyperlinkLine::from("live tail")])
    });

    viewport.replace_cells(vec![cell("replacement")]);
    viewport.sync_live_tail(/*width*/ 24, Some(key), |_| {
        Some(vec![HyperlinkLine::from("live tail")])
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
        |width| Some(live.display_hyperlink_lines(width)),
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
