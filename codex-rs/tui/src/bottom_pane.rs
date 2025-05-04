//! Bottom pane widget for the chat UI.
//!
//! This widget owns everything that is rendered in the terminal's lower
//! portion: either the multiline [`TextArea`] for user input or an active
//! [`UserApprovalWidget`] modal. All state and key-handling logic that is
//! specific to those UI elements lives here so that the parent
//! [`ChatWidget`] only has to forward events and render calls.

use std::sync::mpsc::SendError;
use std::sync::mpsc::Sender;

use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Alignment;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::BorderType;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use tui_textarea::Input;
use tui_textarea::Key;
use tui_textarea::TextArea;

use crate::app_event::AppEvent;
use crate::status_indicator_widget::StatusIndicatorWidget;
use crate::user_approval_widget::ApprovalRequest;
use crate::user_approval_widget::UserApprovalWidget;
use crate::slash_command_overlay::SlashCommandOverlay;
use crate::slash_commands::COMMANDS;

/// Minimum number of visible text rows inside the textarea.
const MIN_TEXTAREA_ROWS: usize = 3;
/// Number of terminal rows consumed by the textarea border (top + bottom).
const TEXTAREA_BORDER_LINES: u16 = 2;

/// Result returned by [`BottomPane::handle_key_event`].
pub enum InputResult {
    /// The user pressed <Enter> - the contained string is the message that
    /// should be forwarded to the agent and appended to the conversation
    /// history.
    Submitted(String),
    /// The user selected a slash command from the overlay.
    ExecuteCommand(String),
    None,
}

/// Internal state of the bottom pane.
///
/// `ApprovalModal` owns a `current` widget that is guaranteed to exist while
/// this variant is active. Additional queued modals are stored in `queue`.
enum PaneState<'a> {
    StatusIndicator {
        view: StatusIndicatorWidget,
    },
    TextInput,
    ApprovalModal {
        current: UserApprovalWidget<'a>,
        queue: Vec<UserApprovalWidget<'a>>,
    },
}

/// Everything that is drawn in the lower half of the chat UI.
pub(crate) struct BottomPane<'a> {
    /// Multiline input widget (always kept around so its history/yank buffer
    /// is preserved even while a modal is open).
    textarea: TextArea<'a>,

    /// Current state (text input vs. approval modal).
    state: PaneState<'a>,

    /// Channel used to notify the application that a redraw is required.
    app_event_tx: Sender<AppEvent>,

    has_input_focus: bool,

    is_task_running: bool,

    /// Whether the slash command overlay is currently shown.
    pub show_slash_overlay: bool,
    /// The current filter string for slash commands (after the '/').
    pub slash_filter: String,
    pub slash_selected: usize,
    pub slash_scroll_offset: usize,
    pub slash_overlay_height: Option<u16>,
    pub slash_overlay_locked: bool,
}

pub(crate) struct BottomPaneParams {
    pub(crate) app_event_tx: Sender<AppEvent>,
    pub(crate) has_input_focus: bool,
}

