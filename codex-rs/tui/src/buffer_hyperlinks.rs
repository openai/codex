use crate::osc8::osc8_hyperlink;
use ratatui::buffer::Buffer;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use unicode_width::UnicodeWidthStr;

#[derive(Clone)]
struct CellSnapshot {
    position: Position,
    symbol: String,
    fg: Color,
    bg: Color,
    modifier: Modifier,
    skip: bool,
}

pub(crate) fn mark_markdown_links(buf: &mut Buffer, area: Rect) {
    let cells = visible_cells(buf, area);

    let mut start = 0;
    while start < cells.len() {
        let Some(link) = MarkdownLinkCells::parse(&cells, start) else {
            start += 1;
            continue;
        };

        for cell in link.label.iter().chain(link.destination_cells.iter()) {
            let cell = &mut buf[(cell.position.x, cell.position.y)];
            cell.set_symbol(&osc8_hyperlink(&link.destination, cell.symbol()));
        }
        start = link.end;
    }
}

fn visible_cells(buf: &Buffer, area: Rect) -> Vec<CellSnapshot> {
    let mut cells = Vec::new();
    for y in area.top()..area.bottom() {
        let row = (area.left()..area.right())
            .map(|x| {
                let cell = &buf[(x, y)];
                CellSnapshot {
                    position: Position::new(x, y),
                    symbol: cell.symbol().to_string(),
                    fg: cell.fg,
                    bg: cell.bg,
                    modifier: cell.modifier,
                    skip: cell.skip,
                }
            })
            .collect::<Vec<_>>();
        let row_content_end = row
            .iter()
            .rposition(|cell| !is_plain_blank_cell(cell))
            .map(|index| index + 1)
            .unwrap_or(0);
        cells.extend(
            row.into_iter()
                .take(row_content_end)
                .filter(|cell| !cell.skip),
        );
    }
    cells
}

struct MarkdownLinkCells<'a> {
    label: &'a [CellSnapshot],
    destination_cells: &'a [CellSnapshot],
    destination: String,
    end: usize,
}

impl<'a> MarkdownLinkCells<'a> {
    fn parse(cells: &'a [CellSnapshot], start: usize) -> Option<Self> {
        if !is_link_label_start(cells.get(start)?) {
            return None;
        }

        let mut separator = start;
        while separator < cells.len() && is_link_label_continue(&cells[separator]) {
            separator += 1;
        }
        let destination_start = match (symbol_at(cells, separator), symbol_at(cells, separator + 1))
        {
            (Some(" "), Some("(")) => separator + 2,
            (Some("("), _) => separator + 1,
            _ => return None,
        };
        let mut close = destination_start;
        while close < cells.len() && is_link_destination_cell(&cells[close]) {
            close += 1;
        }
        if close == destination_start || symbol_at(cells, close)? != ")" {
            return None;
        }

        let destination = cells[destination_start..close]
            .iter()
            .map(|cell| cell.symbol.as_str())
            .collect::<String>();
        if !is_remote_url(&destination) {
            return None;
        }

        Some(Self {
            label: &cells[start..separator],
            destination_cells: &cells[destination_start..close],
            destination,
            end: close + 1,
        })
    }
}

fn is_link_label_start(cell: &CellSnapshot) -> bool {
    is_link_label_continue(cell) && !cell.symbol.trim().is_empty()
}

fn is_link_label_continue(cell: &CellSnapshot) -> bool {
    is_underlined_cell(cell) && cell.fg == Color::Cyan && cell.symbol.width() > 0
}

fn is_underlined_cell(cell: &CellSnapshot) -> bool {
    cell.modifier.contains(Modifier::UNDERLINED)
}

fn is_link_destination_cell(cell: &CellSnapshot) -> bool {
    is_link_label_start(cell)
}

fn is_remote_url(text: &str) -> bool {
    text.starts_with("http://") || text.starts_with("https://")
}

fn symbol_at(cells: &[CellSnapshot], index: usize) -> Option<&str> {
    cells.get(index).map(|cell| cell.symbol.as_str())
}

