//! Catalog and accessors for keymap actions shown by `/keymap`.
//!
//! The catalog declaration generates both the UI-facing descriptors and the
//! matching root-config accessors. Each entry ties together the config path,
//! user-facing context label, stable action name, short description, and
//! optional feature gate used by the picker and action menu.
//!
//! Runtime lookups reuse the inventory owned by [`crate::keymap`]. A catalog
//! action must remain both writable in `TuiKeymap` and readable from the shared
//! runtime inventory.

use std::collections::BTreeSet;

use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyEvent;

use crate::keymap::RuntimeKeymap;
use crate::keymap::bindings_for_action;

#[derive(Clone, Copy, Debug)]
pub(super) struct KeymapActionDescriptor {
    /// Config context segment, such as `composer` in `tui.keymap.composer.submit`.
    pub(super) context: &'static str,
    /// Human-readable group label shown in the picker.
    pub(super) context_label: &'static str,
    /// Config action segment, such as `submit` in `tui.keymap.composer.submit`.
    pub(super) action: &'static str,
    /// Short user-facing explanation of what the action does.
    pub(super) description: &'static str,
    /// Feature required before the action appears in `/keymap`.
    required_feature: Option<KeymapActionFeature>,
}

#[derive(Clone, Copy, Debug)]
enum KeymapActionFeature {
    FastMode,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct KeymapActionFilter {
    pub(crate) fast_mode_enabled: bool,
}

impl KeymapActionDescriptor {
    pub(super) fn is_visible(self, filter: KeymapActionFilter) -> bool {
        match self.required_feature {
            None => true,
            Some(KeymapActionFeature::FastMode) => filter.fast_mode_enabled,
        }
    }
}

macro_rules! required_feature {
    () => {
        None
    };
    ($feature:ident) => {
        Some(KeymapActionFeature::$feature)
    };
}

macro_rules! define_keymap_actions {
    (
        $(
            $context:ident $context_label:literal {
                $(
                    $action:ident $( [$required_feature:ident] )? => $description:literal,
                )*
            }
        )*
    ) => {
        pub(super) const KEYMAP_ACTIONS: &[KeymapActionDescriptor] = &[
            $(
                $(
                    KeymapActionDescriptor {
                        context: stringify!($context),
                        context_label: $context_label,
                        action: stringify!($action),
                        description: $description,
                        required_feature: required_feature!($($required_feature)?),
                    },
                )*
            )*
        ];

        /// Return the mutable root-config binding slot for one catalog action.
        ///
        /// The returned `Option<KeybindingsSpec>` distinguishes three states that the
        /// editor must preserve: absent means use fallback/default resolution, `Some`
        /// with one or more keys is a custom binding, and `Some(Many([]))` is an
        /// explicit unbind.
        pub(super) fn binding_slot<'a>(
            keymap: &'a mut TuiKeymap,
            context: &str,
            action: &str,
        ) -> Option<&'a mut Option<KeybindingsSpec>> {
            match (context, action) {
                $(
                    $(
                        (stringify!($context), stringify!($action)) => {
                            Some(&mut keymap.$context.$action)
                        }
                    )*
                )*
                _ => None,
            }
        }
    };
}

