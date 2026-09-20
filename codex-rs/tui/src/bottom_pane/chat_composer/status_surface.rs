//! Separate persistent transcript context from the composer's transient hint row.
//! Measurement, rendering, and cursor placement share the same reserved status rectangle.
//! Shortcut help grows above the input, preserving status and close rows below it even when clipped.

use super::*;

impl ChatComposer {
    pub(super) fn status_surface_height(&self, options: ComposerRenderOptions<'_>) -> u16 {
        u16::from(options.separate_status_line && self.footer.status_line_enabled)
    }

    pub(super) fn hint_footer_props(&self, options: ComposerRenderOptions<'_>) -> FooterProps {
        let mut props = self.footer_props();
        if self.status_surface_height(options) > 0 {
            props.status_line_enabled = false;
            props.status_line_value = None;
            props.active_agent_label = None;
        }
        props
    }

    pub(super) fn render_status_surface(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let mut props = self.footer_props();
        // Persistent context is independent of help, queue, search, and quit modes.
        props.mode = FooterMode::ComposerHasDraft;
        props.is_task_running = false;
        let line = passive_footer_status_line(&props);
        let right = self.mode_indicator_line(/*show_cycle_hint*/ false);
        let right_width = right
            .as_ref()
            .map_or(/*default*/ 0, |line| line.width() as u16);
        let width = max_left_width_for_right(area, right_width)
            .unwrap_or_else(|| inset_footer_hint_area(area).width);
        let transition = self
            .effort_status_line_transition
            .as_ref()
            .filter(|transition| !transition.is_finished());
        let line = if let Some(transition) = transition {
            transition.render_line(line.as_ref(), width)
        } else {
            line
        };
        if let Some(line) = line {
            render_footer_line(
                area,
                buf,
                truncate_line_with_ellipsis_if_overflow(line, usize::from(width)),
            );
        }
        if let Some(right) = right {
            render_context_right(area, buf, &right);
        }
        if let Some(url) = self.footer.status_line_hyperlink_url.as_deref() {
            mark_underlined_hyperlink(buf, area, url);
        }
        if transition.is_some()
            && let Some(frame_requester) = &self.frame_requester
        {
            frame_requester.schedule_frame_in(EFFORT_STATUS_LINE_FRAME_TICK);
        }
    }
}

#[cfg(test)]
#[path = "status_surface_tests.rs"]
mod tests;