fn is_plain_blank_cell(cell: &CellSnapshot) -> bool {
    cell.symbol == " "
        && cell.fg == Color::Reset
        && cell.bg == Color::Reset
        && cell.modifier.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown_render::render_markdown_text;
    use crate::osc8::parse_osc8_hyperlink;
    use pretty_assertions::assert_eq;
    use ratatui::style::Style;
    use ratatui::style::Stylize;
    use ratatui::widgets::Paragraph;
    use ratatui::widgets::Widget;

    #[test]
    fn marks_rendered_remote_markdown_link_as_osc8_label_and_destination() {
        let area = Rect::new(0, 0, 80, 2);
        let mut buf = Buffer::empty(area);
        Paragraph::new(render_markdown_text(
            "[OpenAI Platform](https://openai.com)",
        ))
        .render(area, &mut buf);

        mark_markdown_links(&mut buf, area);

        assert_eq!(
            collect_osc8_text(&buf, area, "https://openai.com"),
            "OpenAI Platformhttps://openai.com"
        );
    }

    #[test]
    fn marks_remote_markdown_link_when_separator_wraps_to_next_row() {
        let area = Rect::new(0, 0, 14, 4);
        let mut buf = Buffer::empty(area);
        Paragraph::new(render_markdown_text(
            "[OpenAI Platform](https://openai.com)",
        ))
        .wrap(ratatui::widgets::Wrap { trim: false })
        .render(area, &mut buf);

        mark_markdown_links(&mut buf, area);

        assert_eq!(
            collect_osc8_text(&buf, area, "https://openai.com"),
            "OpenAIPlatformhttps://openai.com"
        );
    }

    #[test]
    fn marks_rendered_markdown_link_inside_blockquote() {
        let area = Rect::new(0, 0, 80, 3);
        let mut buf = Buffer::empty(area);
        Paragraph::new(render_markdown_text("> [OpenAI](https://openai.com)"))
            .wrap(ratatui::widgets::Wrap { trim: false })
            .render(area, &mut buf);

        mark_markdown_links(&mut buf, area);

        assert_eq!(
            collect_osc8_text(&buf, area, "https://openai.com"),
            "OpenAIhttps://openai.com"
        );
    }

    #[test]
    fn ignores_blockquote_style_without_markdown_link_destination() {
        let area = Rect::new(0, 0, 80, 3);
        let mut buf = Buffer::empty(area);
        Paragraph::new(render_markdown_text("> underlined but not a link"))
            .wrap(ratatui::widgets::Wrap { trim: false })
            .render(area, &mut buf);

        mark_markdown_links(&mut buf, area);

        assert_eq!(collect_osc8_text(&buf, area, "https://openai.com"), "");
    }

    #[test]
    fn marks_label_owner_cells_but_preserves_wide_glyph_skip_cell() {
        let area = Rect::new(0, 0, 80, 2);
        let mut buf = Buffer::empty(area);
        let link_style = Style::new().cyan().underlined();
        buf[(0, 0)].set_symbol("資").set_style(link_style);
        buf[(1, 0)].set_symbol(" ").set_skip(true);
        buf[(2, 0)].set_symbol("料").set_style(link_style);
        buf[(4, 0)].set_symbol("(");
        "https://example.com"
            .chars()
            .enumerate()
            .for_each(|(index, ch)| {
                buf[(5 + index as u16, 0)]
                    .set_symbol(ch.encode_utf8(&mut [0; 4]))
                    .set_style(link_style);
            });
        buf[(24, 0)].set_symbol(")");

        let skip_position = Position::new(1, 0);
        assert!(buf[(skip_position.x, skip_position.y)].skip);

        mark_markdown_links(&mut buf, area);

        assert!(
            parse_osc8_hyperlink(buf[(0, 0)].symbol()).is_some(),
            "wide glyph owner cell should be OSC-8 wrapped"
        );
        assert!(
            parse_osc8_hyperlink(buf[(2, 0)].symbol()).is_some(),
            "wide glyph second owner cell should be OSC-8 wrapped"
        );
        assert!(
            buf[(skip_position.x, skip_position.y)].skip,
            "wide-glyph continuation cell must stay skipped"
        );
    }

    fn collect_osc8_text(buf: &Buffer, area: Rect, destination: &str) -> String {
        let mut text = String::new();
        for position in area.positions() {
            let symbol = buf[(position.x, position.y)].symbol();
            if let Some(parsed) = parse_osc8_hyperlink(symbol)
                && parsed.destination == destination
            {
                text.push_str(parsed.text);
            }
        }
        text
    }
}
