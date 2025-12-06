use ratatui_core::style::Color;

use crate::canvas::{Line, Painter, Shape};

/// A rectangle to draw on a [`Canvas`](crate::canvas::Canvas)
///
/// Sizes used here are **not** in terminal cell. This is much more similar to the
/// mathematic coordinate system.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Rectangle {
    /// The `x` position of the rectangle.
    ///
    /// The rectangle is positioned from its bottom left corner.
    pub x: f64,
    /// The `y` position of the rectangle.
    ///
    /// The rectangle is positioned from its bottom left corner.
    pub y: f64,
    /// The width of the rectangle.
    pub width: f64,
    /// The height of the rectangle.
    pub height: f64,
    /// The color of the rectangle.
    pub color: Color,
}

impl Rectangle {
    /// Create a new rectangle with the given position, size, and color
    pub const fn new(x: f64, y: f64, width: f64, height: f64, color: Color) -> Self {
        Self {
            x,
            y,
            width,
            height,
            color,
        }
    }
}

impl Shape for Rectangle {
    fn draw(&self, painter: &mut Painter) {
        let lines: [Line; 4] = [
            Line {
                x1: self.x,
                y1: self.y,
                x2: self.x,
                y2: self.y + self.height,
                color: self.color,
            },
            Line {
                x1: self.x,
                y1: self.y + self.height,
                x2: self.x + self.width,
                y2: self.y + self.height,
                color: self.color,
            },
            Line {
                x1: self.x + self.width,
                y1: self.y,
                x2: self.x + self.width,
                y2: self.y + self.height,
                color: self.color,
            },
            Line {
                x1: self.x,
                y1: self.y,
                x2: self.x + self.width,
                y2: self.y,
                color: self.color,
            },
        ];
        for line in &lines {
            line.draw(painter);
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::{Margin, Rect};
    use ratatui_core::style::Style;
    use ratatui_core::symbols::Marker;
    use ratatui_core::widgets::Widget;

    use super::*;
    use crate::canvas::Canvas;

    #[test]
    fn draw_block_lines() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 10));
        let canvas = Canvas::default()
            .marker(Marker::Block)
            .x_bounds([0.0, 10.0])
            .y_bounds([0.0, 10.0])
            .paint(|context| {
                context.draw(&Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                    color: Color::Red,
                });
            });
        canvas.render(buffer.area, &mut buffer);
        let mut expected = Buffer::with_lines([
            "██████████",
            "█        █",
            "█        █",
            "█        █",
            "█        █",
            "█        █",
            "█        █",
            "█        █",
            "█        █",
            "██████████",
        ]);
        expected.set_style(buffer.area, Style::new().red().on_red());
        expected.set_style(buffer.area.inner(Margin::new(1, 1)), Style::reset());
        assert_eq!(buffer, expected);
    }

    #[test]
    fn draw_half_block_lines() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 10));
        let canvas = Canvas::default()
            .marker(Marker::HalfBlock)
            .x_bounds([0.0, 10.0])
            .y_bounds([0.0, 10.0])
            .paint(|context| {
                context.draw(&Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                    color: Color::Red,
                });
            });
        canvas.render(buffer.area, &mut buffer);
        let mut expected = Buffer::with_lines([
            "█▀▀▀▀▀▀▀▀█",
            "█        █",
            "█        █",
            "█        █",
            "█        █",
            "█        █",
            "█        █",
            "█        █",
            "█        █",
            "█▄▄▄▄▄▄▄▄█",
        ]);
        expected.set_style(buffer.area, Style::new().red().on_red());
        expected.set_style(buffer.area.inner(Margin::new(1, 0)), Style::reset().red());
        expected.set_style(buffer.area.inner(Margin::new(1, 1)), Style::reset());
        assert_eq!(buffer, expected);
    }

    #[test]
    fn draw_braille_lines() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 10));
        let canvas = Canvas::default()
            .marker(Marker::Braille)
            .x_bounds([0.0, 20.0])
            .y_bounds([0.0, 20.0])
            .paint(|context| {
                // a rectangle that will draw the outside part of the braille
                context.draw(&Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: 20.0,
                    height: 20.0,
                    color: Color::Red,
                });
                // a rectangle that will draw the inside part of the braille
                context.draw(&Rectangle {
                    x: 4.0,
                    y: 4.0,
                    width: 12.0,
                    height: 12.0,
                    color: Color::Green,
                });
            });
        canvas.render(buffer.area, &mut buffer);
        let mut expected = Buffer::with_lines([
            "⡏⠉⠉⠉⠉⠉⠉⠉⠉⢹",
            "⡇        ⢸",
            "⡇ ⡏⠉⠉⠉⠉⢹ ⢸",
            "⡇ ⡇    ⢸ ⢸",
            "⡇ ⡇    ⢸ ⢸",
            "⡇ ⡇    ⢸ ⢸",
            "⡇ ⡇    ⢸ ⢸",
            "⡇ ⣇⣀⣀⣀⣀⣸ ⢸",
            "⡇        ⢸",
            "⣇⣀⣀⣀⣀⣀⣀⣀⣀⣸",
        ]);
        expected.set_style(buffer.area, Style::new().red());
        expected.set_style(buffer.area.inner(Margin::new(1, 1)), Style::reset());
        expected.set_style(buffer.area.inner(Margin::new(2, 2)), Style::new().green());
        expected.set_style(buffer.area.inner(Margin::new(3, 3)), Style::reset());
        assert_eq!(buffer, expected);
    }
}
