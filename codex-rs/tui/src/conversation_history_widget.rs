use crate::history_cell::CommandOutput;
use crate::history_cell::HistoryCell;
use crate::history_cell::PatchEventType;
use codex_core::config::Config;
use codex_core::protocol::FileChange;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use ratatui::style::Style;
use ratatui::widgets::*;
use serde_json::Value as JsonValue;
use std::cell::Cell as StdCell;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct ConversationHistoryWidget {
    history: Vec<HistoryCell>,
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
            history: Vec::new(),
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
        // map this to a specific scroll position so we can caluate the delta.
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
        let max_scroll = num_rendered_lines
            .saturating_sub(viewport_height)
            .saturating_add(1);

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
        let max_scroll = num_lines.saturating_sub(viewport_height).saturating_add(1);

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

    pub fn add_user_message(&mut self, message: String) {
        self.add_to_history(HistoryCell::new_user_prompt(message));
    }

    pub fn add_agent_message(&mut self, message: String) {
        self.add_to_history(HistoryCell::new_agent_message(message));
    }

    pub fn add_background_event(&mut self, message: String) {
        self.add_to_history(HistoryCell::new_background_event(message));
    }

    /// Add a pending patch entry (before user approval).
    pub fn add_patch_event(
        &mut self,
        event_type: PatchEventType,
        changes: HashMap<PathBuf, FileChange>,
    ) {
        self.add_to_history(HistoryCell::new_patch_event(event_type, changes));
    }

    /// Note `model` could differ from `config.model` if the agent decided to
    /// use a different model than the one requested by the user.
    pub fn add_session_info(&mut self, config: &Config, model: String) {
        self.add_to_history(HistoryCell::new_session_info(config, model));
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
        self.history.push(cell);
    }

    /// Remove all history entries and reset scrolling.
    pub fn clear(&mut self) {
        self.history.clear();
        self.scroll_position = usize::MAX;
    }

    pub fn record_completed_exec_command(
        &mut self,
        call_id: String,
        stdout: String,
        stderr: String,
        exit_code: i32,
    ) {
        for cell in self.history.iter_mut() {
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

        for cell in self.history.iter_mut() {
            if let HistoryCell::ActiveMcpToolCall {
                call_id: history_id,
                fq_tool_name,
                invocation,
                start,
                ..
            } = cell
            {
                if &call_id == history_id {
                    let completed = HistoryCell::new_completed_mcp_tool_call(
                        fq_tool_name.clone(),
                        invocation.clone(),
                        *start,
                        success,
                        result_val,
                    );
                    *cell = completed;
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

        // ------------------------------------------------------------------
        // Build a *window* into the history instead of cloning the entire
        // history into a brand‑new Vec every time we are asked to render.
        //
        // There can be an unbounded number of `Line` objects in the history,
        // but the terminal will only ever display `height` of them at once.
        // By materialising only the `height` lines that are scrolled into
        // view we avoid the potentially expensive clone of the full
        // conversation every frame.
        // ------------------------------------------------------------------

        // Compute the inner area that will be available for the list after
        // the surrounding `Block` is drawn.
        let inner = block.inner(area);
        let viewport_height = inner.height as usize;

        // Collect the lines that will actually be visible in the viewport
        // while keeping track of the total number of lines so the scrollbar
        // stays correct.
        let num_lines: usize = self.history.iter().map(|c| c.lines().len()).sum();

        let max_scroll = num_lines.saturating_sub(viewport_height) + 1;
        let scroll_pos = if self.scroll_position == usize::MAX {
            max_scroll
        } else {
            self.scroll_position.min(max_scroll)
        };

        let mut visible_lines: Vec<Line<'static>> = Vec::with_capacity(viewport_height);

        if self.scroll_position == usize::MAX {
            // Stick‑to‑bottom mode: walk the history backwards and keep the
            // most recent `height` lines.  This touches at most `height`
            // lines regardless of how large the conversation grows.
            'outer_rev: for cell in self.history.iter().rev() {
                for line in cell.lines().iter().rev() {
                    visible_lines.push(line.clone());
                    if visible_lines.len() == viewport_height {
                        break 'outer_rev;
                    }
                }
            }
            visible_lines.reverse();
        } else {
            // Arbitrary scroll position.  Skip lines until we reach the
            // desired offset, then emit the next `height` lines.
            let start_line = scroll_pos;
            let mut current_index = 0usize;
            'outer_fwd: for cell in &self.history {
                for line in cell.lines() {
                    if current_index >= start_line {
                        visible_lines.push(line.clone());
                        if visible_lines.len() == viewport_height {
                            break 'outer_fwd;
                        }
                    }
                    current_index += 1;
                }
            }
        }

        // We track the number of lines in the struct so can let the user take over from
        // something other than usize::MAX when they start scrolling up. This could be
        // removed once we have the vec<Lines> in self.
        self.num_rendered_lines.set(num_lines);
        self.last_viewport_height.set(viewport_height);

        // The widget takes care of drawing the `block` and computing its own
        // inner area, so we render it over the full `area`.
        // We *manually* sliced the set of `visible_lines` to fit within the
        // viewport above, so there is no need to ask the `Paragraph` widget
        // to apply an additional scroll offset. Doing so would cause the
        // content to be shifted *twice* – once by our own logic and then a
        // second time by the widget – which manifested as the entire block
        // drifting off‑screen when the user attempted to scroll.

        let paragraph = Paragraph::new(visible_lines)
            .block(block)
            .wrap(Wrap { trim: false });
        paragraph.render(area, buf);

        let needs_scrollbar = num_lines > viewport_height;
        if needs_scrollbar {
            let mut scroll_state = ScrollbarState::default()
                // TODO(ragona):
                // I don't totally understand this, but it appears to work exactly as expected
                // if we set the content length as the lines minus the height. Maybe I was supposed
                // to use viewport_content_length or something, but this works and I'm backing away.
                .content_length(num_lines.saturating_sub(viewport_height))
                .position(scroll_pos);

            // Choose a thumb colour that stands out only when this pane has focus so that the
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
                // in the underlying buffer cells.  That means if a coloured line (for example a
                // background task notification that we render in blue) happens to be underneath
                // the scrollbar, the track and thumb adopt that colour and the scrollbar appears
                // to “change colour”.  Explicitly setting the *track* and *thumb* styles ensures
                // we always draw the scrollbar with the same palette regardless of what content
                // is behind it.
                //
                // N.B.  Only the *foreground* colour matters here because the scrollbar symbols
                // themselves are filled‐in block glyphs that completely overwrite the prior
                // character cells.  We therefore leave the background at its default value so it
                // blends nicely with the surrounding `Block`.
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("↑"))
                    .end_symbol(Some("↓"))
                    .begin_style(Style::reset().fg(Color::DarkGray))
                    .end_style(Style::reset().fg(Color::DarkGray))
                    // A solid thumb so that we can colour it distinctly from the track.
                    .thumb_symbol("█")
                    // Apply the dynamic thumb colour computed above. We still start from
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
    }
}
