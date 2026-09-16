//! Class and ER grammars, preserving members, endpoint cardinalities, and relationship kinds.

use super::Direction;
use super::Edge;
use super::Graph;
use super::MAX_EDGES;
use super::RenderError;
use super::parse::check_label;
use super::parse::identifier;

pub(super) fn parse(header: &str, body: &[&str]) -> Result<Graph, RenderError> {
    if matches!(header, "stateDiagram" | "stateDiagram-v2") {
        return super::state::parse(body);
    }
    let er = header == "erDiagram";
    let mut graph = Graph::default();
    let mut block = None;
    let mut direction = false;
    for &line in body {
        if let Some(index) = block {
            if line == "}" {
                block = None;
            } else {
                let member = if er {
                    attribute(line)?
                } else {
                    check_label(line)?;
                    line.to_owned()
                };
                let node: &mut super::Node = &mut graph.nodes[index];
                if node.members.len() == 16 {
                    return Err(RenderError::Limit);
                }
                node.members.push(member);
            }
            continue;
        }
        if let Some(value) = line.strip_prefix("direction ") {
            if direction {
                return Err(RenderError::Unsupported);
            }
            graph.direction = Direction::parse(value.trim())?;
            direction = true;
            continue;
        }
        let declaration = !er && line.starts_with("class ");
        let mut rest = line.strip_prefix("class ").filter(|_| !er).unwrap_or(line);
        let id = identifier(&mut rest)?;
        rest = rest.trim_start();
        let title = id == "accTitle" || er && id.eq_ignore_ascii_case("accTitle");
        let description = id == "accDescr" || er && id.eq_ignore_ascii_case("accDescr");
        if !declaration
            && (title && rest.starts_with(':') || description && rest.starts_with([':', '{']))
        {
            return Err(RenderError::Unsupported);
        }
        let from = graph.node(id)?;
        if rest == "{" && (er || declaration) {
            block = Some(from);
            continue;
        }
        if declaration || (er && rest.is_empty()) {
            if !rest.is_empty() {
                return Err(RenderError::Unsupported);
            }
            continue;
        }
        if !er
            && let Some(member) = rest
                .strip_prefix(':')
                .filter(|text| !text.starts_with("::"))
        {
            let member = member.trim();
            check_label(member)?;
            if graph.nodes[from].members.len() == 16 {
                return Err(RenderError::Limit);
            }
            graph.nodes[from].members.push(member.to_owned());
            continue;
        }
        let source_card = if er {
            String::new()
        } else {
            cardinality(&mut rest)?
        };
        rest = rest.trim_start();
        let split = rest
            .find("--")
            .into_iter()
            .chain(rest.find(".."))
            .min()
            .ok_or(RenderError::Unsupported)?;
        if split > 2 {
            return Err(RenderError::Unsupported);
        }
        let left = &rest[..split];
        let dashed = &rest[split..split + 2] == "..";
        rest = &rest[split + 2..];
        let (source_tip, target_tip, source_card, target_card) = if er {
            let source_card = match left {
                "||" => "1",
                "|o" => "0..1",
                "}|" => "1..many",
                "}o" => "0..many",
                _ => return Err(RenderError::Unsupported),
            }
            .to_owned();
            let right = rest.get(..2).ok_or(RenderError::Unsupported)?;
            let target_card = match right {
                "||" => "1",
                "o|" => "0..1",
                "|{" => "1..many",
                "o{" => "0..many",
                _ => return Err(RenderError::Unsupported),
            }
            .to_owned();
            rest = &rest[2..];
            ('─', '─', source_card, target_card)
        } else {
            let source_tip = match left {
                "" => '─',
                "<" => '◄',
                "<|" => '◁',
                "*" => '◆',
                "o" => '◇',
                _ => return Err(RenderError::Unsupported),
            };
            let mut target_tip = '─';
            for (token, tip) in [("|>", '◁'), (">", '◄'), ("*", '◆'), ("o", '◇')] {
                if let Some(after) = rest.strip_prefix(token) {
                    target_tip = tip;
                    rest = after;
                    break;
                }
            }
            let target_card = cardinality(&mut rest)?;
            (source_tip, target_tip, source_card, target_card)
        };
        let to = graph.node(identifier(&mut rest)?)?;
        rest = rest.trim();
        let label = if let Some(label) = rest
            .strip_prefix(':')
            .filter(|text| !text.starts_with("::"))
        {
            let label = label.trim();
            check_label(label)?;
            label.to_owned()
        } else if !rest.is_empty() || er {
            return Err(RenderError::Unsupported);
        } else {
            String::new()
        };
        if graph.edges.len() == MAX_EDGES {
            return Err(RenderError::Limit);
        }
        let label = match (source_card.is_empty(), label.is_empty()) {
            (true, _) => label,
            (false, true) => format!("({source_card})"),
            (false, false) => format!("({source_card}) {label}"),
        };
        graph.edges.push(Edge {
            from,
            to,
            label,
            source_tip,
            target_tip,
            dashed,
            target_label: if target_card.is_empty() {
                target_card
            } else {
                format!("({target_card})")
            },
        });
    }
    if block.is_some() || graph.nodes.is_empty() {
        return Err(RenderError::Unsupported);
    }
    Ok(graph)
}

fn cardinality(rest: &mut &str) -> Result<String, RenderError> {
    *rest = rest.trim_start();
    if let Some(after) = rest.strip_prefix('"') {
        let (value, after) = after.split_once('"').ok_or(RenderError::Unsupported)?;
        check_label(value)?;
        *rest = after;
        Ok(value.to_owned())
    } else {
        Ok(String::new())
    }
}

fn attribute(line: &str) -> Result<String, RenderError> {
    let mut rest = line;
    let data_type = identifier(&mut rest)?;
    if !rest.starts_with(char::is_whitespace) {
        return Err(RenderError::Unsupported);
    }
    let name = identifier(&mut rest)?;
    let (keys, comment) = if let Some((keys, comment)) = rest.trim().split_once('"') {
        let comment = comment.strip_suffix('"').ok_or(RenderError::Unsupported)?;
        check_label(comment)?;
        (keys.trim(), Some(comment))
    } else {
        (rest.trim(), None)
    };
    if !keys.is_empty()
        && !keys
            .split(',')
            .all(|key| matches!(key.trim(), "PK" | "FK" | "UK"))
    {
        return Err(RenderError::Unsupported);
    }
    let mut result = format!("{data_type} {name}");
    if !keys.is_empty() {
        result.push(' ');
        result.push_str(keys);
    }
    if let Some(comment) = comment {
        result.push_str(" — ");
        result.push_str(comment);
    }
    check_label(&result)?;
    Ok(result)
}
