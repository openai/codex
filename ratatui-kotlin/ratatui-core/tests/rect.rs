//! Integration tests for Rect operations visualized with buffers.

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Margin, Offset, Rect};
use ratatui_core::widgets::Widget;

/// A minimal widget that fills its entire area with the given symbol.
struct Filled<'a> {
    symbol: &'a str,
}

impl Widget for Filled<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(self.symbol);
                }
            }
        }
    }
}

#[test]
fn inner() {
    let base = Rect::new(2, 2, 10, 6);
    let inner = base.inner(Margin::new(2, 1));

    let mut buf = Buffer::empty(Rect::new(0, 0, 15, 10));
    Filled { symbol: "█" }.render(base, &mut buf);
    Filled { symbol: "░" }.render(inner, &mut buf);

    let expected = Buffer::with_lines([
        "               ",
        "               ",
        "  ██████████   ",
        "  ██░░░░░░██   ",
        "  ██░░░░░░██   ",
        "  ██░░░░░░██   ",
        "  ██░░░░░░██   ",
        "  ██████████   ",
        "               ",
        "               ",
    ]);
    assert_eq!(buf, expected);
}

#[test]
fn outer() {
    let base = Rect::new(4, 3, 6, 4);
    let outer = base.outer(Margin::new(2, 1));

    let mut buf = Buffer::empty(Rect::new(0, 0, 15, 10));
    Filled { symbol: "░" }.render(outer, &mut buf);
    Filled { symbol: "█" }.render(base, &mut buf);

    let expected = Buffer::with_lines([
        "               ",
        "               ",
        "  ░░░░░░░░░░   ",
        "  ░░██████░░   ",
        "  ░░██████░░   ",
        "  ░░██████░░   ",
        "  ░░██████░░   ",
        "  ░░░░░░░░░░   ",
        "               ",
        "               ",
    ]);
    assert_eq!(buf, expected);
}

#[test]
fn offset() {
    let base = Rect::new(2, 2, 5, 3);
    let moved = base.offset(Offset { x: 4, y: 2 });

    let mut buf = Buffer::empty(Rect::new(0, 0, 15, 10));
    Filled { symbol: "░" }.render(base, &mut buf);
    Filled { symbol: "█" }.render(moved, &mut buf);

    let expected = Buffer::with_lines([
        "               ",
        "               ",
        "  ░░░░░        ",
        "  ░░░░░        ",
        "  ░░░░█████    ",
        "      █████    ",
        "      █████    ",
        "               ",
        "               ",
        "               ",
    ]);
    assert_eq!(buf, expected);
}

#[test]
fn intersection() {
    let a = Rect::new(2, 2, 6, 4);
    let b = Rect::new(5, 3, 6, 4);
    let inter = a.intersection(b);

    let mut buf = Buffer::empty(Rect::new(0, 0, 15, 10));
    Filled { symbol: "░" }.render(a, &mut buf);
    Filled { symbol: "▒" }.render(b, &mut buf);
    Filled { symbol: "█" }.render(inter, &mut buf);

    let expected = Buffer::with_lines([
        "               ",
        "               ",
        "  ░░░░░░       ",
        "  ░░░███▒▒▒    ",
        "  ░░░███▒▒▒    ",
        "  ░░░███▒▒▒    ",
        "     ▒▒▒▒▒▒    ",
        "               ",
        "               ",
        "               ",
    ]);
    assert_eq!(buf, expected);
}

#[test]
fn clamp() {
    let area = Rect::new(2, 2, 10, 6);
    let rect = Rect::new(8, 5, 8, 4);
    let clamped = rect.clamp(area);

    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 12));
    Filled { symbol: "█" }.render(area, &mut buf);
    Filled { symbol: "▒" }.render(rect, &mut buf);
    Filled { symbol: "░" }.render(clamped, &mut buf);

    let expected = Buffer::with_lines([
        "                    ",
        "                    ",
        "  ██████████        ",
        "  ██████████        ",
        "  ██░░░░░░░░        ",
        "  ██░░░░░░░░▒▒▒▒    ",
        "  ██░░░░░░░░▒▒▒▒    ",
        "  ██░░░░░░░░▒▒▒▒    ",
        "        ▒▒▒▒▒▒▒▒    ",
        "                    ",
        "                    ",
        "                    ",
    ]);
    assert_eq!(buf, expected);
}
