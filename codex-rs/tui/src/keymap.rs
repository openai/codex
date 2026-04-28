//! Runtime keymap resolution for the TUI.
//!
//! This module converts deserialized config (`TuiKeymap`) into a concrete
//! `RuntimeKeymap` used by input handlers at runtime.
//!
//! Key responsibilities:
//!
//! 1. Apply deterministic precedence (`context -> global fallback -> defaults`).
//! 2. Parse canonical key spec strings into `KeyBinding` values.
//! 3. Enforce uniqueness across runtime surfaces so one key cannot trigger
//!    multiple actions on the same focused input path.
//! 4. Return actionable, user-facing error messages with config paths and next
//!    steps.
//!
//! Non-responsibilities:
//!
//! 1. This module does not decide which action should run in a given screen.
//!    Callers resolve actions by checking the relevant action binding set.
//! 2. This module does not persist configuration; it only resolves loaded config.

use crate::key_hint;
use crate::key_hint::KeyBinding;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use std::collections::HashMap;

/// Runtime keymap used by TUI input handlers.
///
/// Resolution precedence is:
///
/// 1. Context-specific binding (`tui.keymap.<context>`).
/// 2. `tui.keymap.global` for actions that support global fallback.
/// 3. Built-in defaults.
///
/// This is the only shape UI code should use for dispatch. It represents a
/// fully resolved snapshot with parsing, fallback, explicit unbinding, and
/// duplicate-key validation already applied. If a caller keeps using an older
/// snapshot after config changes, visible hints and active handlers can drift.
#[derive(Clone, Debug)]
pub(crate) struct RuntimeKeymap {
    pub(crate) app: AppKeymap,
    pub(crate) chat: ChatKeymap,
    pub(crate) composer: ComposerKeymap,
    pub(crate) editor: EditorKeymap,
    pub(crate) pager: PagerKeymap,
    pub(crate) list: ListKeymap,
    pub(crate) approval: ApprovalKeymap,
}

#[derive(Clone, Debug)]
pub(crate) struct AppKeymap {
    /// Open transcript overlay.
    pub(crate) open_transcript: Vec<KeyBinding>,
    /// Open external editor for the current draft.
    pub(crate) open_external_editor: Vec<KeyBinding>,
    /// Copy the last agent response to the clipboard.
    pub(crate) copy: Vec<KeyBinding>,
    /// Clear the terminal UI.
    pub(crate) clear_terminal: Vec<KeyBinding>,
}

/// Main chat-surface keybindings.
#[derive(Clone, Debug)]
pub(crate) struct ChatKeymap {
    /// Decrease the active reasoning effort.
    pub(crate) decrease_reasoning_effort: Vec<KeyBinding>,
    /// Increase the active reasoning effort.
    pub(crate) increase_reasoning_effort: Vec<KeyBinding>,
    /// Edit the most recently queued message.
    pub(crate) edit_queued_message: Vec<KeyBinding>,
}

#[derive(Clone, Debug)]
pub(crate) struct ComposerKeymap {
    /// Submit current draft.
    pub(crate) submit: Vec<KeyBinding>,
    /// Queue current draft while a task is running.
    pub(crate) queue: Vec<KeyBinding>,
    /// Toggle composer shortcut overlay.
    pub(crate) toggle_shortcuts: Vec<KeyBinding>,
    /// Open reverse history search or move to the previous match.
    pub(crate) history_search_previous: Vec<KeyBinding>,
    /// Move to the next match in reverse history search.
    pub(crate) history_search_next: Vec<KeyBinding>,
}

/// Editor-specific keybindings used by the composer textarea.
///
/// These bindings are interpreted only by text-editing widgets and do not
/// participate in global/chat fallback resolution.
#[derive(Clone, Debug)]
pub(crate) struct EditorKeymap {
    pub(crate) insert_newline: Vec<KeyBinding>,
    pub(crate) move_left: Vec<KeyBinding>,
    pub(crate) move_right: Vec<KeyBinding>,
    pub(crate) move_up: Vec<KeyBinding>,
    pub(crate) move_down: Vec<KeyBinding>,
    pub(crate) move_word_left: Vec<KeyBinding>,
    pub(crate) move_word_right: Vec<KeyBinding>,
    pub(crate) move_line_start: Vec<KeyBinding>,
    pub(crate) move_line_end: Vec<KeyBinding>,
    pub(crate) delete_backward: Vec<KeyBinding>,
    pub(crate) delete_forward: Vec<KeyBinding>,
    pub(crate) delete_backward_word: Vec<KeyBinding>,
    pub(crate) delete_forward_word: Vec<KeyBinding>,
    pub(crate) kill_line_start: Vec<KeyBinding>,
    pub(crate) kill_line_end: Vec<KeyBinding>,
    pub(crate) yank: Vec<KeyBinding>,
}

/// Pager/overlay keybindings for transcript and static help views.
#[derive(Clone, Debug)]
pub(crate) struct PagerKeymap {
    pub(crate) scroll_up: Vec<KeyBinding>,
    pub(crate) scroll_down: Vec<KeyBinding>,
    pub(crate) page_up: Vec<KeyBinding>,
    pub(crate) page_down: Vec<KeyBinding>,
    pub(crate) half_page_up: Vec<KeyBinding>,
    pub(crate) half_page_down: Vec<KeyBinding>,
    pub(crate) jump_top: Vec<KeyBinding>,
    pub(crate) jump_bottom: Vec<KeyBinding>,
    pub(crate) close: Vec<KeyBinding>,
    pub(crate) close_transcript: Vec<KeyBinding>,
}

/// Generic list picker keybindings shared across popup list views.
#[derive(Clone, Debug)]
pub(crate) struct ListKeymap {
    pub(crate) move_up: Vec<KeyBinding>,
    pub(crate) move_down: Vec<KeyBinding>,
    pub(crate) accept: Vec<KeyBinding>,
    pub(crate) cancel: Vec<KeyBinding>,
}

/// Approval modal keybindings.
///
/// This covers both selection actions and the "open details fullscreen" escape
/// hatch for large approval payloads.
#[derive(Clone, Debug)]
pub(crate) struct ApprovalKeymap {
    pub(crate) open_fullscreen: Vec<KeyBinding>,
    pub(crate) open_thread: Vec<KeyBinding>,
    pub(crate) approve: Vec<KeyBinding>,
    pub(crate) approve_for_session: Vec<KeyBinding>,
    pub(crate) approve_for_prefix: Vec<KeyBinding>,
    pub(crate) deny: Vec<KeyBinding>,
    pub(crate) decline: Vec<KeyBinding>,
    pub(crate) cancel: Vec<KeyBinding>,
}

/// Returns the first binding, used as the primary UI hint for an action.
///
/// Rendering code should prefer this for concise hints while preserving all
/// bindings for actual input matching.
pub(crate) fn primary_binding(bindings: &[KeyBinding]) -> Option<KeyBinding> {
    bindings.first().copied()
}

