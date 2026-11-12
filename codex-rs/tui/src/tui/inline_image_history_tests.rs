//! Inline previews must be erased before their rows can enter terminal history.
#![cfg(test)]

use super::PendingHistoryLines;
use super::ScrollbackStrategy;
use super::Tui;
use crate::custom_terminal::Terminal;
use crate::insert_history::HistoryLineWrapPolicy;
use crate::pets::ImageProtocol;
use crate::terminal_hyperlinks::plain_hyperlink_lines;
use crate::transcript_image::ImagePlacement;
use crate::transcript_image::TranscriptImage;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use pretty_assertions::assert_eq;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::text::Line;
use ratatui::widgets::Widget;
use std::sync::Arc;

fn inline_terminal(viewport: Rect) -> Terminal<CrosstermBackend<Vec<u8>>> {
    let mut terminal = Terminal::with_screen_size_and_cursor_position_for_test(
        CrosstermBackend::new(Vec::new()),
        Size::new(/*width*/ 32, /*height*/ 8),
        Position::new(/*x*/ 0, /*y*/ 0),
    );
    terminal.set_viewport_area(viewport);
    terminal
}

fn preview_at(protocol: ImageProtocol, y: u16) -> ImagePlacement {
    ImagePlacement {
        image: Arc::new(TranscriptImage {
            protocol,
            columns: 2,
            rows: 1,
            pixels: image::RgbaImage::from_pixel(
                /*width*/ 16,
                /*height*/ 16,
                image::Rgba([0, 100, 200, 255]),
            ),
            cell_width: 8,
            cell_height: 16,
        }),
        area: Rect::new(/*x*/ 0, y, /*width*/ 2, /*height*/ 1),
        first_row: 0,
        file: None,
    }
}

#[test]
fn inline_history_erases_preview_before_scrolling_and_preserves_draft() {
    let viewport = Rect::new(
        /*x*/ 0, /*y*/ 4, /*width*/ 32, /*height*/ 4,
    );
    let mut terminal = inline_terminal(viewport);
    let size = terminal.last_known_screen_size;
    terminal
        .draw_with_size(size, &mut Vec::new(), |frame| {
            Line::from("unsent [Image #1]").render(
                Rect::new(
                    /*x*/ 0, /*y*/ 7, /*width*/ 32, /*height*/ 1,
                ),
                frame.buffer_mut(),
            );
        })
        .unwrap();
    // vt100 ignores Sixel. Model its out-of-band pixels with a marker written directly
    // to the terminal, without changing the renderer's text buffers.
    queue!(
        terminal.backend_mut(),
        MoveTo(/*x*/ 0, /*y*/ 4),
        Print("stale preview pixels")
    )
    .unwrap();
    let mut retained = vec![preview_at(ImageProtocol::Sixel, viewport.top())];
    // Unrelated frames must not evict an otherwise unchanged preview.
    Tui::flush_pending_history_lines(
        &mut terminal,
        &mut Vec::new(),
        &mut retained,
        ScrollbackStrategy::Standard,
        size,
    )
    .unwrap();
    assert_eq!(retained.len(), 1);
    let preview = retained[0].registration();
    let before_flush = terminal.backend().writer().len();
    let mut history = vec![PendingHistoryLines {
        lines: plain_hyperlink_lines(
            (1..=12)
                .map(|index| Line::from(format!("streamed response {index}")))
                .collect(),
        ),
        wrap_policy: HistoryLineWrapPolicy::PreWrap,
    }];
    Tui::flush_pending_history_lines(
        &mut terminal,
        &mut history,
        &mut retained,
        ScrollbackStrategy::Standard,
        size,
    )
    .unwrap();
    assert!(retained.is_empty());
    assert!(history.is_empty());
    let output = terminal.backend().writer();
    let flush = String::from_utf8_lossy(&output[before_flush..]);
    let erase = flush.find("\x1b[K").expect("erase preview row");
    let history_write = flush.find("\x1b[1;4r").expect("history scroll region");
    assert!(
        erase < history_write,
        "preview must be erased before history: {flush:?}"
    );
    let mut parser = vt100::Parser::new(
        /*rows*/ 8, /*cols*/ 32, /*scrollback_len*/ 32,
    );
    parser.process(output);
    let contents = parser.screen().contents();
    assert!(!contents.contains("stale preview"), "{contents}");
    assert!(contents.contains("streamed response"), "{contents}");
    assert!(contents.contains("unsent [Image #1]"), "{contents}");
    insta::assert_snapshot!(contents);
    terminal
        .draw_with_size(size, &mut retained, |frame| {
            Line::from("unsent [Image #1]").render(
                Rect::new(
                    /*x*/ 0, /*y*/ 7, /*width*/ 32, /*height*/ 1,
                ),
                frame.buffer_mut(),
            );
            frame.image_placements.push(preview);
        })
        .unwrap();
    assert_eq!(retained.len(), 1);
    assert!(String::from_utf8_lossy(terminal.backend().writer()).contains("\x1bP"));
}

