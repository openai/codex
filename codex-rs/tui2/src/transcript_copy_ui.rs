//! Transcript-selection copy UX helpers.
//!
//! # Background
//!
//! TUI2 owns a logical transcript viewport (with history that can live outside the visible buffer),
//! plus its own selection model. Terminal-native selection/copy does not work reliably in this
//! setup because:
//!
//! - The selection can extend outside the current viewport, while terminal selection can't.
//! - We want to exclude non-content regions (like the left gutter) from copied text.
//! - The terminal may intercept some keybindings before the app ever sees them.
//!
//! This module centralizes:
//!
//! - The effective "copy selection" shortcut (so the footer and affordance stay in sync).
//! - Key matching for triggering copy (with terminal quirks handled in one place).
//! - A small on-screen clickable "⧉ copy …" pill rendered near the current selection.
//!
//! # VS Code shortcut rationale
//!
//! VS Code's integrated terminal commonly captures `Ctrl+Shift+C` for its own copy behavior and
//! does not forward the keypress to applications running inside the terminal. Since we can't
//! observe it via crossterm, we advertise and accept `Ctrl+Y` in that environment.
//!
//! Clipboard text reconstruction (preserving indentation, joining soft-wrapped
//! prose, and emitting Markdown source markers) lives in `transcript_copy`.

use codex_core::terminal::TerminalName;
use codex_core::terminal::terminal_info;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use unicode_width::UnicodeWidthStr;

use crate::key_hint;
use crate::key_hint::KeyBinding;
use crate::transcript_selection::TRANSCRIPT_GUTTER_COLS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// The shortcut we advertise and accept for "copy selection".
pub(crate) enum CopySelectionShortcut {
    CtrlShiftC,
    CtrlY,
}

/// Returns the best shortcut to advertise/accept for "copy selection".
///
/// VS Code's integrated terminal typically captures `Ctrl+Shift+C` for its own copy behavior and
/// does not forward it to applications running inside the terminal. That means we can't reliably
/// observe it via crossterm, so we use `Ctrl+Y` there.
///
/// We use both the terminal name (when available) and `VSCODE_IPC_HOOK_CLI` because the terminal
/// name can be `Unknown` early during startup in some environments.
pub(crate) fn detect_copy_selection_shortcut() -> CopySelectionShortcut {
    let info = terminal_info();
    if info.name == TerminalName::VsCode || std::env::var_os("VSCODE_IPC_HOOK_CLI").is_some() {
        return CopySelectionShortcut::CtrlY;
    }
    CopySelectionShortcut::CtrlShiftC
}

pub(crate) fn key_binding_for(shortcut: CopySelectionShortcut) -> KeyBinding {
    match shortcut {
        CopySelectionShortcut::CtrlShiftC => key_hint::ctrl_shift(KeyCode::Char('c')),
        CopySelectionShortcut::CtrlY => key_hint::ctrl(KeyCode::Char('y')),
    }
}

/// Whether the given `(ch, modifiers)` should trigger "copy selection".
///
/// Terminal/event edge cases:
/// - Some terminals report `Ctrl+Shift+C` as `Char('C')` with `CONTROL` only, baking the shift into
///   the character. We accept both `c` and `C` in `CtrlShiftC` mode (including VS Code).
/// - Some environments intercept `Ctrl+Shift+C` before the app sees it. We keep `Ctrl+Y` as a
///   fallback in `CtrlShiftC` mode to preserve a working key path.
pub(crate) fn is_copy_selection_key(
    shortcut: CopySelectionShortcut,
    ch: char,
    modifiers: KeyModifiers,
) -> bool {
    if !modifiers.contains(KeyModifiers::CONTROL) {
        return false;
    }

    match shortcut {
        CopySelectionShortcut::CtrlY => ch == 'y' && modifiers == KeyModifiers::CONTROL,
        CopySelectionShortcut::CtrlShiftC => {
            (matches!(ch, 'c' | 'C') && (modifiers.contains(KeyModifiers::SHIFT) || ch == 'C'))
                // Fallback for environments that intercept Ctrl+Shift+C.
                || (ch == 'y' && modifiers == KeyModifiers::CONTROL)
        }
    }
}

