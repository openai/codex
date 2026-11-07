//! Show the active conversation’s leader shortcuts after the reveal delay.
//! Dispatch stays with the originating control, including before the reveal delay elapses.

use super::App;
use crate::bottom_pane::ShortcutHud;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use std::time::Duration;

/// Let fast two-key shortcuts complete before showing the panel.
pub(super) const REVEAL_DELAY: Duration = Duration::from_millis(350);

impl App {
    pub(super) fn workspace_hud(&self, tui: &crate::tui::Tui, area: Rect) -> Option<ShortcutHud> {
        if !self.conversation_focus_available(tui) || self.conversation_frame.has_caller_inset() {
            return None;
        }
        if self.chat_widget.quick_dial_menu_open() {
            return Some(self.chat_widget.quick_dial_hud());
        }
        let leader = self.key_chord_matcher.workspace_leader()?;
        let wait = REVEAL_DELAY.saturating_sub(leader.started_at.elapsed());
        if !wait.is_zero() {
            tui.frame_requester().schedule_frame_in(wait);
            return None;
        }
        let side = self.sidebar_has_focus();
        let can_switch = self.chooser_focus().is_some() || self.switchable_side_child().is_some();
        let can_size = super::side_pane::split_layout(self, tui, area).is_some();
        let can_resize = can_size && side && self.zoomed_thread.is_none();
        let mut actions = Vec::<(&str, Vec<String>)>::new();
        for binding in leader.completions(&self.keymap.chords) {
            let action = binding.action.action;
            let key = binding.chord.completion.display_label();
            if let Some((_, keys)) = actions.iter_mut().find(|(id, _)| *id == action) {
                keys.push(key);
            } else {
                actions.push((action, vec![key]));
            }
        }
        let mut groups = [
            ("Panes", Vec::new()),
            ("Move", Vec::new()),
            ("This pane", Vec::new()),
        ];
        for (action, keys) in actions {
            let (group, label) = match action {
                "focus_command_center" => (0, "Command Center"),
                "focus_main_composer" => (0, "Main composer"),
                "focus_sidebar_tabs" => (0, "Open in sidebar"),
                "focus_sidebar_content" if side => (0, "Hide sidebar"),
                "focus_sidebar_content" => (0, "Sidebar content"),
                "resize_panes" if can_size => (2, "Resize panes"),
                "resize_panes" => continue,
                "minimize_pane_width" if can_size => (2, "Minimum width"),
                "maximize_pane_width" if can_size => (2, "Maximum width"),
                "equalize_pane_widths" if can_size => (2, "Equal widths"),
                "minimize_pane_width" | "maximize_pane_width" | "equalize_pane_widths" => continue,
                "move_divider_left" if can_resize => (2, "Divider left"),
                "move_divider_right" if can_resize => (2, "Divider right"),
                "toggle_pane_zoom" if self.zoomed_thread.is_some() => (2, "Restore layout"),
                "toggle_pane_zoom" => (2, "Zoom pane"),
                "move_divider_left" | "move_divider_right" => continue,
                "close_sidebar_tab" if self.active_side_parent_thread_id().is_some() => {
                    (2, "Close tab")
                }
                "close_sidebar_tab" => continue,
                "sidebar_up" => (1, "Previous tab"),
                "sidebar_down" => (1, "Next tab"),
                "hide_pane" if side => (2, "Hide pane"),
                "focus_pane_left" if side && can_switch => (1, "Pane left"),
                "focus_pane_right" if !side && can_switch && self.open_pane_focus().is_some() => {
                    (1, "Pane right")
                }
                "hide_pane" | "focus_pane_left" | "focus_pane_right" => continue,
                "open_presets" => (2, "Presets ›"),
                "find_transcript" => (2, "Find text"),
                "select_transcript" => (2, "Select transcript"),
                "focus_conversation_composer" => (2, "Composer"),
                other => (2, other),
            };
            let key = keys.join(" / ").fg(crate::style::accent_color());
            groups[group].1.push((key, label.replace('_', " ")));
        }
        let origin = if side { "Sidebar" } else { "Conversation" };
        Some(ShortcutHud::leader(
            format!("{} · {origin}", leader.prefix.display_label()),
            groups
                .into_iter()
                .filter_map(|(title, items)| {
                    let width = items.iter().map(|(key, _)| key.width()).max()?.max(7);
                    let lines = items
                        .into_iter()
                        .map(|(key, label)| {
                            let gap = " ".repeat(width - key.width() + 1);
                            vec![key, gap.into(), label.into()].into()
                        })
                        .collect();
                    Some((title, lines))
                })
                .collect(),
        ))
    }
}
