//! Strict parser for a small flowchart grammar; every non-comment byte must be consumed.
//! Node groups expand into explicit edges within the graph edge cap.

use super::Direction;
use super::Edge;
use super::Graph;
use super::MAX_EDGES;
use super::MAX_LABEL;
use super::RenderError;
use super::Shape;
use super::syntax::delimited_label;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

pub(super) fn parse(header: &str, body: &[&str]) -> Result<Graph, RenderError> {
    let tokens = header.split_whitespace().collect::<Vec<_>>();
    let direction = match tokens.as_slice() {
        ["flowchart" | "graph"] => Direction::Down,
        ["flowchart" | "graph", direction] => Direction::parse(direction)?,
        _ => return Err(RenderError::Unsupported),
    };
    let mut graph = Graph {
        direction,
        ..Graph::default()
    };
    for statement in body {
        let mut rest = *statement;
        let mut from = nodes(&mut rest, &mut graph)?;
        while !rest.trim_start().is_empty() {
            rest = rest.trim_start();
            let mut label = String::new();
            let (source_tip, target_tip, dashed) = if let Some((after, source, target, dashed)) = [
                ("<-.->", '◄', '◄', true),
                ("<-->", '◄', '◄', false),
                ("-.->", '─', '◄', true),
                ("-.-", '─', '─', true),
                ("-->", '─', '◄', false),
                ("---", '─', '─', false),
            ]
            .into_iter()
            .find_map(|(token, source, target, dashed)| {
                rest.strip_prefix(token)
                    .map(|after| (after, source, target, dashed))
            }) {
                // Circle/cross tips are unsupported; without a space they are not node IDs.
                if target == '─' && after.starts_with(['o', 'x']) {
                    return Err(RenderError::Unsupported);
                }
                rest = after.trim_start();
                if let Some(after) = rest.strip_prefix('|') {
                    let (text, remaining) = delimited_label(after, "|")?;
                    label = flowchart_label(text)?.to_owned();
                    rest = remaining;
                }
                (source, target, dashed)
            } else {
                let (after, stem, dashed) = if let Some(after) = rest.strip_prefix("--") {
                    (after, "--", false)
                } else if let Some(after) = rest.strip_prefix("-.") {
                    (after, ".-", true)
                } else {
                    return Err(RenderError::Unsupported);
                };
                // Spaced labels keep endpoint markers out of the text. Stop at the first
                // closing stem instead of swallowing an unsupported edge and its target.
                let (text, remaining) = after.split_once(stem).ok_or(RenderError::Unsupported)?;
                if !text.starts_with(char::is_whitespace) || !text.ends_with(char::is_whitespace) {
                    return Err(RenderError::Unsupported);
                }
                rest = remaining
                    .strip_prefix('>')
                    .ok_or(RenderError::Unsupported)?;
                label = flowchart_label(text.trim())?.to_owned();
                ('─', '◄', dashed)
            };
            let to = nodes(&mut rest, &mut graph)?;
            // Check the Cartesian expansion before allocating edges, including repeated IDs.
            if from.len() * to.len() > MAX_EDGES - graph.edges.len() {
                return Err(RenderError::Limit);
            }
            for &from in &from {
                for &to in &to {
                    graph.edges.push(Edge {
                        from,
                        to,
                        label: label.clone(),
                        target_label: String::new(),
                        source_tip,
                        target_tip,
                        dashed,
                    });
                }
            }
            from = to;
        }
    }
    if graph.nodes.is_empty() {
        return Err(RenderError::Unsupported);
    }
    Ok(graph)
}

fn nodes(rest: &mut &str, graph: &mut Graph) -> Result<Vec<usize>, RenderError> {
    let mut nodes = vec![node(rest, graph)?];
    while let Some(after) = rest.trim_start().strip_prefix('&') {
        if nodes.len() == MAX_EDGES {
            return Err(RenderError::Limit);
        }
        *rest = after;
        nodes.push(node(rest, graph)?);
    }
    Ok(nodes)
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
    // Longer shape delimiters must not become punctuation inside a simpler node.
    if ["[(", "[[", "[/", "[\\", "{{"]
        .iter()
        .any(|open| rest.starts_with(open))
    {
        return Err(RenderError::Unsupported);
    }
    let declaration = match rest.chars().next() {
        Some('[') => Some(("[", "]", Shape::Rectangle)),
        Some('{') => Some(("{", "}", Shape::Decision)),
        Some('(') if rest.starts_with("([") => Some(("([", "])", Shape::Stadium)),
        _ => None,
    };
    let index = graph.node(id)?;
    if let Some((open, close, shape)) = declaration {
        let (label, remaining) = delimited_label(&rest[open.len()..], close)?;
        let label = flowchart_label(label)?;
        *rest = remaining;
        let node = &mut graph.nodes[index];
        if node.declared && (node.label != label || node.shape != shape) {
            return Err(RenderError::Unsupported);
        }
        node.label = label.to_owned();
        node.shape = shape;
        node.declared = true;
    }
    Ok(index)
}

fn flowchart_label(label: &str) -> Result<&str, RenderError> {
    // Mermaid Markdown strings require rendering beyond ordinary quoted labels.
    if label.starts_with("\"`") {
        return Err(RenderError::Unsupported);
    }
    let label = if let Some(quoted) = label.strip_prefix('"') {
        quoted.strip_suffix('"').ok_or(RenderError::Unsupported)?
    } else {
        if label.contains(['[', ']', '{', '}', '|']) {
            return Err(RenderError::Unsupported);
        }
        label
    };
    if label.contains('"') {
        return Err(RenderError::Unsupported);
    }
    check_label(label)?;
    Ok(label)
}

pub(super) fn check_label(label: &str) -> Result<(), RenderError> {
    // Markup needs deliberate decoding/layout; printable comparison operators are plain text.
    let markup = label.match_indices('<').any(|(index, _)| {
        let after = &label[index + 1..];
        after.starts_with(|ch: char| ch.is_ascii_alphabetic() || matches!(ch, '/' | '!' | '?'))
    });
    if markup
        || label.trim().is_empty()
        || label.chars().any(|ch| {
            ch.is_control()
                || matches!(ch, '┌' | '┐' | '└' | '┘' | '├' | '┤' | '╪' | '◄')
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
