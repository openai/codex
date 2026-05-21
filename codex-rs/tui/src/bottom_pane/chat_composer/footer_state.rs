//! Footer and status-row presentation state for the chat composer.

use std::time::Instant;

use ratatui::text::Line;

use crate::bottom_pane::footer::CollaborationModeIndicator;
use crate::bottom_pane::footer::FooterMode;
use crate::bottom_pane::footer::GoalStatusIndicator;
use crate::key_hint::KeyBinding;
#[cfg(test)]
use std::time::Duration;

pub(super) struct FooterState {
    pub(super) quit_shortcut_expires_at: Option<Instant>,
    pub(super) quit_shortcut_key: KeyBinding,
    pub(super) esc_backtrack_hint: bool,
    pub(super) use_shift_enter_hint: bool,
    pub(super) mode: FooterMode,
    pub(super) hint_override: Option<Vec<(String, String)>>,
    pub(super) plan_mode_nudge_visible: bool,
    pub(super) flash: Option<FooterFlash>,
    pub(super) context_window_percent: Option<i64>,
    pub(super) context_window_used_tokens: Option<i64>,
    pub(super) collaboration_mode_indicator: Option<CollaborationModeIndicator>,
    pub(super) goal_status_indicator: Option<GoalStatusIndicator>,
    pub(super) ide_context_active: bool,
    pub(super) status_line_value: Option<Line<'static>>,
    pub(super) status_line_hyperlink_url: Option<String>,
    pub(super) status_line_enabled: bool,
    pub(super) side_conversation_context_label: Option<String>,
    pub(super) active_agent_label: Option<String>,
    pub(super) external_editor_key: Option<KeyBinding>,
    pub(super) show_transcript_key: Option<KeyBinding>,
    pub(super) insert_newline_key: Option<KeyBinding>,
    pub(super) queue_key: Option<KeyBinding>,
    pub(super) toggle_shortcuts_key: Option<KeyBinding>,
    pub(super) history_search_key: Option<KeyBinding>,
    pub(super) reasoning_down_key: Option<KeyBinding>,
    pub(super) reasoning_up_key: Option<KeyBinding>,
}

#[derive(Clone, Debug)]
pub(super) struct FooterFlash {
    pub(super) line: Line<'static>,
    pub(super) expires_at: Instant,
}

impl FooterState {
    pub(super) fn flash_visible(&self) -> bool {
        self.flash
            .as_ref()
            .is_some_and(|flash| Instant::now() < flash.expires_at)
    }

    #[cfg(test)]
    pub(super) fn show_flash(&mut self, line: Line<'static>, duration: Duration) {
        let expires_at = Instant::now()
            .checked_add(duration)
            .unwrap_or_else(Instant::now);
        self.flash = Some(FooterFlash { line, expires_at });
    }

    #[cfg(test)]
    pub(super) fn status_line_text(&self) -> Option<String> {
        self.status_line_value.as_ref().map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
    }
}
