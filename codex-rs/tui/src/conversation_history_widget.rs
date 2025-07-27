use crate::cell_widget::CellWidget;
use crate::history_cell::CommandOutput;
use crate::history_cell::HistoryCell;
use crate::history_cell::PatchEventType;
use codex_core::config::Config;
use codex_core::protocol::FileChange;
use codex_core::protocol::SessionConfiguredEvent;
use ratatui::prelude::*;
use ratatui::style::Style;
use ratatui::widgets::*;
use serde_json::Value as JsonValue;
use std::cell::Cell as StdCell;
use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;

/// A single history entry plus its cached wrapped-line count.
struct Entry {
    cell: HistoryCell,
    line_count: Cell<usize>,
}

pub struct ConversationHistoryWidget {
    entries: Vec<Entry>,
    /// The width (in terminal cells/columns) that [`Entry::line_count`] was
    /// computed for. When the available width changes we recompute counts.
    cached_width: StdCell<u16>,
    scroll_position: usize,
    /// Number of lines the last time render_ref() was called
    num_rendered_lines: StdCell<usize>,
    /// The height of the viewport last time render_ref() was called
    last_viewport_height: StdCell<usize>,
    has_input_focus: bool,
}

impl ConversationHistoryWidget {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            cached_width: StdCell::new(0),
            scroll_position: usize::MAX,
            num_rendered_lines: StdCell::new(0),
            last_viewport_height: StdCell::new(0),
            has_input_focus: false,
        }
    }

    /// Negative delta scrolls up; positive delta scrolls down.
    pub(crate) fn scroll(&mut self, delta: i32) {
        match delta.cmp(&0) {
            std::cmp::Ordering::Less => self.scroll_up(-delta as u32),
            std::cmp::Ordering::Greater => self.scroll_down(delta as u32),
            std::cmp::Ordering::Equal => {}
        }
    }

    fn scroll_up(&mut self, num_lines: u32) {
        // If a user is scrolling up from the "stick to bottom" mode, we need to
        // map this to a specific scroll position so we can calculate the delta.
        // This requires us to care about how tall the screen is.
        if self.scroll_position == usize::MAX {
            self.scroll_position = self
                .num_rendered_lines
                .get()
                .saturating_sub(self.last_viewport_height.get());
        }

        self.scroll_position = self.scroll_position.saturating_sub(num_lines as usize);
    }

    fn scroll_down(&mut self, num_lines: u32) {
        // If we're already pinned to the bottom there's nothing to do.
        if self.scroll_position == usize::MAX {
            return;
        }

        let viewport_height = self.last_viewport_height.get().max(1);
        let num_rendered_lines = self.num_rendered_lines.get();

        // Compute the maximum explicit scroll offset that still shows a full
        // viewport. This mirrors the calculation in `scroll_page_down()` and
        // in the render path.
        let max_scroll = num_rendered_lines.saturating_sub(viewport_height);

        let new_pos = self.scroll_position.saturating_add(num_lines as usize);

        if new_pos >= max_scroll {
            // Reached (or passed) the bottom – switch to stick‑to‑bottom mode
            // so that additional output keeps the view pinned automatically.
            self.scroll_position = usize::MAX;
        } else {
            self.scroll_position = new_pos;
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_position = usize::MAX;
    }

    /// Note `model` could differ from `config.model` if the agent decided to
    /// use a different model than the one requested by the user.
    pub fn add_session_info(&mut self, config: &Config, event: SessionConfiguredEvent) {
        // In practice, SessionConfiguredEvent should always be the first entry
        // in the history, but it is possible that an error could be sent
        // before the session info.
        let has_welcome_message = self
            .entries
            .iter()
            .any(|entry| matches!(entry.cell, HistoryCell::WelcomeMessage { .. }));
        self.add_to_history(HistoryCell::new_session_info(
            config,
            event,
            !has_welcome_message,
        ));
    }

    pub fn add_user_message(&mut self, message: String) {
        self.add_to_history(HistoryCell::new_user_prompt(message));
    }

    pub fn add_agent_message(&mut self, config: &Config, message: String) {
        self.add_to_history(HistoryCell::new_agent_message(config, message));
    }

    pub fn add_agent_reasoning(&mut self, config: &Config, text: String) {
        self.add_to_history(HistoryCell::new_agent_reasoning(config, text));
    }

    pub fn add_background_event(&mut self, message: String) {
        self.add_to_history(HistoryCell::new_background_event(message));
    }

    pub fn add_diff_output(&mut self, diff_output: String) {
        self.add_to_history(HistoryCell::new_diff_output(diff_output));
    }

    pub fn add_error(&mut self, message: String) {
        self.add_to_history(HistoryCell::new_error_event(message));
    }

    /// Add a pending patch entry (before user approval).
    pub fn add_patch_event(
        &mut self,
        event_type: PatchEventType,
        changes: HashMap<PathBuf, FileChange>,
    ) {
        self.add_to_history(HistoryCell::new_patch_event(event_type, changes));
    }

    pub fn add_active_exec_command(&mut self, call_id: String, command: Vec<String>) {
        self.add_to_history(HistoryCell::new_active_exec_command(call_id, command));
    }

    pub fn add_active_mcp_tool_call(
        &mut self,
        call_id: String,
        server: String,
        tool: String,
        arguments: Option<JsonValue>,
    ) {
        self.add_to_history(HistoryCell::new_active_mcp_tool_call(
            call_id, server, tool, arguments,
        ));
    }

    fn add_to_history(&mut self, cell: HistoryCell) {
        let width = self.cached_width.get();
        let count = if width > 0 { cell.height(width) } else { 0 };

        self.entries.push(Entry {
            cell,
            line_count: Cell::new(count),
        });
    }

    /// Return the lines for the most recently appended entry (if any) so the
    /// parent widget can surface them via the new scrollback insertion path.
    pub(crate) fn last_entry_plain_lines(&self) -> Option<Vec<Line<'static>>> {
        self.entries.last().map(|e| e.cell.plain_lines())
    }

    pub fn record_completed_exec_command(
        &mut self,
        call_id: String,
        stdout: String,
        stderr: String,
        exit_code: i32,
    ) {
        let width = self.cached_width.get();
        for entry in self.entries.iter_mut() {
            let cell = &mut entry.cell;
            if let HistoryCell::ActiveExecCommand {
                call_id: history_id,
                command,
                start,
                ..
            } = cell
            {
                if &call_id == history_id {
                    *cell = HistoryCell::new_completed_exec_command(
                        command.clone(),
                        CommandOutput {
                            exit_code,
                            stdout,
                            stderr,
                            duration: start.elapsed(),
                        },
                    );

                    // Update cached line count.
                    if width > 0 {
                        entry.line_count.set(cell.height(width));
                    }
                    break;
                }
            }
        }
    }

    pub fn record_completed_mcp_tool_call(
        &mut self,
        call_id: String,
        success: bool,
        result: Result<mcp_types::CallToolResult, String>,
    ) {
        let width = self.cached_width.get();
        for entry in self.entries.iter_mut() {
            if let HistoryCell::ActiveMcpToolCall {
                call_id: history_id,
                invocation,
                start,
                ..
            } = &entry.cell
            {
                if &call_id == history_id {
                    let completed = HistoryCell::new_completed_mcp_tool_call(
                        width,
                        invocation.clone(),
                        *start,
                        success,
                        result,
                    );
                    entry.cell = completed;

                    if width > 0 {
                        entry.line_count.set(entry.cell.height(width));
                    }

                    break;
                }
            }
        }
    }
}

