use std::io::Result;

use crate::copy_target::CopyTarget;
use crate::copy_target::CopyTargetKind;
use crate::key_hint;
use crate::key_hint::KeyBinding;
use crate::tui;
use crate::tui::TuiEvent;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;
use crossterm::event::KeyCode;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use unicode_width::UnicodeWidthStr;

const TARGET_BAR_HEIGHT: u16 = 1;
const FOOTER_HEIGHT: u16 = 2;

const KEY_UP: KeyBinding = key_hint::plain(KeyCode::Up);
const KEY_DOWN: KeyBinding = key_hint::plain(KeyCode::Down);
const KEY_LEFT: KeyBinding = key_hint::plain(KeyCode::Left);
const KEY_RIGHT: KeyBinding = key_hint::plain(KeyCode::Right);
const KEY_K: KeyBinding = key_hint::plain(KeyCode::Char('k'));
const KEY_J: KeyBinding = key_hint::plain(KeyCode::Char('j'));
const KEY_H: KeyBinding = key_hint::plain(KeyCode::Char('h'));
const KEY_L: KeyBinding = key_hint::plain(KeyCode::Char('l'));
const KEY_PAGE_UP: KeyBinding = key_hint::plain(KeyCode::PageUp);
const KEY_PAGE_DOWN: KeyBinding = key_hint::plain(KeyCode::PageDown);
const KEY_HOME: KeyBinding = key_hint::plain(KeyCode::Home);
const KEY_END: KeyBinding = key_hint::plain(KeyCode::End);
const KEY_Q: KeyBinding = key_hint::plain(KeyCode::Char('q'));
const KEY_ESC: KeyBinding = key_hint::plain(KeyCode::Esc);
const KEY_ENTER: KeyBinding = key_hint::plain(KeyCode::Enter);
const KEY_CTRL_C: KeyBinding = key_hint::ctrl(KeyCode::Char('c'));

#[derive(Clone, Copy)]
struct CopyPickerAreas {
    target_bar: Rect,
    preview: Rect,
    footer: Rect,
}

pub(crate) struct CopyPickerOverlay {
    targets: Vec<CopyTarget>,
    selected: usize,
    target_scroll_offset: usize,
    preview_scroll_offset: usize,
    pending_copy: Option<CopyTarget>,
    is_done: bool,
}

impl CopyPickerOverlay {
    pub(crate) fn new(targets: Vec<CopyTarget>) -> Self {
        Self {
            targets,
            selected: 0,
            target_scroll_offset: 0,
            preview_scroll_offset: 0,
            pending_copy: None,
            is_done: false,
        }
    }

    pub(crate) fn handle_event(&mut self, tui: &mut tui::Tui, event: TuiEvent) -> Result<()> {
        match event {
            TuiEvent::Key(key_event) => {
                self.handle_key_event(tui, key_event);
                Ok(())
            }
            TuiEvent::Mouse(mouse_event) => {
                self.handle_mouse_event(tui, mouse_event);
                Ok(())
            }
            TuiEvent::Draw => {
                tui.draw(u16::MAX, |frame| {
                    self.render(frame.area(), frame.buffer);
                })?;
                Ok(())
            }
            TuiEvent::Paste(_) => Ok(()),
        }
    }

    pub(crate) fn is_done(&self) -> bool {
        self.is_done
    }

    pub(crate) fn take_pending_copy(&mut self) -> Option<CopyTarget> {
        self.pending_copy.take()
    }

