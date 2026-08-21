//! Shared inventory of resolved runtime bindings.
//!
//! `/keymap` lookup and chord-prefix conflict detection must inspect the same
//! actions. Declaring those actions once prevents a new binding from appearing
//! in the picker while being silently omitted from chord validation.

use super::RuntimeKeymap;
use crate::key_hint::KeyBinding;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use std::sync::Arc;

/// Config context in which a keymap action is active.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum KeymapContext {
    Global,
    Chat,
    Composer,
    Editor,
    VimNormal,
    VimOperator,
    VimTextObject,
    Pager,
    List,
    Agents,
    Approval,
}

impl KeymapContext {
    pub(crate) const fn config_name(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Chat => "chat",
            Self::Composer => "composer",
            Self::Editor => "editor",
            Self::VimNormal => "vim_normal",
            Self::VimOperator => "vim_operator",
            Self::VimTextObject => "vim_text_object",
            Self::Pager => "pager",
            Self::List => "list",
            Self::Agents => "agents",
            Self::Approval => "approval",
        }
    }

    pub(crate) const fn allows_plain_chord_prefix(self) -> bool {
        matches!(
            self,
            Self::VimNormal | Self::VimOperator | Self::VimTextObject
        )
    }

    pub(crate) fn overlaps(self, other: Self) -> bool {
        if self == other {
            return true;
        }

        matches!(
            (self, other),
            (Self::List, Self::Approval)
                | (Self::Approval, Self::List)
                | (Self::List, Self::Agents)
                | (Self::Agents, Self::List)
                | (Self::Chat, Self::List)
                | (Self::List, Self::Chat)
        ) || self.is_shared_main() && other.is_main_editor()
            || other.is_shared_main() && self.is_main_editor()
            || self.is_shared_main() && other.is_shared_main()
    }

    const fn is_shared_main(self) -> bool {
        matches!(self, Self::Global | Self::Chat | Self::Composer)
    }

    const fn is_main_editor(self) -> bool {
        matches!(
            self,
            Self::Editor | Self::VimNormal | Self::VimOperator | Self::VimTextObject
        )
    }
}

/// Stable identity for one configurable action.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct KeymapActionId {
    pub(crate) context: KeymapContext,
    pub(crate) action: &'static str,
}

impl KeymapActionId {
    pub(crate) fn config_path(self) -> String {
        format!("tui.keymap.{}.{}", self.context.config_name(), self.action)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RuntimeActionBinding<'a> {
    pub(super) id: KeymapActionId,
    pub(super) bindings: &'a [KeyBinding],
}

macro_rules! runtime_group_mut {
    ($runtime_keymap:expr, editor) => {
        Arc::make_mut(&mut $runtime_keymap.editor)
    };
    ($runtime_keymap:expr, $group:ident) => {
        &mut $runtime_keymap.$group
    };
}

macro_rules! define_runtime_action_bindings {
    ($($context:literal => $context_id:ident, $group:ident, $config_group:ident [$($action:ident),+ $(,)?]),+ $(,)?) => {
        /// Resolve a config context/action pair to its runtime identity.
        pub(crate) fn keymap_action_id(
            context: &str,
            action: &str,
        ) -> Option<KeymapActionId> {
            match (context, action) {
                $(
                    $(
                        ($context, stringify!($action)) => Some(KeymapActionId {
                            context: KeymapContext::$context_id,
                            action: stringify!($action),
                        }),
                    )+
                )+
                _ => None,
            }
        }

        /// Return the root-config slot for one runtime action.
        pub(super) fn configured_binding_for_action(
            keymap: &TuiKeymap,
            action: KeymapActionId,
        ) -> Option<&Option<KeybindingsSpec>> {
            match (action.context.config_name(), action.action) {
                $(
                    $(
                        ($context, stringify!($action)) => {
                            Some(&keymap.$config_group.$action)
                        }
                    )+
                )+
                _ => None,
            }
        }

        /// Iterate over every stable runtime action identity.
        pub(crate) fn keymap_action_ids() -> impl Iterator<Item = KeymapActionId> {
            [
                $(
                    $(
                        KeymapActionId {
                            context: KeymapContext::$context_id,
                            action: stringify!($action),
                        },
                    )+
                )+
            ]
            .into_iter()
        }

        /// Return resolved bindings for one stable config context and action.
        pub(crate) fn bindings_for_action<'a>(
            runtime_keymap: &'a RuntimeKeymap,
            context: &str,
            action: &str,
        ) -> Option<&'a [KeyBinding]> {
            match (context, action) {
                $(
                    $(
                        ($context, stringify!($action)) => {
                            Some(super::user_bindings(
                                runtime_keymap.$group.$action.as_slice(),
                            ))
                        }
                    )+
                )+
                _ => None,
            }
        }

