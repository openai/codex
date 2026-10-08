//! Resolves and validates the shared shortcut prefix without assigning action bindings.
//! The default yields to explicit shortcuts; active consumers determine whether a menu opens.

use super::MAIN_RESERVED_BINDINGS;
use super::chords::RuntimeChordKeymap;
use super::chords::normalize_chord_binding;
use super::configured_context_alias_is_used;
use super::resolve_bindings;
use crate::key_hint;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyCode;
use itertools::Itertools;

pub(super) fn resolve(config: &TuiKeymap, chords: &mut RuntimeChordKeymap) -> Result<(), String> {
    let default = key_hint::ctrl(KeyCode::Char('x'));
    let shadowed = config.global.leader.is_none()
        && (configured_context_alias_is_used(config, "ctrl-x")
            || chords
                .bindings
                .iter()
                .any(|binding| binding.chord.prefix == default));
    if config.global.leader.as_ref().is_some_and(|bindings| {
        bindings
            .specs()
            .iter()
            .any(|spec| spec.as_str().contains(' '))
    }) {
        return Err("Invalid `tui.keymap.global.leader`: use a single prefix key.".into());
    }
    let defaults = [default];
    chords.leader = resolve_bindings(
        config.global.leader.as_ref(),
        if shadowed { &[] } else { &defaults },
        "tui.keymap.global.leader",
    )?
    .into_iter()
    .map(normalize_chord_binding)
    .unique()
    .collect();

    for prefix in &chords.leader {
        let (key, modifiers) = prefix.parts();
        let label = prefix.display_label();
        if matches!(key, KeyCode::Char(_))
            && (!key_hint::has_ctrl_or_alt(modifiers) || key_hint::is_altgr(modifiers))
        {
            return Err(format!(
                "Invalid `tui.keymap.global.leader` = `{label}`: use ctrl, alt, or a non-character \
key so ordinary text input is not intercepted. Ctrl-alt characters may be AltGr input on Windows."
            ));
        }
        #[cfg(unix)]
        if *prefix == key_hint::ctrl(KeyCode::Char('z')) {
            return Err(format!(
                "Invalid `tui.keymap.global.leader` = `{label}`: ctrl-z is reserved for suspending \
the terminal on Unix."
            ));
        }
        if let Some((reserved_action, _)) = MAIN_RESERVED_BINDINGS
            .iter()
            .find(|(_, reserved)| prefix.parts() == reserved.parts())
        {
            return Err(format!(
                "Invalid `tui.keymap.global.leader` = `{label}`: this key is reserved by \
`{reserved_action}`."
            ));
        }
    }
    Ok(())
}
