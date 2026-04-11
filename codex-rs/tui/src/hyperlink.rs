use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

pub(crate) fn mark_underlined_hyperlink(buf: &mut Buffer, area: Rect, url: &str) {
    let safe_url = sanitize_osc8_url(url);
    if safe_url.is_empty() {
        return;
    }

    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            if !cell.modifier.contains(Modifier::UNDERLINED) {
                continue;
            }
            let symbol = cell.symbol().to_string();
            if symbol.trim().is_empty() {
                continue;
            }
            cell.set_symbol(&format!("\x1B]8;;{safe_url}\x07{symbol}\x1B]8;;\x07"));
        }
    }
}

fn sanitize_osc8_url(url: &str) -> String {
    url.chars()
        .filter(|&ch| ch != '\x1B' && ch != '\x07')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::style::Stylize;

    #[test]
    fn strips_osc8_control_characters_from_url() {
        assert_eq!(
            sanitize_osc8_url("https://example.com/\x1B]8;;\x07injected"),
            "https://example.com/]8;;injected"
        );
    }

    #[test]
    fn marks_only_underlined_cells() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
        buf[(0, 0)].set_symbol("A").set_style("".underlined().style);
        buf[(1, 0)].set_symbol("B");

        mark_underlined_hyperlink(&mut buf, Rect::new(0, 0, 2, 1), "https://example.com");

        assert!(
            buf[(0, 0)]
                .symbol()
                .contains("\x1B]8;;https://example.com\x07A")
        );
        assert_eq!(buf[(1, 0)].symbol(), "B");
    }
}
