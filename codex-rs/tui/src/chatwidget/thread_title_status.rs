//! Title-generation progress on thread identity items in both status surfaces.
//!
//! The app owns requests across thread switches. Only the displayed thread's pending state
//! lives here; the existing terminal-title timer also redraws the footer while it is pending.

use super::ChatWidget;
use super::status_surfaces::TERMINAL_TITLE_SPINNER_FRAMES;
use crate::bottom_pane::StatusLineItem;
use std::time::Instant;

impl ChatWidget {
    pub(crate) fn set_thread_title_generation_pending(&mut self, pending: bool) {
        if self.status_state.thread_title_generation_pending != pending {
            self.status_state.thread_title_generation_pending = pending;
            self.refresh_status_surfaces();
            self.request_redraw();
        }
    }

    pub(super) fn status_line_value_for_item(&mut self, item: StatusLineItem) -> Option<String> {
        let value = self.status_line_value(item);
        if matches!(
            item,
            StatusLineItem::ThreadName | StatusLineItem::ThreadTitle | StatusLineItem::SessionId
        ) {
            self.with_thread_title_progress(value, Instant::now())
        } else {
            value
        }
    }

    pub(super) fn with_thread_title_progress(
        &self,
        value: Option<String>,
        now: Instant,
    ) -> Option<String> {
        if !self.status_state.thread_title_generation_pending {
            return value;
        }
        let spinner = if self.local_settings.tui.animations {
            self.terminal_title_spinner_frame_at(now)
        } else {
            TERMINAL_TITLE_SPINNER_FRAMES[0]
        };
        Some(match value {
            Some(value) => format!("{value} {spinner}"),
            None => spinner.to_string(),
        })
    }

    pub(crate) fn refresh_thread_title_progress_for_time_tick(&mut self) {
        if self.status_state.thread_title_generation_pending
            && self.local_settings.tui.animations
            && self.configured_status_line_items().iter().any(|item| {
                matches!(
                    item.parse::<StatusLineItem>(),
                    Ok(StatusLineItem::ThreadName
                        | StatusLineItem::ThreadTitle
                        | StatusLineItem::SessionId)
                )
            })
        {
            self.refresh_status_line();
            self.request_redraw();
        }
    }
}
