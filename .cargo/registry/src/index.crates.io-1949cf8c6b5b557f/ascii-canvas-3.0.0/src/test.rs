use crate::style::Style;
use crate::test_util::expect_debug;
use crate::{AsciiCanvas, AsciiView};

#[test]
fn draw_box() {
    let mut canvas = AsciiCanvas::new(5, 10);
    {
        let view: &mut dyn AsciiView = &mut canvas;
        view.draw_vertical_line(2..5, 2);
        view.draw_vertical_line(2..5, 7);
        view.draw_horizontal_line(2, 2..8);
        view.draw_horizontal_line(4, 2..8);
    }
    expect_debug(
        &canvas.to_strings(),
        r#"
[
    "",
    "",
    "  ┌────┐",
    "  │    │",
    "  └────┘",
]
"#
        .trim(),
    );
}

#[test]
fn grow_box() {
    let mut canvas = AsciiCanvas::new(0, 10);
    {
        let view: &mut dyn AsciiView = &mut canvas;
        view.draw_vertical_line(2..5, 2);
        view.draw_vertical_line(2..5, 7);
        view.draw_horizontal_line(2, 2..8);
        view.draw_horizontal_line(4, 2..8);
    }
    expect_debug(
        &canvas.to_strings(),
        r#"
[
    "",
    "",
    "  ┌────┐",
    "  │    │",
    "  └────┘",
]
"#
        .trim(),
    );
}

#[test]
fn shift() {
    let mut canvas = AsciiCanvas::new(0, 10);
    {
        let canvas: &mut dyn AsciiView = &mut canvas;
        let view: &mut dyn AsciiView = &mut canvas.shift(1, 2);
        view.draw_vertical_line(2..5, 2);
        view.draw_vertical_line(2..5, 7);
        view.draw_horizontal_line(2, 2..8);
        view.draw_horizontal_line(4, 2..8);
        view.write_chars(3, 3, "Hi!".chars(), Style::new());
    }
    expect_debug(
        &canvas.to_strings(),
        r#"
[
    "",
    "",
    "",
    "    ┌────┐",
    "    │Hi! │",
    "    └────┘",
]
"#
        .trim(),
    );
}
