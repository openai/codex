//! Bounded sequence timelines with ordered messages and explicitly nested control fragments.

use super::RenderError;
use super::Span;
use super::draw::put_text;
use super::output::Cell;
use super::output::finish;
use super::parse::check_label;
use super::parse::identifier;
use unicode_width::UnicodeWidthStr;

#[derive(Debug)]
enum Event {
    Message {
        from: usize,
        to: usize,
        text: String,
        dashed: bool,
        arrow: char,
    },
    Open(String),
    Branch(String),
    Close,
    Note {
        left: usize,
        right: usize,
        text: String,
    },
}

pub(super) fn render(body: &[&str], max_width: usize) -> Result<Vec<Vec<Span>>, RenderError> {
    let mut people: Vec<(String, String, bool)> = Vec::new();
    let mut events = Vec::new();
    let mut blocks = Vec::new();
    for &line in body {
        let mut rest = line;
        if let Some(after) = line
            .strip_prefix("participant ")
            .or_else(|| line.strip_prefix("actor "))
        {
            rest = after;
            let id = identifier(&mut rest)?;
            let label = if let Some(label) = rest.trim_start().strip_prefix("as ") {
                check_label(label)?;
                label
            } else if rest.trim().is_empty() {
                id
            } else {
                return Err(RenderError::Unsupported);
            };
            let index = participant(&mut people, id)?;
            if people[index].2 {
                return Err(RenderError::Unsupported);
            }
            people[index].1 = if line.starts_with("actor ") {
                format!("{label} (actor)")
            } else {
                label.to_owned()
            };
            people[index].2 = true;
            continue;
        }
        let event = if line == "end" {
            blocks.pop().ok_or(RenderError::Unsupported)?;
            Event::Close
        } else if let Some(text) = line.strip_prefix("else ") {
            let Some(("alt", branched)) = blocks.last_mut() else {
                return Err(RenderError::Unsupported);
            };
            if *branched {
                return Err(RenderError::Unsupported);
            }
            *branched = true;
            check_label(text)?;
            Event::Branch(format!("else {text}"))
        } else if let Some((kind @ ("loop" | "alt" | "opt" | "critical" | "break"), text)) =
            line.split_once(' ')
        {
            check_label(text)?;
            if blocks.len() == 4 {
                return Err(RenderError::Limit);
            }
            blocks.push((kind, false));
            Event::Open(format!("{kind} {text}"))
        } else if let Some(after) = line
            .strip_prefix("Note over ")
            .or_else(|| line.strip_prefix("note over "))
        {
            rest = after;
            let left = participant(&mut people, identifier(&mut rest)?)?;
            let right = if let Some(after) = rest.trim_start().strip_prefix(',') {
                rest = after;
                participant(&mut people, identifier(&mut rest)?)?
            } else {
                left
            };
            let text = rest
                .trim_start()
                .strip_prefix(':')
                .ok_or(RenderError::Unsupported)?
                .trim();
            check_label(text)?;
            Event::Note {
                left: left.min(right),
                right: left.max(right),
                text: text.to_owned(),
            }
        } else {
            let from = participant(&mut people, identifier(&mut rest)?)?;
            rest = rest.trim_start();
            let mut operator = None;
            for (token, dashed, arrow) in [
                ("-->>", true, '▶'),
                ("->>", false, '▶'),
                ("-->", true, '┄'),
                ("->", false, '─'),
                ("--x", true, 'x'),
                ("-x", false, 'x'),
            ] {
                if let Some(after) = rest.strip_prefix(token) {
                    rest = after;
                    operator = Some((dashed, arrow));
                    break;
                }
            }
            let (dashed, arrow) = operator.ok_or(RenderError::Unsupported)?;
            let to = participant(&mut people, identifier(&mut rest)?)?;
            let text = rest
                .trim_start()
                .strip_prefix(':')
                .ok_or(RenderError::Unsupported)?
                .trim();
            check_label(text)?;
            Event::Message {
                from,
                to,
                text: text.to_owned(),
                dashed,
                arrow,
            }
        };
        if events.len() == 64 {
            return Err(RenderError::Limit);
        }
        events.push(event);
    }
    if !blocks.is_empty() || people.is_empty() {
        return Err(RenderError::Unsupported);
    }
    let name_width = people
        .iter()
        .map(|(_, label, _)| label.width())
        .max()
        .unwrap_or(0);
    let text_width = events
        .iter()
        .map(|event| match event {
            Event::Message { text, .. } | Event::Open(text) | Event::Branch(text) => text.width(),
            Event::Note { text, .. } => text.width() + 6,
            Event::Close => 0,
        })
        .max()
        .unwrap_or(0);
    let box_width = name_width + 4;
    let stride = (box_width + 2).max(text_width + 4);
    // Four frame levels consume eight columns on each side, separate from participant lifelines.
    let centers = (0..people.len())
        .map(|i| 10 + box_width / 2 + stride * i)
        .collect::<Vec<_>>();
    let last = centers[people.len() - 1];
    let width = (last + box_width - box_width / 2 + 10).max(last + text_width + 14);
    if width * (3 + 4 * events.len()) > super::MAX_CELLS {
        return Err(RenderError::Limit);
    }
    let mut rows = vec![vec![Cell::edge(' '); width]; 3];
    for (i, (_, name, _)) in people.iter().enumerate() {
        let left = centers[i] - box_width / 2;
        let right = left + box_width - 1;
        rows[0][left] = Cell::node('┌');
        rows[0][right] = Cell::node('┐');
        rows[2][left] = Cell::node('└');
        rows[2][right] = Cell::node('┘');
        rows[0][left + 1..right].fill(Cell::node('─'));
        rows[2][left + 1..right].fill(Cell::node('─'));
        rows[1][left] = Cell::node('│');
        rows[1][right] = Cell::node('│');
        put_text(&mut rows[1], left + 2, name)?;
    }
    let mut depth = 0;
    for event in events {
        let count = if matches!(event, Event::Message { from, to, .. } if from == to) {
            4
        } else {
            3
        };
        let top = rows.len();
        rows.extend((0..count).map(|_| {
            let mut row = vec![Cell::edge(' '); width];
            for x in &centers {
                row[*x] = Cell::edge('│');
            }
            for level in 0..depth {
                row[level * 2] = Cell::node('│');
                row[width - 1 - level * 2] = Cell::node('│');
            }
            row
        }));
        match event {
            Event::Message {
                from,
                to,
                text,
                dashed,
                arrow,
            } => {
                let (a, b) = (centers[from], centers[to]);
                let arrow = if a >= b && arrow == '▶' {
                    '◀'
                } else {
                    arrow
                };
                let stroke = if dashed { '┄' } else { '─' };
                put_text(&mut rows[top], a.min(b) + 2, &text)?;
                if a == b {
                    rows[top + 1][a..a + 4].fill(Cell::edge(stroke));
                    rows[top + 1][a] = Cell::edge('├');
                    rows[top + 1][a + 4] = Cell::edge('┐');
                    rows[top + 2][a..a + 4].fill(Cell::edge(stroke));
                    rows[top + 2][a + 4] = Cell::edge('┘');
                    rows[top + 2][a] = Cell::edge(arrow);
                } else {
                    rows[top + 1][a.min(b)..=a.max(b)].fill(Cell::edge(stroke));
                    for x in &centers {
                        if *x > a.min(b) && *x < a.max(b) {
                            rows[top + 1][*x] = Cell::edge('┼');
                        }
                    }
                    rows[top + 1][a] = Cell::edge(if a < b { '├' } else { '┤' });
                    rows[top + 1][b] = Cell::edge(arrow);
                }
            }
            Event::Note { left, right, text } => {
                let label = format!("Note: {text}");
                let start = centers[left];
                let end = centers[right].max(start + label.width() + 3);
                rows[top][start..=end].fill(Cell::node('─'));
                rows[top + 2][start..=end].fill(Cell::node('─'));
                rows[top][start] = Cell::node('┌');
                rows[top][end] = Cell::node('┐');
                rows[top + 2][start] = Cell::node('└');
                rows[top + 2][end] = Cell::node('┘');
                rows[top + 1][start..=end].fill(Cell::node(' '));
                rows[top + 1][start] = Cell::node('│');
                rows[top + 1][end] = Cell::node('│');
                put_text(&mut rows[top + 1], start + 2, &label)?;
            }
            Event::Open(label) | Event::Branch(label) => {
                let opening = !label.starts_with("else ");
                if opening {
                    depth += 1;
                }
                let left = (depth - 1) * 2;
                let right = width - 1 - left;
                rows[top][left..=right].fill(Cell::node('─'));
                rows[top][left] = Cell::node(if opening { '┌' } else { '├' });
                rows[top][right] = Cell::node(if opening { '┐' } else { '┤' });
                put_text(&mut rows[top], left + 2, &label)?;
                for row in &mut rows[top + 1..top + 3] {
                    row[left] = Cell::node('│');
                    row[right] = Cell::node('│');
                }
            }
            Event::Close => {
                depth -= 1;
                let left = depth * 2;
                let right = width - 1 - left;
                rows[top][left..=right].fill(Cell::node('─'));
                rows[top][left] = Cell::node('└');
                rows[top][right] = Cell::node('┘');
                for row in &mut rows[top + 1..top + 3] {
                    row[left] = Cell::node(' ');
                    row[right] = Cell::node(' ');
                }
            }
        }
    }
    if rows.iter().any(|row| {
        row.iter()
            .rposition(|cell| cell.symbol != ' ')
            .is_some_and(|x| x >= max_width)
    }) {
        return Err(RenderError::TooWide);
    }
    Ok(finish(rows))
}

fn participant(people: &mut Vec<(String, String, bool)>, id: &str) -> Result<usize, RenderError> {
    if let Some(index) = people.iter().position(|(name, _, _)| name == id) {
        return Ok(index);
    }
    if people.len() == 8 {
        return Err(RenderError::Limit);
    }
    people.push((id.to_owned(), id.to_owned(), false));
    Ok(people.len() - 1)
}
