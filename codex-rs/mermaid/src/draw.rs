//! Orthogonal routes with unique lanes and endpoint positions, in all four directions.
//!
//! Routes never share segments. Crossings are explicitly marked; labels occupy reserved gutters.

use super::Direction;
use super::Graph;
use super::RenderError;
use super::Role;
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
            let mut lines = vec![if node.decision {
                format!("◇ {}", node.label)
            } else {
                node.label.clone()
            }];
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
    let mut starts = vec![0; graph.nodes.len()];
    let mut along = 0;
    for i in 0..graph.nodes.len() {
        let i = if matches!(graph.direction, Direction::Up | Direction::Left) {
            graph.nodes.len() - 1 - i
        } else {
            i
        };
        starts[i] = along;
        along += sizes[i] + 2;
    }
    let first_lane = box_cross + if horizontal { 4 } else { label_width + 5 };
    let across = if graph.edges.is_empty() {
        box_cross
    } else {
        first_lane + graph.edges.len() * 2 - 1
    };
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
    for (i, lines) in labels.iter().enumerate() {
        let start = starts[i];
        let end = start + sizes[i] - 1;
        canvas.set(/*across*/ 0, start, Cell::node('┌'));
        canvas.set(box_cross - 1, start, Cell::node('┐'));
        canvas.set(/*across*/ 0, end, Cell::node('└'));
        canvas.set(box_cross - 1, end, Cell::node('┘'));
        for x in 1..box_cross - 1 {
            canvas.set(x, start, Cell::node('─'));
            canvas.set(x, end, Cell::node('─'));
        }
        for y in start + 1..end {
            canvas.set(/*across*/ 0, y, Cell::node('│'));
            canvas.set(box_cross - 1, y, Cell::node('│'));
        }
        for (j, line) in lines.iter().enumerate() {
            let (x, y) = if horizontal {
                (start + 2, j + 1)
            } else {
                (2, start + j + 1)
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
            canvas.set(
                first_lane + 2 * i,
                y,
                Cell::edge(if edge.dashed { '┆' } else { '│' }),
            );
        }
    }
    for (i, edge) in graph.edges.iter().enumerate() {
        let (source, target) = endpoints[i];
        let lane = first_lane + 2 * i;
        for y in [source, target] {
            for x in box_cross..lane {
                let ch = if matches!(canvas.get(x, y), '│' | '┆') {
                    '╪'
                } else if edge.dashed {
                    '┄'
                } else {
                    '─'
                };
                canvas.set(x, y, Cell::edge(ch));
            }
        }
        canvas.set(box_cross - 1, source, Cell::node('├'));
        canvas.set(box_cross - 1, target, Cell::node('├'));
        canvas.set(box_cross, source, Cell::edge(edge.source_tip));
        canvas.set(box_cross, target, Cell::edge(edge.target_tip));
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
                (port + 1, box_cross + 1)
            } else {
                (box_cross + 2, port)
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
