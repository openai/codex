//! Orthogonal routes with unique lanes and endpoint positions, in all four directions.
//!
//! Routes never share segments. Crossings are explicitly marked; labels occupy reserved gutters.
//! Flat flowchart groups reserve frames around their nodes and internal routes.

use super::Direction;
use super::Graph;
use super::RenderError;
use super::Role;
use super::Shape;
use super::Span;
use super::output::Cell;
use super::output::finish;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

pub(super) fn render(graph: &Graph, max_width: usize) -> Result<Vec<Vec<Span>>, RenderError> {
    let horizontal = matches!(graph.direction, Direction::Right | Direction::Left);
    let labels = graph
        .nodes
        .iter()
        .map(|node| {
            let mut lines = node
                .label
                .split('\n')
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if node.shape == Shape::Decision {
                lines[0].insert_str(0, "◇ ");
            }
            if !node.members.is_empty() {
                lines.push("─".repeat(node.label.width()));
                lines.extend(node.members.iter().cloned());
            }
            lines
        })
        .collect::<Vec<_>>();
    let label_width = graph
        .edges
        .iter()
        .flat_map(|edge| [&edge.label, &edge.target_label])
        .map(|label| label.width())
        .max()
        .unwrap_or(0);
    let box_cross = if horizontal {
        labels.iter().map(Vec::len).max().unwrap_or(0) + 2
    } else {
        labels
            .iter()
            .flatten()
            .map(|label| label.width())
            .max()
            .unwrap_or(0)
            + 4
    };
    let mut counts = vec![0; graph.nodes.len()];
    let mut ports = Vec::new();
    for edge in &graph.edges {
        let source = counts[edge.from];
        counts[edge.from] += 1;
        let target = counts[edge.to];
        counts[edge.to] += 1;
        ports.push((source, target));
    }
    let stride = if horizontal { label_width + 2 } else { 1 };
    let sizes = labels
        .iter()
        .enumerate()
        .map(|(i, lines)| {
            if horizontal {
                (lines.iter().map(|line| line.width()).max().unwrap_or(0) + 4)
                    .max(counts[i] * stride + 2)
            } else {
                lines.len() + counts[i] + 2
            }
        })
        .collect::<Vec<_>>();
    let grouped = !graph.subgraphs.is_empty();
    let inset = if grouped { 2 } else { 0 };
    let mut membership = vec![None; graph.nodes.len()];
    for (group, subgraph) in graph.subgraphs.iter().enumerate() {
        for &node in &subgraph.nodes {
            membership[node] = Some(group);
        }
    }
    // Keep a group's members contiguous even when an edge referenced them before its declaration.
    let mut order = Vec::new();
    for (node, &group) in membership.iter().enumerate() {
        if order.contains(&node) {
            continue;
        }
        if let Some(group) = group {
            let mut members = graph.subgraphs[group].nodes.clone();
            members.sort_unstable();
            order.extend(members);
        } else {
            order.push(node);
        }
    }
    if matches!(graph.direction, Direction::Up | Direction::Left) {
        order.reverse();
    }
    let mut starts = vec![0; graph.nodes.len()];
    let mut frames = vec![(0, 0); graph.subgraphs.len()];
    let mut along = 0;
    let mut position = 0;
    while position < order.len() {
        let node = order[position];
        if let Some(group) = membership[node] {
            let frame_start = along;
            along += 2;
            while position < order.len() && membership[order[position]] == Some(group) {
                let node = order[position];
                starts[node] = along;
                along += sizes[node] + 2;
                position += 1;
            }
            if horizontal {
                along = along.max(frame_start + graph.subgraphs[group].label.width() + 4);
            }
            frames[group] = (frame_start, along - 1);
            along += 2;
        } else {
            starts[node] = along;
            along += sizes[node] + 2;
            position += 1;
        }
    }
    let first_lane = inset + box_cross + if horizontal { 4 } else { label_width + 5 };
    let internal = graph
        .edges
        .iter()
        .map(|edge| membership[edge.from].is_some() && membership[edge.from] == membership[edge.to])
        .collect::<Vec<_>>();
    let internal_count = internal.iter().filter(|&&value| value).count();
    let mut frame_cross = inset + box_cross + 2;
    if !graph.edges.is_empty() {
        frame_cross = frame_cross.max(first_lane + internal_count * 2 + 1);
    }
    if !horizontal {
        frame_cross = frame_cross.max(
            graph
                .subgraphs
                .iter()
                .map(|group| group.label.width() + 4)
                .max()
                .unwrap_or(0),
        );
    }
    let external_lane = if grouped { frame_cross + 2 } else { first_lane };
    let mut next_internal = 0;
    let mut next_external = 0;
    let lanes = internal
        .iter()
        .map(|&internal| {
            if internal {
                let lane = first_lane + next_internal * 2;
                next_internal += 1;
                lane
            } else {
                let lane = external_lane + next_external * 2;
                next_external += 1;
                lane
            }
        })
        .collect::<Vec<_>>();
    let across = lanes
        .iter()
        .max()
        .map_or(inset + box_cross, |lane| lane + 1)
        .max(if grouped { frame_cross } else { 0 });
    let (width, height) = if horizontal {
        (along - 2, across)
    } else {
        (across, along - 2)
    };
    if width > max_width {
        return Err(RenderError::TooWide);
    }
    if width * height > super::MAX_CELLS {
        return Err(RenderError::Limit);
    }
    let mut canvas = Canvas {
        cells: vec![vec![Cell::edge(' '); width]; height],
        horizontal,
    };
    for (group, &(start, end)) in frames.iter().enumerate() {
        for x in 1..frame_cross - 1 {
            canvas.set(x, start, Cell::node('─'));
            canvas.set(x, end, Cell::node('─'));
        }
        for y in start + 1..end {
            canvas.set(/*across*/ 0, y, Cell::node('│'));
            canvas.set(frame_cross - 1, y, Cell::node('│'));
        }
        for (x, y, ch) in [
            (0, start, '┌'),
            (frame_cross - 1, start, '┐'),
            (0, end, '└'),
            (frame_cross - 1, end, '┘'),
        ] {
            canvas.set(x, y, Cell::node(ch));
        }
        let (x, y) = if horizontal {
            (start + 2, 0)
        } else {
            (2, start)
        };
        put_text(&mut canvas.cells[y], x, &graph.subgraphs[group].label)?;
    }
    for (i, lines) in labels.iter().enumerate() {
        let start = starts[i];
        let end = start + sizes[i] - 1;
        let [top_left, top_right, bottom_left, bottom_right] = match graph.nodes[i].shape {
            Shape::Stadium => ['╭', '╮', '╰', '╯'],
            Shape::Rectangle | Shape::Decision => ['┌', '┐', '└', '┘'],
        };
        canvas.set(inset, start, Cell::node(top_left));
        canvas.set(inset + box_cross - 1, start, Cell::node(top_right));
        canvas.set(inset, end, Cell::node(bottom_left));
        canvas.set(inset + box_cross - 1, end, Cell::node(bottom_right));
        for x in 1..box_cross - 1 {
            canvas.set(inset + x, start, Cell::node('─'));
            canvas.set(inset + x, end, Cell::node('─'));
        }
        for y in start + 1..end {
            canvas.set(inset, y, Cell::node('│'));
            canvas.set(inset + box_cross - 1, y, Cell::node('│'));
        }
        for (j, line) in lines.iter().enumerate() {
            let (x, y) = if horizontal {
                (start + 2, inset + j + 1)
            } else {
                (inset + 2, start + j + 1)
            };
            put_text(&mut canvas.cells[y], x, line)?;
        }
    }
    let endpoints = graph
        .edges
        .iter()
        .enumerate()
        .map(|(i, edge)| {
            let offset = |node: usize, port: usize| {
                starts[node]
                    + if horizontal {
                        1 + port * stride
                    } else {
                        labels[node].len() + 1 + port
                    }
            };
            (offset(edge.from, ports[i].0), offset(edge.to, ports[i].1))
        })
        .collect::<Vec<_>>();
    // Paint lanes first so every crossing is independent of iteration order.
    for (i, edge) in graph.edges.iter().enumerate() {
        let (source, target) = endpoints[i];
        for y in source.min(target) + 1..source.max(target) {
            canvas.set(lanes[i], y, Cell::edge(if edge.dashed { '┆' } else { '│' }));
        }
    }
    for (i, edge) in graph.edges.iter().enumerate() {
        let (source, target) = endpoints[i];
        let lane = lanes[i];
        for y in [source, target] {
            for x in inset + box_cross..lane {
                let ch = if grouped
                    && x == frame_cross - 1
                    && membership[if y == source { edge.from } else { edge.to }].is_some()
                {
                    '┼'
                } else if matches!(canvas.get(x, y), '│' | '┆') {
                    '╪'
                } else if edge.dashed {
                    '┄'
                } else {
                    '─'
                };
                canvas.set(x, y, Cell::edge(ch));
            }
        }
        canvas.set(inset + box_cross - 1, source, Cell::node('├'));
        canvas.set(inset + box_cross - 1, target, Cell::node('├'));
        canvas.set(inset + box_cross, source, Cell::edge(edge.source_tip));
        canvas.set(inset + box_cross, target, Cell::edge(edge.target_tip));
        canvas.set(
            lane,
            source,
            Cell::edge(if source < target { '┐' } else { '┘' }),
        );
        canvas.set(
            lane,
            target,
            Cell::edge(if source < target { '┘' } else { '┐' }),
        );
        for (port, label) in [(source, &edge.label), (target, &edge.target_label)] {
            let (x, y) = if horizontal {
                (port + 1, inset + box_cross + 1)
            } else {
                (inset + box_cross + 2, port)
            };
            put_text(&mut canvas.cells[y], x, label)?;
        }
    }
    Ok(finish(canvas.cells))
}

