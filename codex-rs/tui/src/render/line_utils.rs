use crate::render::model::RenderCell;
use crate::render::model::RenderLine;

/// Clone a borrowed render line into an owned line.
pub fn line_to_static(line: &RenderLine) -> RenderLine {
    line.clone()
}

/// Append owned copies of borrowed lines to `out`.
pub fn push_owned_lines(src: &[RenderLine], out: &mut Vec<RenderLine>) {
    for l in src {
        out.push(line_to_static(l));
    }
}

/// Consider a line blank if it has no spans or only spans whose contents are
/// empty or consist solely of spaces (no tabs/newlines).
pub fn is_blank_line_spaces_only(line: &RenderLine) -> bool {
    if line.spans.is_empty() {
        return true;
    }
    line.spans
        .iter()
        .all(|cell| cell.content.is_empty() || cell.content.chars().all(|c| c == ' '))
}

/// Prefix each line with `initial_prefix` for the first line and
/// `subsequent_prefix` for following lines. Returns a new Vec of owned lines.
pub fn prefix_lines(
    lines: Vec<RenderLine>,
    initial_prefix: RenderLine,
    subsequent_prefix: RenderLine,
) -> Vec<RenderLine> {
    lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let prefix = if i == 0 {
                initial_prefix.clone()
            } else {
                subsequent_prefix.clone()
            };
            let mut cells = Vec::with_capacity(prefix.spans.len() + l.spans.len());
            cells.extend(prefix.spans);
            cells.extend(l.spans);
            RenderLine::new(cells)
        })
        .collect()
}

pub fn prefix_lines_with_str(
    lines: Vec<RenderLine>,
    initial_prefix: &str,
    subsequent_prefix: &str,
) -> Vec<RenderLine> {
    prefix_lines(
        lines,
        RenderLine::from(vec![RenderCell::dim(initial_prefix)]),
        RenderLine::from(vec![RenderCell::dim(subsequent_prefix)]),
    )
}
