//! Install implicit backend, preset, selection, and side-pane shortcuts for free leader chords.
//! Explicit bindings, including an explicit unbind, always take priority.
//! Implicit chords fit within the shared leader expansion limit.

use super::KeyChord;
use super::KeymapActionId;
use super::KeymapContext;
use super::MAX_LEADER_CHORD_BINDINGS;
use super::RuntimeChordBinding;
use super::RuntimeKeymap;
use super::normalize_chord_binding;
use super::runtime_action_bindings;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyCode;

pub(in crate::keymap) fn register(
    keymap: &TuiKeymap,
    resolved: &mut RuntimeKeymap,
    workspace: bool,
) {
    let mut remaining = MAX_LEADER_CHORD_BINDINGS.saturating_sub(
        resolved
            .chords
            .bindings
            .iter()
            .filter(|binding| binding.is_leader())
            .count(),
    );
    use KeyCode::Char;
    use KeymapContext::Chat;
    use KeymapContext::Global;
    for (context, name, completion, spec) in [
        (Global, "change_host", Char('b'), "b"),
        (Global, "select_transcript", Char('['), "["),
        (Chat, "open_presets", Char('p'), "p"),
        (Global, "focus_main_composer", Char('c'), "c"),
        (Global, "focus_sidebar_content", Char('s'), "s"),
        (Global, "hide_pane", Char('-'), "-"),
        (Global, "focus_pane_left", KeyCode::Left, "left"),
        (Global, "focus_pane_left", Char('h'), "h"),
        (Global, "focus_pane_right", KeyCode::Right, "right"),
        (Global, "focus_pane_right", Char('l'), "l"),
        (Global, "toggle_pane_zoom", Char('z'), "z"),
        (Global, "move_divider_left", Char('H'), "shift-h"),
        (Global, "move_divider_right", Char('L'), "shift-l"),
        (Global, "equalize_pane_widths", Char('='), "="),
        (Global, "resize_panes", Char('r'), "r"),
        (Global, "minimize_pane_width", Char(','), ","),
        (Global, "maximize_pane_width", Char('.'), "."),
        (Global, "focus_sidebar_tabs", Char('t'), "t"),
    ] {
        if remaining == 0 {
            break;
        }
        let action = KeymapActionId {
            context,
            action: name,
        };
        if (!workspace && action.is_side_pane())
            || !matches!(
                super::configured_binding_for_action(keymap, action),
                Some(None)
            )
        {
            continue;
        }
        for prefix in resolved.chords.leader.clone() {
            let binding = RuntimeChordBinding {
                action,
                chord: KeyChord {
                    prefix,
                    completion: crate::key_hint::KeyBinding::from_event(completion.into()),
                },
                spec: format!("leader {spec}"),
            };
            if !resolved.chords.bindings.iter().any(|existing| {
                binding.overlaps(existing.action) && existing.chord == binding.chord
            }) && !runtime_action_bindings(resolved)
                // Owned transcript navigation also runs beside the composer.
                .filter(|single| {
                    binding.overlaps(single.id) || single.id.context == KeymapContext::Pager
                })
                .any(|single| {
                    single
                        .bindings
                        .iter()
                        .any(|key| normalize_chord_binding(*key) == prefix)
                })
            {
                std::sync::Arc::make_mut(&mut resolved.chords)
                    .bindings
                    .push(binding);
                remaining -= 1;
                if remaining == 0 {
                    break;
                }
            }
        }
    }
    resolved.chat.chord_hints = std::sync::Arc::clone(&resolved.chords);
}
