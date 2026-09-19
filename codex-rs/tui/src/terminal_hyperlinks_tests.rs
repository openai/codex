use super::*;
use pretty_assertions::assert_eq;

#[test]
fn oversized_destinations_remain_plain_text() {
    let prefix = "https://example.com/";
    let destination = format!(
        "{prefix}{}",
        "a".repeat(MAX_HYPERLINK_DESTINATION_BYTES - prefix.len())
    );
    assert_eq!(web_destination(&destination), Some(destination.clone()));
    let oversized = format!("{destination}é");
    assert_eq!(web_destination(&oversized), None);
    assert_eq!(osc8_hyperlink(&oversized, "visible"), "visible");

    // Explicit Markdown links can reach the cell marker without bare-URL detection.
    let line = HyperlinkLine {
        source: None,
        line: Line::from("visible"),
        hyperlinks: vec![TerminalHyperlink::web(/*columns*/ 0..7, oversized)],
    };
    let mut buf = Buffer::with_lines(["visible"]);
    let expected = buf.clone();
    let area = buf.area;
    mark_buffer_hyperlinks(&mut buf, area, &[line], /*scroll_rows*/ 0);
    assert_eq!(buf, expected);
}

#[test]
fn discovers_many_web_urls_with_incremental_columns() {
    let destination = "https://x.io";
    let token = format!("👩‍💻 ({destination}).\u{2003}");
    let text = token.repeat(/*n*/ 50_000);
    let expected = (0..50_000)
        .map(|index| {
            let start = index * display_width(&token) + 4;
            TerminalHyperlink::web(start..start + destination.len(), destination.to_string())
        })
        .collect::<Vec<_>>();
    assert_eq!(web_links_in_text(&text), expected);
}

#[test]
fn trims_long_runs_of_unmatched_closing_delimiters() {
    let destination = "https://example.com/a_(b)[c]{d}<e>";
    let text = format!("{destination}{}", ")]}>".repeat(/*n*/ 50_000));
    assert_eq!(
        web_links_in_text(&text),
        vec![TerminalHyperlink::web(
            /*columns*/ 0..destination.len(),
            destination.to_string(),
        )]
    );
}

#[test]
fn decorated_hyperlinks_preserve_joined_emoji_columns() {
    let destination = "https://example.test/👩‍💻/path";
    let line = annotate_web_urls_in_line(Line::from(format!("👩‍💻 {destination} tail")));
    assert_eq!(
        decorate_spans(&line),
        vec![Span::from(format!(
            "👩‍💻 {} tail",
            osc8_hyperlink(destination, destination)
        ))]
    );
}
