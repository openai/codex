//! A compact shortcut reference with shared width-aware measurement and rendering.
//!
//! Groups flow into three, two, or one column without changing key routing. Runtime hints
//! remain authoritative for remapped, chorded, and disabled bindings. A clipped reference
//! keeps customization visible; the composer reserves a separate bottom row for the close hint.

use super::footer::FooterProps;
use crate::key_hint;
use crate::key_hint::ShortcutHint;
use crate::style::accent_color;
use crate::style::secondary_text_style;
use crate::wrapping::word_wrap_lines;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

const COLUMN_GAP: usize = 4;

struct Shortcut {
    key: String,
    action: &'static str,
}

impl Shortcut {
    fn new(key: impl Into<ShortcutHint>, action: &'static str) -> Self {
        Self {
            key: key.into().display_label(),
            action,
        }
    }
}

struct Group {
    title: &'static str,
    entries: Vec<Shortcut>,
}

impl Group {
    fn push(&mut self, key: Option<ShortcutHint>, action: &'static str) {
        if let Some(key) = key {
            self.entries.push(Shortcut::new(key, action));
        }
    }

    fn lines(&self) -> Vec<Line<'static>> {
        let key_width = self
            .entries
            .iter()
            .map(|entry| Span::raw(&entry.key).width())
            .max()
            .unwrap_or(/*default*/ 0);
        let mut lines = vec![self.title.bold().into()];
        for entry in &self.entries {
            let key = entry.key.clone().fg(accent_color());
            let padding = " ".repeat(key_width.saturating_sub(key.width()) + 2);
            lines.push(vec![key, padding.into(), entry.action.into()].into());
        }
        lines
    }
}

pub(super) fn lines(props: &FooterProps, width: u16) -> Vec<Line<'static>> {
    let hints = props.key_hints;
    let mut compose = Group {
        title: "Compose",
        entries: vec![
            Shortcut::new(key_hint::plain(KeyCode::Char('/')), "Commands"),
            Shortcut::new(key_hint::plain(KeyCode::Char('@')), "Mention files"),
            Shortcut::new(key_hint::plain(KeyCode::Char('!')), "Shell command"),
        ],
    };
    compose.push(hints.insert_newline, "New line");
    compose.entries.push(Shortcut::new(
        if props.is_wsl {
            key_hint::ctrl_alt(KeyCode::Char('v'))
        } else {
            key_hint::ctrl(KeyCode::Char('v'))
        },
        "Paste image",
    ));
    compose.push(hints.external_editor, "External editor");
    compose.push(hints.history_search, "Search history");
    if let Some(key) = hints.edit_previous {
        let label = key.display_label();
        compose.entries.push(Shortcut {
            key: if props.esc_backtrack_hint {
                label
            } else {
                format!("{label} {label}")
            },
            action: "Edit last message",
        });
    }

    let mut session = Group {
        title: "Session",
        entries: Vec::new(),
    };
    session.push(
        hints.queue,
        if props.is_task_running || props.queue_submissions {
            "Queue message"
        } else {
            "Send message"
        },
    );
    if props.collaboration_modes_enabled {
        session
            .entries
            .push(Shortcut::new(key_hint::shift(KeyCode::Tab), "Change mode"));
    }
    session.push(hints.reasoning_down, "Less reasoning");
    session.push(hints.reasoning_up, "More reasoning");
    session.push(hints.agents, "Agents (empty prompt)");
    session.entries.push(Shortcut::new(
        key_hint::ctrl(KeyCode::Char('c')),
        if props.is_task_running {
            "Interrupt"
        } else {
            "Quit"
        },
    ));

    let mut transcript = Group {
        title: "Transcript (open first)",
        entries: Vec::new(),
    };
    transcript.push(hints.show_transcript, "Open transcript");
    transcript.push(hints.find_transcript, "Find text");
    transcript.entries.extend([
        Shortcut {
            key: "pgup / pgdn".into(),
            action: "Scroll",
        },
        Shortcut::new(key_hint::ctrl(KeyCode::Char(' ')), "Start selection"),
        Shortcut {
            key: "ctrl+home / ctrl+end".into(),
            action: "Top / latest",
        },
        // Keep both jump alternatives: the terminal may be on another OS over SSH.
        Shortcut {
            key: format!(
                "{} / {}",
                key_hint::KeyBinding::new(
                    KeyCode::Char(','),
                    KeyModifiers::ALT | KeyModifiers::SHIFT
                )
                .display_label(),
                key_hint::KeyBinding::new(
                    KeyCode::Char('.'),
                    KeyModifiers::ALT | KeyModifiers::SHIFT
                )
                .display_label()
            ),
            action: "Top / latest",
        },
    ]);

    let groups = [compose.lines(), session.lines(), transcript.lines()];
    let widths: Vec<usize> = groups
        .iter()
        .map(|group| group.iter().map(Line::width).max().unwrap_or(/*default*/ 0))
        .collect();
    let width = usize::from(width.max(/*other*/ 1));
    let mut result = vec![Line::from("Keyboard shortcuts").bold(), Line::default()];
    if widths.iter().sum::<usize>() + COLUMN_GAP * 2 <= width {
        result.extend(columns(&groups, &widths));
    } else if widths[0] + COLUMN_GAP + widths[1].max(widths[2]) <= width {
        let mut right = groups[1].clone();
        right.push(Line::default());
        right.extend(groups[2].clone());
        result.extend(columns(
            &[groups[0].clone(), right],
            &[widths[0], widths[1].max(widths[2])],
        ));
    } else {
        for (index, group) in groups.into_iter().enumerate() {
            if index > 0 {
                result.push(Line::default());
            }
            result.extend(group);
        }
    }
    result.push(Line::default());
    result.extend(footer_lines(width));
    word_wrap_lines(&result, width)
}