/// Resolve one context-local action binding from config.
///
/// Expands to `resolve_bindings(...)` with:
/// - configured source: `tui.keymap.<context>.<action>`
/// - fallback source: the same action from built-in defaults
/// - error path: a stable string path for user-facing diagnostics
///
/// This keeps the resolution table concise while guaranteeing path strings
/// stay in sync with field names.
macro_rules! resolve_local {
    ($keymap:expr, $defaults:expr, $context:ident, $action:ident) => {
        resolve_bindings(
            ($keymap).$context.$action.as_ref(),
            &($defaults).$context.$action,
            concat!(
                "tui.keymap.",
                stringify!($context),
                ".",
                stringify!($action)
            ),
        )?
    };
}

/// Resolve one action binding with global fallback.
///
/// Expands to `resolve_bindings_with_global_fallback(...)` with precedence:
/// 1. `tui.keymap.<context>.<action>`
/// 2. `tui.keymap.global.<action>`
/// 3. built-in defaults for `<context>.<action>`
///
/// Used only for actions that intentionally support global reuse.
/// Context-local empty lists still count as configured values, so they unbind
/// the action instead of falling back to `global`.
macro_rules! resolve_with_global {
    ($keymap:expr, $defaults:expr, $context:ident, $action:ident) => {
        resolve_bindings_with_global_fallback(
            ($keymap).$context.$action.as_ref(),
            ($keymap).global.$action.as_ref(),
            &($defaults).$context.$action,
            concat!(
                "tui.keymap.",
                stringify!($context),
                ".",
                stringify!($action)
            ),
        )?
    };
}

/// Expand one default-table binding entry into a [`KeyBinding`].
///
/// This is a small declarative layer over `key_hint::{plain, ctrl, alt, shift}`
/// used by `default_bindings!` so `built_in_defaults` stays readable.
///
/// Supported forms:
/// - `plain(<KeyCode>)`
/// - `ctrl(<KeyCode>)`
/// - `alt(<KeyCode>)`
/// - `shift(<KeyCode>)`
/// - `raw(<KeyBinding expression>)` for bindings that do not match the helpers
///   (for example combined modifiers like Ctrl+Shift).
macro_rules! default_binding {
    (plain($key:expr)) => {
        key_hint::plain($key)
    };
    (ctrl($key:expr)) => {
        key_hint::ctrl($key)
    };
    (alt($key:expr)) => {
        key_hint::alt($key)
    };
    (shift($key:expr)) => {
        key_hint::shift($key)
    };
    (raw($binding:expr)) => {
        $binding
    };
}

/// Build a `Vec<KeyBinding>` for built-in defaults.
///
/// This macro is intentionally scoped to built-in keymaps. Runtime
/// config parsing still goes through `parse_bindings(...)` so user errors can
/// be reported with config-path-aware diagnostics.
macro_rules! default_bindings {
    ($($kind:ident($($arg:tt)*)),* $(,)?) => {
        vec![$(default_binding!($kind($($arg)*))),*]
    };
}

impl RuntimeKeymap {
    /// Return built-in defaults.
    ///
    /// This is a convenience for tests and bootstrapping UI state before user
    /// config has been loaded. It should not be used as a fallback after
    /// parsing `TuiKeymap`, because doing so would ignore explicit user
    /// unbindings and conflict diagnostics.
    pub(crate) fn defaults() -> Self {
        Self::built_in_defaults()
    }

    /// Resolve a runtime keymap from config, applying precedence and validation.
    ///
    /// Returns an error when:
    ///
    /// 1. A keybinding spec cannot be parsed.
    /// 2. A context has ambiguous bindings (same key assigned to multiple actions).
    ///
    /// The error text includes the relevant config path and a concrete next step.
    /// Calling code should not merge bindings across unrelated contexts before
    /// dispatch, or conflict guarantees from this resolver no longer hold.
    pub(crate) fn from_config(keymap: &TuiKeymap) -> Result<Self, String> {
        let defaults = Self::built_in_defaults();

        let app = AppKeymap {
            open_transcript: resolve_bindings(
                keymap.global.open_transcript.as_ref(),
                &defaults.app.open_transcript,
                "tui.keymap.global.open_transcript",
            )?,
            open_external_editor: resolve_bindings(
                keymap.global.open_external_editor.as_ref(),
                &defaults.app.open_external_editor,
                "tui.keymap.global.open_external_editor",
            )?,
            copy: resolve_bindings(
                keymap.global.copy.as_ref(),
                &defaults.app.copy,
                "tui.keymap.global.copy",
            )?,
            clear_terminal: resolve_bindings(
                keymap.global.clear_terminal.as_ref(),
                &defaults.app.clear_terminal,
                "tui.keymap.global.clear_terminal",
            )?,
        };

        let chat = ChatKeymap {
            decrease_reasoning_effort: resolve_bindings(
                keymap.chat.decrease_reasoning_effort.as_ref(),
                &defaults.chat.decrease_reasoning_effort,
                "tui.keymap.chat.decrease_reasoning_effort",
            )?,
            increase_reasoning_effort: resolve_bindings(
                keymap.chat.increase_reasoning_effort.as_ref(),
                &defaults.chat.increase_reasoning_effort,
                "tui.keymap.chat.increase_reasoning_effort",
            )?,
            edit_queued_message: resolve_bindings(
                keymap.chat.edit_queued_message.as_ref(),
                &defaults.chat.edit_queued_message,
                "tui.keymap.chat.edit_queued_message",
            )?,
        };

        let composer = ComposerKeymap {
            submit: resolve_with_global!(keymap, defaults, composer, submit),
            queue: resolve_with_global!(keymap, defaults, composer, queue),
            toggle_shortcuts: resolve_with_global!(keymap, defaults, composer, toggle_shortcuts),
            history_search_previous: resolve_local!(
                keymap,
                defaults,
                composer,
                history_search_previous
            ),
            history_search_next: resolve_local!(keymap, defaults, composer, history_search_next),
        };

        let editor = EditorKeymap {
            insert_newline: resolve_local!(keymap, defaults, editor, insert_newline),
            move_left: resolve_local!(keymap, defaults, editor, move_left),
            move_right: resolve_local!(keymap, defaults, editor, move_right),
            move_up: resolve_local!(keymap, defaults, editor, move_up),
            move_down: resolve_local!(keymap, defaults, editor, move_down),
            move_word_left: resolve_local!(keymap, defaults, editor, move_word_left),
            move_word_right: resolve_local!(keymap, defaults, editor, move_word_right),
            move_line_start: resolve_local!(keymap, defaults, editor, move_line_start),
            move_line_end: resolve_local!(keymap, defaults, editor, move_line_end),
            delete_backward: resolve_local!(keymap, defaults, editor, delete_backward),
            delete_forward: resolve_local!(keymap, defaults, editor, delete_forward),
            delete_backward_word: resolve_local!(keymap, defaults, editor, delete_backward_word),
            delete_forward_word: resolve_local!(keymap, defaults, editor, delete_forward_word),
            kill_line_start: resolve_local!(keymap, defaults, editor, kill_line_start),
            kill_line_end: resolve_local!(keymap, defaults, editor, kill_line_end),
            yank: resolve_local!(keymap, defaults, editor, yank),
        };

        let pager = PagerKeymap {
            scroll_up: resolve_local!(keymap, defaults, pager, scroll_up),
            scroll_down: resolve_local!(keymap, defaults, pager, scroll_down),
            page_up: resolve_local!(keymap, defaults, pager, page_up),
            page_down: resolve_local!(keymap, defaults, pager, page_down),
            half_page_up: resolve_local!(keymap, defaults, pager, half_page_up),
            half_page_down: resolve_local!(keymap, defaults, pager, half_page_down),
            jump_top: resolve_local!(keymap, defaults, pager, jump_top),
            jump_bottom: resolve_local!(keymap, defaults, pager, jump_bottom),
            close: resolve_local!(keymap, defaults, pager, close),
            close_transcript: resolve_local!(keymap, defaults, pager, close_transcript),
        };

        let list = ListKeymap {
            move_up: resolve_local!(keymap, defaults, list, move_up),
            move_down: resolve_local!(keymap, defaults, list, move_down),
            accept: resolve_local!(keymap, defaults, list, accept),
            cancel: resolve_local!(keymap, defaults, list, cancel),
        };

        let approval = ApprovalKeymap {
            open_fullscreen: resolve_local!(keymap, defaults, approval, open_fullscreen),
            open_thread: resolve_local!(keymap, defaults, approval, open_thread),
            approve: resolve_local!(keymap, defaults, approval, approve),
            approve_for_session: resolve_local!(keymap, defaults, approval, approve_for_session),
            approve_for_prefix: resolve_local!(keymap, defaults, approval, approve_for_prefix),
            deny: resolve_local!(keymap, defaults, approval, deny),
            decline: resolve_local!(keymap, defaults, approval, decline),
            cancel: resolve_local!(keymap, defaults, approval, cancel),
        };

        let resolved = Self {
            app,
            chat,
            composer,
            editor,
            pager,
            list,
            approval,
        };

        resolved.validate_conflicts()?;
        Ok(resolved)
    }

