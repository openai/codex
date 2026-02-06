//! TUI keymap config schema and canonical key-spec normalization.
//!
//! This module defines the on-disk `[tui.keymap]` contract used by
//! `~/.codex/config.toml` and normalizes user-entered key specs into canonical
//! forms consumed by runtime keymap resolution in `codex-rs/tui/src/keymap.rs`.
//!
//! Responsibilities:
//!
//! 1. Define strongly typed config contexts/actions with unknown-field
//!    rejection.
//! 2. Normalize accepted key aliases into canonical names.
//! 3. Reject malformed bindings early with user-facing diagnostics.
//!
//! Non-responsibilities:
//!
//! 1. Dispatch precedence and conflict validation.
//! 2. Input event matching at runtime.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::de::Error as SerdeError;
use std::collections::BTreeMap;

/// Versioned keymap defaults.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum TuiKeymapPreset {
    /// Pointer alias to the latest shipped preset.
    ///
    /// Today this resolves to `v1`.
    #[default]
    Latest,
    /// Frozen keymap defaults that preserve legacy/current shortcut behavior.
    V1,
}

/// Normalized string representation of a keybinding (for example `ctrl-a`).
///
/// The parser accepts a small alias set (for example `escape` -> `esc`,
/// `pageup` -> `page-up`) and stores the canonical form.
#[derive(Serialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[serde(transparent)]
pub struct KeybindingSpec(#[schemars(with = "String")] pub String);

impl KeybindingSpec {
    /// Returns the canonical key-spec string (for example `ctrl-a`).
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl<'de> Deserialize<'de> for KeybindingSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        let normalized = normalize_keybinding_spec(&raw).map_err(SerdeError::custom)?;
        Ok(Self(normalized))
    }
}

/// One action binding value in config.
///
/// This accepts either:
///
/// 1. A single key spec string (`"ctrl-a"`).
/// 2. A list of key spec strings (`["ctrl-a", "alt-a"]`).
///
/// An empty list explicitly unbinds the action in that scope.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[serde(untagged)]
pub enum KeybindingsSpec {
    One(KeybindingSpec),
    Many(Vec<KeybindingSpec>),
}

impl KeybindingsSpec {
    /// Returns all configured key specs for one action in declaration order.
    ///
    /// Callers should preserve this ordering when deriving UI hints so the
    /// first binding remains the primary affordance shown to users.
    pub fn specs(&self) -> Vec<&KeybindingSpec> {
        match self {
            Self::One(spec) => vec![spec],
            Self::Many(specs) => specs.iter().collect(),
        }
    }
}

/// Global keybindings. These are used when a context does not define an override.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TuiGlobalKeymap {
    /// Open the transcript overlay.
    pub open_transcript: Option<KeybindingsSpec>,
    /// Open the external editor for the current draft.
    pub open_external_editor: Option<KeybindingsSpec>,
    /// In an empty composer, begin or advance "edit previous message" flow.
    pub edit_previous_message: Option<KeybindingsSpec>,
    /// Confirm editing the selected previous message.
    pub confirm_edit_previous_message: Option<KeybindingsSpec>,
    /// Submit the current composer draft.
    pub submit: Option<KeybindingsSpec>,
    /// Queue the current composer draft while a task is running.
    pub queue: Option<KeybindingsSpec>,
    /// Toggle the composer shortcut overlay.
    pub toggle_shortcuts: Option<KeybindingsSpec>,
}

/// Chat context keybindings. These override corresponding `global` actions.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TuiChatKeymap {
    /// In an empty composer, begin or advance "edit previous message" flow.
    pub edit_previous_message: Option<KeybindingsSpec>,
    /// Confirm editing the selected previous message.
    pub confirm_edit_previous_message: Option<KeybindingsSpec>,
}

/// Composer context keybindings. These override corresponding `global` actions.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TuiComposerKeymap {
    /// Submit the current composer draft.
    pub submit: Option<KeybindingsSpec>,
    /// Queue the current composer draft while a task is running.
    pub queue: Option<KeybindingsSpec>,
    /// Toggle the composer shortcut overlay.
    pub toggle_shortcuts: Option<KeybindingsSpec>,
}

