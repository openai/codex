//! Install implicit preset and selection shortcuts only for free leader chords.
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

pub(in crate::keymap) fn register(keymap: &TuiKeymap, resolved: &mut RuntimeKeymap) {
    let mut remaining = MAX_LEADER_CHORD_BINDINGS.saturating_sub(
        resolved
            .chords
            .bindings
            .iter()
            .filter(|binding| binding.is_leader())
            .count(),
    );
    for (configured, context, action, completion, spec) in [
        (
            keymap.global.select_transcript.as_ref(),
            KeymapContext::Global,
            "select_transcript",
            '[',
            "leader [",
        ),
        (
            keymap.chat.open_presets.as_ref(),
            KeymapContext::Chat,
            "open_presets",
            'p',
            "leader p",
        ),
    ] {
        if remaining == 0 {
            break;
        }
        if configured.is_some() {
            continue;
        }
        let action = KeymapActionId { context, action };
        for prefix in resolved.chords.leader.clone() {
            let binding = RuntimeChordBinding {
                action,
                chord: KeyChord {
                    prefix,
                    completion: crate::key_hint::plain(KeyCode::Char(completion)),
                },
                spec: spec.into(),
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