impl BottomPane<'_> {
    pub fn new(
        BottomPaneParams {
            app_event_tx,
            has_input_focus,
        }: BottomPaneParams,
    ) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_placeholder_text("send a message");
        textarea.set_cursor_line_style(Style::default());
        update_border_for_input_focus(&mut textarea, has_input_focus);

        Self {
            textarea,
            state: PaneState::TextInput,
            app_event_tx,
            has_input_focus,
            is_task_running: false,
            show_slash_overlay: false,
            slash_filter: String::new(),
            slash_selected: 0,
            slash_scroll_offset: 0,
            slash_overlay_height: None,
            slash_overlay_locked: false,
        }
    }

    /// Update the status indicator with the latest log line.  Only effective
    /// when the pane is currently in `StatusIndicator` mode.
    pub(crate) fn update_status_text(&mut self, text: String) -> Result<(), SendError<AppEvent>> {
        if let PaneState::StatusIndicator { view } = &mut self.state {
            view.update_text(text);
            self.request_redraw()?;
        }
        Ok(())
    }

    pub(crate) fn set_input_focus(&mut self, has_input_focus: bool) {
        self.has_input_focus = has_input_focus;
        update_border_for_input_focus(&mut self.textarea, has_input_focus);
    }

    /// Forward a key event to the appropriate child widget.
    pub fn handle_key_event(
        &mut self,
        key_event: KeyEvent,
    ) -> Result<InputResult, SendError<AppEvent>> {
        match &mut self.state {
            PaneState::StatusIndicator { view } => {
                if view.handle_key_event(key_event)? {
                    self.request_redraw()?;
                }
                Ok(InputResult::None)
            }
            PaneState::ApprovalModal { current, queue } => {
                // While in modal mode we always consume the Event.
                current.handle_key_event(key_event)?;

                // If the modal has finished, either advance to the next one
                // in the queue or fall back to the textarea.
                if current.is_complete() {
                    if !queue.is_empty() {
                        // Replace `current` with the first queued modal and
                        // drop the old value.
                        *current = queue.remove(0);
                    } else if self.is_task_running {
                        let desired_height = {
                            let text_rows = self.textarea.lines().len().max(MIN_TEXTAREA_ROWS);
                            text_rows as u16 + TEXTAREA_BORDER_LINES
                        };

                        self.state = PaneState::StatusIndicator {
                            view: StatusIndicatorWidget::new(
                                self.app_event_tx.clone(),
                                desired_height,
                            ),
                        };
                    } else {
                        self.state = PaneState::TextInput;
                    }
                }

                // Always request a redraw while a modal is up to ensure the
                // UI stays responsive.
                self.request_redraw()?;
                Ok(InputResult::None)
            }
            PaneState::TextInput => {
                if self.show_slash_overlay {
                    let filtered = self.filtered_slash_commands();
                    let overlay_height = filtered.len().min(12);
                    match key_event.code {
                        crossterm::event::KeyCode::Up => {
                            if self.slash_selected > 0 {
                                self.slash_selected -= 1;
                                if self.slash_selected < self.slash_scroll_offset {
                                    self.slash_scroll_offset = self.slash_selected;
                                }
                            }
                            self.request_redraw()?;
                            return Ok(InputResult::None);
                        }
                        crossterm::event::KeyCode::Down => {
                            if self.slash_selected + 1 < filtered.len() {
                                self.slash_selected += 1;
                                if self.slash_selected >= self.slash_scroll_offset + overlay_height {
                                    self.slash_scroll_offset = self.slash_selected + 1 - overlay_height;
                                }
                            }
                            self.request_redraw()?;
                            return Ok(InputResult::None);
                        }
                        crossterm::event::KeyCode::Enter => {
                            if let Some(cmd) = filtered.get(self.slash_selected) {
                                self.textarea.select_all();
                                self.textarea.cut();
                                self.show_slash_overlay = false;
                                self.slash_filter.clear();
                                self.slash_selected = 0;
                                self.slash_scroll_offset = 0;
                                self.request_redraw()?;
                                return Ok(InputResult::ExecuteCommand(cmd.name.to_string()));
                            }
                        }
                        _ => {}
                    }
                }
                match key_event.into() {
                    Input {
                        key: Key::Enter,
                        shift: false,
                        alt: false,
                        ctrl: false,
                    } => {
                        let text = self.textarea.lines().join("\n");
                        self.textarea.select_all();
                        self.textarea.cut();
                        self.show_slash_overlay = false;
                        self.slash_filter.clear();
                        self.slash_selected = 0;
                        self.slash_scroll_offset = 0;
                        self.request_redraw()?;
                        Ok(InputResult::Submitted(text))
                    }
                    input => {
                        self.textarea.input(input);
                        let current_input = self.textarea.lines().join("\n");
                        if current_input.starts_with('/') {
                            self.slash_filter = current_input[1..].to_string();
                            let filter_trimmed = self.slash_filter.trim();
                            let filtered = self.filtered_slash_commands();
                            if filter_trimmed.is_empty() || !filtered.is_empty() {
                                self.lock_overlay_height(80);
                                self.show_slash_overlay = true;
                            } else {
                                self.show_slash_overlay = false;
                                self.reset_overlay_height_lock();
                            }
                            self.slash_selected = 0;
                            self.slash_scroll_offset = 0;
                        } else {
                            self.show_slash_overlay = false;
                            self.slash_filter.clear();
                            self.slash_selected = 0;
                            self.slash_scroll_offset = 0;
                            self.reset_overlay_height_lock();
                        }
                        self.request_redraw()?;
                        Ok(InputResult::None)
                    }
                }
            }
        }
    }

    pub fn set_task_running(&mut self, is_task_running: bool) -> Result<(), SendError<AppEvent>> {
        self.is_task_running = is_task_running;

        match self.state {
            PaneState::TextInput => {
                if is_task_running {
                    self.state = PaneState::StatusIndicator {
                        view: StatusIndicatorWidget::new(self.app_event_tx.clone(), {
                            let text_rows =
                                self.textarea.lines().len().max(MIN_TEXTAREA_ROWS) as u16;
                            text_rows + TEXTAREA_BORDER_LINES
                        }),
                    };
                } else {
                    return Ok(());
                }
            }
            PaneState::StatusIndicator { .. } => {
                if is_task_running {
                    return Ok(());
                } else {
                    self.state = PaneState::TextInput;
                }
            }
            PaneState::ApprovalModal { .. } => {
                // Do not change state if a modal is showing.
                return Ok(());
            }
        }

        self.request_redraw()?;
        Ok(())
    }

    /// Enqueue a new approval request coming from the agent.
    ///
    /// Returns `true` when this is the *first* modal - in that case the caller
    /// should trigger a redraw so that the modal becomes visible.
    pub fn push_approval_request(&mut self, request: ApprovalRequest) -> bool {
        let widget = UserApprovalWidget::new(request, self.app_event_tx.clone());

        match &mut self.state {
            PaneState::StatusIndicator { .. } => {
                self.state = PaneState::ApprovalModal {
                    current: widget,
                    queue: Vec::new(),
                };
                true // Needs redraw so the modal appears.
            }
            PaneState::TextInput => {
                // Transition to modal state with an empty queue.
                self.state = PaneState::ApprovalModal {
                    current: widget,
                    queue: Vec::new(),
                };
                true // Needs redraw so the modal appears.
            }
            PaneState::ApprovalModal { queue, .. } => {
                queue.push(widget);
                false // Already in modal mode - no redraw required.
            }
        }
    }

    fn request_redraw(&self) -> Result<(), SendError<AppEvent>> {
        self.app_event_tx.send(AppEvent::Redraw)
    }

    /// Height (terminal rows) required to render the pane in its current
    /// state (modal or textarea).
    pub fn required_height(&self, area: &Rect) -> u16 {
        match &self.state {
            PaneState::StatusIndicator { view } => view.get_height(),
            PaneState::ApprovalModal { current, .. } => current.get_height(area),
            PaneState::TextInput => {
                let text_rows = self.textarea.lines().len();
                let input_height = std::cmp::max(text_rows, MIN_TEXTAREA_ROWS) as u16 + TEXTAREA_BORDER_LINES;
                let overlay_height = self.calc_overlay_height(area);
                let total = input_height + overlay_height;
                total.min(area.height)
            }
        }
    }

    /// Returns the current input value from the textarea.
    #[allow(dead_code)]
    pub fn current_input(&self) -> String {
        self.textarea.lines().join("\n")
    }

    fn filtered_slash_commands(&self) -> Vec<&'static crate::slash_commands::CommandInfo> {
        // Trim whitespace from the filter string
        let trimmed_filter = self.slash_filter.trim();
        
        // If filter is empty or contains only whitespace, show all commands
        if trimmed_filter.is_empty() {
            COMMANDS.iter().collect()
        } else {
            // Otherwise filter commands that contain the trimmed filter string
            COMMANDS
                .iter()
                .filter(|cmd| cmd.name.contains(trimmed_filter) || cmd.description.contains(trimmed_filter))
                .collect()
        }
    }

    fn calc_overlay_height(&self, area: &Rect) -> u16 {
        let filtered = self.filtered_slash_commands();
        if !self.show_slash_overlay || filtered.is_empty() {
            return 0;
        }
        if let Some(h) = self.slash_overlay_height {
            // Use locked height if set
            let available_height = area.height.saturating_sub(5);
            return h.min(available_height);
        }
        // Fallback: calculate as before
        let available_height = area.height.saturating_sub(5);
        let chevron_width = 2;
        let all_cmds = crate::slash_commands::COMMANDS;
        let cmd_max_len = all_cmds.iter().map(|c| c.name.len()).max().unwrap_or(0);
        let desc_start_x = 1 + chevron_width + cmd_max_len + 1;
        let desc_width = area.width.saturating_sub(desc_start_x as u16) as usize;
        let mut total_lines = 0;
        for cmd in all_cmds {
            let desc_lines = crate::slash_command_overlay::SlashCommandOverlay::wrap_text(&cmd.description, desc_width);
            total_lines += 1.max(desc_lines.len());
        }
        total_lines.min(available_height as usize) as u16
    }

    /// Calculate the height needed to display all commands, given a width.
    fn calculate_overlay_height_for_all_commands(&self, area_width: u16) -> u16 {
        let all_cmds = crate::slash_commands::COMMANDS;
        let chevron_width = 2;
        let cmd_max_len = all_cmds.iter().map(|c| c.name.len()).max().unwrap_or(0);
        let desc_start_x = 1 + chevron_width + cmd_max_len + 1;
        let desc_width = area_width.saturating_sub(desc_start_x as u16) as usize;
        let mut total_lines = 0;
        for cmd in all_cmds {
            let desc_lines = crate::slash_command_overlay::SlashCommandOverlay::wrap_text(&cmd.description, desc_width);
            total_lines += 1.max(desc_lines.len());
        }
        total_lines.min(20 as usize) as u16
    }

    /// Lock the overlay height if not already locked.
    fn lock_overlay_height(&mut self, area_width: u16) {
        if !self.slash_overlay_locked {
            self.slash_overlay_height = Some(self.calculate_overlay_height_for_all_commands(area_width));
            self.slash_overlay_locked = true;
        }
    }

    /// Reset the overlay height lock.
    fn reset_overlay_height_lock(&mut self) {
        self.slash_overlay_height = None;
        self.slash_overlay_locked = false;
    }
}