/// UI state for the on-screen copy affordance shown near an active selection.
///
/// This tracks a `Rect` for hit-testing so we can treat the pill as a clickable button.
#[derive(Debug)]
pub(crate) struct TranscriptCopyUi {
    shortcut: CopySelectionShortcut,
    dragging: bool,
    affordance_rect: Option<Rect>,
}

impl TranscriptCopyUi {
    /// Creates a new instance using the provided shortcut.
    pub(crate) fn new_with_shortcut(shortcut: CopySelectionShortcut) -> Self {
        Self {
            shortcut,
            dragging: false,
            affordance_rect: None,
        }
    }

    pub(crate) fn key_binding(&self) -> KeyBinding {
        key_binding_for(self.shortcut)
    }

    pub(crate) fn is_copy_key(&self, ch: char, modifiers: KeyModifiers) -> bool {
        is_copy_selection_key(self.shortcut, ch, modifiers)
    }

    pub(crate) fn set_dragging(&mut self, dragging: bool) {
        self.dragging = dragging;
    }

    pub(crate) fn clear_affordance(&mut self) {
        self.affordance_rect = None;
    }

    /// Returns `true` if the last rendered pill contains `(x, y)`.
    ///
    /// `render_copy_pill()` sets `affordance_rect` and `clear_affordance()` clears it, so callers
    /// should treat this as "hit test against the current frame's affordance".
    pub(crate) fn hit_test(&self, x: u16, y: u16) -> bool {
        self.affordance_rect
            .is_some_and(|r| x >= r.x && x < r.right() && y >= r.y && y < r.bottom())
    }

