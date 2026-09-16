//! Strict parser for a small flowchart grammar; every non-comment byte must be consumed.

use super::Direction;
use super::Edge;
use super::Graph;
use super::MAX_EDGES;
use super::MAX_LABEL;
use super::RenderError;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

pub(super) fn parse(header: &str, body: &[&str]) -> Result<Graph, RenderError> {
    let tokens = header.split_whitespace().collect::<Vec<_>>();
    let ["flowchart" | "graph", direction] = tokens.as_slice() else {
        return Err(RenderError::Unsupported);
    };
    let mut graph = Graph {
        direction: Direction::parse(direction)?,
        ..Graph::default()
    };
    for statement in body {
        let mut rest = *statement;
        let mut from = node(&mut rest, &mut graph)?;
        while !rest.trim_start().is_empty() {
            rest = rest
                .trim_start()
                .strip_prefix("-->")
                .ok_or(RenderError::Unsupported)?;
            rest = rest.trim_start();
            let label = if let Some(after) = rest.strip_prefix('|') {
                let (label, remaining) = after.split_once('|').ok_or(RenderError::Unsupported)?;
                check_label(label)?;
                rest = remaining;
                label.to_owned()
            } else {
                String::new()
            };
            let to = node(&mut rest, &mut graph)?;
            if graph.edges.len() == MAX_EDGES {
                return Err(RenderError::Limit);
            }
            graph.edges.push(Edge::directed(from, to, label));
            from = to;
        }
    }
    if graph.nodes.is_empty() {
        return Err(RenderError::Unsupported);
    }
    Ok(graph)
}

fn node(rest: &mut &str, graph: &mut Graph) -> Result<usize, RenderError> {
    let id = identifier(rest)?;
    // Reserved constructs must not be interpreted as ordinary node declarations.
    if matches!(
        id,
        "end" | "subgraph" | "direction" | "style" | "class" | "classDef" | "linkStyle" | "click"
    ) {
        return Err(RenderError::Unsupported);
    }
    let declaration = match rest.chars().next() {
        Some(open @ ('[' | '{')) => {
            let close = if open == '[' { ']' } else { '}' };
            let (label, remaining) = rest[1..]
                .split_once(close)
                .ok_or(RenderError::Unsupported)?;
            check_label(label)?;
            *rest = remaining;
            Some((label, open == '{'))
        }
        _ => None,
    };
    let index = graph.node(id)?;
    if let Some((label, decision)) = declaration {
        let node = &mut graph.nodes[index];
        if node.declared && (node.label != label || node.decision != decision) {
            return Err(RenderError::Unsupported);
        }
        node.label = label.to_owned();
        node.decision = decision;
        node.declared = true;
    }
    Ok(index)
}

pub(super) fn check_label(label: &str) -> Result<(), RenderError> {
    if label.trim().is_empty()
        || label.chars().any(|ch| {
            ch.is_control()
                || matches!(
                    ch,
                    '[' | ']'
                        | '{'
                        | '}'
                        | '|'
                        | '<'
                        | '>'
                        | '&'
                        | '"'
                        | '\\'
                        | '┌'
                        | '┐'
                        | '└'
                        | '┘'
                        | '├'
                        | '┤'
                        | '╪'
                        | '◄'
                )
                || UnicodeWidthChar::width(ch).is_none_or(|width| width == 0)
        })
    {
        return Err(RenderError::Unsupported);
    }
    // Labels are drawn one Unicode scalar at a time. Reject ligatures whose string width differs
    // from those scalar widths rather than misaligning borders or underallocating the canvas.
    if label
        .chars()
        .filter_map(UnicodeWidthChar::width)
        .sum::<usize>()
        != label.width()
    {
        return Err(RenderError::Unsupported);
    }
    if UnicodeWidthStr::width(label) > MAX_LABEL {
        return Err(RenderError::Limit);
    }
    Ok(())
}

pub(super) fn identifier<'a>(rest: &mut &'a str) -> Result<&'a str, RenderError> {
    *rest = rest.trim_start();
    let len = rest
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
        .count();
    let id = &rest[..len];
    if id.is_empty() || !id.as_bytes()[0].is_ascii_alphabetic() || id.len() > MAX_LABEL {
        return Err(RenderError::Unsupported);
    }
    *rest = &rest[len..];
    Ok(id)
}
