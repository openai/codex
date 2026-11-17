//! A shortcut guide built from runtime bindings. It opens as a grouped action list.
//! The optional tenkeyless keyboard highlights keys with unmodified bindings.
//! Keys outside the diagram remain available in the list. If space is too small,
//! the guide shows the full action list.

mod keyboard;

use super::footer::FooterMode;
use super::footer::FooterProps;
use crate::key_hint;
use crate::key_hint::KeyBinding;
use crate::key_hint::ShortcutHint;
use crate::keymap::KeymapContext;
use crate::keymap::RuntimeKeymap;
use crate::keymap::bindings_for_action;
use crate::multi_agents::next_agent_shortcut;
use crate::multi_agents::previous_agent_shortcut;
use crate::shortcut_help::Group;
use crate::shortcut_help::Shortcut;
use crate::style::accent_color;
use crate::style::secondary_text_style;
use crate::wrapping::word_wrap_lines;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::collections::HashSet;

/// The view switch takes the first free key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ShortcutGuideKeys {
    view: Option<KeyCode>,
}

impl ShortcutGuideKeys {
    pub(super) fn from_keymap(keymap: &RuntimeKeymap) -> Self {
        let conflicts = |context: KeymapContext| {
            context == KeymapContext::Pager || context.overlaps(KeymapContext::Composer)
        };
        let view =
            std::iter::once(KeyCode::Tab)
                .chain((1..=12).map(KeyCode::F))
                .find(|&code| {
                    let key = key_hint::plain(code);
                    // The guide opens on an empty draft, so queue has nothing to send.
                    !crate::keymap::keymap_action_ids().any(|id| {
                        conflicts(id.context)
                            && (id.context != KeymapContext::Composer || id.action != "queue")
                            && bindings_for_action(keymap, id.context.config_name(), id.action)
                                .is_some_and(|bindings| bindings.contains(&key))
                    }) && !keymap.chords.bindings.iter().any(|binding| {
                        conflicts(binding.action.context) && binding.chord.prefix == key
                    })
                });
        Self { view }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ShortcutView {
    #[default]
    List,
    Keyboard,
}

impl ShortcutView {
    pub(super) fn handle_key(&mut self, event: KeyEvent, props: &FooterProps) -> bool {
        if !props
            .key_hints
            .shortcut_guide
            .view
            .is_some_and(|key| key_hint::plain(key).is_press(event))
        {
            return false;
        }
        if event.kind == KeyEventKind::Press {
            *self = match self {
                Self::List => Self::Keyboard,
                Self::Keyboard => Self::List,
            };
        }
        true
    }
}

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

// Map runtime shortcuts to US QWERTY key positions. Chords use their prefix.
fn key_position(hint: ShortcutHint) -> (KeyCode, KeyModifiers) {
    let binding = match hint {
        ShortcutHint::Single(key) | ShortcutHint::Chord { prefix: key, .. } => key,
    };
    let (code, modifiers) = binding.parts();
    let (code, modifiers) = key_hint::normalize_key_parts(code, modifiers);
    match code {
        KeyCode::BackTab => (KeyCode::Tab, modifiers | KeyModifiers::SHIFT),
        KeyCode::Char(ch) => keyboard::unshift(ch).map_or((code, modifiers), |base| {
            (KeyCode::Char(base), modifiers | KeyModifiers::SHIFT)
        }),
        _ => (code, modifiers),
    }
}

pub(super) fn keyboard_fits_width(width: u16) -> bool {
    width >= keyboard::WIDTH
}

pub(super) fn lines(props: &FooterProps, width: u16) -> Vec<Line<'static>> {
    guide_lines(props, width, usize::MAX)
}

fn guide_lines(props: &FooterProps, width: u16, height: usize) -> Vec<Line<'static>> {
    let FooterMode::ShortcutOverlay(view) = props.mode else {
        return Vec::new();
    };
    let width_usize = usize::from(width.max(1));
    let heading = Line::from("Keyboard shortcuts".bold());
    let footer = footer_lines(width_usize);
    let groups = bindings(props);
    let keys = props.key_hints.shortcut_guide;

    if view == ShortcutView::Keyboard && keyboard_fits_width(width) {
        let mut actions: HashSet<_> = groups
            .iter()
            .flat_map(|(_, entries)| entries)
            .flat_map(|entry| entry.keys.into_iter().flatten())
            .map(key_position)
            .filter_map(|(code, modifiers)| modifiers.is_empty().then_some(code))
            .collect();
        let empty = actions.is_empty();
        let diagram = keyboard::lines(&mut actions, width);
        let mut title = heading.clone();
        title.extend([Span::raw("  "), "Base".fg(accent_color())]);
        if !actions.is_empty() {
            title.push_span(" · More keys in list".set_style(secondary_text_style()));
        }
        title.extend(
            keys.view
                .map(|key| format!("  ·  {key}: List").set_style(secondary_text_style())),
        );
        let mut result = word_wrap_lines([title], width_usize);
        result.extend(diagram);
        if empty {
            result.push(Line::from(
                "No shortcuts in this layer".set_style(secondary_text_style()),
            ));
        }
        if result.len() + footer.len() <= height {
            result.extend(footer);
            return result;
        }
    }

    let groups = groups.map(|(title, entries)| {
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
    let mut heading = heading;
    match view {
        ShortcutView::List if keyboard_fits_width(width) => {
            heading.extend(
                keys.view
                    .map(|key| format!("  ·  {key}: Keyboard").set_style(secondary_text_style())),
            );
        }
        ShortcutView::Keyboard => {
            heading.push_span("  ·  Keyboard unavailable".set_style(secondary_text_style()));
            heading.extend(
                keys.view
                    .map(|key| format!("  ·  {key}: List").set_style(secondary_text_style())),
            );
        }
        ShortcutView::List => {}
    }
    let mut result = word_wrap_lines([heading], width_usize);
    result.push(Line::default());
    result.extend(word_wrap_lines(
        crate::shortcut_help::group_lines(groups, width),
        width_usize,
    ));
    result.push(Line::default());
    result.extend(footer);
    result
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
    let mut lines = guide_lines(props, area.width, usize::from(area.height));
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
