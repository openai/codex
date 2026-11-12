//! Separate persistent transcript context from the composer's transient hint row.
//! Measurement, rendering, and cursor placement share the same reserved status rectangle.
//! Shortcut help grows above the input, preserving status and close rows below it even when clipped.

use super::*;
use ratatui::layout::Size;

impl ChatComposer {
    pub(crate) fn set_status_line(&mut self, status_line: Option<StatusLine>) -> bool {
        if self.footer.status_line_value == status_line {
            return false;
        }
        if status_line
            .as_ref()
            .is_some_and(|status| status.as_native().is_none())
        {
            self.effort_status_line_transition = None;
        }
        *self.footer.labeled_status_line.get_mut() = None;
        self.footer.status_line_value = status_line;
        true
    }

    pub(crate) fn set_status_line_enabled(&mut self, enabled: bool) -> bool {
        if self.footer.status_line_enabled == enabled {
            return false;
        }
        self.footer.status_line_enabled = enabled;
        true
    }

    pub(crate) fn set_side_conversation_context_label(&mut self, label: Option<String>) -> bool {
        if self.footer.side_conversation_context_label == label {
            return false;
        }
        self.footer.side_conversation_context_label = label;
        true
    }

    pub(super) fn status_surface_height(
        &self,
        mut bounds: Size,
        options: ComposerRenderOptions<'_>,
    ) -> u16 {
        let focus = options.status_line_focus();
        if focus == StatusLineFocus::Focused && self.active_status_transition().is_some() {
            bounds.height = bounds.height.min(1);
        }
        self.with_status_surface_content(focus, |status| {
            status.map_or(0, |status| status.height(bounds))
        })
    }

    pub(super) fn hint_footer_props(&self) -> FooterProps {
        let mut props = self.footer_props();
        if props.status_line_enabled {
            props.status_line_enabled = false;
            props.status_line_value = None;
            props.active_agent_label = None;
        }
        props
    }

    pub(super) fn with_status_surface_content<T>(
        &self,
        focus: StatusLineFocus,
        use_status: impl FnOnce(Option<&StatusLine>) -> T,
    ) -> T {
        if !self.footer.status_line_enabled {
            return use_status(None);
        }
        let status = self.footer.status_line_value.as_ref();
        if status.is_some_and(|status| status.as_native().is_none())
            || self.footer.active_agent_label.is_none()
            || focus == StatusLineFocus::Unfocused
        {
            return use_status(status);
        }
        let mut props = self.footer_props();
        // The viewed agent belongs to native status even during a transient composer hint.
        props.mode = FooterMode::ComposerHasDraft;
        props.is_task_running = false;
        let next = passive_footer_status_line(&props);
        let mut labeled = self.footer.labeled_status_line.borrow_mut();
        // Preserve wrapped rows while the effective line, including the label style, stays equal.
        if *labeled != next {
            *labeled = next;
        }
        use_status(labeled.as_ref())
    }

    fn active_status_transition(&self) -> Option<&EffortStatusLineTransition> {
        self.effort_status_line_transition
            .as_ref()
            .filter(|transition| !transition.is_finished())
    }

    pub(super) fn render_status_surface(
        &self,
        area: Rect,
        buf: &mut Buffer,
        options: ComposerRenderOptions<'_>,
    ) {
        if area.is_empty() {
            return;
        }
        let focus = options.status_line_focus();
        self.with_status_surface_content(focus, |status| {
            if focus == StatusLineFocus::Focused
                && let Some(transition) = self.active_status_transition()
            {
                let target = status.and_then(StatusLine::as_native).cloned();
                if let Some(frame) = transition.render_line(target, area.width) {
                    StatusLine::from(frame).render(area, buf, focus);
                }
                if let Some(requester) = &self.frame_requester {
                    requester.schedule_frame_in(EFFORT_STATUS_LINE_FRAME_TICK);
                }
            } else if let Some(status) = status {
                status.render(area, buf, focus);
            }
        });
    }
}

impl ComposerRenderOptions<'_> {
    fn status_line_focus(self) -> StatusLineFocus {
        if self.unfocused {
            StatusLineFocus::Unfocused
        } else {
            StatusLineFocus::Focused
        }
    }
}

#[cfg(test)]
#[path = "status_surface_tests.rs"]
mod tests;
