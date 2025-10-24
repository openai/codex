use codex_core::bash::try_parse_bash;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;

/// Convert the full bash script into per-line styled content by first
/// computing operator-dimmed spans across the entire script, then splitting
/// by newlines and dimming heredoc body lines. Performs a single parse and
/// reuses it for both highlighting and heredoc detection.
pub(crate) fn highlight_bash_to_lines(script: &str) -> Vec<Line<'static>> {
    // Parse once; use the tree for both highlighting and heredoc body detection.
    let spans: Vec<Span<'static>> = if let Some(tree) = try_parse_bash(script) {
        // Single walk: collect operator ranges and heredoc rows.
        let root = tree.root_node();
        let mut cursor = root.walk();
        let mut stack = vec![root];
        let mut ranges: Vec<(usize, usize)> = Vec::new();
        while let Some(node) = stack.pop() {
            if !node.is_named() && !node.is_extra() {
                let kind = node.kind();
                let is_quote = matches!(kind, "\"" | "'" | "`");
                let is_whitespace = kind.trim().is_empty();
                if !is_quote && !is_whitespace {
                    ranges.push((node.start_byte(), node.end_byte()));
                }
            } else if node.kind() == "heredoc_body" {
                ranges.push((node.start_byte(), node.end_byte()));
            }
            for child in node.children(&mut cursor) {
                stack.push(child);
            }
        }
        if ranges.is_empty() {
            ranges.push((script.len(), script.len()));
        }
        ranges.sort_by_key(|(st, _)| *st);
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut i = 0usize;
        for (start, end) in ranges.into_iter() {
            let dim_start = start.max(i);
            let dim_end = end;
            if dim_start < dim_end {
                if dim_start > i {
                    spans.push(script[i..dim_start].to_string().into());
                }
                spans.push(script[dim_start..dim_end].to_string().dim());
                i = dim_end;
            }
        }
        if i < script.len() {
            spans.push(script[i..].to_string().into());
        }
        spans
    } else {
        vec![script.to_string().into()]
    };
    // Split spans into lines preserving style boundaries and highlights across newlines.
    let mut lines: Vec<Line<'static>> = vec![Line::from("")];
    for sp in spans {
        let style = sp.style;
        let text = sp.content.into_owned();
        for (i, part) in text.split('\n').enumerate() {
            if i > 0 {
                lines.push(Line::from(""));
            }
            if part.is_empty() {
                continue;
            }
            let span = Span {
                style,
                content: std::borrow::Cow::Owned(part.to_string()),
            };
            if let Some(last) = lines.last_mut() {
                last.spans.push(span);
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::style::Modifier;

    #[test]
    fn dims_expected_bash_operators() {
        let s = "echo foo && bar || baz | qux & (echo hi)";
        let lines = highlight_bash_to_lines(s);
        let reconstructed: String = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|sp| sp.content.clone())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(reconstructed, s);

        fn is_dim(span: &Span<'_>) -> bool {
            span.style.add_modifier.contains(Modifier::DIM)
        }
        let dimmed: Vec<String> = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .filter(|sp| is_dim(sp))
            .map(|sp| sp.content.clone().into_owned())
            .collect();
        assert_eq!(dimmed, vec!["&&", "||", "|", "&", "(", ")"]);
    }

    #[test]
    fn does_not_dim_quotes_but_dims_other_punct() {
        let s = "echo \"hi\" > out.txt; echo 'ok'";
        let lines = highlight_bash_to_lines(s);
        let reconstructed: String = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|sp| sp.content.clone())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(reconstructed, s);

        fn is_dim(span: &Span<'_>) -> bool {
            span.style.add_modifier.contains(Modifier::DIM)
        }
        let dimmed: Vec<String> = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .filter(|sp| is_dim(sp))
            .map(|sp| sp.content.clone().into_owned())
            .collect();
        assert!(dimmed.contains(&">".to_string()));
        assert!(dimmed.contains(&";".to_string()));
        assert!(!dimmed.contains(&"\"".to_string()));
        assert!(!dimmed.contains(&"'".to_string()));
    }
}
