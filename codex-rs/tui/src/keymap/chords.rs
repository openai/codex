//! Bounded, context-aware two-stroke shortcuts for TUI actions.
//!
//! Single-event bindings remain in the ordinary runtime keymaps. A completed
//! chord becomes an internal function-key token appended to the target action,
//! so existing handlers remain the only action dispatch table.

use super::MAIN_RESERVED_BINDINGS;
use super::RuntimeKeymap;
use super::TRANSCRIPT_BACKTRACK_RESERVED_BINDINGS;
use super::bindings::KeymapActionId;
use super::bindings::KeymapContext;
use super::bindings::configured_binding_for_action;
use super::bindings::keymap_action_ids;
use super::bindings::push_binding_for_action;
use super::parse_keybinding;
use super::runtime_action_bindings;
use crate::key_hint::KeyBinding;
use crate::key_hint::ctrl;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use std::time::Duration;
use tokio::time::Instant;

const FIRST_DISPATCH_FUNCTION_KEY: u8 = 128;
const LAST_DISPATCH_FUNCTION_KEY: u8 = u8::MAX;
const LIST_RESERVED_BINDINGS: &[(&str, KeyBinding)] = &[
    ("cancel", ctrl(KeyCode::Char('c'))),
    ("resume_picker.toggle_transcript", ctrl(KeyCode::Char('t'))),
    ("resume_picker.toggle_expansion", ctrl(KeyCode::Char('e'))),
    ("resume_picker.toggle_density", ctrl(KeyCode::Char('o'))),
];

/// Time allowed between the first and second strokes of a key chord.
pub(crate) const KEY_CHORD_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 1);

/// Compact set of keymap contexts that share one active input path.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct KeymapContextSet(u16);

impl KeymapContextSet {
    pub(crate) const fn new(context: KeymapContext) -> Self {
        Self(context_bit(context))
    }

    pub(crate) const fn with(self, context: KeymapContext) -> Self {
        Self(self.0 | context_bit(context))
    }

    pub(crate) const fn contains(self, context: KeymapContext) -> bool {
        self.0 & context_bit(context) != 0
    }
}

