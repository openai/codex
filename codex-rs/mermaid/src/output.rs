//! Semantic spans for theme-independent diagram output; drawing cells never contain ANSI escapes.

/// The diagram element a caller can style with its own theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Node,
    Edge,
    Text,
}

/// Adjacent characters with one semantic role, independent of terminal styling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub role: Role,
}

#[derive(Clone, Copy)]
pub(super) struct Cell {
    pub symbol: char,
    pub role: Role,
}

impl Cell {
    pub(super) fn edge(symbol: char) -> Self {
        Self {
            symbol,
            role: Role::Edge,
        }
    }

    pub(super) fn node(symbol: char) -> Self {
        Self {
            symbol,
            role: Role::Node,
        }
    }
}

pub(super) fn finish(rows: Vec<Vec<Cell>>) -> Vec<Vec<Span>> {
    rows.into_iter()
        .map(|row| {
            let mut spans: Vec<Span> = Vec::new();
            for Cell { symbol, role } in row {
                if symbol == '\0' {
                    continue;
                }
                if let Some(last) = spans.last_mut()
                    && last.role == role
                {
                    last.text.push(symbol);
                } else {
                    spans.push(Span {
                        text: symbol.to_string(),
                        role,
                    });
                }
            }
            while let Some(last) = spans.last_mut() {
                last.text.truncate(last.text.trim_end().len());
                if !last.text.is_empty() {
                    break;
                }
                spans.pop();
            }
            spans
        })
        .collect()
}