    /// Built-in keymap defaults.
    ///
    /// Some actions intentionally include compatibility variants (for example
    /// both `?` and `shift-?`) because terminals disagree on whether SHIFT is
    /// preserved for certain printable/control chords.
    fn built_in_defaults() -> Self {
        Self {
            app: AppKeymap {
                open_transcript: default_bindings![ctrl(KeyCode::Char('t'))],
                open_external_editor: default_bindings![ctrl(KeyCode::Char('g'))],
                copy: default_bindings![ctrl(KeyCode::Char('o'))],
                clear_terminal: default_bindings![ctrl(KeyCode::Char('l'))],
            },
            chat: ChatKeymap {
                decrease_reasoning_effort: default_bindings![alt(KeyCode::Char(','))],
                increase_reasoning_effort: default_bindings![alt(KeyCode::Char('.'))],
                edit_queued_message: default_bindings![alt(KeyCode::Up), shift(KeyCode::Left)],
            },
            composer: ComposerKeymap {
                submit: default_bindings![plain(KeyCode::Enter)],
                queue: default_bindings![plain(KeyCode::Tab)],
                toggle_shortcuts: default_bindings![
                    plain(KeyCode::Char('?')),
                    shift(KeyCode::Char('?'))
                ],
                history_search_previous: default_bindings![ctrl(KeyCode::Char('r'))],
                history_search_next: default_bindings![ctrl(KeyCode::Char('s'))],
            },
            editor: EditorKeymap {
                insert_newline: default_bindings![
                    ctrl(KeyCode::Char('j')),
                    ctrl(KeyCode::Char('m')),
                    plain(KeyCode::Enter),
                    shift(KeyCode::Enter)
                ],
                move_left: default_bindings![plain(KeyCode::Left), ctrl(KeyCode::Char('b'))],
                move_right: default_bindings![plain(KeyCode::Right), ctrl(KeyCode::Char('f'))],
                move_up: default_bindings![plain(KeyCode::Up), ctrl(KeyCode::Char('p'))],
                move_down: default_bindings![plain(KeyCode::Down), ctrl(KeyCode::Char('n'))],
                move_word_left: default_bindings![
                    alt(KeyCode::Char('b')),
                    raw(KeyBinding::new(KeyCode::Left, KeyModifiers::ALT)),
                    raw(KeyBinding::new(KeyCode::Left, KeyModifiers::CONTROL))
                ],
                move_word_right: default_bindings![
                    alt(KeyCode::Char('f')),
                    raw(KeyBinding::new(KeyCode::Right, KeyModifiers::ALT)),
                    raw(KeyBinding::new(KeyCode::Right, KeyModifiers::CONTROL))
                ],
                move_line_start: default_bindings![plain(KeyCode::Home), ctrl(KeyCode::Char('a'))],
                move_line_end: default_bindings![plain(KeyCode::End), ctrl(KeyCode::Char('e'))],
                delete_backward: default_bindings![
                    plain(KeyCode::Backspace),
                    ctrl(KeyCode::Char('h'))
                ],
                delete_forward: default_bindings![plain(KeyCode::Delete), ctrl(KeyCode::Char('d'))],
                delete_backward_word: default_bindings![
                    alt(KeyCode::Backspace),
                    ctrl(KeyCode::Char('w')),
                    raw(KeyBinding::new(
                        KeyCode::Char('h'),
                        KeyModifiers::CONTROL | KeyModifiers::ALT,
                    ))
                ],
                delete_forward_word: default_bindings![
                    alt(KeyCode::Delete),
                    alt(KeyCode::Char('d'))
                ],
                kill_line_start: default_bindings![ctrl(KeyCode::Char('u'))],
                kill_line_end: default_bindings![ctrl(KeyCode::Char('k'))],
                yank: default_bindings![ctrl(KeyCode::Char('y'))],
            },
            pager: PagerKeymap {
                scroll_up: default_bindings![plain(KeyCode::Up), plain(KeyCode::Char('k'))],
                scroll_down: default_bindings![plain(KeyCode::Down), plain(KeyCode::Char('j'))],
                page_up: default_bindings![
                    plain(KeyCode::PageUp),
                    shift(KeyCode::Char(' ')),
                    ctrl(KeyCode::Char('b'))
                ],
                page_down: default_bindings![
                    plain(KeyCode::PageDown),
                    plain(KeyCode::Char(' ')),
                    ctrl(KeyCode::Char('f'))
                ],
                half_page_up: default_bindings![ctrl(KeyCode::Char('u'))],
                half_page_down: default_bindings![ctrl(KeyCode::Char('d'))],
                jump_top: default_bindings![plain(KeyCode::Home)],
                jump_bottom: default_bindings![plain(KeyCode::End)],
                close: default_bindings![plain(KeyCode::Char('q')), ctrl(KeyCode::Char('c'))],
                close_transcript: default_bindings![ctrl(KeyCode::Char('t'))],
            },
            list: ListKeymap {
                move_up: default_bindings![
                    plain(KeyCode::Up),
                    ctrl(KeyCode::Char('p')),
                    plain(KeyCode::Char('k'))
                ],
                move_down: default_bindings![
                    plain(KeyCode::Down),
                    ctrl(KeyCode::Char('n')),
                    plain(KeyCode::Char('j'))
                ],
                accept: default_bindings![plain(KeyCode::Enter)],
                cancel: default_bindings![plain(KeyCode::Esc)],
            },
            approval: ApprovalKeymap {
                open_fullscreen: default_bindings![
                    ctrl(KeyCode::Char('a')),
                    raw(KeyBinding::new(
                        KeyCode::Char('a'),
                        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
                    ))
                ],
                open_thread: default_bindings![plain(KeyCode::Char('o'))],
                approve: default_bindings![plain(KeyCode::Char('y'))],
                approve_for_session: default_bindings![plain(KeyCode::Char('a'))],
                approve_for_prefix: default_bindings![plain(KeyCode::Char('p'))],
                deny: default_bindings![plain(KeyCode::Char('d'))],
                decline: default_bindings![plain(KeyCode::Esc), plain(KeyCode::Char('n'))],
                cancel: default_bindings![plain(KeyCode::Char('c'))],
            },
        }
    }

