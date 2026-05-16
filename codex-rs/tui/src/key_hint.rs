//! Key binding primitives and input matching for the TUI.
//!
//! This module provides `KeyBinding`, the runtime representation of a single
//! keybinding (key code + modifier set), along with matching logic that handles
//! cross-terminal inconsistencies in how shifted letters and raw C0 control
//! characters are reported.
//!
//! List and picker code should match navigation through these helpers instead
//! of comparing `KeyEvent` values directly. The matcher owns compatibility for
//! terminals that report control chords as C0 characters, while
//! `is_plain_text_key_event` gives searchable pickers a shared boundary between
//! text input and navigation commands.
//!
//! It also supplies rendering helpers that convert bindings into styled
//! `ratatui::text::Span` values for UI hint display.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Span;

#[cfg(test)]
const ALT_PREFIX: &str = "⌥ + ";
#[cfg(all(not(test), target_os = "macos"))]
const ALT_PREFIX: &str = "⌥ + ";
#[cfg(all(not(test), not(target_os = "macos")))]
const ALT_PREFIX: &str = "alt + ";
const CTRL_PREFIX: &str = "ctrl + ";
const SHIFT_PREFIX: &str = "shift + ";

/// One concrete key event that can trigger a TUI action.
///
/// Matching via `is_press` handles exact equality plus compatibility fallbacks
/// for terminals that report uppercase letters without SHIFT and Ctrl keys as
/// raw C0 control characters. This means a binding defined as `shift-a` will
/// match either `Shift+a` or plain `A`, and `ctrl-j` will match raw LF.
///
/// This does not model multi-key chords or partial matches; callers that need
/// sequences must keep that state outside this type.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct KeyBinding {
    key: KeyCode,
    modifiers: KeyModifiers,
}

impl KeyBinding {
    pub(crate) const fn new(key: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { key, modifiers }
    }

    pub(crate) fn from_event(event: KeyEvent) -> Self {
        let (key, modifiers) = normalize_key_parts(event.code, event.modifiers);
        Self { key, modifiers }
    }

    pub fn is_press(&self, event: KeyEvent) -> bool {
        normalize_key_parts(self.key, self.modifiers)
            == normalize_key_parts(event.code, event.modifiers)
            && (event.kind == KeyEventKind::Press || event.kind == KeyEventKind::Repeat)
    }

    pub(crate) const fn parts(&self) -> (KeyCode, KeyModifiers) {
        (self.key, self.modifiers)
    }

    pub(crate) fn display_label(&self) -> String {
        let modifiers = modifiers_to_string(self.modifiers);
        let key = match self.key {
            KeyCode::Enter => "enter".to_string(),
            KeyCode::Char(' ') => "space".to_string(),
            KeyCode::Up => "↑".to_string(),
            KeyCode::Down => "↓".to_string(),
            KeyCode::Left => "←".to_string(),
            KeyCode::Right => "→".to_string(),
            KeyCode::PageUp => "pgup".to_string(),
            KeyCode::PageDown => "pgdn".to_string(),
            _ => self.key.to_string().to_ascii_lowercase(),
        };
        format!("{modifiers}{key}")
    }
}

pub(crate) fn normalize_key_parts(
    key: KeyCode,
    mut modifiers: KeyModifiers,
) -> (KeyCode, KeyModifiers) {
    let KeyCode::Char(ch) = key else {
        return (key, modifiers);
    };
    if modifiers.is_empty()
        && let Some(ctrl_char) = c0_control_char_to_ctrl_char(ch)
    {
        return (KeyCode::Char(ctrl_char), KeyModifiers::CONTROL | modifiers);
    }
    if ch.is_ascii_uppercase() {
        modifiers.insert(KeyModifiers::SHIFT);
        return (KeyCode::Char(ch.to_ascii_lowercase()), modifiers);
    }
    (key, modifiers)
}

fn c0_control_char_to_ctrl_char(ch: char) -> Option<char> {
    let code = u32::from(ch);
    match code {
        0x00 => Some(' '),
        0x01..=0x1a => char::from_u32(code - 0x01 + u32::from('a')),
        0x1c..=0x1f => char::from_u32(code - 0x1c + u32::from('4')),
        _ => None,
    }
}

/// Matching helpers for one action's keybinding set.
///
/// Implementations are expected to treat the slice as alternatives for one
/// action. They should not interpret order as priority for dispatch; order is
/// reserved for UI hint selection via `primary_binding`.
pub(crate) trait KeyBindingListExt {
    /// True when any binding in this set matches `event`.
    fn is_pressed(&self, event: KeyEvent) -> bool;
}