/// Capture history output and the following frame separately from the initial Kitty upload.
fn kitty_history_output(
    protocol: ImageProtocol,
    viewport: Rect,
    scrollback: ScrollbackStrategy,
) -> (String, String) {
    let mut terminal = inline_terminal(viewport);
    let size = terminal.last_known_screen_size;
    let preview = preview_at(protocol, viewport.top());
    let draw = |frame: &mut crate::custom_terminal::Frame<'_>| {
        let mut placement = preview.registration();
        placement.area.y = frame.area().top();
        frame.image_placements.push(placement);
    };
    let mut retained = Vec::new();
    terminal.draw_with_size(size, &mut retained, draw).unwrap();
    let output = terminal.backend().writer();
    assert!(String::from_utf8_lossy(output).contains("_Ga=T,"));
    let before_flush = output.len();
    let mut history = vec![PendingHistoryLines {
        lines: plain_hyperlink_lines(vec![Line::from("streamed response")]),
        wrap_policy: HistoryLineWrapPolicy::PreWrap,
    }];
    Tui::flush_pending_history_lines(&mut terminal, &mut history, &mut retained, scrollback, size)
        .unwrap();
    let before_draw = terminal.backend().writer().len();
    terminal.draw_with_size(size, &mut retained, draw).unwrap();
    let output = terminal.backend().writer();
    (
        String::from_utf8_lossy(&output[before_flush..before_draw]).into_owned(),
        String::from_utf8_lossy(&output[before_draw..]).into_owned(),
    )
}

#[test]
fn direct_kitty_preview_survives_history_above_the_viewport() {
    let viewport = Rect::new(
        /*x*/ 0, /*y*/ 4, /*width*/ 32, /*height*/ 4,
    );
    let (flush, redraw) =
        kitty_history_output(ImageProtocol::Kitty, viewport, ScrollbackStrategy::Standard);
    assert!(flush.contains("streamed response"));
    assert!(!flush.contains("_G"), "{flush:?}");
    assert!(!redraw.contains("_G"), "{redraw:?}");
}

#[test]
fn iterm2_file_preview_is_deleted_before_history_and_uploaded_on_the_next_draw() {
    let viewport = Rect::new(
        /*x*/ 0, /*y*/ 4, /*width*/ 32, /*height*/ 4,
    );
    let (flush, redraw) = kitty_history_output(
        ImageProtocol::KittyLocalFile,
        viewport,
        ScrollbackStrategy::Standard,
    );
    let (before_history, _) = flush
        .split_once("\x1b[1;4r")
        .expect("history scroll region");
    let delete = crate::pets::kitty_delete_image(crate::transcript_image::FIRST_IMAGE_ID);
    assert!(before_history.contains(&delete), "{flush:?}");
    assert!(!flush.contains("_Ga=T,"), "{flush:?}");
    assert!(redraw.contains("_Ga=T,t=f,"), "{redraw:?}");
}

#[test]
fn direct_kitty_preview_is_repainted_when_history_can_move_the_viewport() {
    for (viewport, scrollback) in [
        (
            Rect::new(
                /*x*/ 0, /*y*/ 2, /*width*/ 32, /*height*/ 4,
            ),
            ScrollbackStrategy::Standard,
        ),
        (
            Rect::new(
                /*x*/ 0, /*y*/ 1, /*width*/ 32, /*height*/ 7,
            ),
            ScrollbackStrategy::Standard,
        ),
        (
            Rect::new(
                /*x*/ 0, /*y*/ 4, /*width*/ 32, /*height*/ 4,
            ),
            ScrollbackStrategy::FullScreen,
        ),
    ] {
        let (flush, redraw) = kitty_history_output(ImageProtocol::Kitty, viewport, scrollback);
        let (before_history, _) = flush.split_once("streamed response").expect("history line");
        assert!(
            before_history.contains("_Ga=d,"),
            "{viewport:?} {scrollback:?}: {flush:?}"
        );
        assert!(
            redraw.contains("_Ga=T,t=d,"),
            "{viewport:?} {scrollback:?}: {redraw:?}"
        );
    }
}
