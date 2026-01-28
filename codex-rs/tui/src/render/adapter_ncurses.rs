use std::collections::HashMap;

use ncurses::*;

use crate::render::model::RenderColor;
use crate::render::model::RenderLine;
use crate::render::model::RenderStyle;

pub struct NcursesRenderer {
    next_pair: i16,
    pairs: HashMap<(Option<RenderColor>, Option<RenderColor>), i16>,
}

impl NcursesRenderer {
    pub fn new() -> Self {
        if has_colors() {
            start_color();
            use_default_colors();
        }
        Self {
            next_pair: 1,
            pairs: HashMap::new(),
        }
    }

    pub fn render_line(&mut self, window: WINDOW, line: &RenderLine, y: i32, x: i32, width: i32) {
        let mut cursor_x = x;
        let max_x = x.saturating_add(width);
        if let Some(alignment) = line.alignment {
            let line_width = line.width() as i32;
            if line_width < width {
                let shift = match alignment {
                    crate::render::model::RenderAlignment::Left => 0,
                    crate::render::model::RenderAlignment::Center => (width - line_width) / 2,
                    crate::render::model::RenderAlignment::Right => width - line_width,
                };
                cursor_x = cursor_x.saturating_add(shift);
            }
        }
        for cell in &line.spans {
            if cursor_x >= max_x {
                break;
            }
            let remaining = (max_x - cursor_x) as usize;
            let text = truncate_to_width(&cell.content, remaining);
            if text.is_empty() {
                continue;
            }
            let attr = self.style_attr(cell.style);
            wattron(window, attr);
            let _ = mvwaddnstr(window, y, cursor_x, &text, text.len() as i32);
            wattroff(window, attr);
            cursor_x += unicode_width::UnicodeWidthStr::width(text.as_str()) as i32;
        }
    }

    fn style_attr(&mut self, style: RenderStyle) -> attr_t {
        let mut attr: attr_t = 0;
        if style.bold {
            attr |= A_BOLD;
        }
        if style.dim || style.fg == Some(RenderColor::DarkGray) {
            attr |= A_DIM;
        }
        if style.underline {
            attr |= A_UNDERLINE;
        }
        if has_colors() {
            let pair = self.pair_for(style.fg, style.bg);
            if pair > 0 {
                attr |= COLOR_PAIR(pair);
            }
        }
        attr
    }

    fn pair_for(&mut self, fg: Option<RenderColor>, bg: Option<RenderColor>) -> i16 {
        let key = (fg, bg);
        if let Some(pair) = self.pairs.get(&key) {
            return *pair;
        }
        let max_pairs = COLOR_PAIRS();
        if i32::from(self.next_pair) >= max_pairs {
            return 0;
        }
        let pair = self.next_pair;
        self.next_pair += 1;
        let fg_color = fg.and_then(to_ncurses_color).unwrap_or(-1);
        let bg_color = bg.and_then(to_ncurses_color).unwrap_or(-1);
        init_pair(pair, fg_color, bg_color);
        self.pairs.insert(key, pair);
        pair
    }
}

fn to_ncurses_color(color: RenderColor) -> Option<i16> {
    let mapped = match color {
        RenderColor::Default => -1,
        RenderColor::Red => COLOR_RED,
        RenderColor::Green => COLOR_GREEN,
        RenderColor::Yellow => COLOR_YELLOW,
        RenderColor::Blue => COLOR_BLUE,
        RenderColor::Magenta => COLOR_MAGENTA,
        RenderColor::Cyan => COLOR_CYAN,
        RenderColor::LightBlue => COLOR_BLUE,
        RenderColor::DarkGray => COLOR_WHITE,
        RenderColor::Rgb(_, _, _) => -1,
        RenderColor::Indexed(_) => -1,
    };
    Some(mapped)
}

fn truncate_to_width(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut width = 0usize;
    for ch in text.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > max_width {
            break;
        }
        out.push(ch);
        width += ch_width;
    }
    out
}
