#![cfg(test)]
//! Regression coverage for shaded spaces across terminal redraws.

use super::*;
use crate::terminal_palette::indexed_color;

#[test]
fn shaded_spaces_survive_narrowing_a_shaded_row() {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 1,
    );
    let mut terminal = Terminal::with_options(VT100Backend::new(area.width, area.height)).unwrap();
    terminal.set_viewport_area(area);

    // Restoring the sidebar moves the message up onto its full-width top padding.
    for (width, text) in [(area.width, ""), (28, "Show the two demo files.")] {
        terminal
            .draw(|frame| {
                Paragraph::new(text)
                    .style(Style::default().bg(indexed_color(/*index*/ 238)))
                    .render(Rect { width, ..area }, frame.buffer_mut());
            })
            .unwrap();
    }

    // Erased cells have empty contents in vt100, while printed spaces retain " ".
    let screen = terminal.backend().vt100().screen();
    let shaded = (0..area.width)
        .map(|x| screen.cell(/*row*/ 0, x).unwrap())
        .filter(|cell| cell.bgcolor() == vt100::Color::Idx(238))
        .map(vt100::Cell::contents)
        .collect::<String>();
    assert_snapshot!(format!("[{shaded}]"), @"[Show the two demo files.    ]");
}
