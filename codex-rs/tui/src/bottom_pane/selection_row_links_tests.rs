use super::super::*;

#[test]
fn choice_labels_and_descriptions_keep_complete_url_destinations() {
    let label = "https://github.com/openai/codex/pull/12345?diff=split";
    let description = "https://example.com/releases/review/complete-destination";
    let mut snapshots = Vec::new();
    for prefix_label in [false, true] {
        for wrap_indent in [Some(0), None] {
            let mut row = GenericDisplayRow {
                name: format!("日本語 {label}"),
                description: Some(description.into()),
                wrap_indent,
                ..Default::default()
            };
            if prefix_label {
                row.name_prefix_spans = vec!["1. ".into(), std::mem::take(&mut row.name).bold()];
                row.description = None;
            }
            let rows = [row];
            for single_line in [false, true] {
                let area = Rect::new(0, 0, 48, if single_line { 1 } else { 12 });
                let mut buf = Buffer::empty(area);
                if single_line {
                    render_rows_single_line_with_col_width_mode(
                        area,
                        &mut buf,
                        &rows,
                        &ScrollState::new(),
                        /*max_results*/ 1,
                        "",
                        ColumnWidthConfig::default(),
                    );
                } else {
                    render_rows(
                        area,
                        &mut buf,
                        &rows,
                        &ScrollState::new(),
                        /*max_results*/ 1,
                        "",
                    );
                }
                let mut linked = 0;
                for cell in &buf.content {
                    if cell.symbol().contains("\x1b]8;;") {
                        linked += 1;
                        assert!(
                            [label, description].iter().any(|url| cell
                                .symbol()
                                .starts_with(&format!("\x1b]8;;{url}\x07")))
                        );
                        assert_ne!(crate::terminal_hyperlinks::strip_osc8(cell.symbol()), "…");
                    }
                }
                assert!(linked > 0);
                let urls = if prefix_label {
                    vec![label]
                } else {
                    vec![label, description]
                };
                for url in urls {
                    let visible = buf
                        .content
                        .iter()
                        .filter(|cell| cell.symbol().starts_with(&format!("\x1b]8;;{url}\x07")))
                        .map(|cell| crate::terminal_hyperlinks::strip_osc8(cell.symbol()))
                        .collect::<String>();
                    assert!(!visible.is_empty() && url.starts_with(&visible));
                }
                let visible = buf
                    .content
                    .chunks(48)
                    .map(|row| {
                        row.iter()
                            .map(|cell| crate::terminal_hyperlinks::strip_osc8(cell.symbol()))
                            .collect::<String>()
                            .trim_end()
                            .to_owned()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                snapshots.push(format!(
                "prefix label: {prefix_label}, wrapped label: {}, single line: {single_line}\n{visible}",
                wrap_indent.is_some()
            ));
            }
        }
    }
    insta::assert_snapshot!("choice_wrapped_urls", snapshots.join("\n\n"));
}
