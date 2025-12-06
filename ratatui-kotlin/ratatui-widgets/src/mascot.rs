//! A Ratatui mascot widget
//!
//! The mascot takes 32x16 cells and is rendered using half block characters.
use itertools::Itertools;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::Color;
use ratatui_core::widgets::Widget; // tuples();

const RATATUI_MASCOT: &str = indoc::indoc! {"
                   hhh
                 hhhhhh
                hhhhhhh
               hhhhhhhh
              hhhhhhhhh
             hhhhhhhhhh
            hhhhhhhhhhhh
            hhhhhhhhhhhhh
            hhhhhhhhhhhhh     ██████
             hhhhhhhhhhh    ████████
                  hhhhh ███████████
                   hhh ██ee████████
                    h █████████████
                ████ █████████████
               █████████████████
               ████████████████
               ████████████████
                ███ ██████████
              ▒▒    █████████
             ▒░░▒   █████████
            ▒░░░░▒ ██████████
           ▒░░▓░░░▒ █████████
          ▒░░▓▓░░░░▒ ████████
         ▒░░░░░░░░░░▒ ██████████
        ▒░░░░░░░░░░░░▒ ██████████
       ▒░░░░░░░▓▓░░░░░▒ █████████
      ▒░░░░░░░░░▓▓░░░░░▒ ████  ███
     ▒░░░░░░░░░░░░░░░░░░▒ ██   ███
    ▒░░░░░░░░░░░░░░░░░░░░▒ █   ███
    ▒░░░░░░░░░░░░░░░░░░░░░▒   ███
     ▒░░░░░░░░░░░░░░░░░░░░░▒ ███
      ▒░░░░░░░░░░░░░░░░░░░░░▒ █"
};

const EMPTY: char = ' ';
const RAT: char = '█';
const HAT: char = 'h';
const EYE: char = 'e';
const TERM: char = '░';
const TERM_BORDER: char = '▒';
const TERM_CURSOR: char = '▓';

/// State for the mascot's eye
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MascotEyeColor {
    /// The default eye color
    #[default]
    Default,

    /// The red eye color
    Red,
}

/// A widget that renders the Ratatui mascot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RatatuiMascot {
    eye_state: MascotEyeColor,
    /// The color of the rat
    rat_color: Color,
    /// The color of the rat's eye
    rat_eye_color: Color,
    /// The color of the rat's eye when blinking
    rat_eye_blink: Color,
    /// The color of the rat's hat
    hat_color: Color,
    /// The color of the terminal
    term_color: Color,
    /// The color of the terminal border
    term_border_color: Color,
    /// The color of the terminal cursor
    term_cursor_color: Color,
}

impl Default for RatatuiMascot {
    fn default() -> Self {
        Self {
            rat_color: Color::Indexed(252),         // light_gray #d0d0d0
            hat_color: Color::Indexed(231),         // white #ffffff
            rat_eye_color: Color::Indexed(236),     // dark_charcoal #303030
            rat_eye_blink: Color::Indexed(196),     // red #ff0000
            term_color: Color::Indexed(232),        // vampire_black #080808
            term_border_color: Color::Indexed(237), // gray  #808080
            term_cursor_color: Color::Indexed(248), // dark_gray #a8a8a8
            eye_state: MascotEyeColor::Default,
        }
    }
}

impl RatatuiMascot {
    /// Create a new Ratatui mascot widget
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    /// Set the eye state (open / blinking)
    #[must_use]
    pub const fn set_eye(self, rat_eye: MascotEyeColor) -> Self {
        Self {
            eye_state: rat_eye,
            ..self
        }
    }

    const fn color_for(&self, c: char) -> Option<Color> {
        match c {
            RAT => Some(self.rat_color),
            HAT => Some(self.hat_color),
            EYE => Some(match self.eye_state {
                MascotEyeColor::Default => self.rat_eye_color,
                MascotEyeColor::Red => self.rat_eye_blink,
            }),
            TERM => Some(self.term_color),
            TERM_CURSOR => Some(self.term_cursor_color),
            TERM_BORDER => Some(self.term_border_color),
            _ => None,
        }
    }
}