impl KeyBindingListExt for [KeyBinding] {
    fn is_pressed(&self, event: KeyEvent) -> bool {
        self.iter().any(|binding| binding.is_press(event))
    }
}

/// Returns whether an event should be treated as literal text input.
///
/// Searchable pickers use this to avoid stealing plain printable characters for
/// navigation when the same character might be a valid query. For example, a
/// list may bind `j` and `k` for movement, but a searchable list must let
/// plain `j` update the query while still allowing `Ctrl+J` to move. Calling
/// this after normalizing keybindings would blur that distinction and cause
/// printable search input to disappear.
pub(crate) fn is_plain_text_key_event(event: KeyEvent) -> bool {
    matches!(
        event,
        KeyEvent {
            code: KeyCode::Char(ch),
            modifiers,
            ..
        } if !ch.is_ascii_control()
            && !modifiers.contains(KeyModifiers::CONTROL)
            && !modifiers.contains(KeyModifiers::ALT)
    )
}

pub(crate) const fn plain(key: KeyCode) -> KeyBinding {
    KeyBinding::new(key, KeyModifiers::NONE)
}

pub(crate) const fn alt(key: KeyCode) -> KeyBinding {
    KeyBinding::new(key, KeyModifiers::ALT)
}

pub(crate) const fn shift(key: KeyCode) -> KeyBinding {
    KeyBinding::new(key, KeyModifiers::SHIFT)
}

pub(crate) const fn ctrl(key: KeyCode) -> KeyBinding {
    KeyBinding::new(key, KeyModifiers::CONTROL)
}

pub(crate) const fn ctrl_alt(key: KeyCode) -> KeyBinding {
    KeyBinding::new(key, KeyModifiers::CONTROL.union(KeyModifiers::ALT))
}

fn modifiers_to_string(modifiers: KeyModifiers) -> String {
    let mut result = String::new();
    if modifiers.contains(KeyModifiers::CONTROL) {
        result.push_str(CTRL_PREFIX);
    }
    if modifiers.contains(KeyModifiers::SHIFT) {
        result.push_str(SHIFT_PREFIX);
    }
    if modifiers.contains(KeyModifiers::ALT) {
        result.push_str(ALT_PREFIX);
    }
    result
}

impl From<KeyBinding> for Span<'static> {
    fn from(binding: KeyBinding) -> Self {
        (&binding).into()
    }
}
impl From<&KeyBinding> for Span<'static> {
    fn from(binding: &KeyBinding) -> Self {
        Span::styled(binding.display_label(), key_hint_style())
    }
}

fn key_hint_style() -> Style {
    Style::default().dim()
}

pub(crate) fn has_ctrl_or_alt(mods: KeyModifiers) -> bool {
    (mods.contains(KeyModifiers::CONTROL) || mods.contains(KeyModifiers::ALT)) && !is_altgr(mods)
}

#[cfg(windows)]
#[inline]
pub(crate) fn is_altgr(mods: KeyModifiers) -> bool {
    mods.contains(KeyModifiers::ALT) && mods.contains(KeyModifiers::CONTROL)
}

