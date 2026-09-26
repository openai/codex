//! First-frame owned layout: the banner stays at the top and the normal composer at the bottom.
//! Measurement, paint, and cursor placement use the same bottom rectangle, including both footers.
//! Only a new conversation paints decoration in the unused area, so it cannot enter scrollback.

use crossterm::cursor::SetCursorStyle;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::widgets::Widget;
use std::cell::Cell;
use std::time::Duration;

use super::StartupDraftPump;
use super::StartupDraftSessionAction;
use crate::bottom_pane::CommandPopupPlacement;
use crate::bottom_pane::ComposerRenderOptions;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableItem;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::HyperlinkParagraph;

pub(super) struct OwnedStartupLayout<'a> {
    pump: &'a StartupDraftPump,
    bottom: RenderableItem<'a>,
    pub(super) next_frame: Cell<Option<Duration>>,
}

impl<'a> OwnedStartupLayout<'a> {
    pub(super) fn new(pump: &'a StartupDraftPump) -> Self {
        Self {
            pump,
            next_frame: Cell::new(/*value*/ None),
            bottom: pump
                .bottom_pane
                .as_renderable_with_options(ComposerRenderOptions {
                    separate_status_line: true,
                    command_popup_placement: CommandPopupPlacement::Overlay,
                    ..ComposerRenderOptions::default()
                }),
        }
    }

    fn header_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        match self.pump.session_action {
            StartupDraftSessionAction::New | StartupDraftSessionAction::NewFromCommandCenter => {
                self.pump.header.compact_hyperlink_lines(width)
            }
            StartupDraftSessionAction::Resume | StartupDraftSessionAction::Fork => {
                self.pump.header.display_hyperlink_lines(width)
            }
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
        let lines = self.header_lines(area.width);
        let paragraph = HyperlinkParagraph::new(&lines, Style::default());
        let header = Rect {
            height: u16::try_from(paragraph.line_count(area.width))
                .unwrap_or(u16::MAX)
                .min(bottom.y.saturating_sub(area.y)),
            ..area
        };
        paragraph.render(header, buf);
        let message = match self.pump.session_action {
            StartupDraftSessionAction::New | StartupDraftSessionAction::NewFromCommandCenter => {
                let composer = (!self.pump.submission_pending)
                    .then(|| self.pump.bottom_pane.empty_state_composer())
                    .flatten();
                self.next_frame
                    .set(self.pump.blossom.borrow_mut().render_first_screen(
                        Rect {
                            y: header.bottom(),
                            height: bottom.y.saturating_sub(header.bottom()),
                            ..area
                        },
                        buf,
                        composer,
                        self.pump.motion,
                    ));
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
        let lines = self.header_lines(width);
        u16::try_from(HyperlinkParagraph::new(&lines, Style::default()).line_count(width))
            .unwrap_or(u16::MAX)
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
