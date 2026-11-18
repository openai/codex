//! Scroll choices by visual line without changing the answer; only rendered content authorizes it.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use super::AsyncQuestions;
use crate::terminal_hyperlinks::HyperlinkLine;
use ratatui::widgets::Widget;

#[derive(Default)]
pub(super) struct OptionViewport {
    pub(super) area: Rect,
    offset: usize,
    total: usize,
    pub(super) visible: std::ops::Range<usize>,
    pub(super) selected: Option<usize>,
    selected_start: usize,
    seen_end: usize,
    selected_height: usize,
    horizontally_clipped: Vec<usize>,
}

impl OptionViewport {
    pub(super) fn selected_page(&self, down: bool) -> Option<isize> {
        let step = usize::from(self.area.height);
        if self.selected_height <= step || step == 0 {
            return None;
        }
        if down && self.offset + step < self.selected_start + self.selected_height {
            Some(step as isize)
        } else if !down && self.offset > self.selected_start {
            Some(-(step.min(self.offset - self.selected_start) as isize))
        } else {
            None
        }
    }

    pub(super) fn scroll(&mut self, delta: isize) {
        let page = self.area.height as isize;
        let max = self.total.saturating_sub(usize::from(self.area.height));
        self.offset = self
            .offset
            .saturating_add_signed(delta.clamp(-page, page))
            .min(max);
    }
}

impl AsyncQuestions {
    pub(super) fn page_content(&mut self, key: super::KeyEvent) -> bool {
        if self.focus_is_notes() {
            return false;
        }
        let down = match self.keymap.list.action_for(key) {
            Some(super::ListAction::PageUp) => false,
            Some(super::ListAction::PageDown) => true,
            _ => return false,
        };
        // Selection can change between paints; wait for its current measurements.
        if self.has_options()
            && self.option_viewport.borrow().selected != self.selected_option_index()
        {
            return true;
        }
        let Some(rows) = self.option_viewport.borrow().selected_page(down) else {
            return false;
        };
        self.option_viewport.get_mut().scroll(rows);
        true
    }

    pub(super) fn option_fully_seen(&self, index: usize) -> bool {
        let viewport = self.option_viewport.borrow();
        (viewport.visible.contains(&index) && !viewport.horizontally_clipped.contains(&index))
            || (viewport.selected == Some(index)
                && (viewport.selected_start..viewport.selected_start + viewport.selected_height)
                    .contains(&viewport.offset)
                && viewport.selected_height > usize::from(viewport.area.height)
                && viewport.seen_end == viewport.selected_height)
    }

    pub(super) fn render_scrollable_options(
        &self,
        area: Rect,
        buf: &mut Buffer,
        lines: &[Vec<HyperlinkLine>],
    ) {
        let mut viewport = self.option_viewport.borrow_mut();
        if area.is_empty() {
            *viewport = OptionViewport::default();
            return;
        }
        let selected = self.selected_option_index().unwrap_or(0);
        let total = lines.iter().map(Vec::len).sum::<usize>();
        let start = lines.iter().take(selected).map(Vec::len).sum::<usize>();
        let height = lines.get(selected).map_or(0, Vec::len);
        let available = usize::from(area.height);
        if viewport.selected != Some(selected) || viewport.area.width != area.width {
            viewport.selected = Some(selected);
            viewport.seen_end = 0;
            let offset = viewport.offset;
            if start < offset || height > available {
                viewport.offset = start;
            } else if start + height > offset + available {
                viewport.offset = (start + height).saturating_sub(available);
            }
        } else if viewport.area.height != area.height && height <= available {
            viewport.offset = viewport
                .offset
                .clamp((start + height).saturating_sub(available), start);
        }
        viewport.area = area;
        viewport.total = total;
        viewport.selected_height = height;
        viewport.selected_start = start;
        let offset = viewport.offset.min(total.saturating_sub(available));
        viewport.offset = offset;
        let bottom_padding = available.saturating_sub(total);
        let mut start = 0;
        let mut first_full = None;
        let mut full_count = 0;
        viewport.horizontally_clipped.clear();
        let fits = |line: &HyperlinkLine| line.line.width() <= usize::from(area.width);
        for (index, lines) in lines.iter().enumerate() {
            let end = start + lines.len();
            let visible_start = start.max(offset);
            let visible_end = end.min(offset + available);
            if visible_start < visible_end {
                for line in visible_start..visible_end {
                    let y = area.y + (bottom_padding + line - offset) as u16;
                    (&lines[line - start]).render(Rect::new(area.x, y, area.width, 1), buf);
                }
                if start >= offset && end <= offset + available {
                    first_full.get_or_insert(index);
                    full_count += 1;
                    if !lines.iter().all(fits) {
                        viewport.horizontally_clipped.push(index);
                    }
                }
                if index == selected {
                    for line in visible_start..visible_end {
                        let relative = line - start;
                        if relative == viewport.seen_end && fits(&lines[relative]) {
                            viewport.seen_end += 1;
                        }
                    }
                }
            }
            start = end;
        }
        let first = first_full.unwrap_or(0);
        viewport.visible = first..first + full_count;
    }
}
