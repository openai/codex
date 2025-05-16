use crate::history_cell::CommandOutput;
use crate::history_cell::HistoryCell;
use crate::history_cell::PatchEventType;
use codex_core::config::Config;
use codex_core::protocol::FileChange;
use codex_core::protocol::SessionConfiguredEvent;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
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

    pub(crate) fn set_input_focus(&mut self, has_input_focus: bool) {
        self.has_input_focus = has_input_focus;
    }

    /// Returns true if it needs a redraw.
    pub(crate) fn handle_key_event(&mut self, key_event: KeyEvent) -> bool {
        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_up(1);
                true
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_down(1);
                true
            }
            KeyCode::PageUp | KeyCode::Char('b') => {
                self.scroll_page_up();
                true
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.scroll_page_down();
                true
            }
            _ => false,
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

    /// Scroll up by one full viewport height (Page Up).
    fn scroll_page_up(&mut self) {
        let viewport_height = self.last_viewport_height.get().max(1);

        // If we are currently in the "stick to bottom" mode, first convert the
        // implicit scroll position (`usize::MAX`) into an explicit offset that
        // represents the very bottom of the scroll region.  This mirrors the
        // logic from `scroll_up()`.
        if self.scroll_position == usize::MAX {
            self.scroll_position = self
                .num_rendered_lines
                .get()
                .saturating_sub(viewport_height);
        }

        // Move up by a full page.
        self.scroll_position = self.scroll_position.saturating_sub(viewport_height);
    }

    /// Scroll down by one full viewport height (Page Down).
    fn scroll_page_down(&mut self) {
        // Nothing to do if we're already stuck to the bottom.
        if self.scroll_position == usize::MAX {
            return;
        }

        let viewport_height = self.last_viewport_height.get().max(1);
        let num_lines = self.num_rendered_lines.get();

        // Calculate the maximum explicit scroll offset that is still within
        // range. This matches the logic in `scroll_down()` and the render
        // method.
        let max_scroll = num_lines.saturating_sub(viewport_height);

        // Attempt to move down by a full page.
        let new_pos = self.scroll_position.saturating_add(viewport_height);

        if new_pos >= max_scroll {
            // We have reached (or passed) the bottom – switch back to
            // automatic stick‑to‑bottom mode so that subsequent output keeps
            // the viewport pinned.
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
        let is_first_event = self.entries.is_empty();
        self.add_to_history(HistoryCell::new_session_info(config, event, is_first_event));
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
        let count = if width > 0 {
            wrapped_line_count_for_cell(&cell, width)
        } else {
            0
        };

        self.entries.push(Entry {
            cell,
            line_count: Cell::new(count),
        });
    }

    /// Remove all history entries and reset scrolling.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.scroll_position = usize::MAX;
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
                        entry
                            .line_count
                            .set(wrapped_line_count_for_cell(cell, width));
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
        result: Option<mcp_types::CallToolResult>,
    ) {
        // Convert result into serde_json::Value early so we don't have to
        // worry about lifetimes inside the match arm.
        let result_val = result.map(|r| {
            serde_json::to_value(r)
                .unwrap_or_else(|_| serde_json::Value::String("<serialization error>".into()))
        });

        let width = self.cached_width.get();
        for entry in self.entries.iter_mut() {
            if let HistoryCell::ActiveMcpToolCall {
                call_id: history_id,
                fq_tool_name,
                invocation,
                start,
                ..
            } = &entry.cell
            {
                if &call_id == history_id {
                    let completed = HistoryCell::new_completed_mcp_tool_call(
                        fq_tool_name.clone(),
                        invocation.clone(),
                        *start,
                        success,
                        result_val,
                    );
                    entry.cell = completed;

                    if width > 0 {
                        entry
                            .line_count
                            .set(wrapped_line_count_for_cell(&entry.cell, width));
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

        // Cache (and if necessary recalculate) the wrapped line counts for
        // every [`HistoryCell`] so that our scrolling math accounts for text
        // wrapping.
        let width = inner.width; // Width of the viewport in terminal cells.
        if width == 0 {
            return; // Nothing to draw – avoid division by zero.
        }

        // Recompute cache if the width changed.
        let num_lines: usize = if self.cached_width.get() != width {
            self.cached_width.set(width);

            let mut num_lines: usize = 0;
            for entry in &self.entries {
                let count = wrapped_line_count_for_cell(&entry.cell, width);
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
        // Build a *window* into the history so we only clone the `Line`s that
        // may actually be visible in this frame. We still hand the slice off
        // to a `Paragraph` with an additional scroll offset to avoid slicing
        // inside a wrapped line (we don’t have per-subline granularity).
        // ------------------------------------------------------------------

        // Find the first entry that intersects the current scroll position.
        let mut cumulative = 0usize;
        let mut first_idx = 0usize;
        for (idx, entry) in self.entries.iter().enumerate() {
            let next = cumulative + entry.line_count.get();
            if next > scroll_pos {
                first_idx = idx;
                break;
            }
            cumulative = next;
        }

        let offset_into_first = scroll_pos - cumulative;

        // Collect enough raw lines from `first_idx` onward to cover the
        // viewport. We may fetch *slightly* more than necessary (whole cells)
        // but never the entire history.
        let mut collected_wrapped = 0usize;
        let mut visible_lines: Vec<Line<'static>> = Vec::new();

        for entry in &self.entries[first_idx..] {
            visible_lines.extend(entry.cell.lines().iter().cloned());
            collected_wrapped += entry.line_count.get();
            if collected_wrapped >= offset_into_first + viewport_height {
                break;
            }
        }

        // Build the Paragraph with wrapping enabled so long lines are not
        // clipped. Apply vertical scroll so that `offset_into_first` wrapped
        // lines are hidden at the top.
        let paragraph = Paragraph::new(visible_lines)
            .block(block)
            .wrap(wrap_cfg())
            .scroll((offset_into_first as u16, 0));

        paragraph.render(area, buf);

        // Draw scrollbar if necessary.
        let needs_scrollbar = num_lines > viewport_height;
        if needs_scrollbar {
            let mut scroll_state = ScrollbarState::default()
                // The Scrollbar widget expects the *content* height minus the
                // viewport height, mirroring the calculation used previously.
                .content_length(num_lines.saturating_sub(viewport_height))
                .position(scroll_pos);

            // Choose a thumb color that stands out only when this pane has focus so that the
            // user’s attention is naturally drawn to the active viewport. When unfocused we show
            // a low‑contrast thumb so the scrollbar fades into the background without becoming
            // invisible.
            let thumb_style = if self.has_input_focus {
                Style::reset().fg(Color::LightYellow)
            } else {
                Style::reset().fg(Color::Gray)
            };

            StatefulWidget::render(
                // By default the Scrollbar widget inherits the style that was already present
                // in the underlying buffer cells. That means if a colored line (for example a
                // background task notification that we render in blue) happens to be underneath
                // the scrollbar, the track and thumb adopt that color and the scrollbar appears
                // to "change color." Explicitly setting the *track* and *thumb* styles ensures
                // we always draw the scrollbar with the same palette regardless of what content
                // is behind it.
                //
                // N.B. Only the *foreground* color matters here because the scrollbar symbols
                // themselves are filled‐in block glyphs that completely overwrite the prior
                // character cells. We therefore leave the background at its default value so it
                // blends nicely with the surrounding `Block`.
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("↑"))
                    .end_symbol(Some("↓"))
                    .begin_style(Style::reset().fg(Color::DarkGray))
                    .end_style(Style::reset().fg(Color::DarkGray))
                    // A solid thumb so that we can color it distinctly from the track.
                    .thumb_symbol("█")
                    // Apply the dynamic thumb color computed above. We still start from
                    // Style::reset() to clear any inherited modifiers.
                    .thumb_style(thumb_style)
                    // Thin vertical line for the track.
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
const fn wrap_cfg() -> ratatui::widgets::Wrap {
    ratatui::widgets::Wrap { trim: false }
}

/// Returns the wrapped line count for `cell` at the given `width` using the
/// same wrapping rules that `ConversationHistoryWidget` uses during
/// rendering.
fn wrapped_line_count_for_cell(cell: &HistoryCell, width: u16) -> usize {
    Paragraph::new(cell.lines().clone())
        .wrap(wrap_cfg())
        .line_count(width)
}
