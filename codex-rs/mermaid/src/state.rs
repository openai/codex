//! Flat state machines, with distinct initial/final pseudostates and labeled transitions.

use super::Direction;
use super::Edge;
use super::Graph;
use super::MAX_EDGES;
use super::RenderError;
use super::parse::check_label;
use super::parse::identifier;

pub(super) fn parse(body: &[&str]) -> Result<Graph, RenderError> {
    let mut graph = Graph::default();
    let mut direction = false;
    for &line in body {
        if matches!(line, "state" | "direction" | "note" | "end" | "hide") {
            return Err(RenderError::Unsupported);
        }
        if let Some(value) = line.strip_prefix("direction ") {
            if direction {
                return Err(RenderError::Unsupported);
            }
            graph.direction = Direction::parse(value.trim())?;
            direction = true;
            continue;
        }
        let mut rest = line;
        if let Some(after) = rest.strip_prefix("state \"") {
            let (label, after) = after.split_once('"').ok_or(RenderError::Unsupported)?;
            check_label(label)?;
            rest = after
                .trim_start()
                .strip_prefix("as ")
                .ok_or(RenderError::Unsupported)?;
            let index = graph.node(identifier(&mut rest)?)?;
            if !rest.trim().is_empty() || graph.nodes[index].declared {
                return Err(RenderError::Unsupported);
            }
            graph.nodes[index].label = label.to_owned();
            graph.nodes[index].declared = true;
            continue;
        }
        let from = if let Some(after) = rest.strip_prefix("[*]") {
            rest = after;
            let index = graph.node("[initial]")?;
            graph.nodes[index].label = "● initial".to_owned();
            index
        } else {
            let id = identifier(&mut rest)?;
            if (id.eq_ignore_ascii_case("accTitle") || id.eq_ignore_ascii_case("accDescr"))
                && rest.trim_start().starts_with(':')
            {
                return Err(RenderError::Unsupported);
            }
            graph.node(id)?
        };
        rest = rest.trim();
        if rest.is_empty() && graph.nodes[from].id != "[initial]" {
            continue;
        }
        if let Some(description) = rest
            .strip_prefix(':')
            .filter(|text| !text.starts_with("::"))
        {
            if graph.nodes[from].id == "[initial]" {
                return Err(RenderError::Unsupported);
            }
            let description = description.trim();
            check_label(description)?;
            if graph.nodes[from].members.len() == 16 {
                return Err(RenderError::Limit);
            }
            graph.nodes[from].members.push(description.to_owned());
            continue;
        }
        rest = rest
            .strip_prefix("-->")
            .ok_or(RenderError::Unsupported)?
            .trim_start();
        let to = if let Some(after) = rest.strip_prefix("[*]") {
            rest = after;
            let index = graph.node("[final]")?;
            graph.nodes[index].label = "◎ final".to_owned();
            index
        } else {
            graph.node(identifier(&mut rest)?)?
        };
        let label = if let Some(label) = rest
            .trim()
            .strip_prefix(':')
            .filter(|text| !text.starts_with("::"))
        {
            let label = label.trim();
            check_label(label)?;
            label.to_owned()
        } else if rest.trim().is_empty() {
            String::new()
        } else {
            return Err(RenderError::Unsupported);
        };
        if graph.edges.len() == MAX_EDGES {
            return Err(RenderError::Limit);
        }
        graph.edges.push(Edge::directed(from, to, label));
    }
    if graph.nodes.is_empty() {
        return Err(RenderError::Unsupported);
    }
    Ok(graph)
}