define_keymap_actions! {
    global "Global" {
        open_agents => "Open the shared agent-session overview.",
        open_transcript => "Open the transcript overlay.",
        find_transcript => "Find text in the full transcript.",
        // Keep the new warnings action out of shared-config writes until older strict readers
        // accept it. Its built-in shortcuts remain available.
        focus_activity => "Focus transcript activity groups to expand or collapse their details.",
        open_external_editor => "Open the current draft in an external editor.",
        copy => "Copy the last agent response to the clipboard.",
        clear_terminal => "Clear the terminal UI.",
        toggle_vim_mode => "Turn Vim composer mode on or off.",
        toggle_fast_mode [FastMode] => "Turn Fast mode on or off.",
        toggle_raw_output => "Toggle raw scrollback mode.",
        toggle_side_conversation => "Switch between a side conversation and its parent.",
        leader => "Set the prefix used by shared leader shortcuts.",
    }
    chat "Chat" {
        interrupt_turn => "Interrupt the active turn.",
        decrease_reasoning_effort => "Decrease reasoning effort.",
        increase_reasoning_effort => "Increase reasoning effort.",
        previous_permission_mode => "Switch to the previous available permission mode.",
        next_permission_mode => "Switch to the next available permission mode.",
        edit_queued_message => "Move forward through questions, then edit the last queued message.",
        prompt_stack_back => "Move back through questions toward the composer.",
        skip_question => "Skip the focused question.",
        toggle_voice => "Start or stop a voice conversation.",
        open_presets => "Run or save reusable inputs.",
        toggle_voice_mute => "Mute or unmute the voice microphone.",
    }
    composer "Composer" {
        submit => "Submit the current composer draft.",
        queue => "Queue the draft while a task is running.",
        toggle_shortcuts => "Show or hide the composer shortcut overlay.",
        history_search_previous => "Open history search or move to the previous match.",
        history_search_next => "Move to the next history search match.",
    }
    editor "Editor" {
        insert_newline => "Insert a newline in the editor.",
        move_left => "Move the cursor left.",
        move_right => "Move the cursor right.",
        move_up => "Move the cursor up.",
        move_down => "Move the cursor down.",
        move_word_left => "Move to the beginning of the previous word.",
        move_word_right => "Move to the end of the next word.",
        move_line_start => "Move to the beginning of the line.",
        move_line_end => "Move to the end of the line.",
        delete_backward => "Delete one grapheme to the left.",
        delete_forward => "Delete one grapheme to the right.",
        delete_backward_word => "Delete the previous word.",
        delete_forward_word => "Delete the next word.",
        kill_line_start => "Delete from cursor to line start.",
        kill_whole_line => "Delete the current line.",
        kill_line_end => "Delete from cursor to line end.",
        yank => "Paste the kill buffer.",
    }
    vim_normal "Vim normal" {
        enter_insert => "Enter insert mode at the cursor.",
        append_after_cursor => "Enter insert mode after the cursor.",
        append_line_end => "Enter insert mode at end of line.",
        insert_line_start => "Enter insert mode at the first non-blank character.",
        open_line_below => "Open a new line below and enter insert mode.",
        open_line_above => "Open a new line above and enter insert mode.",
        enter_replace_mode => "Enter replace mode and overwrite characters.",
        move_left => "Move left in Vim normal mode.",
        move_right => "Move right in Vim normal mode.",
        move_up => "Move up or recall older history in Vim normal mode.",
        move_down => "Move down or recall newer history in Vim normal mode.",
        move_word_forward => "Move to the start of the next word.",
        move_word_backward => "Move to the start of the previous word.",
        move_word_end => "Move to the end of the current or next word.",
        move_line_start => "Move to the start of the line.",
        move_line_end => "Move to the end of the line.",
        find_forward => "Find the next character on the current line.",
        find_backward => "Find the previous character on the current line.",
        till_forward => "Stop before the next character on the current line.",
        till_backward => "Stop after the previous character on the current line.",
        jump_top => "Jump to the first buffer line.",
        jump_bottom => "Jump to the last buffer line.",
        delete_char => "Delete the character under the cursor.",
        replace_char => "Replace the character under the cursor.",
        repeat_last_change => "Repeat the last complete edit.",
        substitute_char => "Delete the character under the cursor and enter insert mode.",
        delete_to_line_end => "Delete from cursor to end of line.",
        change_to_line_end => "Change from cursor to end of line and enter insert mode.",
        yank_line => "Yank the entire line.",
        paste_after => "Paste after the cursor.",
        start_delete_operator => "Begin a delete operator and wait for a motion.",
        start_yank_operator => "Begin a yank operator and wait for a motion.",
        start_change_operator => "Begin a change operator and wait for a motion or text object.",
        undo => "Undo the last complete edit.",
        redo => "Redo the last undone edit.",
        cancel_operator => "Cancel a pending Vim operator.",
    }
    vim_search "Vim search" {
        forward => "Search forward in the active buffer.",
        backward => "Search backward in the active buffer.",
        next => "Repeat the accepted search.",
        previous => "Repeat the search in the opposite direction.",
    }
    vim_operator "Vim operator" {
        delete_line => "Repeat delete operator to delete the whole line.",
        yank_line => "Repeat yank operator to yank the whole line.",
        motion_left => "Operator motion left.",
        motion_right => "Operator motion right.",
        motion_up => "Operator motion up.",
        motion_down => "Operator motion down.",
        motion_word_forward => "Operator motion to start of next word.",
        motion_word_backward => "Operator motion to start of previous word.",
        motion_word_end => "Operator motion to end of word.",
        motion_line_start => "Operator motion to line start.",
        motion_line_end => "Operator motion to line end.",
        motion_find_forward => "Operator motion to the next character on the current line.",
        motion_find_backward => "Operator motion to the previous character on the current line.",
        motion_till_forward => "Stop before the next character on the current line.",
        motion_till_backward => "Stop after the previous character on the current line.",
        motion_jump_top => "Operator motion to the first buffer line.",
        motion_jump_bottom => "Operator motion to the last buffer line.",
        select_inner_text_object => "Select an inner text object.",
        select_around_text_object => "Select an around text object.",
        cancel => "Cancel the pending operator.",
    }
    vim_text_object "Vim text object" {
        word => "Target the current word.",
        big_word => "Target the current WORD.",
        parentheses => "Target enclosing parentheses.",
        brackets => "Target enclosing brackets.",
        braces => "Target enclosing braces.",
        double_quote => "Target enclosing double quotes.",
        single_quote => "Target enclosing single quotes.",
        backtick => "Target enclosing backticks.",
        cancel => "Cancel the pending text object.",
    }
    pager "Pager" {
        scroll_up => "Scroll up by one row.",
        scroll_down => "Scroll down by one row.",
        page_up => "Scroll up by one page.",
        page_down => "Scroll down by one page.",
        half_page_up => "Scroll up by half a page.",
        half_page_down => "Scroll down by half a page.",
        jump_top => "Jump to the beginning.",
        jump_bottom => "Jump to the end.",
        close => "Close the pager overlay.",
        close_transcript => "Close the transcript overlay.",
        find => "Find text in a transcript pager.",
    }
    list "List" {
        move_up => "Move list selection up.",
        move_down => "Move list selection down.",
        move_left => "Move horizontally left in list pickers.",
        move_right => "Move horizontally right in list pickers.",
        page_up => "Move list selection up by one page.",
        page_down => "Move list selection down by one page.",
        jump_top => "Jump to the first list item.",
        jump_bottom => "Jump to the last list item.",
        accept => "Accept the current list selection.",
        cancel => "Cancel and close selection views.",
    }
    agents "Agents" {
        resume => "Open the session resume picker.",
        search => "Search the available agent tasks.",
        new_task => "Open a new session in the selected checkout.",
        new_worktree => "Open a new session in a worktree from the project default branch.",
        fork => "Fork the selected conversation and open the new session.",
        rename => "Rename the selected task.",
        stop => "Stop the selected running task.",
        archive => "Archive the selected task and its child agents.",
        delete => "Permanently delete the selected task and its child agents.",
        hide => "Hide the selected task until explicitly resumed.",
        toggle_grouping => "Cycle task grouping by project, directory, status, or model.",
    }
    approval "Approval" {
        open_fullscreen => "Open approval details fullscreen.",
        open_thread => "Open the approval source thread when available.",
        approve => "Approve the primary option.",
        approve_for_session => "Approve for the session when available.",
        approve_for_prefix => "Approve with an exec-policy prefix when available.",
        deny => "Choose the explicit deny option when available.",
        decline => "Decline and provide corrective guidance.",
        cancel => "Cancel an elicitation request.",
    }
}