    /// Render the copy "pill" just below the visible end of the selection.
    ///
    /// Inputs are expressed in logical transcript coordinates:
    /// - `anchor`/`head`: `(line_index, column)` in the wrapped transcript (not screen rows).
    /// - `view_top`: first logical line index currently visible in `area`.
    /// - `total_lines`: total number of logical transcript lines.
    ///
    /// Placement details / edge cases:
    /// - We hide the pill while dragging to avoid accidental clicks during selection updates.
    /// - We only render if some part of the selection is visible, and there's room for a line
    ///   below it inside `area`.
    /// - We scan the buffer to find the last non-space cell on each candidate row so the pill can
    ///   sit "near content", not far to the right past trailing whitespace.
    ///
    /// Important: this assumes the transcript content has already been rendered into `buf` for the
    /// current frame, since the placement logic derives `text_end` by inspecting buffer contents.
    pub(crate) fn render_copy_pill(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        anchor: (usize, u16),
        head: (usize, u16),
        view_top: usize,
        total_lines: usize,
    ) {
        // Reset every frame. If we don't render (e.g. selection is off-screen) we shouldn't keep
        // an old hit target around.
        self.affordance_rect = None;

        if self.dragging || total_lines == 0 {
            return;
        }

        // Skip the transcript gutter (line numbers, diff markers, etc.). Selection/copy operates on
        // transcript content only.
        let base_x = area.x.saturating_add(TRANSCRIPT_GUTTER_COLS);
        let max_x = area.right().saturating_sub(1);
        if base_x > max_x {
            return;
        }

        // Normalize to a start/end pair so the rest of the code can assume forward order.
        let mut start = anchor;
        let mut end = head;
        if (end.0 < start.0) || (end.0 == start.0 && end.1 < start.1) {
            std::mem::swap(&mut start, &mut end);
        }

        // We want to place the pill *near the visible end of the selection*, which means:
        // - Find the last visible transcript line that intersects the selection.
        // - Find the rightmost selected column on that line (clamped to actual rendered text).
        // - Place the pill one row below that point.
        let visible_start = view_top;
        let visible_end = view_top
            .saturating_add(area.height as usize)
            .min(total_lines);
        let mut last_visible_segment: Option<(u16, u16)> = None;

        for (row_index, line_index) in (visible_start..visible_end).enumerate() {
            // Skip lines outside the selection range.
            if line_index < start.0 || line_index > end.0 {
                continue;
            }

            let y = area.y + row_index as u16;

            // Look for the rightmost non-space cell on this row so we can clamp the pill placement
            // to real content. (The transcript renderer often pads the row with spaces.)
            let mut last_text_x = None;
            for x in base_x..=max_x {
                let cell = &buf[(x, y)];
                if cell.symbol() != " " {
                    last_text_x = Some(x);
                }
            }

            let Some(text_end) = last_text_x else {
                continue;
            };

            let line_end_col = if line_index == end.0 {
                end.1
            } else {
                // For multi-line selections, treat intermediate lines as selected "to the end" so
                // the pill doesn't jump left unexpectedly when only the final line has an explicit
                // end column.
                max_x.saturating_sub(base_x)
            };

            let row_sel_end = base_x.saturating_add(line_end_col).min(max_x);
            if row_sel_end < base_x {
                continue;
            }

            // Clamp the selection end to `text_end` so we don't place the pill far to the right on
            // lines that are mostly blank (or padded).
            let to_x = row_sel_end.min(text_end);
            last_visible_segment = Some((y, to_x));
        }

        // If nothing in the selection is visible, don't show the affordance.
        let Some((y, to_x)) = last_visible_segment else {
            return;
        };
        // Place the pill on the row below the last visible selection segment.
        let Some(y) = y.checked_add(1).filter(|y| *y < area.bottom()) else {
            return;
        };

        let key_label: Span<'static> = self.key_binding().into();
        let key_label = key_label.content.as_ref().to_string();

        let pill_text = format!(" ⧉ copy {key_label} ");
        let pill_width = UnicodeWidthStr::width(pill_text.as_str());
        if pill_width == 0 || area.width == 0 {
            return;
        }

        let pill_width = (pill_width as u16).min(area.width);
        // Prefer a small gap between the selected content and the pill so we don't visually merge
        // into the highlighted selection block.
        let desired_x = to_x.saturating_add(2);
        let max_start_x = area.right().saturating_sub(pill_width);
        let x = if max_start_x < area.x {
            area.x
        } else {
            desired_x.clamp(area.x, max_start_x)
        };

        let pill_area = Rect::new(x, y, pill_width, 1);
        let base_style = Style::new().bg(Color::DarkGray);
        let icon_style = base_style.fg(Color::Cyan);
        let bold_style = base_style.add_modifier(Modifier::BOLD);

        let mut spans: Vec<Span<'static>> = vec![
            Span::styled(" ", base_style),
            Span::styled("⧉", icon_style),
            Span::styled(" ", base_style),
            Span::styled("copy", bold_style),
            Span::styled(" ", base_style),
            Span::styled(key_label, base_style),
        ];
        spans.push(Span::styled(" ", base_style));

        Paragraph::new(vec![Line::from(spans)]).render_ref(pill_area, buf);
        self.affordance_rect = Some(pill_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    fn buf_to_string(buf: &Buffer, area: Rect) -> String {
        let mut s = String::new();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                s.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn ctrl_y_pill_does_not_include_ctrl_shift_c() {
        let area = Rect::new(0, 0, 60, 3);
        let mut buf = Buffer::empty(area);
        for y in 0..area.height {
            for x in 2..area.width.saturating_sub(1) {
                buf[(x, y)].set_symbol("X");
            }
        }

        let mut ui = TranscriptCopyUi::new_with_shortcut(CopySelectionShortcut::CtrlY);
        ui.render_copy_pill(area, &mut buf, (1, 2), (1, 6), 0, 3);

        let rendered = buf_to_string(&buf, area);
        assert!(rendered.contains("copy"));
        assert!(rendered.contains("ctrl + y"));
        assert!(!rendered.contains("ctrl + shift + c"));
        assert!(ui.affordance_rect.is_some());
    }
}
