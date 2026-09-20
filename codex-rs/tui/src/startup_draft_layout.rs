//! First-frame owned layout: the banner stays at the top and the normal composer at the bottom.
//! Measurement, paint, and cursor placement use the same bottom rectangle, including both footers.

use crossterm::cursor::SetCursorStyle;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;

use super::StartupDraftSessionAction;
use crate::bottom_pane::BottomPane;
use crate::bottom_pane::CommandPopupPlacement;
use crate::bottom_pane::ComposerRenderOptions;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableItem;

pub(super) struct OwnedStartupLayout<'a> {
    header: &'a dyn Renderable,
    bottom: RenderableItem<'a>,
    session_action: StartupDraftSessionAction,
}

impl<'a> OwnedStartupLayout<'a> {
    pub(super) fn new(
        header: &'a dyn Renderable,
        bottom_pane: &'a BottomPane,
        session_action: StartupDraftSessionAction,
    ) -> Self {
        Self {
            header,
            bottom: bottom_pane.as_renderable_with_options(ComposerRenderOptions {
                separate_status_line: true,
                command_popup_placement: CommandPopupPlacement::Overlay,
                ..ComposerRenderOptions::default()
            }),
            session_action,
        }
    }

    pub(super) fn bottom_area(&self, area: Rect) -> Rect {
        let height = self.bottom.desired_height(area.width).min(area.height);
        Rect {
            y: area.bottom().saturating_sub(height),
            height,
            ..area
        }
    }
}

impl Renderable for OwnedStartupLayout<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let bottom = self.bottom_area(area);
        let header = Rect {
            height: self
                .header
                .desired_height(area.width)
                .min(bottom.y.saturating_sub(area.y)),
            ..area
        };
        self.header.render(header, buf);
        let message = match self.session_action {
            StartupDraftSessionAction::New | StartupDraftSessionAction::NewFromCommandCenter => {
                None
            }
            StartupDraftSessionAction::Resume => Some("  Resuming session…"),
            StartupDraftSessionAction::Fork => Some("  Forking session…"),
        };
        if let Some(message) = message
            && header.bottom() < bottom.y
        {
            message.dim().render(
                Rect {
                    y: header.bottom(),
                    height: 1,
                    ..area
                },
                buf,
            );
        }
        self.bottom.render(bottom, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.header
            .desired_height(width)
            .saturating_add(self.bottom.desired_height(width))
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.bottom.cursor_pos(self.bottom_area(area))
    }

    fn cursor_style(&self, area: Rect) -> SetCursorStyle {
        self.bottom.cursor_style(self.bottom_area(area))
    }
}

#[cfg(test)]
#[path = "startup_draft_layout_tests.rs"]
mod tests;