const fn context_bit(context: KeymapContext) -> u16 {
    1 << match context {
        KeymapContext::Global => 0,
        KeymapContext::Chat => 1,
        KeymapContext::Composer => 2,
        KeymapContext::Editor => 3,
        KeymapContext::VimNormal => 4,
        KeymapContext::VimOperator => 5,
        KeymapContext::VimTextObject => 6,
        KeymapContext::Pager => 7,
        KeymapContext::List => 8,
        KeymapContext::Approval => 9,
        KeymapContext::Agents => 10,
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct KeyChord {
    pub(crate) prefix: KeyBinding,
    pub(crate) completion: KeyBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeChordBinding {
    pub(crate) action: KeymapActionId,
    pub(crate) chord: KeyChord,
    spec: String,
}

/// Chord bindings plus configured alternatives in declaration order.
#[derive(Clone, Debug, Default)]
pub(crate) struct RuntimeChordKeymap {
    pub(crate) bindings: Vec<RuntimeChordBinding>,
    configured_specs: Vec<(KeymapActionId, Vec<String>)>,
}

impl RuntimeChordKeymap {
    pub(super) fn from_config(keymap: &TuiKeymap) -> Result<Self, String> {
        let mut keymap_chords = Self::default();
        for action in keymap_action_ids() {
            let Some(configured) = effective_configured_binding(keymap, action) else {
                continue;
            };
            let mut configured_specs = Vec::new();
            for spec in configured.specs() {
                let raw = spec.as_str();
                if configured_specs.iter().any(|configured| configured == raw) {
                    continue;
                }
                if let Some((prefix, completion)) = raw.split_once(' ') {
                    let invalid_binding = || {
                        format!(
                            "Invalid `{}` = `{raw}`. Use a single key such as `ctrl-a` \
or a two-stroke chord such as `ctrl-x ctrl-t`.",
                            action.config_path()
                        )
                    };
                    keymap_chords.bindings.push(RuntimeChordBinding {
                        action,
                        chord: KeyChord {
                            prefix: normalize_chord_binding(
                                parse_keybinding(prefix).ok_or_else(invalid_binding)?,
                            ),
                            completion: normalize_chord_binding(
                                parse_keybinding(completion).ok_or_else(invalid_binding)?,
                            ),
                        },
                        spec: raw.to_string(),
                    });
                }
                configured_specs.push(raw.to_string());
            }
            keymap_chords
                .configured_specs
                .push((action, configured_specs));
        }
        Ok(keymap_chords)
    }

    pub(crate) fn configured_specs(&self, action: KeymapActionId) -> Option<&[String]> {
        self.configured_specs
            .iter()
            .find_map(|(configured_action, specs)| {
                (*configured_action == action).then_some(specs.as_slice())
            })
    }

    /// Return the user's first configured shortcut without exposing dispatch tokens.
    pub(crate) fn primary_hint(
        &self,
        action: KeymapActionId,
        bindings: &[KeyBinding],
    ) -> Option<crate::key_hint::ShortcutHint> {
        if let Some(spec) = self
            .configured_specs(action)
            .and_then(|specs| specs.first())
        {
            return if let Some((prefix, completion)) = spec.split_once(' ') {
                Some(crate::key_hint::ShortcutHint::Chord {
                    prefix: parse_keybinding(prefix)?,
                    completion: parse_keybinding(completion)?,
                })
            } else {
                parse_keybinding(spec).map(crate::key_hint::ShortcutHint::Single)
            };
        }

        super::primary_binding(bindings).map(crate::key_hint::ShortcutHint::Single)
    }
}

/// Outcome of routing one physical key through the chord state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KeyChordMatch {
    PassThrough,
    Pending(KeyBinding),
    Completed(KeyEvent),
    Cancelled,
    Ignored,
}

#[derive(Clone, Copy, Debug)]
struct PendingChord {
    prefix: KeyBinding,
    contexts: KeymapContextSet,
    started_at: Instant,
}

/// Tracks one pending two-stroke chord without buffering ordinary input.
#[derive(Debug, Default)]
pub(crate) struct KeyChordMatcher {
    pending: Option<PendingChord>,
}

impl KeyChordMatcher {
    pub(crate) fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(crate) fn cancel(&mut self) -> bool {
        self.pending.take().is_some()
    }

    pub(crate) fn expire(&mut self, contexts: KeymapContextSet, now: Instant) -> bool {
        if self.pending.is_some_and(|pending| {
            pending.contexts != contexts
                || now.saturating_duration_since(pending.started_at) >= KEY_CHORD_TIMEOUT
        }) {
            self.pending = None;
            return true;
        }
        false
    }

    pub(crate) fn advance(
        &mut self,
        key_event: KeyEvent,
        keymap: &RuntimeChordKeymap,
        contexts: KeymapContextSet,
        now: Instant,
    ) -> KeyChordMatch {
        if is_dispatch_token_event(key_event) {
            return KeyChordMatch::Ignored;
        }

        self.expire(contexts, now);

        if self.pending.is_some() && key_event.kind != KeyEventKind::Press {
            return KeyChordMatch::Ignored;
        }
        if key_event.kind != KeyEventKind::Press {
            return KeyChordMatch::PassThrough;
        }

        if let Some(pending) = self.pending.take() {
            if crate::key_hint::plain(KeyCode::Esc).is_press(key_event) {
                return KeyChordMatch::Cancelled;
            }
            if let Some(binding) = keymap.bindings.iter().find(|binding| {
                contexts.contains(binding.action.context)
                    && binding.chord.prefix == pending.prefix
                    && chord_stroke_matches(binding.chord.completion, key_event)
            }) {
                let Some(dispatch_event) = dispatch_event(binding.action) else {
                    return KeyChordMatch::Ignored;
                };
                return KeyChordMatch::Completed(dispatch_event);
            }
        }

        if let Some(prefix) = keymap
            .bindings
            .iter()
            .find(|binding| {
                contexts.contains(binding.action.context)
                    && chord_stroke_matches(binding.chord.prefix, key_event)
            })
            .map(|binding| binding.chord.prefix)
        {
            self.pending = Some(PendingChord {
                prefix,
                contexts,
                started_at: now,
            });
            return KeyChordMatch::Pending(prefix);
        }

        KeyChordMatch::PassThrough
    }
}

pub(super) fn install_dispatch_bindings(keymap: &mut RuntimeKeymap) -> Result<(), String> {
    let actions = keymap
        .chords
        .bindings
        .iter()
        .map(|binding| binding.action)
        .collect::<std::collections::HashSet<_>>();
    for action in actions {
        let binding = dispatch_binding(action).ok_or_else(|| {
            format!(
                "Cannot dispatch `{}`: the keymap action inventory exceeds {} internal tokens.",
                action.config_path(),
                LAST_DISPATCH_FUNCTION_KEY - FIRST_DISPATCH_FUNCTION_KEY + 1,
            )
        })?;
        if !push_binding_for_action(keymap, action, binding) {
            return Err(format!(
                "Cannot dispatch unknown keymap action `{}`.",
                action.config_path()
            ));
        }
    }
    Ok(())
}

fn dispatch_binding(action: KeymapActionId) -> Option<KeyBinding> {
    let index = keymap_action_ids().position(|candidate| candidate == action)?;
    let function_key = FIRST_DISPATCH_FUNCTION_KEY.checked_add(u8::try_from(index).ok()?)?;
    Some(KeyBinding::new(
        KeyCode::F(function_key),
        KeyModifiers::NONE,
    ))
}

fn dispatch_event(action: KeymapActionId) -> Option<KeyEvent> {
    let (code, modifiers) = dispatch_binding(action)?.parts();
    Some(KeyEvent::new(code, modifiers))
}

pub(crate) fn is_dispatch_token(binding: KeyBinding) -> bool {
    matches!(
        binding.parts(),
        (
            KeyCode::F(FIRST_DISPATCH_FUNCTION_KEY..=LAST_DISPATCH_FUNCTION_KEY),
            _
        )
    )
}

fn is_dispatch_token_event(event: KeyEvent) -> bool {
    matches!(
        event.code,
        KeyCode::F(FIRST_DISPATCH_FUNCTION_KEY..=LAST_DISPATCH_FUNCTION_KEY)
    )
}

fn chord_stroke_matches(binding: KeyBinding, key_event: KeyEvent) -> bool {
    let ctrl_slash = crate::key_hint::ctrl(KeyCode::Char('/'));
    let ctrl_seven = crate::key_hint::ctrl(KeyCode::Char('7'));

    binding.is_press(key_event)
        || binding.parts() == ctrl_slash.parts() && ctrl_seven.is_press(key_event)
        || binding.parts() == ctrl_seven.parts() && ctrl_slash.is_press(key_event)
}

fn effective_configured_binding(
    keymap: &TuiKeymap,
    action: KeymapActionId,
) -> Option<&KeybindingsSpec> {
    if let Some(configured) = configured_binding_for_action(keymap, action)?.as_ref() {
        return Some(configured);
    }
    if action.context != KeymapContext::Composer {
        return None;
    }
    match action.action {
        "submit" => keymap.global.submit.as_ref(),
        "queue" => keymap.global.queue.as_ref(),
        "toggle_shortcuts" => keymap.global.toggle_shortcuts.as_ref(),
        _ => None,
    }
}

pub(crate) fn normalize_chord_binding(binding: KeyBinding) -> KeyBinding {
    if binding.parts() == crate::key_hint::ctrl(KeyCode::Char('7')).parts() {
        crate::key_hint::ctrl(KeyCode::Char('/'))
    } else {
        binding
    }
}

/// Reject chords that would shadow an active single binding or fixed shortcut.
pub(super) fn validate_chord_conflicts(keymap: &RuntimeKeymap) -> Result<(), String> {
    for (index, binding) in keymap.chords.bindings.iter().enumerate() {
        validate_binding_shape(binding)?;
        validate_reserved_strokes(binding)?;

        if let Some(conflict) = runtime_action_bindings(keymap)
            .filter(|candidate| binding.action.context.overlaps(candidate.id.context))
            .find(|candidate| {
                candidate.bindings.iter().any(|single| {
                    normalize_chord_binding(*single).parts() == binding.chord.prefix.parts()
                })
            })
        {
            return Err(format!(
                "Ambiguous `{}` = `{}`: its prefix shadows `{}`. \
Unbind or remap the existing shortcut before using it as a chord prefix.",
                binding.action.config_path(),
                binding.spec,
                conflict.id.config_path(),
            ));
        }

        for previous in &keymap.chords.bindings[..index] {
            if previous.action != binding.action
                && previous.action.context.overlaps(binding.action.context)
                && previous.chord == binding.chord
            {
                return Err(format!(
                    "Ambiguous `{}` = `{}`: the same chord is already assigned to `{}`. \
Choose a unique chord and retry.",
                    binding.action.config_path(),
                    binding.spec,
                    previous.action.config_path(),
                ));
            }
        }
    }

    Ok(())
}

fn validate_binding_shape(binding: &RuntimeChordBinding) -> Result<(), String> {
    let path = binding.action.config_path();
    let (prefix_key, prefix_modifiers) = binding.chord.prefix.parts();

    if matches!(prefix_key, KeyCode::Char(_))
        && crate::key_hint::is_altgr(prefix_modifiers)
        && !binding.action.context.allows_plain_chord_prefix()
    {
        return Err(format!(
            "Invalid `{path}` = `{}`: a ctrl-alt character prefix may be AltGr text input on \
Windows. Choose a different chord and retry.",
            binding.spec
        ));
    }

    if matches!(prefix_key, KeyCode::Char(_))
        && !crate::key_hint::has_ctrl_or_alt(prefix_modifiers)
        && !binding.action.context.allows_plain_chord_prefix()
    {
        return Err(format!(
            "Invalid `{path}` = `{}`: a chord prefix outside Vim must use ctrl, \
alt, or a non-character key so ordinary text input is not intercepted.",
            binding.spec
        ));
    }

    Ok(())
}

fn validate_reserved_strokes(binding: &RuntimeChordBinding) -> Result<(), String> {
    let path = binding.action.config_path();
    let strokes = [
        binding.chord.prefix.parts(),
        binding.chord.completion.parts(),
    ];

    if strokes.contains(&(KeyCode::Esc, KeyModifiers::NONE)) {
        return Err(format!(
            "Ambiguous `{path}` = `{}`: plain `esc` is reserved for cancelling a pending chord.",
            binding.spec
        ));
    }

    if binding.action.context == KeymapContext::Agents
        && binding.chord.prefix.parts() == (KeyCode::Backspace, KeyModifiers::NONE)
    {
        return Err(format!(
            "Invalid `{path}` = `{}`: `backspace` is reserved for editing task input.",
            binding.spec
        ));
    }

    #[cfg(unix)]
    if strokes.contains(&crate::key_hint::ctrl(KeyCode::Char('z')).parts()) {
        return Err(format!(
            "Ambiguous `{path}` = `{}`: `ctrl-z` is reserved for suspending the terminal on Unix. \
Choose a different chord and retry.",
            binding.spec
        ));
    }

    let reserved = match binding.action.context {
        KeymapContext::Pager => TRANSCRIPT_BACKTRACK_RESERVED_BINDINGS,
        KeymapContext::Global
        | KeymapContext::Chat
        | KeymapContext::Composer
        | KeymapContext::Editor
        | KeymapContext::VimNormal
        | KeymapContext::VimOperator
        | KeymapContext::VimTextObject => MAIN_RESERVED_BINDINGS,
        KeymapContext::List => LIST_RESERVED_BINDINGS,
        KeymapContext::Agents | KeymapContext::Approval => &LIST_RESERVED_BINDINGS[..1],
    };
    if let Some((reserved_action, _)) = reserved.iter().find(|(_, reserved)| {
        binding.chord.prefix.parts() == reserved.parts()
            || binding.chord.completion.parts() == reserved.parts()
                && (binding.action.context != KeymapContext::List
                    || reserved.parts() == ctrl(KeyCode::Char('c')).parts())
    }) {
        return Err(format!(
            "Ambiguous `{path}` = `{}`: the chord uses the key reserved by \
`{reserved_action}`. Choose a different chord and retry.",
            binding.spec
        ));
    }

    Ok(())
}

#[cfg(test)]
#[path = "chords_tests.rs"]
mod tests;