    /// Reject ambiguous bindings in scopes that are evaluated together.
    ///
    /// We validate in multiple passes because runtime handling has mixed
    /// precedence:
    ///
    /// 1. `app` actions can shadow composer actions because app checks run
    ///    before forwarding to the composer.
    /// 2. Contexts with hard-coded sequence behavior, such as edit-previous
    ///    backtracking, intentionally stay outside this configurable keymap.
    fn validate_conflicts(&self) -> Result<(), String> {
        validate_unique(
            "app",
            [
                ("open_transcript", self.app.open_transcript.as_slice()),
                (
                    "open_external_editor",
                    self.app.open_external_editor.as_slice(),
                ),
                ("copy", self.app.copy.as_slice()),
                ("clear_terminal", self.app.clear_terminal.as_slice()),
                (
                    "chat.decrease_reasoning_effort",
                    self.chat.decrease_reasoning_effort.as_slice(),
                ),
                (
                    "chat.increase_reasoning_effort",
                    self.chat.increase_reasoning_effort.as_slice(),
                ),
                (
                    "chat.edit_queued_message",
                    self.chat.edit_queued_message.as_slice(),
                ),
                ("composer.submit", self.composer.submit.as_slice()),
                ("composer.queue", self.composer.queue.as_slice()),
                (
                    "composer.toggle_shortcuts",
                    self.composer.toggle_shortcuts.as_slice(),
                ),
                (
                    "composer.history_search_previous",
                    self.composer.history_search_previous.as_slice(),
                ),
                (
                    "composer.history_search_next",
                    self.composer.history_search_next.as_slice(),
                ),
            ],
        )?;

        validate_no_reserved(
            "main",
            [
                ("open_transcript", self.app.open_transcript.as_slice()),
                (
                    "open_external_editor",
                    self.app.open_external_editor.as_slice(),
                ),
                ("copy", self.app.copy.as_slice()),
                ("clear_terminal", self.app.clear_terminal.as_slice()),
                (
                    "chat.decrease_reasoning_effort",
                    self.chat.decrease_reasoning_effort.as_slice(),
                ),
                (
                    "chat.increase_reasoning_effort",
                    self.chat.increase_reasoning_effort.as_slice(),
                ),
                (
                    "chat.edit_queued_message",
                    self.chat.edit_queued_message.as_slice(),
                ),
                ("composer.submit", self.composer.submit.as_slice()),
                ("composer.queue", self.composer.queue.as_slice()),
                (
                    "composer.toggle_shortcuts",
                    self.composer.toggle_shortcuts.as_slice(),
                ),
                (
                    "composer.history_search_previous",
                    self.composer.history_search_previous.as_slice(),
                ),
                (
                    "composer.history_search_next",
                    self.composer.history_search_next.as_slice(),
                ),
            ],
            MAIN_RESERVED_BINDINGS,
        )?;

        validate_no_shadow(
            "app",
            [
                ("open_transcript", self.app.open_transcript.as_slice()),
                (
                    "open_external_editor",
                    self.app.open_external_editor.as_slice(),
                ),
                ("copy", self.app.copy.as_slice()),
                ("clear_terminal", self.app.clear_terminal.as_slice()),
            ],
            [
                ("list.move_up", self.list.move_up.as_slice()),
                ("list.move_down", self.list.move_down.as_slice()),
                ("list.accept", self.list.accept.as_slice()),
                ("list.cancel", self.list.cancel.as_slice()),
                (
                    "approval.open_fullscreen",
                    self.approval.open_fullscreen.as_slice(),
                ),
                ("approval.open_thread", self.approval.open_thread.as_slice()),
                ("approval.approve", self.approval.approve.as_slice()),
                (
                    "approval.approve_for_session",
                    self.approval.approve_for_session.as_slice(),
                ),
                (
                    "approval.approve_for_prefix",
                    self.approval.approve_for_prefix.as_slice(),
                ),
                ("approval.deny", self.approval.deny.as_slice()),
                ("approval.decline", self.approval.decline.as_slice()),
                ("approval.cancel", self.approval.cancel.as_slice()),
            ],
        )?;

        // While the composer is focused, these main-surface handlers always
        // consume matching keys before the event reaches the textarea editor.
        validate_no_shadow_with_allowed_overlaps(
            "main",
            [
                ("open_transcript", self.app.open_transcript.as_slice()),
                (
                    "open_external_editor",
                    self.app.open_external_editor.as_slice(),
                ),
                ("copy", self.app.copy.as_slice()),
                ("clear_terminal", self.app.clear_terminal.as_slice()),
                (
                    "chat.decrease_reasoning_effort",
                    self.chat.decrease_reasoning_effort.as_slice(),
                ),
                (
                    "chat.increase_reasoning_effort",
                    self.chat.increase_reasoning_effort.as_slice(),
                ),
                ("composer.submit", self.composer.submit.as_slice()),
                (
                    "composer.history_search_previous",
                    self.composer.history_search_previous.as_slice(),
                ),
            ],
            [
                (
                    "editor.insert_newline",
                    self.editor.insert_newline.as_slice(),
                ),
                ("editor.move_left", self.editor.move_left.as_slice()),
                ("editor.move_right", self.editor.move_right.as_slice()),
                ("editor.move_up", self.editor.move_up.as_slice()),
                ("editor.move_down", self.editor.move_down.as_slice()),
                (
                    "editor.move_word_left",
                    self.editor.move_word_left.as_slice(),
                ),
                (
                    "editor.move_word_right",
                    self.editor.move_word_right.as_slice(),
                ),
                (
                    "editor.move_line_start",
                    self.editor.move_line_start.as_slice(),
                ),
                ("editor.move_line_end", self.editor.move_line_end.as_slice()),
                (
                    "editor.delete_backward",
                    self.editor.delete_backward.as_slice(),
                ),
                (
                    "editor.delete_forward",
                    self.editor.delete_forward.as_slice(),
                ),
                (
                    "editor.delete_backward_word",
                    self.editor.delete_backward_word.as_slice(),
                ),
                (
                    "editor.delete_forward_word",
                    self.editor.delete_forward_word.as_slice(),
                ),
                (
                    "editor.kill_line_start",
                    self.editor.kill_line_start.as_slice(),
                ),
                ("editor.kill_line_end", self.editor.kill_line_end.as_slice()),
                ("editor.yank", self.editor.yank.as_slice()),
            ],
            [(
                "composer.submit",
                "editor.insert_newline",
                key_hint::plain(KeyCode::Enter),
            )],
        )?;

        validate_unique(
            "editor",
            [
                ("insert_newline", self.editor.insert_newline.as_slice()),
                ("move_left", self.editor.move_left.as_slice()),
                ("move_right", self.editor.move_right.as_slice()),
                ("move_up", self.editor.move_up.as_slice()),
                ("move_down", self.editor.move_down.as_slice()),
                ("move_word_left", self.editor.move_word_left.as_slice()),
                ("move_word_right", self.editor.move_word_right.as_slice()),
                ("move_line_start", self.editor.move_line_start.as_slice()),
                ("move_line_end", self.editor.move_line_end.as_slice()),
                ("delete_backward", self.editor.delete_backward.as_slice()),
                ("delete_forward", self.editor.delete_forward.as_slice()),
                (
                    "delete_backward_word",
                    self.editor.delete_backward_word.as_slice(),
                ),
                (
                    "delete_forward_word",
                    self.editor.delete_forward_word.as_slice(),
                ),
                ("kill_line_start", self.editor.kill_line_start.as_slice()),
                ("kill_line_end", self.editor.kill_line_end.as_slice()),
                ("yank", self.editor.yank.as_slice()),
            ],
        )?;

        validate_unique(
            "pager",
            [
                ("scroll_up", self.pager.scroll_up.as_slice()),
                ("scroll_down", self.pager.scroll_down.as_slice()),
                ("page_up", self.pager.page_up.as_slice()),
                ("page_down", self.pager.page_down.as_slice()),
                ("half_page_up", self.pager.half_page_up.as_slice()),
                ("half_page_down", self.pager.half_page_down.as_slice()),
                ("jump_top", self.pager.jump_top.as_slice()),
                ("jump_bottom", self.pager.jump_bottom.as_slice()),
                ("close", self.pager.close.as_slice()),
                ("close_transcript", self.pager.close_transcript.as_slice()),
            ],
        )?;

        validate_no_reserved(
            "pager",
            [
                ("scroll_up", self.pager.scroll_up.as_slice()),
                ("scroll_down", self.pager.scroll_down.as_slice()),
                ("page_up", self.pager.page_up.as_slice()),
                ("page_down", self.pager.page_down.as_slice()),
                ("half_page_up", self.pager.half_page_up.as_slice()),
                ("half_page_down", self.pager.half_page_down.as_slice()),
                ("jump_top", self.pager.jump_top.as_slice()),
                ("jump_bottom", self.pager.jump_bottom.as_slice()),
                ("close", self.pager.close.as_slice()),
                ("close_transcript", self.pager.close_transcript.as_slice()),
            ],
            TRANSCRIPT_BACKTRACK_RESERVED_BINDINGS,
        )?;

        validate_unique(
            "list",
            [
                ("move_up", self.list.move_up.as_slice()),
                ("move_down", self.list.move_down.as_slice()),
                ("accept", self.list.accept.as_slice()),
                ("cancel", self.list.cancel.as_slice()),
            ],
        )?;

        validate_unique(
            "approval",
            [
                ("open_fullscreen", self.approval.open_fullscreen.as_slice()),
                ("open_thread", self.approval.open_thread.as_slice()),
                ("approve", self.approval.approve.as_slice()),
                (
                    "approve_for_session",
                    self.approval.approve_for_session.as_slice(),
                ),
                (
                    "approve_for_prefix",
                    self.approval.approve_for_prefix.as_slice(),
                ),
                ("deny", self.approval.deny.as_slice()),
                ("decline", self.approval.decline.as_slice()),
                ("cancel", self.approval.cancel.as_slice()),
            ],
        )?;

        let mut seen: HashMap<(KeyCode, KeyModifiers), &'static str> = HashMap::new();
        for (action, bindings) in [
            ("list.move_up", self.list.move_up.as_slice()),
            ("list.move_down", self.list.move_down.as_slice()),
            ("list.accept", self.list.accept.as_slice()),
            ("list.cancel", self.list.cancel.as_slice()),
            (
                "approval.open_fullscreen",
                self.approval.open_fullscreen.as_slice(),
            ),
            ("approval.open_thread", self.approval.open_thread.as_slice()),
            ("approval.approve", self.approval.approve.as_slice()),
            (
                "approval.approve_for_session",
                self.approval.approve_for_session.as_slice(),
            ),
            (
                "approval.approve_for_prefix",
                self.approval.approve_for_prefix.as_slice(),
            ),
            ("approval.deny", self.approval.deny.as_slice()),
            ("approval.decline", self.approval.decline.as_slice()),
            ("approval.cancel", self.approval.cancel.as_slice()),
        ] {
            for binding in bindings {
                let key = binding.parts();
                if let Some(previous) = seen.insert(key, action) {
                    // Approval overlays intentionally reserve Esc as a stable
                    // cancellation path even though decline options may also
                    // display it in contexts where that is safe.
                    if previous == "list.cancel"
                        && action == "approval.decline"
                        && key == (KeyCode::Esc, KeyModifiers::NONE)
                    {
                        continue;
                    }
                    return Err(format!(
                        "Ambiguous approval overlay keymap bindings: `{previous}` and `{action}` use the same key. \
Set unique keys in `~/.codex/config.toml` and retry. \
See the Codex keymap documentation for supported actions and examples."
                    ));
                }
            }
        }

        Ok(())
    }
}