pub(super) fn close_hint(props: &FooterProps, width: u16) -> Line<'static> {
    let mut line = Line::default();
    if let Some(key) = props.key_hints.toggle_shortcuts {
        line.extend(key.spans());
        line.push_span(" / ".set_style(secondary_text_style()));
    }
    line.extend(key_hint::plain(KeyCode::Esc).spans());
    line.push_span(" close".set_style(secondary_text_style()));
    if line.width() > usize::from(width) {
        line = Line::from(key_hint::plain(KeyCode::Esc).spans());
        line.push_span(" close".set_style(secondary_text_style()));
    }
    line
}

fn footer_lines(width: usize) -> Vec<Line<'static>> {
    word_wrap_lines(
        [Line::from(vec![
            "/keymap".fg(accent_color()),
            " customize".set_style(secondary_text_style()),
        ])],
        width,
    )
}

fn columns(groups: &[Vec<Line<'static>>], widths: &[usize]) -> Vec<Line<'static>> {
    let height = groups.iter().map(Vec::len).max().unwrap_or(/*default*/ 0);
    (0..height)
        .map(|row| {
            let mut line = Line::default();
            for (column, group) in groups.iter().enumerate() {
                let entry = group.get(row).cloned().unwrap_or_default();
                let padding = widths[column].saturating_sub(entry.width()) + COLUMN_GAP;
                line.extend(entry.spans);
                if column + 1 < groups.len() {
                    line.push_span(" ".repeat(padding));
                }
            }
            line
        })
        .collect()
}

pub(super) fn render(props: &FooterProps, area: Rect, buf: &mut Buffer) {
    let mut lines = lines(props, area.width);
    let height = usize::from(area.height);
    if lines.len() > height && height > 0 {
        let mut footer = footer_lines(usize::from(area.width.max(/*other*/ 1)));
        footer.truncate(height);
        let body_height = height.saturating_sub(footer.len());
        lines.truncate(body_height.saturating_sub(/*rhs*/ 1));
        if body_height > 0 {
            lines.push(Line::from("… resize to see all").dim());
        }
        lines.extend(footer);
    }
    Paragraph::new(lines).render(area, buf);
}

#[cfg(test)]
#[path = "shortcut_overlay_tests.rs"]
mod tests;
