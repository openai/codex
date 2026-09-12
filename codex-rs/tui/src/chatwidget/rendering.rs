//! Render composition for the main chat widget surface.

use super::transcript::ActiveCellLayoutCache;
use super::transcript::ActiveCellLayoutCacheKey;
use super::*;
use crate::render::RectExt;
use crate::terminal_hyperlinks::HyperlinkParagraph;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;
use ratatui::style::Styled as _;
use ratatui::text::Span;
use ratatui::widgets::Block;
use std::cell::Cell;

struct ExternalWriterNotice {
    command_center_available: bool,
    transcript_hint: Option<crate::key_hint::ShortcutHint>,
}

impl Renderable for ExternalWriterNotice {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let content_width = area.width.saturating_sub(/*rhs*/ 4);
        let card_lines = self.card_lines(content_width);
        let card_height = (card_lines.len() as u16).saturating_add(/*rhs*/ 2);
        let card = Rect::new(area.x, area.y, area.width, card_height.min(area.height));
        Widget::render(
            Block::default().style(crate::style::user_message_style()),
            card,
            buf,
        );
        let content = card.inset(Insets::tlbr(
            /*top*/ 1, /*left*/ 2, /*bottom*/ 1, /*right*/ 2,
        ));
        Renderable::render(&Paragraph::new(card_lines), content, buf);
        let footer_y = card.bottom();
        if footer_y < area.bottom() {
            let footer = Rect::new(
                area.x.saturating_add(/*rhs*/ 2),
                footer_y,
                area.width.saturating_sub(/*rhs*/ 2),
                area.bottom().saturating_sub(footer_y),
            );
            Renderable::render(
                &Paragraph::new(self.footer_lines(footer.width)),
                footer,
                buf,
            );
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        (self.card_lines(width.saturating_sub(/*rhs*/ 4)).len() as u16)
            .saturating_add(/*rhs*/ 2)
            .saturating_add(self.footer_lines(width.saturating_sub(/*rhs*/ 2)).len() as u16)
    }
}

impl ExternalWriterNotice {
    fn card_lines(&self, width: u16) -> Vec<Line<'static>> {
        let title: Line<'static> = vec![
            "🔒".into(),
            "  ".into(),
            "This conversation is open in another app".bold(),
        ]
        .into();
        let retry: Line<'static> = vec![
            Span::styled("R", crate::style::accent_style()),
            " to Retry".into(),
        ]
        .into();
        let mut lines = word_wrap_lines(&[title], usize::from(width));
        if lines.len() == 1 && lines[0].width() + retry.width() + 2 <= usize::from(width) {
            let gap = usize::from(width) - lines[0].width() - retry.width();
            lines[0].spans.push(" ".repeat(gap).into());
            lines[0].spans.extend(retry.spans);
        } else {
            lines.push(retry);
        }
        lines.extend(word_wrap_lines(
            &[Line::from(
                "Close it there and press R to continue here.".dim(),
            )],
            RtOptions::new(usize::from(width))
                .initial_indent("    ".into())
                .subsequent_indent("    ".into()),
        ));
        lines
    }

    fn footer_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut items = vec![("r".to_string(), "retry".to_string())];
        let escape = crate::key_hint::plain(KeyCode::Esc).display_label();
        let mut quit_keys = vec![
            crate::key_hint::ctrl(KeyCode::Char('c')).display_label(),
            crate::key_hint::plain(KeyCode::Char('q')).display_label(),
        ];
        if self.command_center_available {
            items.push((escape, "command center".to_string()));
        } else {
            quit_keys.insert(/*index*/ 0, escape);
        }
        items.push((quit_keys.join("/").replace(" + ", "+"), "exit".to_string()));
        if let Some(hint) = self.transcript_hint {
            items.push((
                hint.display_label().replace(" + ", "+"),
                "transcript".to_string(),
            ));
        }
        let mut spans = vec![" ".set_style(crate::style::footer_hint_label_style())];
        for (idx, (key, label)) in items.into_iter().enumerate() {
            if idx > 0 {
                spans.push("   ".set_style(crate::style::footer_hint_label_style()));
            }
            spans.push(key.set_style(crate::style::footer_hint_key_style()));
            spans.push(format!(" {label}").set_style(crate::style::footer_hint_label_style()));
        }
        word_wrap_lines(&[Line::from(spans)], usize::from(width))
    }
}

impl ChatWidget {
    pub(crate) fn as_renderable(&self) -> RenderableItem<'_> {
        if self
            .bottom_pane
            .selected_index_for_active_view(crate::app::AGENTS_OVERVIEW_VIEW_ID)
            .is_some()
        {
            return self
                .bottom_pane
                .as_renderable_with_composer_right_reserve(/*composer_right_reserve*/ 0);
        }