/// Editor context keybindings for text editing inside text areas.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TuiEditorKeymap {
    /// Insert a newline in the editor.
    pub insert_newline: Option<KeybindingsSpec>,
    /// Move cursor left by one grapheme.
    pub move_left: Option<KeybindingsSpec>,
    /// Move cursor right by one grapheme.
    pub move_right: Option<KeybindingsSpec>,
    /// Move cursor up one visual line.
    pub move_up: Option<KeybindingsSpec>,
    /// Move cursor down one visual line.
    pub move_down: Option<KeybindingsSpec>,
    /// Move cursor to beginning of previous word.
    pub move_word_left: Option<KeybindingsSpec>,
    /// Move cursor to end of next word.
    pub move_word_right: Option<KeybindingsSpec>,
    /// Move cursor to beginning of line.
    pub move_line_start: Option<KeybindingsSpec>,
    /// Move cursor to end of line.
    pub move_line_end: Option<KeybindingsSpec>,
    /// Delete one grapheme to the left.
    pub delete_backward: Option<KeybindingsSpec>,
    /// Delete one grapheme to the right.
    pub delete_forward: Option<KeybindingsSpec>,
    /// Delete the previous word.
    pub delete_backward_word: Option<KeybindingsSpec>,
    /// Delete the next word.
    pub delete_forward_word: Option<KeybindingsSpec>,
    /// Kill text from cursor to line start.
    pub kill_line_start: Option<KeybindingsSpec>,
    /// Kill text from cursor to line end.
    pub kill_line_end: Option<KeybindingsSpec>,
    /// Yank the kill buffer.
    pub yank: Option<KeybindingsSpec>,
}

/// Pager context keybindings for transcript and static overlays.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TuiPagerKeymap {
    /// Scroll up by one row.
    pub scroll_up: Option<KeybindingsSpec>,
    /// Scroll down by one row.
    pub scroll_down: Option<KeybindingsSpec>,
    /// Scroll up by one page.
    pub page_up: Option<KeybindingsSpec>,
    /// Scroll down by one page.
    pub page_down: Option<KeybindingsSpec>,
    /// Scroll up by half a page.
    pub half_page_up: Option<KeybindingsSpec>,
    /// Scroll down by half a page.
    pub half_page_down: Option<KeybindingsSpec>,
    /// Jump to the beginning.
    pub jump_top: Option<KeybindingsSpec>,
    /// Jump to the end.
    pub jump_bottom: Option<KeybindingsSpec>,
    /// Close the pager overlay.
    pub close: Option<KeybindingsSpec>,
    /// Close the transcript overlay via its dedicated toggle key.
    pub close_transcript: Option<KeybindingsSpec>,
    /// In backtrack preview mode, step to older message.
    pub edit_previous_message: Option<KeybindingsSpec>,
    /// In backtrack preview mode, step to newer message.
    pub edit_next_message: Option<KeybindingsSpec>,
    /// In backtrack preview mode, confirm selected message.
    pub confirm_edit_message: Option<KeybindingsSpec>,
}

/// List selection context keybindings for popup-style selectable lists.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TuiListKeymap {
    /// Move list selection up.
    pub move_up: Option<KeybindingsSpec>,
    /// Move list selection down.
    pub move_down: Option<KeybindingsSpec>,
    /// Accept current selection.
    pub accept: Option<KeybindingsSpec>,
    /// Cancel and close selection view.
    pub cancel: Option<KeybindingsSpec>,
}

/// Approval overlay keybindings.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TuiApprovalKeymap {
    /// Open the full-screen approval details view.
    pub open_fullscreen: Option<KeybindingsSpec>,
    /// Approve the primary option.
    pub approve: Option<KeybindingsSpec>,
    /// Approve for session when that option exists.
    pub approve_for_session: Option<KeybindingsSpec>,
    /// Approve with exec-policy prefix when that option exists.
    pub approve_for_prefix: Option<KeybindingsSpec>,
    /// Decline and provide corrective guidance.
    pub decline: Option<KeybindingsSpec>,
    /// Cancel an elicitation request.
    pub cancel: Option<KeybindingsSpec>,
}

/// Onboarding keybindings.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TuiOnboardingKeymap {
    /// Move selection up.
    pub move_up: Option<KeybindingsSpec>,
    /// Move selection down.
    pub move_down: Option<KeybindingsSpec>,
    /// Pick first option.
    pub select_first: Option<KeybindingsSpec>,
    /// Pick second option.
    pub select_second: Option<KeybindingsSpec>,
    /// Pick third option.
    pub select_third: Option<KeybindingsSpec>,
    /// Confirm current selection.
    pub confirm: Option<KeybindingsSpec>,
    /// Cancel current screen action.
    pub cancel: Option<KeybindingsSpec>,
    /// Quit onboarding flow.
    pub quit: Option<KeybindingsSpec>,
    /// Cycle welcome animation variant.
    pub toggle_animation: Option<KeybindingsSpec>,
}

