use super::super::TextArea;
use super::super::TextAreaState;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ChatComposer;
use crate::render::renderable::Renderable;
use crate::terminal_hyperlinks::strip_osc8;
use crate::width::display_width;
use codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::widgets::StatefulWidgetRef;
use tokio::sync::mpsc::unbounded_channel;

type LinkedCell = (u16, u16, String, String);

fn linked_cells(buf: &Buffer) -> Vec<LinkedCell> {
    buf.area
        .positions()
        .filter_map(|position| {
            let symbol = buf[position].symbol();
            let (destination, _) = symbol.strip_prefix("\x1b]8;;")?.split_once('\x07')?;
            Some((
                position.x,
                position.y,
                strip_osc8(symbol),
                destination.to_string(),
            ))
        })
        .collect()
}

fn render(text: &str, width: u16, height: u16) -> (TextArea, TextAreaState, Buffer) {
    let mut textarea = TextArea::new();
    textarea.insert_str(text);
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
    let mut buf = Buffer::empty(area);
    let mut state = TextAreaState::default();
    StatefulWidgetRef::render_ref(&(&textarea), area, &mut buf, &mut state);
    (textarea, state, buf)
}

fn linked_text(cells: &[LinkedCell]) -> String {
    cells.iter().map(|(_, _, text, _)| text.as_str()).collect()
}

fn hyperlink_snapshot(buf: &Buffer) -> String {
    let mut snapshot = (0..buf.area.height)
        .map(|row| {
            let text = (0..buf.area.width)
                .map(|column| strip_osc8(buf[(column, row)].symbol()))
                .collect::<String>();
            format!("|{text}|\n")
        })
        .collect::<String>();
    let linked = linked_cells(buf);
    snapshot.push_str("Hyperlinks:\n");
    if linked.is_empty() {
        snapshot.push_str("  none\n");
    }
    for cells in linked.chunk_by(|left, right| {
        left.1 == right.1
            && left.3 == right.3
            && usize::from(left.0) + display_width(&left.2) == usize::from(right.0)
    }) {
        let (column, row, _, destination) = &cells[0];
        let last = cells.last().unwrap();
        let end = usize::from(last.0) + display_width(&last.2);
        snapshot.push_str(&format!(
            "  row {row}, columns {column}..{end} -> {destination}\n"
        ));
    }
    snapshot
}

fn assert_destination(buf: &Buffer, destination: &str) -> Vec<LinkedCell> {
    let linked = linked_cells(buf);
    assert!(!linked.is_empty());
    assert!(linked.iter().all(|(_, _, _, target)| target == destination));
    linked
}