/// Reject duplicate keys inside one effective context map.
///
/// This intentionally allows the same key across different contexts; handlers
/// only evaluate one context at a time.
fn validate_unique<const N: usize>(
    context: &str,
    pairs: [(&'static str, &[KeyBinding]); N],
) -> Result<(), String> {
    let mut seen: HashMap<(KeyCode, KeyModifiers), &'static str> = HashMap::new();
    for (action, bindings) in pairs {
        for binding in bindings {
            let key = binding.parts();
            if let Some(previous) = seen.insert(key, action) {
                return Err(format!(
                    "Ambiguous `tui.keymap.{context}` bindings: `{previous}` and `{action}` use the same key. \
Set unique keys in `~/.codex/config.toml` and retry. \
See the Codex keymap documentation for supported actions and examples."
                ));
            }
        }
    }
    Ok(())
}

fn validate_no_shadow<const N: usize, const M: usize>(
    context: &str,
    primary: [(&'static str, &[KeyBinding]); N],
    shadowed: [(&'static str, &[KeyBinding]); M],
) -> Result<(), String> {
    validate_no_shadow_with_allowed_overlaps(context, primary, shadowed, [])
}

fn validate_no_shadow_with_allowed_overlaps<const N: usize, const M: usize, const A: usize>(
    context: &str,
    primary: [(&'static str, &[KeyBinding]); N],
    shadowed: [(&'static str, &[KeyBinding]); M],
    allowed_overlaps: [(&'static str, &'static str, KeyBinding); A],
) -> Result<(), String> {
    let mut seen: HashMap<(KeyCode, KeyModifiers), &'static str> = HashMap::new();
    for (action, bindings) in primary {
        for binding in bindings {
            seen.insert(binding.parts(), action);
        }
    }
    for (action, bindings) in shadowed {
        for binding in bindings {
            let key = binding.parts();
            if let Some(previous) = seen.get(&key) {
                if allowed_overlaps.iter().any(
                    |(allowed_primary, allowed_shadowed, allowed_binding)| {
                        *allowed_primary == *previous
                            && *allowed_shadowed == action
                            && allowed_binding.parts() == key
                    },
                ) {
                    continue;
                }
                return Err(format!(
                    "Ambiguous `tui.keymap.{context}` bindings: `{previous}` shadows `{action}` with the same key. \
Set unique keys in `~/.codex/config.toml` and retry. \
See the Codex keymap documentation for supported actions and examples."
                ));
            }
        }
    }
    Ok(())
}

fn validate_no_reserved<const N: usize>(
    context: &str,
    pairs: [(&'static str, &[KeyBinding]); N],
    reserved: &[(&'static str, KeyBinding)],
) -> Result<(), String> {
    for (action, bindings) in pairs {
        for binding in bindings {
            let key = binding.parts();
            if let Some((reserved_action, _)) = reserved
                .iter()
                .find(|(_, reserved_binding)| reserved_binding.parts() == key)
            {
                return Err(format!(
                    "Ambiguous `tui.keymap.{context}` bindings: `{action}` uses a key reserved by `{reserved_action}`. \
Set a different key in `~/.codex/config.toml` and retry. \
See the Codex keymap documentation for supported actions and examples."
                ));
            }
        }
    }
    Ok(())
}

const MAIN_RESERVED_BINDINGS: &[(&str, KeyBinding)] = &[
    (
        "fixed.interrupt_or_quit",
        key_hint::ctrl(KeyCode::Char('c')),
    ),
    ("fixed.quit", key_hint::ctrl(KeyCode::Char('d'))),
    ("fixed.paste_image", key_hint::ctrl(KeyCode::Char('v'))),
    ("fixed.paste_image", key_hint::ctrl_alt(KeyCode::Char('v'))),
    (
        "fixed.cycle_collaboration_mode",
        key_hint::shift(KeyCode::Tab),
    ),
    (
        "fixed.return_from_side_or_backtrack",
        key_hint::plain(KeyCode::Esc),
    ),
    ("fixed.previous_agent", key_hint::alt(KeyCode::Left)),
    ("fixed.next_agent", key_hint::alt(KeyCode::Right)),
    ("fixed.slash_command", key_hint::plain(KeyCode::Char('/'))),
    ("fixed.shell_command", key_hint::plain(KeyCode::Char('!'))),
    ("fixed.file_paths", key_hint::plain(KeyCode::Char('@'))),
    (
        "fixed.connector_mentions",
        key_hint::plain(KeyCode::Char('$')),
    ),
];

const TRANSCRIPT_BACKTRACK_RESERVED_BINDINGS: &[(&str, KeyBinding)] = &[
    (
        "fixed.transcript_edit_previous",
        key_hint::plain(KeyCode::Esc),
    ),
    (
        "fixed.transcript_edit_previous",
        key_hint::plain(KeyCode::Left),
    ),
    (
        "fixed.transcript_edit_next",
        key_hint::plain(KeyCode::Right),
    ),
    (
        "fixed.transcript_confirm_edit",
        key_hint::plain(KeyCode::Enter),
    ),
];

/// Resolve one action with context -> global -> default precedence.
///
/// `path` should be the context-specific config path so parser errors point
/// users at the override they attempted to set.
///
/// A configured empty list is authoritative: it returns an empty binding set
/// and does not continue to the global or built-in fallback. This is what makes
/// explicit unbinding work for globally reusable actions like composer submit.
fn resolve_bindings_with_global_fallback(
    configured: Option<&KeybindingsSpec>,
    global: Option<&KeybindingsSpec>,
    fallback: &[KeyBinding],
    path: &str,
) -> Result<Vec<KeyBinding>, String> {
    if let Some(configured) = configured {
        return parse_bindings(configured, path);
    }
    if let Some(global) = global {
        return parse_bindings(global, path);
    }
    Ok(fallback.to_vec())
}

/// Resolve one action binding in a context without global fallback.
///
/// Missing values inherit from the built-in fallback; configured values, including
/// empty lists, replace that fallback for the action.
fn resolve_bindings(
    configured: Option<&KeybindingsSpec>,
    fallback: &[KeyBinding],
    path: &str,
) -> Result<Vec<KeyBinding>, String> {
    let Some(spec) = configured else {
        return Ok(fallback.to_vec());
    };
    parse_bindings(spec, path)
}

/// Parse one keybinding value (`string` or `list[string]`) into concrete bindings.
///
/// Duplicate entries are de-duplicated while preserving first-seen order so the
/// first key can remain the primary UI hint.
fn parse_bindings(spec: &KeybindingsSpec, path: &str) -> Result<Vec<KeyBinding>, String> {
    let mut parsed = Vec::new();
    for raw in spec.specs() {
        let binding = parse_keybinding(raw.as_str()).ok_or_else(|| {
            format!(
                "Invalid `{path}` = `{}`. Use values like `ctrl-a`, `shift-enter`, or `page-down`. \
See the Codex keymap documentation for supported actions and examples.",
                raw.as_str()
            )
        })?;

        if !parsed.contains(&binding) {
            parsed.push(binding);
        }
    }
    Ok(parsed)
}

/// Parse one normalized keybinding spec such as `ctrl-a` or `shift-enter`.
///
/// Specs are expected to be normalized by config deserialization, but this
/// parser remains strict to keep runtime error messages precise.
fn parse_keybinding(spec: &str) -> Option<KeyBinding> {
    let mut parts = spec.split('-');
    let mut modifiers = KeyModifiers::NONE;
    let mut key_name = None;

    for part in parts.by_ref() {
        match part {
            "ctrl" => modifiers |= KeyModifiers::CONTROL,
            "alt" => modifiers |= KeyModifiers::ALT,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            other => {
                key_name = Some(other.to_string());
                break;
            }
        }
    }

    let mut key_name = key_name?;
    for trailing in parts {
        key_name.push('-');
        key_name.push_str(trailing);
    }

    let key = match key_name.as_str() {
        "enter" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "esc" => KeyCode::Esc,
        "delete" => KeyCode::Delete,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "page-up" => KeyCode::PageUp,
        "page-down" => KeyCode::PageDown,
        "space" => KeyCode::Char(' '),
        other if other.len() == 1 => KeyCode::Char(char::from(other.as_bytes()[0])),
        other if other.starts_with('f') => {
            let number = other[1..].parse::<u8>().ok()?;
            if (1..=12).contains(&number) {
                KeyCode::F(number)
            } else {
                return None;
            }
        }
        _ => return None,
    };

    Some(KeyBinding::new(key, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_config::types::KeybindingSpec;

    fn one(spec: &str) -> KeybindingsSpec {
        KeybindingsSpec::One(KeybindingSpec(spec.to_string()))
    }

    fn expect_conflict(keymap: &TuiKeymap, first: &str, second: &str) {
        let err = RuntimeKeymap::from_config(keymap).expect_err("expected conflict");
        assert!(err.contains(first));
        assert!(err.contains(second));
    }

    #[test]
    fn parses_canonical_binding() {
        let binding = parse_keybinding("ctrl-alt-shift-a").expect("binding should parse");
        assert_eq!(binding.parts().0, KeyCode::Char('a'));
        assert_eq!(
            binding.parts().1,
            KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT
        );
    }

    #[test]
    fn rejects_shadowing_composer_binding_in_app_scope() {
        let mut keymap = TuiKeymap::default();
        keymap.global.open_transcript = Some(one("ctrl-t"));
        keymap.composer.submit = Some(one("ctrl-t"));

        let err = RuntimeKeymap::from_config(&keymap).expect_err("expected shadowing conflict");
        assert!(err.contains("composer.submit"));
        assert!(err.contains("open_transcript"));
    }

    #[test]
    fn rejects_shadowing_composer_queue_in_app_scope() {
        let mut keymap = TuiKeymap::default();
        keymap.global.open_external_editor = Some(one("ctrl-g"));
        keymap.composer.queue = Some(one("ctrl-g"));

        let err = RuntimeKeymap::from_config(&keymap).expect_err("expected shadowing conflict");
        assert!(err.contains("composer.queue"));
        assert!(err.contains("open_external_editor"));
    }

    #[test]
    fn rejects_shadowing_composer_toggle_shortcuts_in_app_scope() {
        let mut keymap = TuiKeymap::default();
        keymap.global.open_transcript = Some(one("ctrl-k"));
        keymap.composer.toggle_shortcuts = Some(one("ctrl-k"));

        let err = RuntimeKeymap::from_config(&keymap).expect_err("expected shadowing conflict");
        assert!(err.contains("composer.toggle_shortcuts"));
        assert!(err.contains("open_transcript"));
    }

    #[test]
    fn rejects_shadowing_editor_binding_in_main_scope() {
        let mut keymap = TuiKeymap::default();
        keymap.composer.submit = Some(one("ctrl-j"));
        keymap.editor.insert_newline = Some(one("ctrl-j"));

        let err = RuntimeKeymap::from_config(&keymap).expect_err("expected shadowing conflict");
        assert!(err.contains("composer.submit"));
        assert!(err.contains("editor.insert_newline"));
    }

    #[test]
    fn rejects_shadowing_editor_binding_from_outer_main_handler() {
        let mut keymap = TuiKeymap::default();
        keymap.global.copy = Some(one("ctrl-y"));
        keymap.editor.yank = Some(one("ctrl-y"));

        let err = RuntimeKeymap::from_config(&keymap).expect_err("expected shadowing conflict");
        assert!(err.contains("copy"));
        assert!(err.contains("editor.yank"));
    }

    #[test]
    fn rejects_shadowing_approval_binding_in_app_scope() {
        let mut keymap = TuiKeymap::default();
        keymap.global.open_transcript = Some(one("y"));

        let err = RuntimeKeymap::from_config(&keymap).expect_err("expected shadowing conflict");
        assert!(err.contains("approval.approve"));
        assert!(err.contains("open_transcript"));
    }

    #[test]
    fn rejects_shadowing_list_binding_in_app_scope() {
        let mut keymap = TuiKeymap::default();
        keymap.global.copy = Some(one("down"));

        let err = RuntimeKeymap::from_config(&keymap).expect_err("expected shadowing conflict");
        assert!(err.contains("list.move_down"));
        assert!(err.contains("copy"));
    }

    #[test]
    fn supports_string_or_array_bindings() {
        let mut keymap = TuiKeymap::default();
        keymap.composer.submit = Some(KeybindingsSpec::Many(vec![
            KeybindingSpec("ctrl-enter".to_string()),
            KeybindingSpec("meta-enter".to_string()),
        ]));

        let err = RuntimeKeymap::from_config(&keymap).expect_err("meta is not a valid modifier");
        assert!(err.contains("tui.keymap.composer.submit"));

        keymap.composer.submit = Some(KeybindingsSpec::Many(vec![
            KeybindingSpec("ctrl-enter".to_string()),
            KeybindingSpec("alt-enter".to_string()),
        ]));

        let runtime = RuntimeKeymap::from_config(&keymap).expect("valid multi-binding");
        assert_eq!(runtime.composer.submit.len(), 2);
    }

    #[test]
    fn deduplicates_repeated_bindings_while_preserving_first_seen_order() {
        let mut keymap = TuiKeymap::default();
        keymap.composer.submit = Some(KeybindingsSpec::Many(vec![
            KeybindingSpec("ctrl-enter".to_string()),
            KeybindingSpec("ctrl-enter".to_string()),
            KeybindingSpec("alt-enter".to_string()),
        ]));

        let runtime = RuntimeKeymap::from_config(&keymap).expect("valid multi-binding");
        assert_eq!(
            runtime.composer.submit,
            vec![
                key_hint::ctrl(KeyCode::Enter),
                key_hint::alt(KeyCode::Enter)
            ]
        );
    }

    #[test]
    fn falls_back_to_global_binding_when_context_override_is_not_set() {
        let mut keymap = TuiKeymap::default();
        keymap.global.queue = Some(one("ctrl-q"));

        let runtime = RuntimeKeymap::from_config(&keymap).expect("config should parse");
        assert_eq!(
            runtime.composer.queue,
            vec![key_hint::ctrl(KeyCode::Char('q'))]
        );
    }

    #[test]
    fn invalid_global_open_transcript_binding_reports_global_path() {
        let mut keymap = TuiKeymap::default();
        keymap.global.open_transcript = Some(one("meta-t"));

        let err = RuntimeKeymap::from_config(&keymap).expect_err("expected parse error");
        assert!(err.contains("tui.keymap.global.open_transcript"));
    }

    #[test]
    fn invalid_global_open_external_editor_binding_reports_global_path() {
        let mut keymap = TuiKeymap::default();
        keymap.global.open_external_editor = Some(one("meta-g"));

        let err = RuntimeKeymap::from_config(&keymap).expect_err("expected parse error");
        assert!(err.contains("tui.keymap.global.open_external_editor"));
    }

    #[test]
    fn default_copy_binding_is_ctrl_o() {
        let runtime = RuntimeKeymap::defaults();
        assert_eq!(runtime.app.copy, vec![key_hint::ctrl(KeyCode::Char('o'))]);
    }

    #[test]
    fn defaults_include_reassignable_main_surface_actions() {
        let runtime = RuntimeKeymap::defaults();

        assert_eq!(
            runtime.app.clear_terminal,
            vec![key_hint::ctrl(KeyCode::Char('l'))]
        );
        assert_eq!(
            runtime.chat.decrease_reasoning_effort,
            vec![key_hint::alt(KeyCode::Char(','))]
        );
        assert_eq!(
            runtime.chat.increase_reasoning_effort,
            vec![key_hint::alt(KeyCode::Char('.'))]
        );
        assert_eq!(
            runtime.chat.edit_queued_message,
            vec![key_hint::alt(KeyCode::Up), key_hint::shift(KeyCode::Left)]
        );
        assert_eq!(
            runtime.composer.history_search_previous,
            vec![key_hint::ctrl(KeyCode::Char('r'))]
        );
        assert_eq!(
            runtime.composer.history_search_next,
            vec![key_hint::ctrl(KeyCode::Char('s'))]
        );
    }

    #[test]
    fn invalid_global_copy_binding_reports_global_path() {
        let mut keymap = TuiKeymap::default();
        keymap.global.copy = Some(one("meta-o"));

        let err = RuntimeKeymap::from_config(&keymap).expect_err("expected parse error");
        assert!(err.contains("tui.keymap.global.copy"));
    }

    #[test]
    fn rejects_conflicting_editor_bindings() {
        let mut keymap = TuiKeymap::default();
        keymap.editor.move_left = Some(one("ctrl-h"));
        keymap.editor.move_right = Some(one("ctrl-h"));

        expect_conflict(&keymap, "move_left", "move_right");
    }

    #[test]
    fn rejects_conflicting_pager_bindings() {
        let mut keymap = TuiKeymap::default();
        keymap.pager.scroll_up = Some(one("ctrl-u"));
        keymap.pager.scroll_down = Some(one("ctrl-u"));

        expect_conflict(&keymap, "scroll_up", "scroll_down");
    }

    #[test]
    fn rejects_conflicting_list_bindings() {
        let mut keymap = TuiKeymap::default();
        keymap.list.move_up = Some(one("up"));
        keymap.list.move_down = Some(one("up"));

        expect_conflict(&keymap, "move_up", "move_down");
    }

    #[test]
    fn rejects_conflicting_approval_bindings() {
        let mut keymap = TuiKeymap::default();
        keymap.approval.approve = Some(one("y"));
        keymap.approval.decline = Some(one("y"));

        expect_conflict(&keymap, "approve", "decline");
    }

    #[test]
    fn rejects_conflicting_approval_deny_binding() {
        let mut keymap = TuiKeymap::default();
        keymap.approval.approve = Some(one("y"));
        keymap.approval.deny = Some(one("y"));

        expect_conflict(&keymap, "approve", "deny");
    }

    #[test]
    fn rejects_conflicting_approval_overlay_accept_binding() {
        let mut keymap = TuiKeymap::default();
        keymap.list.accept = Some(one("y"));

        expect_conflict(&keymap, "list.accept", "approval.approve");
    }

    #[test]
    fn rejects_conflicting_approval_overlay_cancel_binding() {
        let mut keymap = TuiKeymap::default();
        keymap.list.cancel = Some(one("c"));

        expect_conflict(&keymap, "list.cancel", "approval.cancel");
    }

    #[test]
    fn reassignable_fixed_shortcuts_conflict_until_original_action_is_unbound() {
        let mut keymap = TuiKeymap::default();
        keymap.global.copy = Some(one("alt-."));

        expect_conflict(&keymap, "copy", "chat.increase_reasoning_effort");

        keymap.chat.increase_reasoning_effort = Some(KeybindingsSpec::Many(vec![]));
        let runtime = RuntimeKeymap::from_config(&keymap).expect("remapped key should be free");
        assert_eq!(runtime.app.copy, vec![key_hint::alt(KeyCode::Char('.'))]);
    }

    #[test]
    fn rejects_main_bindings_that_collide_with_remaining_fixed_shortcuts() {
        let mut keymap = TuiKeymap::default();
        keymap.composer.submit = Some(one("ctrl-v"));

        expect_conflict(&keymap, "composer.submit", "fixed.paste_image");
    }

    #[test]
    fn rejects_pager_bindings_that_collide_with_transcript_backtrack_keys() {
        let mut keymap = TuiKeymap::default();
        keymap.pager.close = Some(one("left"));

        expect_conflict(&keymap, "close", "fixed.transcript_edit_previous");
    }

    #[test]
    fn parses_function_keys_and_rejects_out_of_range_function_keys() {
        assert_eq!(
            parse_keybinding("f1").map(|binding| binding.parts()),
            Some((KeyCode::F(1), KeyModifiers::NONE))
        );
        assert_eq!(parse_keybinding("f13"), None);
    }

    #[test]
    fn parses_all_named_non_character_keys() {
        let cases = [
            ("tab", KeyCode::Tab),
            ("backspace", KeyCode::Backspace),
            ("esc", KeyCode::Esc),
            ("delete", KeyCode::Delete),
            ("up", KeyCode::Up),
            ("down", KeyCode::Down),
            ("left", KeyCode::Left),
            ("right", KeyCode::Right),
            ("home", KeyCode::Home),
            ("end", KeyCode::End),
            ("page-up", KeyCode::PageUp),
            ("page-down", KeyCode::PageDown),
            ("space", KeyCode::Char(' ')),
        ];

        for (spec, expected_key) in cases {
            assert_eq!(
                parse_keybinding(spec).map(|binding| binding.parts()),
                Some((expected_key, KeyModifiers::NONE)),
                "failed to parse {spec}"
            );
        }
    }

    #[test]
    fn rejects_modifier_only_and_nonnumeric_function_key_specs() {
        assert_eq!(parse_keybinding("ctrl"), None);
        assert_eq!(parse_keybinding("ff"), None);
    }

    #[test]
    fn explicit_empty_array_unbinds_action() {
        let mut keymap = TuiKeymap::default();
        keymap.composer.toggle_shortcuts = Some(KeybindingsSpec::Many(vec![]));
        let runtime = RuntimeKeymap::from_config(&keymap).expect("config should parse");
        assert!(runtime.composer.toggle_shortcuts.is_empty());
    }

    #[test]
    fn default_editor_insert_newline_includes_shift_enter() {
        let runtime = RuntimeKeymap::defaults();
        assert!(
            runtime
                .editor
                .insert_newline
                .contains(&key_hint::shift(KeyCode::Enter))
        );
    }

    #[test]
    fn default_editor_delete_forward_word_includes_alt_d() {
        let runtime = RuntimeKeymap::defaults();
        assert!(
            runtime
                .editor
                .delete_forward_word
                .contains(&key_hint::alt(KeyCode::Char('d')))
        );
    }

    #[test]
    fn default_composer_toggle_shortcuts_includes_shift_question_mark() {
        let runtime = RuntimeKeymap::defaults();
        assert!(
            runtime
                .composer
                .toggle_shortcuts
                .contains(&key_hint::shift(KeyCode::Char('?')))
        );
    }

    #[test]
    fn default_approval_open_fullscreen_includes_ctrl_shift_a() {
        let runtime = RuntimeKeymap::defaults();
        assert!(runtime.approval.open_fullscreen.contains(&KeyBinding::new(
            KeyCode::Char('a'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        )));
    }

    #[test]
    fn primary_binding_returns_first_or_none() {
        let bindings = vec![
            key_hint::ctrl(KeyCode::Char('a')),
            key_hint::shift(KeyCode::Char('b')),
        ];
        assert_eq!(
            primary_binding(&bindings),
            Some(key_hint::ctrl(KeyCode::Char('a')))
        );
        assert_eq!(primary_binding(&[]), None);
    }

    #[test]
    fn defaults_pass_conflict_validation() {
        RuntimeKeymap::defaults()
            .validate_conflicts()
            .expect("default keymap should be conflict free");
    }
}