/// Raw keymap configuration from `[tui.keymap]`.
///
/// Each context contains action-level overrides. Missing actions inherit from
/// runtime preset defaults, and selected chat/composer actions can fall back
/// through `global` during runtime resolution.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TuiKeymap {
    #[serde(default)]
    pub preset: TuiKeymapPreset,
    #[serde(default)]
    pub global: TuiGlobalKeymap,
    #[serde(default)]
    pub chat: TuiChatKeymap,
    #[serde(default)]
    pub composer: TuiComposerKeymap,
    #[serde(default)]
    pub editor: TuiEditorKeymap,
    #[serde(default)]
    pub pager: TuiPagerKeymap,
    #[serde(default)]
    pub list: TuiListKeymap,
    #[serde(default)]
    pub approval: TuiApprovalKeymap,
    #[serde(default)]
    pub onboarding: TuiOnboardingKeymap,
}

/// Normalize one user-entered key spec into canonical storage format.
///
/// The output always orders modifiers as `ctrl-alt-shift-<key>` when present
/// and applies accepted aliases (`escape` -> `esc`, `pageup` -> `page-up`).
/// Inputs that cannot be represented unambiguously are rejected.
fn normalize_keybinding_spec(raw: &str) -> Result<String, String> {
    let lower = raw.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return Err(
            "keybinding cannot be empty. Use values like `ctrl-a` or `shift-enter`.\n\
Keymap template: https://github.com/openai/codex/blob/main/docs/default-keymap.toml"
                .to_string(),
        );
    }

    let segments: Vec<&str> = lower
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.is_empty() {
        return Err(format!(
            "invalid keybinding `{raw}`. Use values like `ctrl-a`, `shift-enter`, or `page-down`."
        ));
    }

    let mut modifiers =
        BTreeMap::<&str, bool>::from([("ctrl", false), ("alt", false), ("shift", false)]);
    let mut key_segments = Vec::new();
    let mut saw_key = false;

    for segment in segments {
        let canonical_mod = match segment {
            "ctrl" | "control" => Some("ctrl"),
            "alt" | "option" => Some("alt"),
            "shift" => Some("shift"),
            _ => None,
        };

        if !saw_key && let Some(modifier) = canonical_mod {
            if modifiers.get(modifier).copied().unwrap_or(false) {
                return Err(format!(
                    "duplicate modifier in keybinding `{raw}`. Use each modifier at most once."
                ));
            }
            modifiers.insert(modifier, true);
            continue;
        }

        saw_key = true;
        key_segments.push(segment);
    }

    if key_segments.is_empty() {
        return Err(format!(
            "missing key in keybinding `{raw}`. Add a key name like `a`, `enter`, or `page-down`."
        ));
    }

    if key_segments
        .iter()
        .any(|segment| matches!(*segment, "ctrl" | "control" | "alt" | "option" | "shift"))
    {
        return Err(format!(
            "invalid keybinding `{raw}`: modifiers must come before the key (for example `ctrl-a`)."
        ));
    }

    let key = normalize_key_name(&key_segments.join("-"), raw)?;
    let mut normalized = Vec::new();
    if modifiers.get("ctrl").copied().unwrap_or(false) {
        normalized.push("ctrl".to_string());
    }
    if modifiers.get("alt").copied().unwrap_or(false) {
        normalized.push("alt".to_string());
    }
    if modifiers.get("shift").copied().unwrap_or(false) {
        normalized.push("shift".to_string());
    }
    normalized.push(key);
    Ok(normalized.join("-"))
}

/// Normalize and validate one key name segment.
///
/// This accepts a constrained key vocabulary to keep runtime parser behavior
/// deterministic across platforms.
fn normalize_key_name(key: &str, original: &str) -> Result<String, String> {
    let alias = match key {
        "escape" => "esc",
        "return" => "enter",
        "spacebar" => "space",
        "pgup" | "pageup" => "page-up",
        "pgdn" | "pagedown" => "page-down",
        "del" => "delete",
        other => other,
    };

    if alias.len() == 1 {
        let ch = alias.chars().next().unwrap_or_default();
        if ch.is_ascii() && !ch.is_ascii_control() && ch != '-' {
            return Ok(alias.to_string());
        }
    }

    if matches!(
        alias,
        "enter"
            | "tab"
            | "backspace"
            | "esc"
            | "delete"
            | "up"
            | "down"
            | "left"
            | "right"
            | "home"
            | "end"
            | "page-up"
            | "page-down"
            | "space"
    ) {
        return Ok(alias.to_string());
    }

    if let Some(number) = alias.strip_prefix('f')
        && let Ok(number) = number.parse::<u8>()
        && (1..=12).contains(&number)
    {
        return Ok(alias.to_string());
    }

    Err(format!(
        "unknown key `{key}` in keybinding `{original}`. \
Use a printable character (for example `a`), function keys (`f1`-`f12`), \
or one of: enter, tab, backspace, esc, delete, arrows, home/end, page-up/page-down, space.\n\
Keymap template: https://github.com/openai/codex/blob/main/docs/default-keymap.toml"
    ))
}
