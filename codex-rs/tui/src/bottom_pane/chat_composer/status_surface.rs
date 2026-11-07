//! Separate persistent transcript context from the composer's transient hint row.
//! Measurement, rendering, and cursor placement share the same reserved status rectangle.
//! Shortcut help grows above the input, preserving status and close rows below it even when clipped.

use super::*;
use ratatui::layout::Size;

const MAX_FOOTER_ROWS: u16 = 4;

impl ChatComposer {
    pub(crate) fn set_status_line(&mut self, status_line: Option<FooterContent>) -> bool {
        if self.footer.status_line_value == status_line {
            return false;
        }
        if matches!(status_line, Some(FooterContent::Extension { .. })) {
            self.effort_status_line_transition = None;
        }
        self.footer.measured_status_line.set(None);
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

    pub(super) fn has_separate_status(&self, options: ComposerRenderOptions<'_>) -> bool {
        options.separate_status_line && self.footer.status_line_enabled
    }

    pub(super) fn shows_shared_status(&self, options: ComposerRenderOptions<'_>) -> bool {
        !options.separate_status_line
            && options.footer.is_none()
            && self.custom_footer_height().is_none()
            && self.footer.status_line_enabled
            && self.footer_mode().shows_passive_line(self.is_task_running)
    }

    pub(super) fn status_surface_height(
        &self,
        width: u16,
        options: ComposerRenderOptions<'_>,
    ) -> u16 {
        if self.has_separate_status(options) {
            self.status_line_height(width, options)
        } else {
            0
        }
    }

    pub(super) fn status_line_height(&self, width: u16, options: ComposerRenderOptions<'_>) -> u16 {
        // The unfocused composer paints one dim status line.
        if options.unfocused {
            return 1;
        }
        let Some(FooterContent::Extension { content }) = &self.footer.status_line_value else {
            return 1;
        };
        let mut area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 1);
        if !options.separate_status_line {
            let warning = self.warning_notice_layout(inset_footer_hint_area(area), options);
            area = warning_notice::before_warning(area, warning.map(|(rect, _)| rect));
        }
        let (_, width) = self.status_line_layout(area, options, content.measure_natural().width);
        let cache = &self.footer.measured_status_line;
        if let Some((previous_width, height)) = cache.get()
            && previous_width == width
        {
            return height;
        }
        let height = content
            .measure(Size::new(width, MAX_FOOTER_ROWS))
            .height
            .max(1);
        cache.set(Some((width, height)));
        height
    }

    pub(super) fn hint_footer_props(&self, options: ComposerRenderOptions<'_>) -> FooterProps {
        let mut props = self.footer_props();
        if self.has_separate_status(options) {
            props.status_line_enabled = false;
            props.status_line_value = None;
            props.active_agent_label = None;
        }
        props
    }

    fn status_line_layout(
        &self,
        area: Rect,
        options: ComposerRenderOptions<'_>,
        left_width: u16,
    ) -> (Option<Line<'static>>, u16) {
        let show_cycle_hint = !self.is_task_running
            && if options.separate_status_line {
                !self.blocks_direct_input
                    && self.draft.input_enabled
                    && !self.popup_active()
                    && matches!(
                        self.footer_mode(),
                        FooterMode::ComposerEmpty | FooterMode::ComposerHasDraft
                    )
                    && options.footer.is_none_or(|footer| !footer.is_interactive)
            } else {
                self.footer.collaboration_mode_indicator.is_some()
            };
        let right = if !options.separate_status_line
            && let Some(label) = &self.footer.side_conversation_context_label
        {
            Some(side_conversation_context_line(label))
        } else if !options.separate_status_line
            && let Some(shell) = self.shell_mode_footer_line()
        {
            Some(shell)
        } else if !options.separate_status_line
            && self
                .effort_status_line_transition
                .as_ref()
                .is_some_and(|transition| !transition.is_finished())
        {
            None
        } else {
            let full = self.mode_indicator_line(show_cycle_hint);
            let full_fits = full.as_ref().is_none_or(|right| {
                let right_width = right.width() as u16;
                (!options.separate_status_line || right_width <= inset_footer_hint_area(area).width)
                    && can_show_left_with_context(area, left_width, right_width)
            });
            if full_fits {
                full
            } else {
                self.mode_indicator_line(/*show_cycle_hint*/ false)
            }
        };
        let right_width = right
            .as_ref()
            .map_or(/*default*/ 0, |line| line.width() as u16);
        let width = max_left_width_for_right(area, right_width).unwrap_or_else(|| {
            if options.separate_status_line {
                inset_footer_hint_area(area).width
            } else {
                area.width.saturating_sub(FOOTER_INDENT_COLS as u16)
            }
        });
        (right, width)
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
        if options.unfocused {
            if let Some(content) = self.status_line_content() {
                crate::bottom_pane::footer::render_unfocused_status(
                    content.clone().into_hyperlink_line().line.to_string(),
                    area,
                    buf,
                );
            }
            return;
        }
        let mut props = self.footer_props();
        if options.separate_status_line {
            // Persistent context is independent of help, queue, search, and quit modes.
            props.mode = FooterMode::ComposerHasDraft;
            props.is_task_running = false;
        }
        let line = passive_footer_status_line(&props);
        let left_width = line.as_ref().map_or(/*default*/ 0, FooterContent::width);
        let (right, width) = self.status_line_layout(area, options, left_width);
        let transition = self
            .effort_status_line_transition
            .as_ref()
            .filter(|transition| !transition.is_finished());
        let content = if let Some(transition) = transition {
            transition
                .render_line(line.map(FooterContent::into_hyperlink_line), width)
                .map(FooterContent::HyperlinkLine)
        } else {
            line
        };
        if let Some(content) = content {
            content.render(
                Rect {
                    width,
                    ..inset_footer_hint_area(area)
                },
                buf,
            );
        }
        if let Some(right) = right {
            render_context_right(area, buf, &right);
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