impl WidgetRef for ConversationHistoryWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let (title, border_style) = if self.has_input_focus {
            (
                "Messages (↑/↓ or j/k = line,  b/space = page)",
                Style::default().fg(Color::LightYellow),
            )
        } else {
            ("Messages (tab to focus)", Style::default().dim())
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style);

        // Compute the inner area that will be available for the list after
        // the surrounding `Block` is drawn.
        let inner = block.inner(area);
        let viewport_height = inner.height as usize;

        // Cache (and if necessary recalculate) the wrapped line counts for every
        // [`HistoryCell`] so that our scrolling math accounts for text
        // wrapping.  We always reserve one column on the right-hand side for the
        // scrollbar so that the content never renders "under" the scrollbar.
        let effective_width = inner.width.saturating_sub(1);

        if effective_width == 0 {
            return; // Nothing to draw – avoid division by zero.
        }

        // Recompute cache if the effective width changed.
        let num_lines: usize = if self.cached_width.get() != effective_width {
            self.cached_width.set(effective_width);

            let mut num_lines: usize = 0;
            for entry in &self.entries {
                let count = entry.cell.height(effective_width);
                num_lines += count;
                entry.line_count.set(count);
            }
            num_lines
        } else {
            self.entries.iter().map(|e| e.line_count.get()).sum()
        };

        // Determine the scroll position. Note the existing value of
        // `self.scroll_position` could exceed the maximum scroll offset if the
        // user made the window wider since the last render.
        let max_scroll = num_lines.saturating_sub(viewport_height);
        let scroll_pos = if self.scroll_position == usize::MAX {
            max_scroll
        } else {
            self.scroll_position.min(max_scroll)
        };

        // ------------------------------------------------------------------
        // Render order:
        //   1. Clear full widget area (avoid artifacts from prior frame).
        //   2. Draw the surrounding Block (border and title).
        //   3. Render *each* visible HistoryCell into its own sub-Rect while
        //      respecting partial visibility at the top and bottom.
        //   4. Draw the scrollbar track / thumb in the reserved column.
        // ------------------------------------------------------------------

        // Clear entire widget area first.
        Clear.render(area, buf);

        // Draw border + title.
        block.render(area, buf);

        // ------------------------------------------------------------------
        // Calculate which cells are visible for the current scroll position
        // and paint them one by one.
        // ------------------------------------------------------------------

        let mut y_cursor = inner.y; // first line inside viewport
        let mut remaining_height = inner.height as usize;
        let mut lines_to_skip = scroll_pos; // number of wrapped lines to skip (above viewport)

        for entry in &self.entries {
            let cell_height = entry.line_count.get();

            // Completely above viewport? Skip whole cell.
            if lines_to_skip >= cell_height {
                lines_to_skip -= cell_height;
                continue;
            }

            // Determine how much of this cell is visible.
            let visible_height = (cell_height - lines_to_skip).min(remaining_height);

            if visible_height == 0 {
                break; // no space left
            }

            let cell_rect = Rect {
                x: inner.x,
                y: y_cursor,
                width: effective_width,
                height: visible_height as u16,
            };

            entry.cell.render_window(lines_to_skip, cell_rect, buf);

            // Advance cursor inside viewport.
            y_cursor += visible_height as u16;
            remaining_height -= visible_height;

            // After the first (possibly partially skipped) cell, we no longer
            // need to skip lines at the top.
            lines_to_skip = 0;

            if remaining_height == 0 {
                break; // viewport filled
            }
        }

        // Always render a scrollbar *track* so the reserved column is filled.
        let overflow = num_lines.saturating_sub(viewport_height);

        let mut scroll_state = ScrollbarState::default()
            // The Scrollbar widget expects the *content* height minus the
            // viewport height.  When there is no overflow we still provide 0
            // so that the widget renders only the track without a thumb.
            .content_length(overflow)
            .position(scroll_pos);

        {
            // Choose a thumb color that stands out only when this pane has focus so that the
            // user's attention is naturally drawn to the active viewport. When unfocused we show
            // a low-contrast thumb so the scrollbar fades into the background without becoming
            // invisible.
            let thumb_style = if self.has_input_focus {
                Style::reset().fg(Color::LightYellow)
            } else {
                Style::reset().fg(Color::Gray)
            };

            // By default the Scrollbar widget inherits any style that was
            // present in the underlying buffer cells. That means if a colored
            // line happens to be underneath the scrollbar, the track (and
            // potentially the thumb) adopt that color. Explicitly setting the
            // track/thumb styles ensures we always draw the scrollbar with a
            // consistent palette regardless of what content is behind it.
            StatefulWidget::render(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("↑"))
                    .end_symbol(Some("↓"))
                    .begin_style(Style::reset().fg(Color::DarkGray))
                    .end_style(Style::reset().fg(Color::DarkGray))
                    .thumb_symbol("█")
                    .thumb_style(thumb_style)
                    .track_symbol(Some("│"))
                    .track_style(Style::reset().fg(Color::DarkGray)),
                inner,
                buf,
                &mut scroll_state,
            );
        }

        // Update auxiliary stats that the scroll handlers rely on.
        self.num_rendered_lines.set(num_lines);
        self.last_viewport_height.set(viewport_height);
    }
}

/// Common [`Wrap`] configuration used for both measurement and rendering so
/// they stay in sync.
#[inline]
pub(crate) const fn wrap_cfg() -> ratatui::widgets::Wrap {
    ratatui::widgets::Wrap { trim: false }
}
