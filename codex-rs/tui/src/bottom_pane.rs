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
                match key_event.into() {
                    Input {
                        key: Key::Enter,
                        shift: false,
                        alt: false,
                        ctrl: false,
                    } => {
                        let text = self.textarea.lines().join("\n");
                        // Clear the textarea (there is no dedicated clear API).
                        self.textarea.select_all();
                        self.textarea.cut();
                        self.request_redraw()?;
                        Ok(InputResult::Submitted(text))
                    }
                    input => {
                        self.textarea.input(input);
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
                std::cmp::max(text_rows, MIN_TEXTAREA_ROWS) as u16 + TEXTAREA_BORDER_LINES
            }
        }
    }
}

impl WidgetRef for &BottomPane<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        match &self.state {
            PaneState::StatusIndicator { view } => view.render_ref(area, buf),
            PaneState::ApprovalModal { current, .. } => current.render(area, buf),
            PaneState::TextInput => self.textarea.render(area, buf),
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