#[cfg(not(windows))]
#[inline]
pub(crate) fn is_altgr(_mods: KeyModifiers) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_press_accepts_press_and_repeat_but_rejects_release() {
        let binding = ctrl(KeyCode::Char('k'));
        let press = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL);
        let repeat = KeyEvent {
            kind: KeyEventKind::Repeat,
            ..press
        };
        let release = KeyEvent {
            kind: KeyEventKind::Release,
            ..press
        };
        let wrong_modifiers = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);

        assert!(binding.is_press(press));
        assert!(binding.is_press(repeat));
        assert!(!binding.is_press(release));
        assert!(!binding.is_press(wrong_modifiers));
    }

    #[test]
    fn keybinding_list_ext_matches_any_binding() {
        let bindings = [plain(KeyCode::Char('a')), ctrl(KeyCode::Char('b'))];

        assert!(bindings.is_pressed(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)));
        assert!(bindings.is_pressed(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL)));
        assert!(!bindings.is_pressed(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)));
    }

    #[test]
    fn shifted_letter_binding_matches_uppercase_char_events() {
        let binding = shift(KeyCode::Char('a'));

        assert!(binding.is_press(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::SHIFT)));
        assert!(binding.is_press(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::NONE)));
        assert!(binding.is_press(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT)));
    }

    #[test]
    fn shift_letter_binding_preserves_other_modifiers_with_uppercase_compat() {
        let binding = KeyBinding::new(
            KeyCode::Char('i'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );

        assert!(binding.is_press(KeyEvent::new(KeyCode::Char('I'), KeyModifiers::CONTROL)));
    }

    #[test]
    fn shift_letter_binding_does_not_match_plain_lowercase_or_other_uppercase() {
        let binding = shift(KeyCode::Char('o'));

        assert!(!binding.is_press(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE)));
        assert!(!binding.is_press(KeyEvent::new(KeyCode::Char('P'), KeyModifiers::NONE)));
    }

    #[test]
    fn ctrl_letter_binding_matches_c0_control_char_events() {
        let binding = ctrl(KeyCode::Char('p'));

        assert!(binding.is_press(KeyEvent::new(KeyCode::Char('\u{0010}'), KeyModifiers::NONE)));
        assert!(!binding.is_press(KeyEvent::new(KeyCode::Char('\u{0010}'), KeyModifiers::ALT)));
    }

    #[test]
    fn ctrl_bindings_match_all_supported_c0_control_char_events() {
        let cases = [
            (' ', '\u{0000}'),
            ('a', '\u{0001}'),
            ('b', '\u{0002}'),
            ('c', '\u{0003}'),
            ('d', '\u{0004}'),
            ('e', '\u{0005}'),
            ('f', '\u{0006}'),
            ('g', '\u{0007}'),
            ('h', '\u{0008}'),
            ('i', '\u{0009}'),
            ('j', '\u{000a}'),
            ('k', '\u{000b}'),
            ('l', '\u{000c}'),
            ('m', '\u{000d}'),
            ('n', '\u{000e}'),
            ('o', '\u{000f}'),
            ('p', '\u{0010}'),
            ('q', '\u{0011}'),
            ('r', '\u{0012}'),
            ('s', '\u{0013}'),
            ('t', '\u{0014}'),
            ('u', '\u{0015}'),
            ('v', '\u{0016}'),
            ('w', '\u{0017}'),
            ('x', '\u{0018}'),
            ('y', '\u{0019}'),
            ('z', '\u{001a}'),
            ('4', '\u{001c}'),
            ('5', '\u{001d}'),
            ('6', '\u{001e}'),
            ('7', '\u{001f}'),
        ];

        for (ctrl_char, c0_char) in cases {
            assert!(
                ctrl(KeyCode::Char(ctrl_char))
                    .is_press(KeyEvent::new(KeyCode::Char(c0_char), KeyModifiers::NONE)),
                "expected raw C0 {c0_char:?} to match ctrl-{ctrl_char}"
            );
            assert!(
                !ctrl(KeyCode::Char(ctrl_char))
                    .is_press(KeyEvent::new(KeyCode::Char(c0_char), KeyModifiers::ALT)),
                "expected modified raw C0 {c0_char:?} not to match ctrl-{ctrl_char}"
            );
        }
    }

    #[test]
    fn ctrl_binding_does_not_match_ambiguous_c0_escape_or_delete() {
        assert!(
            !ctrl(KeyCode::Char('['))
                .is_press(KeyEvent::new(KeyCode::Char('\u{001b}'), KeyModifiers::NONE,))
        );
        assert!(
            !ctrl(KeyCode::Char('?'))
                .is_press(KeyEvent::new(KeyCode::Char('\u{007f}'), KeyModifiers::NONE,))
        );
    }

    #[test]
    fn history_search_ctrl_bindings_match_c0_control_char_events() {
        assert!(
            ctrl(KeyCode::Char('r'))
                .is_press(KeyEvent::new(KeyCode::Char('\u{0012}'), KeyModifiers::NONE))
        );
        assert!(
            ctrl(KeyCode::Char('s'))
                .is_press(KeyEvent::new(KeyCode::Char('\u{0013}'), KeyModifiers::NONE))
        );
    }

    #[test]
    fn ctrl_alt_sets_both_modifiers() {
        assert_eq!(
            ctrl_alt(KeyCode::Char('v')).parts(),
            (
                KeyCode::Char('v'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            )
        );
    }

    #[test]
    fn has_ctrl_or_alt_checks_supported_modifier_combinations() {
        assert!(!has_ctrl_or_alt(KeyModifiers::NONE));
        assert!(has_ctrl_or_alt(KeyModifiers::CONTROL));
        assert!(has_ctrl_or_alt(KeyModifiers::ALT));

        #[cfg(windows)]
        assert!(!has_ctrl_or_alt(KeyModifiers::CONTROL | KeyModifiers::ALT));
        #[cfg(not(windows))]
        assert!(has_ctrl_or_alt(KeyModifiers::CONTROL | KeyModifiers::ALT));
    }
}
