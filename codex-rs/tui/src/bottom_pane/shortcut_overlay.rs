//! A compact shortcut reference with shared width-aware measurement and rendering.
//!
//! Groups flow into three, two, or one column without changing key routing. Runtime hints
//! remain authoritative for remapped, chorded, and disabled bindings. A clipped reference
//! keeps customization visible; the composer reserves a separate bottom row for the close hint.

use super::footer::FooterProps;
use crate::key_hint;
use crate::key_hint::KeyBinding;
use crate::key_hint::ShortcutHint;
use crate::multi_agents::next_agent_shortcut;
use crate::multi_agents::previous_agent_shortcut;
use crate::shortcut_help::Group;
use crate::shortcut_help::Shortcut;
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
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

struct Entry {
    keys: [Option<ShortcutHint>; 2],
    action: &'static str,
    repeat_label: bool,
}

impl Entry {
    fn hint(key: Option<ShortcutHint>, action: &'static str) -> Self {
        Self {
            keys: [key, None],
            action,
            repeat_label: false,
        }
    }

    fn key(key: KeyBinding, action: &'static str) -> Self {
        Self::hint(Some(key.into()), action)
    }

    fn pair(action: &'static str, keys: [KeyBinding; 2]) -> Self {
        Self {
            keys: keys.map(|key| Some(key.into())),
            action,
            repeat_label: false,
        }
    }
}

fn bindings(props: &FooterProps) -> [(&'static str, Vec<Entry>); 3] {
    use KeyCode as K;
    let hints = props.key_hints;
    let paste = if props.is_wsl {
        key_hint::ctrl_alt(K::Char('v'))
    } else {
        key_hint::ctrl(K::Char('v'))
    };
    let compose = vec![
        Entry::key(key_hint::plain(K::Char('/')), "Commands"),
        Entry::key(key_hint::plain(K::Char('@')), "Mention files"),
        Entry::key(key_hint::plain(K::Char('!')), "Shell command"),
        Entry::hint(hints.insert_newline, "New line"),
        Entry::key(paste, "Paste image"),
        Entry::hint(hints.external_editor, "External editor"),
        Entry::hint(hints.history_search, "Search history"),
        Entry {
            keys: [hints.edit_previous, None],
            action: "Edit last message",
            repeat_label: !props.esc_backtrack_hint,
        },
    ];

    let session = vec![
        Entry::hint(
            hints.queue,
            if props.is_task_running || props.queue_submissions {
                "Queue message"
            } else {
                "Send message"
            },
        ),
        Entry::hint(
            props
                .collaboration_modes_enabled
                .then_some(key_hint::shift(K::Tab).into()),
            "Change mode",
        ),
        Entry::hint(hints.reasoning_down, "Less reasoning"),
        Entry::hint(hints.reasoning_up, "More reasoning"),
        Entry::hint(hints.toggle_voice, "Voice"),
        Entry::hint(hints.agents, "Agents (empty prompt)"),
        Entry::pair(
            "Subagents (empty prompt)",
            [previous_agent_shortcut(), next_agent_shortcut()],
        ),
        Entry::hint(hints.focus_activity, "Inspect activity"),
        Entry::key(
            key_hint::ctrl(K::Char('c')),
            if props.is_task_running {
                "Interrupt"
            } else {
                "Quit"
            },
        ),
    ];

    let alt_shift = KeyModifiers::ALT | KeyModifiers::SHIFT;
    let transcript = vec![
        Entry::hint(hints.show_transcript, "Open transcript"),
        Entry::hint(hints.find_transcript, "Find text"),
        Entry::pair(
            "Scroll",
            [key_hint::plain(K::PageUp), key_hint::plain(K::PageDown)],
        ),
        Entry::key(key_hint::ctrl(K::Char(' ')), "Start selection"),
        Entry::pair(
            "Top / latest",
            [key_hint::ctrl(K::Home), key_hint::ctrl(K::End)],
        ),
        // Keep both jump alternatives: the terminal may be on another OS over SSH.
        Entry::pair(
            "Top / latest",
            [
                KeyBinding::new(K::Char(','), alt_shift),
                KeyBinding::new(K::Char('.'), alt_shift),
            ],
        ),
    ];

    [
        ("Compose", compose),
        ("Session", session),
        ("Transcript (open first)", transcript),
    ]
}

pub(super) fn lines(props: &FooterProps, width: u16) -> Vec<Line<'static>> {
    let groups = bindings(props).map(|(title, entries)| {
        let entries = entries
            .into_iter()
            .filter_map(|entry| {
                let mut keys = entry
                    .keys
                    .into_iter()
                    .flatten()
                    .map(ShortcutHint::display_label);
                let key = keys.next()?;
                Some(Shortcut {
                    key: if entry.repeat_label {
                        format!("{key} {key}")
                    } else if let Some(alternative) = keys.next() {
                        format!("{key} / {alternative}")
                    } else {
                        key
                    },
                    action: entry.action,
                })
            })
            .collect();
        Group { title, entries }
    });
    let mut result = vec![Line::from("Keyboard shortcuts").bold(), Line::default()];
    result.extend(crate::shortcut_help::group_lines(groups, width));
    let width = usize::from(width.max(/*other*/ 1));
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
