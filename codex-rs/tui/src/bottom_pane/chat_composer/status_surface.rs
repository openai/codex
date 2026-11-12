//! Separate persistent transcript context from the composer's transient hint row.
//! Measurement, rendering, and cursor placement share the same reserved status rectangle.
//! Shortcut help grows above the input, preserving status and close rows below it even when clipped.

use super::*;
use crate::bottom_pane::footer_content::StatusLineFocus;

use std::borrow::Cow;

impl ChatComposer {
    pub(crate) fn set_status_line(&mut self, status_line: Option<FooterContent>) -> bool {
        if self.footer.status_line_value == status_line {
            return false;
        }
        if matches!(status_line, Some(FooterContent::Extension { .. })) {
            self.effort_status_line_transition = None;
        }
        self.footer.status_layout = Default::default();
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
        width: u16,
        available: u16,
        options: ComposerRenderOptions<'_>,
    ) -> u16 {
        let focus = options.status_line_focus();
        let available =
            if focus == StatusLineFocus::Focused && self.active_status_transition().is_some() {
                available.min(1)
            } else {
                available
            };
        self.footer.status_layout.height(
            self.status_surface_content(focus).as_deref(),
            width,
            available,
        )
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

    pub(super) fn status_surface_content(
        &self,
        focus: StatusLineFocus,
    ) -> Option<Cow<'_, FooterContent>> {
        if !self.footer.status_line_enabled {
            return None;
        }
        let status = self.footer.status_line_value.as_ref();
        if matches!(status, Some(FooterContent::Extension { .. }))
            || self.footer.active_agent_label.is_none()
            || focus == StatusLineFocus::Unfocused
        {
            return status.map(Cow::Borrowed);
        }
        let mut props = self.footer_props();
        // The viewed agent belongs to native status even during a transient composer hint.
        props.mode = FooterMode::ComposerHasDraft;
        props.is_task_running = false;
        passive_footer_status_line(&props).map(Cow::Owned)
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
        let mut content = self.status_surface_content(focus);
        if focus == StatusLineFocus::Focused
            && let Some(transition) = self.active_status_transition()
        {
            content = transition
                .render_line(
                    content.map(|line| line.into_owned().into_hyperlink_line()),
                    inset_footer_hint_area(area).width,
                )
                .map(|frame| Cow::Owned(frame.into()));
            if let Some(requester) = &self.frame_requester {
                requester.schedule_frame_in(EFFORT_STATUS_LINE_FRAME_TICK);
            }
        }
        self.footer
            .status_layout
            .render(content.as_deref(), area, buf, focus);
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
