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
/// The binding stores the terminal key code plus the exact modifier set that
/// must be present on an incoming press or repeat event. It does not model
/// multi-key chords or partial matches; callers that need sequences must keep
/// that state outside this type.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct KeyBinding {
    key: KeyCode,
    modifiers: KeyModifiers,
}

impl KeyBinding {
    pub(crate) const fn new(key: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { key, modifiers }
    }

    pub fn is_press(&self, event: KeyEvent) -> bool {
        normalize_shifted_ascii_char(self.key, self.modifiers)
            == normalize_shifted_ascii_char(event.code, event.modifiers)
            && (event.kind == KeyEventKind::Press || event.kind == KeyEventKind::Repeat)
    }

    pub(crate) const fn parts(&self) -> (KeyCode, KeyModifiers) {
        (self.key, self.modifiers)
    }
}

fn normalize_shifted_ascii_char(
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
    match ch {
        '\u{0002}' => Some('b'),
        '\u{0006}' => Some('f'),
        '\u{000e}' => Some('n'),
        '\u{0010}' => Some('p'),
        '\u{0012}' => Some('r'),
        '\u{0013}' => Some('s'),
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
        let KeyBinding { key, modifiers } = binding;
        let modifiers = modifiers_to_string(*modifiers);
        let key = match key {
            KeyCode::Enter => "enter".to_string(),
            KeyCode::Char(' ') => "space".to_string(),
            KeyCode::Up => "↑".to_string(),
            KeyCode::Down => "↓".to_string(),
            KeyCode::Left => "←".to_string(),
            KeyCode::Right => "→".to_string(),
            KeyCode::PageUp => "pgup".to_string(),
            KeyCode::PageDown => "pgdn".to_string(),
            _ => format!("{key}").to_ascii_lowercase(),
        };
        Span::styled(format!("{modifiers}{key}"), key_hint_style())
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

        assert!(binding.is_press(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::NONE)));
        assert!(binding.is_press(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT)));
    }

    #[test]
    fn ctrl_letter_binding_matches_c0_control_char_events() {
        let binding = ctrl(KeyCode::Char('p'));

        assert!(binding.is_press(KeyEvent::new(KeyCode::Char('\u{0010}'), KeyModifiers::NONE)));
        assert!(!binding.is_press(KeyEvent::new(KeyCode::Char('\u{0010}'), KeyModifiers::ALT)));
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