impl Widget for RatatuiMascot {
    /// Use half block characters to render a logo based on the `RATATUI_LOGO` const.
    ///
    /// The logo colors are hardcorded in the widget.
    /// The eye color depends on whether it's open / blinking
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }

        for (y, (line1, line2)) in RATATUI_MASCOT.lines().tuples().enumerate() {
            for (x, (ch1, ch2)) in line1.chars().zip(line2.chars()).enumerate() {
                let x = area.left() + x as u16;
                let y = area.top() + y as u16;

                // Check if coordinates are within the buffer area
                if x >= area.right() || y >= area.bottom() {
                    continue;
                }

                let cell = &mut buf[(x, y)];
                // given two cells which make up the top and bottom of the character,
                // Foreground color should be the non-space, non-terminal
                let (fg, bg) = match (ch1, ch2) {
                    (EMPTY, EMPTY) => (None, None),
                    (c, EMPTY) | (EMPTY, c) => (self.color_for(c), None),
                    (TERM, TERM_BORDER) => (self.color_for(TERM_BORDER), self.color_for(TERM)),
                    (TERM, c) | (c, TERM) => (self.color_for(c), self.color_for(TERM)),
                    (c1, c2) => (self.color_for(c1), self.color_for(c2)),
                };
                // symbol should make the empty space or terminal bg as the empty part of the block
                let symbol = match (ch1, ch2) {
                    (EMPTY, EMPTY) => None,
                    (TERM, TERM) => Some(EMPTY),
                    (_, EMPTY | TERM) => Some('▀'),
                    (EMPTY | TERM, _) => Some('▄'),
                    (c, d) if c == d => Some('█'),
                    (_, _) => Some('▀'),
                };
                if let Some(fg) = fg {
                    cell.fg = fg;
                }
                if let Some(bg) = bg {
                    cell.bg = bg;
                }
                if let Some(symb) = symbol {
                    cell.set_char(symb);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use super::*;

    #[test]
    fn new_mascot() {
        let mascot = RatatuiMascot::new();
        assert_eq!(mascot.eye_state, MascotEyeColor::Default);
    }

    #[test]
    fn set_eye_color() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 32, 16));
        let mascot = RatatuiMascot::new().set_eye(MascotEyeColor::Red);
        mascot.render(buf.area, &mut buf);
        assert_eq!(mascot.eye_state, MascotEyeColor::Red);
        assert_eq!(buf[(21, 5)].bg, Color::Indexed(196));
    }

    #[test]
    fn render_mascot() {
        let mascot = RatatuiMascot::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 32, 16));
        mascot.render(buf.area, &mut buf);
        assert_eq!(buf.area.as_size(), (32, 16).into());
        assert_eq!(buf[(21, 5)].bg, Color::Indexed(236));
        assert_eq!(
            buf.content
                .iter()
                .map(ratatui_core::buffer::Cell::symbol)
                .collect::<String>(),
            Buffer::with_lines([
                "             ▄▄███              ",
                "           ▄███████             ",
                "         ▄█████████             ",
                "        ████████████            ",
                "        ▀███████████▀   ▄▄██████",
                "              ▀███▀▄█▀▀████████ ",
                "            ▄▄▄▄▀▄████████████  ",
                "           ████████████████     ",
                "           ▀███▀██████████      ",
                "         ▄▀▀▄   █████████       ",
                "       ▄▀ ▄  ▀▄▀█████████       ",
                "     ▄▀  ▀▀    ▀▄▀███████       ",
                "   ▄▀      ▄▄    ▀▄▀█████████   ",
                " ▄▀         ▀▀     ▀▄▀██▀  ███  ",
                "█                    ▀▄▀  ▄██   ",
                " ▀▄                    ▀▄▀█     ",
            ])
            .content
            .iter()
            .map(ratatui_core::buffer::Cell::symbol)
            .collect::<String>()
        );
    }

    #[test]
    fn render_in_minimal_buffer() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let mascot = RatatuiMascot::new();
        // This should not panic, even if the buffer is too small to render the mascot.
        mascot.render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines([" "]));
    }

    #[test]
    fn render_in_zero_size_buffer() {
        let mut buffer = Buffer::empty(Rect::ZERO);
        let mascot = RatatuiMascot::new();
        // This should not panic, even if the buffer has zero size.
        mascot.render(buffer.area, &mut buffer);
    }
}
