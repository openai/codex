//! Catalog and accessors for keymap actions shown by `/keymap`.

use std::collections::BTreeSet;

use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;

use crate::key_hint::KeyBinding;
use crate::keymap::RuntimeKeymap;

#[derive(Clone, Copy, Debug)]
pub(super) struct KeymapActionDescriptor {
    pub(super) context: &'static str,
    pub(super) context_label: &'static str,
    pub(super) action: &'static str,
    pub(super) description: &'static str,
}

const fn action(
    context: &'static str,
    context_label: &'static str,
    action: &'static str,
    description: &'static str,
) -> KeymapActionDescriptor {
    KeymapActionDescriptor {
        context,
        context_label,
        action,
        description,
    }
}

#[rustfmt::skip]
pub(super) const KEYMAP_ACTIONS: &[KeymapActionDescriptor] = &[
    action("global", "Global", "open_transcript", "Open the transcript overlay."),
    action("global", "Global", "open_external_editor", "Open the current draft in an external editor."),
    action("global", "Global", "copy", "Copy the last agent response to the clipboard."),
    action("chat", "Chat", "edit_previous_message", "Begin or advance edit-previous-message when the composer is empty."),
    action("chat", "Chat", "confirm_edit_previous_message", "Confirm the selected previous message to edit."),
    action("composer", "Composer", "submit", "Submit the current composer draft."),
    action("composer", "Composer", "queue", "Queue the draft while a task is running."),
    action("composer", "Composer", "toggle_shortcuts", "Show or hide the composer shortcut overlay."),
    action("editor", "Editor", "insert_newline", "Insert a newline in the editor."),
    action("editor", "Editor", "move_left", "Move the cursor left."),
    action("editor", "Editor", "move_right", "Move the cursor right."),
    action("editor", "Editor", "move_up", "Move the cursor up."),
    action("editor", "Editor", "move_down", "Move the cursor down."),
    action("editor", "Editor", "move_word_left", "Move to the beginning of the previous word."),
    action("editor", "Editor", "move_word_right", "Move to the end of the next word."),
    action("editor", "Editor", "move_line_start", "Move to the beginning of the line."),
    action("editor", "Editor", "move_line_end", "Move to the end of the line."),
    action("editor", "Editor", "delete_backward", "Delete one grapheme to the left."),
    action("editor", "Editor", "delete_forward", "Delete one grapheme to the right."),
    action("editor", "Editor", "delete_backward_word", "Delete the previous word."),
    action("editor", "Editor", "delete_forward_word", "Delete the next word."),
    action("editor", "Editor", "kill_line_start", "Delete from cursor to line start."),
    action("editor", "Editor", "kill_line_end", "Delete from cursor to line end."),
    action("editor", "Editor", "yank", "Paste the kill buffer."),
    action("pager", "Pager", "scroll_up", "Scroll up by one row."),
    action("pager", "Pager", "scroll_down", "Scroll down by one row."),
    action("pager", "Pager", "page_up", "Scroll up by one page."),
    action("pager", "Pager", "page_down", "Scroll down by one page."),
    action("pager", "Pager", "half_page_up", "Scroll up by half a page."),
    action("pager", "Pager", "half_page_down", "Scroll down by half a page."),
    action("pager", "Pager", "jump_top", "Jump to the beginning."),
    action("pager", "Pager", "jump_bottom", "Jump to the end."),
    action("pager", "Pager", "close", "Close the pager overlay."),
    action("pager", "Pager", "close_transcript", "Close the transcript overlay."),
    action("pager", "Pager", "edit_previous_message", "Step to an older message in backtrack preview."),
    action("pager", "Pager", "edit_next_message", "Step to a newer message in backtrack preview."),
    action("pager", "Pager", "confirm_edit_message", "Confirm the selected message in backtrack preview."),
    action("list", "List", "move_up", "Move list selection up."),
    action("list", "List", "move_down", "Move list selection down."),
    action("list", "List", "accept", "Accept the current list selection."),
    action("list", "List", "cancel", "Cancel and close selection views."),
    action("approval", "Approval", "open_fullscreen", "Open approval details fullscreen."),
    action("approval", "Approval", "approve", "Approve the primary option."),
    action("approval", "Approval", "approve_for_session", "Approve for the session when available."),
    action("approval", "Approval", "approve_for_prefix", "Approve with an exec-policy prefix when available."),
    action("approval", "Approval", "decline", "Decline and provide corrective guidance."),
    action("approval", "Approval", "cancel", "Cancel an elicitation request."),
];

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