struct Canvas {
    cells: Vec<Vec<Cell>>,
    horizontal: bool,
}

impl Canvas {
    fn set(&mut self, across: usize, along: usize, mut cell: Cell) {
        if self.horizontal {
            cell.symbol = transpose(cell.symbol);
            self.cells[across][along] = cell;
        } else {
            self.cells[along][across] = cell;
        }
    }

    fn get(&self, across: usize, along: usize) -> char {
        if self.horizontal {
            transpose(self.cells[across][along].symbol)
        } else {
            self.cells[along][across].symbol
        }
    }
}

fn transpose(ch: char) -> char {
    match ch {
        '─' => '│',
        '│' => '─',
        '┄' => '┆',
        '┆' => '┄',
        '┐' => '└',
        '└' => '┐',
        '╮' => '╰',
        '╰' => '╮',
        '├' => '┬',
        '┬' => '├',
        '◄' => '▲',
        '▲' => '◄',
        '◁' => '△',
        '△' => '◁',
        other => other,
    }
}

pub(super) fn put_text(row: &mut [Cell], mut column: usize, text: &str) -> Result<(), RenderError> {
    for ch in text.chars() {
        let width = UnicodeWidthChar::width(ch).ok_or(RenderError::Unsupported)?;
        row[column] = Cell {
            symbol: ch,
            role: Role::Text,
        };
        row[column + 1..column + width].fill(Cell {
            symbol: '\0',
            role: Role::Text,
        });
        column += width;
    }
    Ok(())
}