    fn handle_key_event(&mut self, tui: &mut tui::Tui, key_event: crossterm::event::KeyEvent) {
        let areas = self.areas(tui.terminal.viewport_area);
        let line_count = self.preview_lines_for_width(areas.preview.width).len();
        let page = areas.preview.height.max(1) as usize;
        match key_event {
            e if KEY_Q.is_press(e) || KEY_ESC.is_press(e) || KEY_CTRL_C.is_press(e) => {
                self.is_done = true;
            }
            e if KEY_ENTER.is_press(e) => {
                self.copy_selected();
            }
            e if KEY_LEFT.is_press(e) || KEY_H.is_press(e) => {
                self.select_target(self.selected.saturating_sub(1), areas.target_bar.width);
            }
            e if KEY_RIGHT.is_press(e) || KEY_L.is_press(e) => {
                self.select_target(
                    (self.selected + 1).min(self.targets.len().saturating_sub(1)),
                    areas.target_bar.width,
                );
            }
            e if KEY_UP.is_press(e) || KEY_K.is_press(e) => {
                self.scroll_preview_up(1);
            }
            e if KEY_DOWN.is_press(e) || KEY_J.is_press(e) => {
                self.scroll_preview_down(1, line_count, page);
            }
            e if KEY_PAGE_UP.is_press(e) => {
                self.scroll_preview_up(page);
            }
            e if KEY_PAGE_DOWN.is_press(e) => {
                self.scroll_preview_down(page, line_count, page);
            }
            e if KEY_HOME.is_press(e) => {
                self.preview_scroll_offset = 0;
            }
            e if KEY_END.is_press(e) => {
                self.preview_scroll_offset = max_preview_scroll(line_count, page);
            }
            _ => return,
        }
        tui.frame_requester()
            .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL);
    }

    fn handle_mouse_event(&mut self, tui: &mut tui::Tui, mouse_event: MouseEvent) {
        let areas = self.areas(tui.terminal.viewport_area);
        let line_count = self.preview_lines_for_width(areas.preview.width).len();
        let changed = self.handle_mouse_event_in_area(areas, line_count, mouse_event);
        if changed {
            tui.frame_requester()
                .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL);
        }
    }

    fn handle_mouse_event_in_area(
        &mut self,
        areas: CopyPickerAreas,
        line_count: usize,
        mouse_event: MouseEvent,
    ) -> bool {
        let page = areas.preview.height.max(1) as usize;
        match mouse_event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(index) =
                    self.target_index_at(areas.target_bar, mouse_event.column, mouse_event.row)
                {
                    self.select_target(index, areas.target_bar.width);
                    return true;
                }
                if point_in_rect(areas.preview, mouse_event.column, mouse_event.row) {
                    self.copy_selected();
                    return true;
                }
            }
            MouseEventKind::ScrollUp => {
                self.scroll_preview_up(1);
                return true;
            }
            MouseEventKind::ScrollDown => {
                self.scroll_preview_down(1, line_count, page);
                return true;
            }
            _ => {}
        }
        false
    }

    fn select_target(&mut self, index: usize, target_bar_width: u16) {
        if self.targets.is_empty() {
            return;
        }
        self.selected = index.min(self.targets.len() - 1);
        self.preview_scroll_offset = 0;
        self.ensure_selected_target_visible(target_bar_width);
    }

    fn scroll_preview_up(&mut self, amount: usize) {
        self.preview_scroll_offset = self.preview_scroll_offset.saturating_sub(amount);
    }

    fn scroll_preview_down(&mut self, amount: usize, line_count: usize, page: usize) {
        self.preview_scroll_offset =
            (self.preview_scroll_offset + amount).min(max_preview_scroll(line_count, page));
    }

    fn copy_selected(&mut self) {
        if let Some(target) = self.targets.get(self.selected) {
            self.pending_copy = Some(target.clone());
            self.is_done = true;
        }
    }

    pub(crate) fn render(&mut self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let areas = self.areas(area);
        self.ensure_selected_target_visible(areas.target_bar.width);
        self.render_target_bar(areas.target_bar, buf);
        self.render_preview(areas.preview, buf);
        self.render_footer(areas.footer, buf);
    }

    fn render_target_bar(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let selected_style = Style::default().add_modifier(Modifier::REVERSED);
        let mut spans = Vec::new();
        let mut width = 0usize;
        for idx in self.target_scroll_offset..self.targets.len() {
            let label = self.target_tab_label(idx);
            let label_width = UnicodeWidthStr::width(label.as_str());
            let style = if idx == self.selected {
                selected_style
            } else {
                Style::default().dim()
            };
            spans.push(Span::styled(label, style));
            width = width.saturating_add(label_width);
            if width >= area.width as usize {
                break;
            }
        }
        Paragraph::new(Line::from(spans)).render(area, buf);
    }

    fn render_preview(&mut self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let lines = self.preview_lines_for_width(area.width);
        let page = area.height.max(1) as usize;
        self.preview_scroll_offset = self
            .preview_scroll_offset
            .min(max_preview_scroll(lines.len(), page));
        let end = (self.preview_scroll_offset + page).min(lines.len());
        let visible = lines[self.preview_scroll_offset..end].to_vec();
        Paragraph::new(visible).render(area, buf);
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        if area.height == 1 {
            render_key_hints(
                area,
                buf,
                &[
                    (&[KEY_LEFT, KEY_RIGHT, KEY_H, KEY_L], "target"),
                    (&[KEY_UP, KEY_DOWN, KEY_K, KEY_J], "scroll"),
                    (&[KEY_ENTER], "copy"),
                    (&[KEY_Q, KEY_ESC], "close"),
                ],
            );
            return;
        }

        render_key_hints(
            Rect::new(area.x, area.y, area.width, 1),
            buf,
            &[
                (&[KEY_LEFT, KEY_RIGHT, KEY_H, KEY_L], "target"),
                (&[KEY_UP, KEY_DOWN, KEY_K, KEY_J], "scroll"),
                (&[KEY_PAGE_UP, KEY_PAGE_DOWN], "page"),
            ],
        );
        render_key_hints(
            Rect::new(area.x, area.y + 1, area.width, 1),
            buf,
            &[(&[KEY_ENTER], "copy"), (&[KEY_Q, KEY_ESC], "close")],
        );
    }

    fn preview_lines_for_width(&self, width: u16) -> Vec<Line<'static>> {
        let Some(target) = self.targets.get(self.selected) else {
            return Vec::new();
        };
        let options = RtOptions::new(width.max(1) as usize);
        word_wrap_lines(target.content.split('\n'), options)
    }

    fn areas(&self, area: Rect) -> CopyPickerAreas {
        let target_bar_height = area.height.min(TARGET_BAR_HEIGHT);
        let footer_height = area
            .height
            .saturating_sub(target_bar_height)
            .min(FOOTER_HEIGHT);
        let preview_height = area
            .height
            .saturating_sub(target_bar_height)
            .saturating_sub(footer_height);
        CopyPickerAreas {
            target_bar: Rect::new(area.x, area.y, area.width, target_bar_height),
            preview: Rect::new(
                area.x,
                area.y.saturating_add(target_bar_height),
                area.width,
                preview_height,
            ),
            footer: Rect::new(
                area.x,
                area.bottom().saturating_sub(footer_height),
                area.width,
                footer_height,
            ),
        }
    }

    fn ensure_selected_target_visible(&mut self, target_bar_width: u16) {
        if self.targets.is_empty() || target_bar_width == 0 {
            return;
        }
        if self.selected < self.target_scroll_offset {
            self.target_scroll_offset = self.selected;
        }
        while self.target_scroll_offset < self.selected
            && !self.selected_target_starts_in_bar(target_bar_width)
        {
            self.target_scroll_offset += 1;
        }
    }

    fn selected_target_starts_in_bar(&self, target_bar_width: u16) -> bool {
        let mut used = 0usize;
        let target_bar_width = target_bar_width as usize;
        for idx in self.target_scroll_offset..self.targets.len() {
            if idx == self.selected {
                return used < target_bar_width;
            }
            used = used.saturating_add(self.target_tab_width(idx));
            if used >= target_bar_width {
                return false;
            }
        }
        false
    }

    fn target_index_at(&self, area: Rect, column: u16, row: u16) -> Option<usize> {
        if area.height == 0
            || row != area.y
            || column < area.x
            || column >= area.right()
            || area.width == 0
        {
            return None;
        }

        let mut x = area.x;
        for idx in self.target_scroll_offset..self.targets.len() {
            let width = self.target_tab_width_u16(idx);
            let right = x.saturating_add(width);
            if column >= x && column < right {
                return Some(idx);
            }
            x = right;
            if x >= area.right() {
                break;
            }
        }
        None
    }

    fn target_tab_label(&self, index: usize) -> String {
        let Some(target) = self.targets.get(index) else {
            return String::new();
        };
        let label = match &target.kind {
            CopyTargetKind::CodeBlock => target.title.as_str(),
            kind => kind.label(),
        };
        format!(" {} {label} ", index + 1)
    }

    fn target_tab_width(&self, index: usize) -> usize {
        UnicodeWidthStr::width(self.target_tab_label(index).as_str())
    }

    fn target_tab_width_u16(&self, index: usize) -> u16 {
        self.target_tab_width(index).min(u16::MAX as usize) as u16
    }
}