        /// Append an internal dispatch binding to one resolved action.
        pub(super) fn push_binding_for_action(
            runtime_keymap: &mut RuntimeKeymap,
            action: KeymapActionId,
            binding: KeyBinding,
        ) -> bool {
            match (action.context.config_name(), action.action) {
                $(
                    $(
                        ($context, stringify!($action)) => {
                            runtime_group_mut!(runtime_keymap, $group).$action.push(binding);
                            true
                        }
                    )+
                )+
                _ => false,
            }
        }

        /// Iterate over the same runtime bindings exposed to the keymap editor.
        pub(super) fn runtime_action_bindings(
            runtime_keymap: &RuntimeKeymap,
        ) -> impl Iterator<Item = RuntimeActionBinding<'_>> {
            [
                $(
                    $(
                        RuntimeActionBinding {
                            id: KeymapActionId {
                                context: KeymapContext::$context_id,
                                action: stringify!($action),
                            },
                            bindings: super::user_bindings(
                                runtime_keymap.$group.$action.as_slice(),
                            ),
                        },
                    )+
                )+
            ]
            .into_iter()
        }
    };
}

define_runtime_action_bindings! {
    "global" => Global, app, global [
        open_agents,
        open_transcript,
        open_external_editor,
        copy,
        clear_terminal,
        toggle_vim_mode,
        toggle_fast_mode,
        toggle_raw_output,
        toggle_side_conversation,
    ],
    "chat" => Chat, chat, chat [
        interrupt_turn,
        decrease_reasoning_effort,
        increase_reasoning_effort,
        previous_permission_mode,
        next_permission_mode,
        edit_queued_message,
    ],
    "composer" => Composer, composer, composer [
        submit,
        queue,
        toggle_shortcuts,
        history_search_previous,
        history_search_next,
    ],
    "editor" => Editor, editor, editor [
        insert_newline,
        move_left,
        move_right,
        move_up,
        move_down,
        move_word_left,
        move_word_right,
        move_line_start,
        move_line_end,
        delete_backward,
        delete_forward,
        delete_backward_word,
        delete_forward_word,
        kill_line_start,
        kill_whole_line,
        kill_line_end,
        yank,
    ],
    "vim_normal" => VimNormal, vim_normal, vim_normal [
        enter_insert,
        append_after_cursor,
        append_line_end,
        insert_line_start,
        open_line_below,
        open_line_above,
        move_left,
        move_right,
        move_up,
        move_down,
        move_word_forward,
        move_word_backward,
        move_word_end,
        move_line_start,
        move_line_end,
        delete_char,
        replace_char,
        substitute_char,
        delete_to_line_end,
        change_to_line_end,
        yank_line,
        paste_after,
        start_delete_operator,
        start_yank_operator,
        start_change_operator,
        cancel_operator,
    ],
    "vim_operator" => VimOperator, vim_operator, vim_operator [
        delete_line,
        yank_line,
        motion_left,
        motion_right,
        motion_up,
        motion_down,
        motion_word_forward,
        motion_word_backward,
        motion_word_end,
        motion_line_start,
        motion_line_end,
        select_inner_text_object,
        select_around_text_object,
        cancel,
    ],
    "vim_text_object" => VimTextObject, vim_text_object, vim_text_object [
        word,
        big_word,
        parentheses,
        brackets,
        braces,
        double_quote,
        single_quote,
        backtick,
        cancel,
    ],
    "pager" => Pager, pager, pager [
        scroll_up,
        scroll_down,
        page_up,
        page_down,
        half_page_up,
        half_page_down,
        jump_top,
        jump_bottom,
        close,
        close_transcript,
    ],
    "list" => List, list, list [
        move_up,
        move_down,
        move_left,
        move_right,
        page_up,
        page_down,
        jump_top,
        jump_bottom,
        accept,
        cancel,
    ],
    "agents" => Agents, agents, agents [
        search,
        new_task,
        rename,
        stop,
        toggle_grouping,
    ],
    "approval" => Approval, approval, approval [
        open_fullscreen,
        open_thread,
        approve,
        approve_for_session,
        approve_for_prefix,
        deny,
        decline,
        cancel,
    ],
}