#[test]
fn wrapped_url_fragments_keep_the_complete_destination() {
    let url = "https://github.com/openai/codex/pull/20252";
    let (_, _, buf) = render(
        &format!("Fix CI on {url}"),
        /*width*/ 26,
        /*height*/ 5,
    );
    let area = buf.area;
    let linked = assert_destination(&buf, url);
    assert_eq!(linked_text(&linked), url);
    assert!(linked.windows(2).any(|pair| pair[0].1 != pair[1].1));
    assert!(!buf[(0, 0)].symbol().contains("\x1b]8;;"));

    let visible = (0..area.height)
        .map(|row| {
            (0..area.width)
                .map(|column| strip_osc8(buf[(column, row)].symbol()))
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!("wrapped_textarea_url_keeps_original_appearance", visible);
}

#[test]
fn composer_wrapped_url_fragments_keep_the_complete_destination() {
    let url = "https://github.com/openai/codex/pull/20252";
    let (sender, _receiver) = unbounded_channel::<AppEvent>();
    let mut composer = ChatComposer::new(
        /*has_input_focus*/ true,
        AppEventSender::new(sender),
        /*enhanced_keys_supported*/ true,
        "Ask Codex to do anything".to_string(),
        /*disable_paste_burst*/ false,
    );
    composer.set_text_content(format!("Fix CI on {url}"), Vec::new(), Vec::new());
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 34, /*height*/ 8,
    );
    let mut buf = Buffer::empty(area);

    composer.render(area, &mut buf);

    let linked = assert_destination(&buf, url);
    assert_eq!(linked_text(&linked), url);
    assert!(linked.windows(2).any(|pair| pair[0].1 != pair[1].1));
}

#[test]
fn scrolled_url_fragments_keep_the_offscreen_destination() {
    let url = "https://example.test/alpha/beta/gamma/delta/epsilon";
    let (_, state, buf) = render(&format!("لا\n{url}"), /*width*/ 16, /*height*/ 2);
    let visible_url = linked_text(&assert_destination(&buf, url));
    assert!(state.scroll > 0);
    assert_ne!(visible_url, url);
    assert!(url.ends_with(&visible_url));
}

#[test]
fn long_drafts_reuse_hyperlink_detection_across_cursor_redraws() {
    let text = "x".repeat(MAX_USER_INPUT_TEXT_CHARS);
    let (mut textarea, mut state, mut buf) = render(&text, /*width*/ 80, /*height*/ 2);
    let area = buf.area;

    let first_cache = {
        let cache = textarea.wrap_cache.borrow();
        let hyperlinks = cache.as_ref().unwrap().hyperlinks.get().unwrap();
        assert!(hyperlinks.hyperlinks.is_empty());
        hyperlinks as *const super::HyperlinkCache
    };

    textarea.set_cursor(text.len() - 1);
    StatefulWidgetRef::render_ref(&(&textarea), area, &mut buf, &mut state);

    let cache = textarea.wrap_cache.borrow();
    let second_cache = cache.as_ref().unwrap().hyperlinks.get().unwrap();
    assert!(std::ptr::eq(first_cache, second_cache));
}

#[test]
fn maximum_length_urls_render_without_osc8_annotations() {
    let prefix = "https://example.test/";
    let text = format!(
        "{prefix}{}",
        "a".repeat(MAX_USER_INPUT_TEXT_CHARS - prefix.len())
    );
    let (textarea, _, buf) = render(&text, /*width*/ 80, /*height*/ 2);

    assert_eq!(textarea.text(), text);
    assert_eq!(linked_cells(&buf), Vec::new());
    assert!(buf.content.iter().all(|cell| cell.symbol().len() == 1));
    insta::assert_snapshot!(
        "maximum_length_url_without_annotations",
        hyperlink_snapshot(&buf)
    );
}

#[test]
fn many_urls_render_with_the_complete_destination() {
    let destination = "https://x.io";
    let text = format!("{destination} ").repeat(/*n*/ 50_000);
    let (textarea, state, buf) = render(&text, /*width*/ 80, /*height*/ 2);

    assert!(state.scroll > 0);
    assert_destination(&buf, destination);
    assert_eq!(
        textarea
            .wrap_cache
            .borrow()
            .as_ref()
            .unwrap()
            .hyperlinks
            .get()
            .unwrap()
            .hyperlinks
            .len(),
        50_000
    );
}

#[test]
fn joined_emoji_preserve_complete_url_cell_ranges() {
    let mut snapshots = Vec::new();
    for url in ["https://example.test/path", "https://example.test/👩‍💻/path"] {
        for width in [64, 24, 16] {
            let (_, _, buf) = render(&format!("👩‍💻 {url} tail"), width, /*height*/ 6);
            let linked = assert_destination(&buf, url);
            assert_eq!(linked_text(&linked), url);
            for row in 0..buf.area.height {
                let cells = linked
                    .iter()
                    .filter(|(_, y, _, _)| *y == row)
                    .cloned()
                    .collect::<Vec<_>>();
                if let (Some(first), Some(last)) = (cells.first(), cells.last()) {
                    snapshots.push(format!(
                        "width {width}, row {row}, columns {}..={}: {}",
                        first.0,
                        last.0,
                        linked_text(&cells)
                    ));
                }
            }
        }
    }
    insta::assert_snapshot!("joined_emoji_url_cell_ranges", snapshots.join("\n"));
}

#[test]
fn unicode_whitespace_separates_url_destinations() {
    let first = "https://one.test/a";
    let second = "https://two.test/b";
    let mut snapshots = Vec::new();
    for separator in ['\u{2003}', '\u{a0}', '\u{2007}', '\u{202f}', '\u{2028}'] {
        for width in [48, 20] {
            let (_, _, buf) = render(
                &format!("{first}{separator}{second}"),
                width,
                /*height*/ 4,
            );
            let linked = linked_cells(&buf);
            for destination in [first, second] {
                let cells = linked
                    .iter()
                    .filter(|(_, _, _, target)| target == destination)
                    .cloned()
                    .collect::<Vec<_>>();
                assert_eq!(linked_text(&cells), destination);
            }
            assert_eq!(linked.len(), first.len() + second.len());
            snapshots.push(format!(
                "separator U+{:04X}, width {width}\n{}",
                separator as u32,
                hyperlink_snapshot(&buf)
            ));
        }
    }
    insta::assert_snapshot!("unicode_whitespace_url_destinations", snapshots.join("\n"));
}

#[test]
fn hyperlink_cache_is_invalidated_when_text_changes() {
    let first = "https://one.test/before";
    let second = "https://two.test/after";
    let (mut textarea, mut state, mut buf) = render(first, /*width*/ 40, /*height*/ 1);
    let area = buf.area;

    textarea.set_text_clearing_elements(second);
    assert!(textarea.wrap_cache.borrow().is_none());

    buf = Buffer::empty(area);
    StatefulWidgetRef::render_ref(&(&textarea), area, &mut buf, &mut state);
    assert_eq!(linked_text(&assert_destination(&buf, second)), second);
}

#[test]
fn distinct_urls_respect_punctuation_wide_prefixes_and_tabs() {
    let first = "https://one.test/a";
    let second = "https://two.test/b";
    let (_, _, buf) = render(
        &format!("界\t({first}),\n{second}!"),
        /*width*/ 48,
        /*height*/ 2,
    );
    let linked = linked_cells(&buf);
    for (row, column, destination) in [(0, 4, first), (1, 0, second)] {
        let cells = linked
            .iter()
            .filter(|(_, cell_row, _, _)| *cell_row == row)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(cells.first().map(|cell| cell.0), Some(column));
        assert!(cells.iter().all(|cell| cell.3 == destination));
        assert_eq!(linked_text(&cells), destination);
    }
}

#[test]
fn url_hyperlinks_preserve_existing_highlight_styles() {
    let url = "https://example.test/highlight";
    let mut textarea = TextArea::new();
    textarea.insert_str(url);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 1,
    );
    let mut buf = Buffer::empty(area);
    let mut state = TextAreaState::default();
    let highlight = Style::default().fg(Color::Magenta);

    textarea.render_ref_styled_with_highlights(
        area,
        &mut buf,
        &mut state,
        Style::default(),
        &[(0..url.len(), highlight)],
    );

    let linked = assert_destination(&buf, url);
    assert_eq!(linked.len(), url.len());
    assert!(
        linked
            .iter()
            .all(|(column, row, _, _)| { buf[(*column, *row)].fg == Color::Magenta })
    );
}

#[test]
fn masked_url_input_never_exposes_hyperlink_destinations() {
    let url = "https://example.test/secret";
    let mut textarea = TextArea::new();
    textarea.insert_str(url);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 1,
    );
    let mut buf = Buffer::empty(area);
    let mut state = TextAreaState::default();

    textarea.render_ref_masked(area, &mut buf, &mut state, '*');

    assert_eq!(linked_cells(&buf), Vec::new());
    assert!(
        area.positions()
            .all(|position| !buf[position].symbol().contains(url))
    );
}