fn max_preview_scroll(line_count: usize, page: usize) -> usize {
    line_count.saturating_sub(page.max(1))
}

fn point_in_rect(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x && column < area.right() && row >= area.y && row < area.bottom()
}

fn render_key_hints(area: Rect, buf: &mut Buffer, pairs: &[(&[KeyBinding], &str)]) {
    let mut spans: Vec<Span<'static>> = vec![" ".into()];
    let mut first = true;
    for (keys, desc) in pairs {
        if !first {
            spans.push("   ".into());
        }
        for (i, key) in keys.iter().enumerate() {
            if i > 0 {
                spans.push("/".into());
            }
            spans.push(Span::from(key));
        }
        if !keys.is_empty() {
            spans.push(" ".into());
        }
        spans.push(Span::from(desc.to_string()));
        first = false;
    }
    Paragraph::new(vec![Line::from(spans).dim()]).render_ref(area, buf);
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::copy_target::CopyTargetKind;

    fn target(kind: CopyTargetKind, title: &str, content: &str) -> CopyTarget {
        CopyTarget::new(kind, title, content)
    }

    #[test]
    fn copy_picker_snapshot_basic() {
        let mut overlay = CopyPickerOverlay::new(vec![
            target(
                CopyTargetKind::AssistantResponse,
                "Last response",
                "first paragraph\n\nsecond paragraph\nthird paragraph",
            ),
            target(CopyTargetKind::Command, "Command", "rg copy target"),
            target(CopyTargetKind::Output, "Output", "src/lib.rs\nsrc/app.rs"),
        ]);
        let mut term = Terminal::new(TestBackend::new(64, 12)).expect("term");
        term.draw(|f| overlay.render(f.area(), f.buffer_mut()))
            .expect("draw");
        assert_snapshot!(term.backend());
    }

    #[test]
    fn enter_copies_selected_target() {
        let mut overlay = CopyPickerOverlay::new(vec![
            target(CopyTargetKind::AssistantResponse, "Last response", "hello"),
            target(CopyTargetKind::Command, "Command", "rg copy target"),
        ]);
        overlay.selected = 1;
        overlay.copy_selected();

        assert_eq!(
            overlay.take_pending_copy().map(|target| target.content),
            Some("rg copy target".to_string())
        );
        assert!(overlay.is_done());
    }

    #[test]
    fn mouse_click_target_bar_selects_target() {
        let mut overlay = CopyPickerOverlay::new(vec![
            target(CopyTargetKind::AssistantResponse, "Last response", "hello"),
            target(CopyTargetKind::Command, "Command", "rg copy target"),
        ]);
        let areas = overlay.areas(Rect::new(0, 0, 64, 12));
        let command_column = overlay.target_tab_width_u16(0) + 1;
        overlay.handle_mouse_event_in_area(
            areas,
            /*line_count*/ 1,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: command_column,
                row: areas.target_bar.y,
                modifiers: crossterm::event::KeyModifiers::NONE,
            },
        );

        assert_eq!(overlay.selected, 1);
        assert_eq!(overlay.take_pending_copy(), None);
    }

    #[test]
    fn mouse_click_preview_copies_selected_target() {
        let mut overlay = CopyPickerOverlay::new(vec![
            target(CopyTargetKind::AssistantResponse, "Last response", "hello"),
            target(CopyTargetKind::Command, "Command", "rg copy target"),
        ]);
        let areas = overlay.areas(Rect::new(0, 0, 64, 12));
        overlay.handle_mouse_event_in_area(
            areas,
            /*line_count*/ 1,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: areas.preview.x,
                row: areas.preview.y,
                modifiers: crossterm::event::KeyModifiers::NONE,
            },
        );

        assert_eq!(
            overlay.take_pending_copy().map(|target| target.content),
            Some("hello".to_string())
        );
    }
}