#[rustfmt::skip]
pub(super) fn binding_slot<'a>(
    keymap: &'a mut TuiKeymap,
    context: &str,
    action: &str,
) -> Option<&'a mut Option<KeybindingsSpec>> {
    match (context, action) {
        ("global", "open_transcript") => Some(&mut keymap.global.open_transcript),
        ("global", "open_external_editor") => Some(&mut keymap.global.open_external_editor),
        ("global", "copy") => Some(&mut keymap.global.copy),
        ("chat", "edit_previous_message") => Some(&mut keymap.chat.edit_previous_message),
        ("chat", "confirm_edit_previous_message") => Some(&mut keymap.chat.confirm_edit_previous_message),
        ("composer", "submit") => Some(&mut keymap.composer.submit),
        ("composer", "queue") => Some(&mut keymap.composer.queue),
        ("composer", "toggle_shortcuts") => Some(&mut keymap.composer.toggle_shortcuts),
        ("editor", "insert_newline") => Some(&mut keymap.editor.insert_newline),
        ("editor", "move_left") => Some(&mut keymap.editor.move_left),
        ("editor", "move_right") => Some(&mut keymap.editor.move_right),
        ("editor", "move_up") => Some(&mut keymap.editor.move_up),
        ("editor", "move_down") => Some(&mut keymap.editor.move_down),
        ("editor", "move_word_left") => Some(&mut keymap.editor.move_word_left),
        ("editor", "move_word_right") => Some(&mut keymap.editor.move_word_right),
        ("editor", "move_line_start") => Some(&mut keymap.editor.move_line_start),
        ("editor", "move_line_end") => Some(&mut keymap.editor.move_line_end),
        ("editor", "delete_backward") => Some(&mut keymap.editor.delete_backward),
        ("editor", "delete_forward") => Some(&mut keymap.editor.delete_forward),
        ("editor", "delete_backward_word") => Some(&mut keymap.editor.delete_backward_word),
        ("editor", "delete_forward_word") => Some(&mut keymap.editor.delete_forward_word),
        ("editor", "kill_line_start") => Some(&mut keymap.editor.kill_line_start),
        ("editor", "kill_line_end") => Some(&mut keymap.editor.kill_line_end),
        ("editor", "yank") => Some(&mut keymap.editor.yank),
        ("pager", "scroll_up") => Some(&mut keymap.pager.scroll_up),
        ("pager", "scroll_down") => Some(&mut keymap.pager.scroll_down),
        ("pager", "page_up") => Some(&mut keymap.pager.page_up),
        ("pager", "page_down") => Some(&mut keymap.pager.page_down),
        ("pager", "half_page_up") => Some(&mut keymap.pager.half_page_up),
        ("pager", "half_page_down") => Some(&mut keymap.pager.half_page_down),
        ("pager", "jump_top") => Some(&mut keymap.pager.jump_top),
        ("pager", "jump_bottom") => Some(&mut keymap.pager.jump_bottom),
        ("pager", "close") => Some(&mut keymap.pager.close),
        ("pager", "close_transcript") => Some(&mut keymap.pager.close_transcript),
        ("pager", "edit_previous_message") => Some(&mut keymap.pager.edit_previous_message),
        ("pager", "edit_next_message") => Some(&mut keymap.pager.edit_next_message),
        ("pager", "confirm_edit_message") => Some(&mut keymap.pager.confirm_edit_message),
        ("list", "move_up") => Some(&mut keymap.list.move_up),
        ("list", "move_down") => Some(&mut keymap.list.move_down),
        ("list", "accept") => Some(&mut keymap.list.accept),
        ("list", "cancel") => Some(&mut keymap.list.cancel),
        ("approval", "open_fullscreen") => Some(&mut keymap.approval.open_fullscreen),
        ("approval", "approve") => Some(&mut keymap.approval.approve),
        ("approval", "approve_for_session") => Some(&mut keymap.approval.approve_for_session),
        ("approval", "approve_for_prefix") => Some(&mut keymap.approval.approve_for_prefix),
        ("approval", "decline") => Some(&mut keymap.approval.decline),
        ("approval", "cancel") => Some(&mut keymap.approval.cancel),
        _ => None,
    }
}