/// Convert a stable action identifier into a display label.
///
/// This is intentionally presentation-only: the returned string must never be
/// parsed back into an action name, because underscores and casing are part of
/// the stable config contract.
pub(super) fn action_label(action: &str) -> String {
    action
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format an action's active single-key and chord alternatives in config order.
///
/// Duplicate runtime variants that normalize to the same config spec are shown
/// once so compatibility defaults do not appear as separate user choices.
pub(super) fn format_action_binding_summary(
    runtime_keymap: &RuntimeKeymap,
    context: &str,
    action: &str,
) -> String {
    let specs = super::active_binding_specs(runtime_keymap, context, action).unwrap_or_else(|_| {
        bindings_for_action(runtime_keymap, context, action)
            .unwrap_or_default()
            .iter()
            .filter_map(|binding| super::binding_to_config_key_spec(*binding).ok())
            .collect()
    });
    let mut seen = BTreeSet::new();
    let specs = specs
        .into_iter()
        .filter(|spec| seen.insert(spec.clone()))
        .map(|spec| {
            let Some(completion) = spec.strip_prefix("leader ") else {
                return spec;
            };
            let resolved = runtime_keymap
                .chords
                .leader
                .iter()
                .filter_map(|prefix| super::binding_to_config_key_spec(*prefix).ok())
                .map(|prefix| format!("{prefix} {completion}"))
                .collect::<Vec<_>>();
            if resolved.is_empty() {
                format!("{spec} (inactive)")
            } else {
                format!("{spec} ({})", resolved.join(", "))
            }
        })
        .collect::<Vec<_>>();
    if specs.is_empty() {
        "unbound".to_string()
    } else {
        specs.join(", ")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum KeymapDebugBindingSource {
    Custom,
    CustomGlobal,
    Default,
}

impl KeymapDebugBindingSource {
    pub(super) const fn label(&self) -> &'static str {
        match self {
            Self::Custom => "Custom",
            Self::CustomGlobal => "Custom global",
            Self::Default => "Default",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct KeymapDebugActionMatch {
    pub(super) context: &'static str,
    pub(super) action: &'static str,
    pub(super) label: String,
    pub(super) description: &'static str,
    pub(super) source: KeymapDebugBindingSource,
}

pub(super) fn matching_actions_for_key_event(
    runtime_keymap: &RuntimeKeymap,
    keymap_config: &TuiKeymap,
    event: KeyEvent,
) -> Vec<KeymapDebugActionMatch> {
    KEYMAP_ACTIONS
        .iter()
        .filter_map(|descriptor| {
            let bindings =
                bindings_for_action(runtime_keymap, descriptor.context, descriptor.action)?;
            bindings
                .iter()
                .any(|binding| binding.is_press(event))
                .then(|| KeymapDebugActionMatch {
                    context: descriptor.context,
                    action: descriptor.action,
                    label: action_label(descriptor.action),
                    description: descriptor.description,
                    source: debug_binding_source(keymap_config, descriptor),
                })
        })
        .collect()
}

fn debug_binding_source(
    keymap_config: &TuiKeymap,
    descriptor: &KeymapActionDescriptor,
) -> KeymapDebugBindingSource {
    let mut keymap_config = keymap_config.clone();
    let Some(slot) = binding_slot(&mut keymap_config, descriptor.context, descriptor.action) else {
        return KeymapDebugBindingSource::Default;
    };
    if slot.is_some() {
        return KeymapDebugBindingSource::Custom;
    }

    let Some(global_slot) = global_fallback_slot(&mut keymap_config, descriptor) else {
        return KeymapDebugBindingSource::Default;
    };
    if global_slot.is_some() {
        KeymapDebugBindingSource::CustomGlobal
    } else {
        KeymapDebugBindingSource::Default
    }
}

fn global_fallback_slot<'a>(
    keymap: &'a mut TuiKeymap,
    descriptor: &KeymapActionDescriptor,
) -> Option<&'a mut Option<KeybindingsSpec>> {
    if descriptor.context != "composer" {
        return None;
    }

    match descriptor.action {
        "submit" => Some(&mut keymap.global.submit),
        "queue" => Some(&mut keymap.global.queue),
        "toggle_shortcuts" => Some(&mut keymap.global.toggle_shortcuts),
        _ => None,
    }
}
