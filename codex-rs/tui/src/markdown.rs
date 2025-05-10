use ratatui::text::Line;
use ratatui::text::Span;

pub(crate) fn append_markdown(markdown_source: &str, lines: &mut Vec<Line<'static>>) {
    let markdown = tui_markdown::from_str(markdown_source);

    // `tui_markdown` returns a `ratatui::text::Text` where every `Line` borrows
    // from the input `message` string. Since the `HistoryCell` stores its lines
    // with a `'static` lifetime we must create an **owned** copy of each line
    // so that it is no longer tied to `message`. We do this by cloning the
    // content of every `Span` into an owned `String`.

    for borrowed_line in markdown.lines {
        let mut owned_spans = Vec::with_capacity(borrowed_line.spans.len());
        for span in &borrowed_line.spans {
            // Create a new owned String for the span's content to break the lifetime link.
            let owned_span = Span::styled(span.content.to_string(), span.style);
            owned_spans.push(owned_span);
        }

        let owned_line: Line<'static> = Line::from(owned_spans).style(borrowed_line.style);
        // Preserve alignment if it was set on the source line.
        let owned_line = match borrowed_line.alignment {
            Some(alignment) => owned_line.alignment(alignment),
            None => owned_line,
        };

        lines.push(owned_line);
    }
}