#[rustfmt::skip]
pub(super) fn bindings_for_action<'a>(
    runtime_keymap: &'a RuntimeKeymap,
    context: &str,
    action: &str,
) -> Option<&'a [KeyBinding]> {
    match (context, action) {
        ("global", "open_transcript") => Some(runtime_keymap.app.open_transcript.as_slice()),
        ("global", "open_external_editor") => Some(runtime_keymap.app.open_external_editor.as_slice()),
        ("global", "copy") => Some(runtime_keymap.app.copy.as_slice()),
        ("chat", "edit_previous_message") => Some(runtime_keymap.chat.edit_previous_message.as_slice()),
        ("chat", "confirm_edit_previous_message") => Some(runtime_keymap.chat.confirm_edit_previous_message.as_slice()),
        ("composer", "submit") => Some(runtime_keymap.composer.submit.as_slice()),
        ("composer", "queue") => Some(runtime_keymap.composer.queue.as_slice()),
        ("composer", "toggle_shortcuts") => Some(runtime_keymap.composer.toggle_shortcuts.as_slice()),
        ("editor", "insert_newline") => Some(runtime_keymap.editor.insert_newline.as_slice()),
        ("editor", "move_left") => Some(runtime_keymap.editor.move_left.as_slice()),
        ("editor", "move_right") => Some(runtime_keymap.editor.move_right.as_slice()),
        ("editor", "move_up") => Some(runtime_keymap.editor.move_up.as_slice()),
        ("editor", "move_down") => Some(runtime_keymap.editor.move_down.as_slice()),
        ("editor", "move_word_left") => Some(runtime_keymap.editor.move_word_left.as_slice()),
        ("editor", "move_word_right") => Some(runtime_keymap.editor.move_word_right.as_slice()),
        ("editor", "move_line_start") => Some(runtime_keymap.editor.move_line_start.as_slice()),
        ("editor", "move_line_end") => Some(runtime_keymap.editor.move_line_end.as_slice()),
        ("editor", "delete_backward") => Some(runtime_keymap.editor.delete_backward.as_slice()),
        ("editor", "delete_forward") => Some(runtime_keymap.editor.delete_forward.as_slice()),
        ("editor", "delete_backward_word") => Some(runtime_keymap.editor.delete_backward_word.as_slice()),
        ("editor", "delete_forward_word") => Some(runtime_keymap.editor.delete_forward_word.as_slice()),
        ("editor", "kill_line_start") => Some(runtime_keymap.editor.kill_line_start.as_slice()),
        ("editor", "kill_line_end") => Some(runtime_keymap.editor.kill_line_end.as_slice()),
        ("editor", "yank") => Some(runtime_keymap.editor.yank.as_slice()),
        ("pager", "scroll_up") => Some(runtime_keymap.pager.scroll_up.as_slice()),
        ("pager", "scroll_down") => Some(runtime_keymap.pager.scroll_down.as_slice()),
        ("pager", "page_up") => Some(runtime_keymap.pager.page_up.as_slice()),
        ("pager", "page_down") => Some(runtime_keymap.pager.page_down.as_slice()),
        ("pager", "half_page_up") => Some(runtime_keymap.pager.half_page_up.as_slice()),
        ("pager", "half_page_down") => Some(runtime_keymap.pager.half_page_down.as_slice()),
        ("pager", "jump_top") => Some(runtime_keymap.pager.jump_top.as_slice()),
        ("pager", "jump_bottom") => Some(runtime_keymap.pager.jump_bottom.as_slice()),
        ("pager", "close") => Some(runtime_keymap.pager.close.as_slice()),
        ("pager", "close_transcript") => Some(runtime_keymap.pager.close_transcript.as_slice()),
        ("pager", "edit_previous_message") => Some(runtime_keymap.pager.edit_previous_message.as_slice()),
        ("pager", "edit_next_message") => Some(runtime_keymap.pager.edit_next_message.as_slice()),
        ("pager", "confirm_edit_message") => Some(runtime_keymap.pager.confirm_edit_message.as_slice()),
        ("list", "move_up") => Some(runtime_keymap.list.move_up.as_slice()),
        ("list", "move_down") => Some(runtime_keymap.list.move_down.as_slice()),
        ("list", "accept") => Some(runtime_keymap.list.accept.as_slice()),
        ("list", "cancel") => Some(runtime_keymap.list.cancel.as_slice()),
        ("approval", "open_fullscreen") => Some(runtime_keymap.approval.open_fullscreen.as_slice()),
        ("approval", "approve") => Some(runtime_keymap.approval.approve.as_slice()),
        ("approval", "approve_for_session") => Some(runtime_keymap.approval.approve_for_session.as_slice()),
        ("approval", "approve_for_prefix") => Some(runtime_keymap.approval.approve_for_prefix.as_slice()),
        ("approval", "decline") => Some(runtime_keymap.approval.decline.as_slice()),
        ("approval", "cancel") => Some(runtime_keymap.approval.cancel.as_slice()),
        _ => None,
    }
}

pub(super) fn format_binding_summary(bindings: &[KeyBinding]) -> String {
    let mut seen = BTreeSet::new();
    let specs = bindings
        .iter()
        .filter_map(|binding| super::binding_to_config_key_spec(*binding).ok())
        .filter(|spec| seen.insert(spec.clone()))
        .collect::<Vec<_>>();
    if specs.is_empty() {
        "unbound".to_string()
    } else {
        specs.join(", ")
    }
}