impl WidgetRef for &BottomPane<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        match &self.state {
            PaneState::StatusIndicator { view } => view.render_ref(area, buf),
            PaneState::ApprovalModal { current, .. } => current.render(area, buf),
            PaneState::TextInput => {
                let textarea_height = std::cmp::max(self.textarea.lines().len(), MIN_TEXTAREA_ROWS) as u16 + TEXTAREA_BORDER_LINES;
                let filtered = self.filtered_slash_commands();
                let overlay_height = if self.show_slash_overlay && !filtered.is_empty() {
                    self.calc_overlay_height(&area)
                } else {
                    0
                };
                let total_height = textarea_height + overlay_height;
                let input_area = Rect {
                    x: area.x,
                    y: area.y + area.height - total_height,
                    width: area.width,
                    height: textarea_height,
                };
                let overlay_area = if self.show_slash_overlay && overlay_height > 0 && !filtered.is_empty() {
                    Rect {
                        x: area.x,
                        y: input_area.y + input_area.height,
                        width: area.width,
                        height: overlay_height,
                    }
                } else {
                    Rect { x: 0, y: 0, width: 0, height: 0 }
                };
                self.textarea.render(input_area, buf);
                if self.show_slash_overlay && overlay_height > 0 && !filtered.is_empty() {
                    let overlay = SlashCommandOverlay {
                        filter: &self.slash_filter,
                        selected: self.slash_selected,
                        scroll_offset: self.slash_scroll_offset,
                        max_height: overlay_height as usize,
                    };
                    overlay.render(overlay_area, buf);
                }
            }
        }
    }
}

fn update_border_for_input_focus(textarea: &mut TextArea, has_input_focus: bool) {
    let (title, border_style) = if has_input_focus {
        (
            "use Enter to send for now (Ctrl‑D to quit)",
            Style::default().dim(),
        )
    } else {
        ("", Style::default())
    };
    let right_title = if has_input_focus {
        Line::from("press enter to send").alignment(Alignment::Right)
    } else {
        Line::from("")
    };

    textarea.set_block(
        ratatui::widgets::Block::default()
            .title_bottom(title)
            .title_bottom(right_title)
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style),
    );
}