        let active_cell_right_reserve = self.ambient_pet_wrap_reserved_cols();
        let active_cell_renderable = match &self.transcript.active_cell {
            Some(cell) => RenderableItem::Owned(Box::new(TranscriptAreaRenderable {
                child: cell.as_ref(),
                // The initial header becomes the first history cell, which has no leading separator.
                top: if cell.as_any().is::<history_cell::SessionHeaderHistoryCell>() {
                    0
                } else {
                    1
                },
                right: active_cell_right_reserve,
                // Externally backed transcript cells can also change viewport height without an
                // active-cell revision. Spinner cells remain safe because their indicator width
                // is stable and their display lines are still rebuilt on every frame.
                persistent_layout: cell.has_stable_transcript_height().then_some(
                    PersistentActiveCellLayout {
                        cache: &self.transcript.active_cell_layout,
                        cell_identity: cell.as_ref() as *const dyn HistoryCell as *const ()
                            as usize,
                        revision: self.transcript.active_cell_revision,
                        render_mode: self.history_render_mode(),
                    },
                ),
            })),
            None => RenderableItem::Owned(Box::new(())),
        };
        let mut flex = FlexRenderable::new();
        flex.push(/*flex*/ 1, active_cell_renderable);
        for cell in self
            .realtime_conversation
            .pending_history_cells
            .iter()
            .chain(self.realtime_conversation.live_transcript_cells())
        {
            flex.push(
                /*flex*/ 1,
                RenderableItem::Owned(Box::new(TranscriptAreaRenderable {
                    child: cell.as_ref(),
                    top: 1,
                    right: active_cell_right_reserve,
                    persistent_layout: None,
                })),
            );
        }
        if let Some(cell) = self.pending_token_activity_output() {
            flex.push(
                /*flex*/ 1,
                RenderableItem::Owned(Box::new(TranscriptAreaRenderable {
                    child: cell,
                    top: 1,
                    right: active_cell_right_reserve,
                    persistent_layout: None,
                })),
            );
        }
        if let Some(cell) = self.pending_rate_limit_reset_hint() {
            flex.push(
                /*flex*/ 1,
                RenderableItem::Owned(Box::new(TranscriptAreaRenderable {
                    child: cell,
                    top: 1,
                    right: active_cell_right_reserve,
                    persistent_layout: None,
                })),
            );
        }
        let bottom = if self.external_writer_view && !self.bottom_pane.has_active_view() {
            RenderableItem::Owned(Box::new(ExternalWriterNotice {
                command_center_available: self.remote_connection.is_some(),
                transcript_hint: self.bottom_pane.transcript_shortcut_hint(),
            }))
        } else {
            self.bottom_pane
                .as_renderable_with_composer_right_reserve(active_cell_right_reserve)
        };
        flex.push(
            /*flex*/ 0,
            bottom.inset(Insets::tlbr(
                /*top*/ 1, /*left*/ 0, /*bottom*/ 0, /*right*/ 0,
            )),
        );
        RenderableItem::Owned(Box::new(flex))
    }

    pub(crate) fn note_rendered_width(&self, width: u16) {
        self.last_rendered_width.set(Some(width));
    }
}

struct TranscriptAreaRenderable<'a> {
    child: &'a dyn HistoryCell,
    top: u16,
    right: u16,
    persistent_layout: Option<PersistentActiveCellLayout<'a>>,
}

struct PersistentActiveCellLayout<'a> {
    cache: &'a Cell<Option<ActiveCellLayoutCache>>,
    cell_identity: usize,
    revision: u64,
    render_mode: HistoryRenderMode,
}

impl Renderable for TranscriptAreaRenderable<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let area = self.child_area(area);
        let lines = self.child.display_hyperlink_lines(area.width);
        let paragraph = HyperlinkParagraph::new(&lines, Style::default());
        let y = if area.height == 0 {
            0
        } else {
            let rendered_height = if let Some((cache, mut layout)) = self.layout(area.width) {
                if let Some(height) = layout.rendered_height {
                    height
                } else {
                    let height = paragraph.line_count(area.width);
                    layout.rendered_height = Some(height);
                    cache.set(Some(layout));
                    height
                }
            } else {
                paragraph.line_count(area.width)
            };
            let overflow = rendered_height.saturating_sub(usize::from(area.height));
            u16::try_from(overflow).unwrap_or(u16::MAX)
        };
        Clear.render(area, buf);
        paragraph.scroll(y).render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        let child_width = width.saturating_sub(self.right).max(1);
        let desired_height = if let Some((cache, mut layout)) = self.layout(child_width) {
            if let Some(height) = layout.desired_height {
                height
            } else {
                let height = HistoryCell::desired_height(self.child, child_width);
                layout.desired_height = Some(height);
                cache.set(Some(layout));
                height
            }
        } else {
            HistoryCell::desired_height(self.child, child_width)
        };
        desired_height + self.top
    }
}

impl TranscriptAreaRenderable<'_> {
    fn layout(
        &self,
        width: u16,
    ) -> Option<(&Cell<Option<ActiveCellLayoutCache>>, ActiveCellLayoutCache)> {
        let persistent = self.persistent_layout.as_ref()?;
        let key = ActiveCellLayoutCacheKey {
            cell_identity: persistent.cell_identity,
            revision: persistent.revision,
            width,
            render_mode: persistent.render_mode,
            syntax_theme_revision: crate::render::highlight::syntax_theme_revision(),
        };
        let layout = persistent
            .cache
            .get()
            .filter(|layout| layout.key == key)
            .unwrap_or(ActiveCellLayoutCache {
                key,
                desired_height: None,
                rendered_height: None,
            });
        Some((persistent.cache, layout))
    }

    fn child_area(&self, area: Rect) -> Rect {
        let y = area.y.saturating_add(self.top);
        let height = area.height.saturating_sub(self.top);
        Rect::new(
            area.x,
            y,
            area.width.saturating_sub(self.right).max(1),
            height,
        )
    }
}

#[cfg(test)]
#[path = "rendering_tests.rs"]
mod tests;

impl Renderable for ChatWidget {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.as_renderable().render(area, buf);
        self.note_rendered_width(area.width);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.as_renderable().desired_height(width)
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.as_renderable().cursor_pos(area)
    }

    fn cursor_style(&self, area: Rect) -> crossterm::cursor::SetCursorStyle {
        self.as_renderable().cursor_style(area)
    }
}
