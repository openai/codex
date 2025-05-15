//! A modal widget that prompts the user to approve or deny an action
//! requested by the agent.
//!
//! This is a (very) rough port of
//! `src/components/chat/terminal-chat-command-review.tsx` from the TypeScript
//! UI to Rust using [`ratatui`]. The goal is feature‑parity for the keyboard
//! driven workflow – a fully‑fledged visual match is not required.

use std::path::PathBuf;

use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::*;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::List;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::exec_command::relativize_to_home;
use crate::exec_command::strip_bash_lc_and_escape;

/// Request coming from the agent that needs user approval.
pub(crate) enum ApprovalRequest {
    Exec {
        id: String,
        command: Vec<String>,
        cwd: PathBuf,
        reason: Option<String>,
    },
    ApplyPatch {
        id: String,
        reason: Option<String>,
        grant_root: Option<PathBuf>,
    },
}

/// Options displayed in the *select* mode.
struct SelectOption {
    label: &'static str,
    decision: Option<ReviewDecision>,
    /// `true` when this option switches the widget to *input* mode.
    enters_input_mode: bool,
}

// keep in same order as in the TS implementation
const SELECT_OPTIONS: &[SelectOption] = &[
    SelectOption {
        label: "Yes (y)",
        decision: Some(ReviewDecision::Approved),

        enters_input_mode: false,
    },
    SelectOption {
        label: "Yes, always approve this exact command for this session (a)",
        decision: Some(ReviewDecision::ApprovedForSession),

        enters_input_mode: false,
    },
    SelectOption {
        label: "Edit or give feedback (e)",
        decision: None,

        enters_input_mode: true,
    },
    SelectOption {
        label: "No, and keep going (n)",
        decision: Some(ReviewDecision::Denied),

        enters_input_mode: false,
    },
    SelectOption {
        label: "No, and stop for now (esc)",
        decision: Some(ReviewDecision::Abort),

        enters_input_mode: false,
    },
];

/// Internal mode the widget is in – mirrors the TypeScript component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Select,
    Input,
}

/// A modal prompting the user to approve or deny the pending request.
pub(crate) struct UserApprovalWidget<'a> {
    approval_request: ApprovalRequest,
    app_event_tx: AppEventSender,
    confirmation_prompt: Paragraph<'a>,

    /// Currently selected index in *select* mode.
    selected_option: usize,

    /// State for the optional input widget.
    input: Input,

    /// Current mode.
    mode: Mode,

    /// Set to `true` once a decision has been sent – the parent view can then
    /// remove this widget from its queue.
    done: bool,
}

// Number of lines automatically added by ratatui’s [`Block`] when
// borders are enabled (one at the top, one at the bottom).
const BORDER_LINES: u16 = 2;

impl UserApprovalWidget<'_> {
    pub(crate) fn new(approval_request: ApprovalRequest, app_event_tx: AppEventSender) -> Self {
        let input = Input::default();
        let confirmation_prompt = match &approval_request {
            ApprovalRequest::Exec {
                command,
                cwd,
                reason,
                ..
            } => {
                let cmd = strip_bash_lc_and_escape(command);
                // Maybe try to relativize to the cwd of this process first?
                // Will make cwd_str shorter in the common case.
                let cwd_str = match relativize_to_home(cwd) {
                    Some(rel) => format!("~/{}", rel.display()),
                    None => cwd.display().to_string(),
                };
                let mut contents: Vec<Line> = vec![
                    Line::from("Shell Command".bold()),
                    Line::from(""),
                    Line::from(vec![
                        format!("{cwd_str}$").dim(),
                        Span::from(format!(" {cmd}")),
                    ]),
                    Line::from(""),
                ];
                if let Some(reason) = reason {
                    contents.push(Line::from(reason.clone().italic()));
                    contents.push(Line::from(""));
                }
                contents.extend(vec![Line::from("Allow command?"), Line::from("")]);
                Paragraph::new(contents)
            }
            ApprovalRequest::ApplyPatch {
                reason, grant_root, ..
            } => {
                let mut contents: Vec<Line> =
                    vec![Line::from("Apply patch".bold()), Line::from("")];

                if let Some(r) = reason {
                    contents.push(Line::from(r.clone().italic()));
                    contents.push(Line::from(""));
                }

                if let Some(root) = grant_root {
                    contents.push(Line::from(format!(
                        "This will grant write access to {} for the remainder of this session.",
                        root.display()
                    )));
                    contents.push(Line::from(""));
                }

                contents.push(Line::from("Allow changes?"));
                contents.push(Line::from(""));

                Paragraph::new(contents)
            }
        };

        Self {
            approval_request,
            app_event_tx,
            confirmation_prompt,
            selected_option: 0,
            input,
            mode: Mode::Select,
            done: false,
        }
    }

    pub(crate) fn get_height(&self, area: &Rect) -> u16 {
        let confirmation_prompt_height =
            self.get_confirmation_prompt_height(area.width - BORDER_LINES);

        match self.mode {
            Mode::Select => {
                let num_option_lines = SELECT_OPTIONS.len() as u16;
                confirmation_prompt_height + num_option_lines + BORDER_LINES
            }
            Mode::Input => {
                //   1. "Give the model feedback ..." prompt
                //   2. A single‑line input field (we allocate exactly one row;
                //      the `tui-input` widget will scroll horizontally if the
                //      text exceeds the width).
                const INPUT_PROMPT_LINES: u16 = 1;
                const INPUT_FIELD_LINES: u16 = 1;

                confirmation_prompt_height + INPUT_PROMPT_LINES + INPUT_FIELD_LINES + BORDER_LINES
            }
        }
    }

    fn get_confirmation_prompt_height(&self, width: u16) -> u16 {
        // Should cache this for last value of width.
        self.confirmation_prompt.line_count(width) as u16
    }

    /// Process a `KeyEvent` coming from crossterm. Always consumes the event
    /// while the modal is visible.
    /// Process a key event originating from crossterm. As the modal fully
    /// captures input while visible, we don’t need to report whether the event
    /// was consumed—callers can assume it always is.
    pub(crate) fn handle_key_event(&mut self, key: KeyEvent) {
        match self.mode {
            Mode::Select => self.handle_select_key(key),
            Mode::Input => self.handle_input_key(key),
        }
    }

    fn handle_select_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Up => {
                if self.selected_option == 0 {
                    self.selected_option = SELECT_OPTIONS.len() - 1;
                } else {
                    self.selected_option -= 1;
                }
            }
            KeyCode::Down => {
                self.selected_option = (self.selected_option + 1) % SELECT_OPTIONS.len();
            }
            KeyCode::Char('y') => {
                self.send_decision(ReviewDecision::Approved);
            }
            KeyCode::Char('a') => {
                self.send_decision(ReviewDecision::ApprovedForSession);
            }
            KeyCode::Char('n') => {
                self.send_decision(ReviewDecision::Denied);
            }
            KeyCode::Char('e') => {
                self.mode = Mode::Input;
            }
            KeyCode::Enter => {
                let opt = &SELECT_OPTIONS[self.selected_option];
                if opt.enters_input_mode {
                    self.mode = Mode::Input;
                } else if let Some(decision) = opt.decision {
                    self.send_decision(decision);
                }
            }
            KeyCode::Esc => {
                self.send_decision(ReviewDecision::Abort);
            }
            _ => {}
        }
    }

    fn handle_input_key(&mut self, key_event: KeyEvent) {
        // Handle special keys first.
        match key_event.code {
            KeyCode::Enter => {
                let feedback = self.input.value().to_string();
                self.send_decision_with_feedback(ReviewDecision::Denied, feedback);
            }
            KeyCode::Esc => {
                // Cancel input – treat as deny without feedback.
                self.send_decision(ReviewDecision::Denied);
            }
            _ => {
                // Feed into input widget for normal editing.
                let ct_event = crossterm::event::Event::Key(key_event);
                self.input.handle_event(&ct_event);
            }
        }
    }

    fn send_decision(&mut self, decision: ReviewDecision) {
        self.send_decision_with_feedback(decision, String::new())
    }

    fn send_decision_with_feedback(&mut self, decision: ReviewDecision, _feedback: String) {
        let op = match &self.approval_request {
            ApprovalRequest::Exec { id, .. } => Op::ExecApproval {
                id: id.clone(),
                decision,
            },
            ApprovalRequest::ApplyPatch { id, .. } => Op::PatchApproval {
                id: id.clone(),
                decision,
            },
        };

        // Ignore feedback for now – the current `Op` variants do not carry it.

        // Forward the Op to the agent. The caller (ChatWidget) will trigger a
        // redraw after it processes the resulting state change, so we avoid
        // issuing an extra Redraw here to prevent a transient frame where the
        // modal is still visible.
        self.app_event_tx.send(AppEvent::CodexOp(op));
        self.done = true;
    }

    /// Returns `true` once the user has made a decision and the widget no
    /// longer needs to be displayed.
    pub(crate) fn is_complete(&self) -> bool {
        self.done
    }
}

const PLAIN: Style = Style::new();
const BLUE_FG: Style = Style::new().fg(Color::Blue);

impl WidgetRef for &UserApprovalWidget<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        // Take the area, wrap it in a block with a border, and divide up the
        // remaining area into two chunks: one for the confirmation prompt and
        // one for the response.
        let outer = Block::default()
            .title("Review")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);
        let inner = outer.inner(area);
        let prompt_height = self.get_confirmation_prompt_height(inner.width);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(prompt_height), Constraint::Min(0)])
            .split(inner);
        let prompt_chunk = chunks[0];
        let response_chunk = chunks[1];

        // Build the inner lines based on the mode. Collect them into a List of
        // non-wrapping lines rather than a Paragraph because get_height(Rect)
        // depends on this behavior for its calculation.
        let lines = match self.mode {
            Mode::Select => SELECT_OPTIONS
                .iter()
                .enumerate()
                .map(|(idx, opt)| {
                    let (prefix, style) = if idx == self.selected_option {
                        ("▶", BLUE_FG)
                    } else {
                        (" ", PLAIN)
                    };
                    Line::styled(format!("  {prefix} {}", opt.label), style)
                })
                .collect(),
            Mode::Input => {
                vec![
                    Line::from("Give the model feedback on this command:"),
                    Line::from(self.input.value()),
                ]
            }
        };

        outer.render(area, buf);
        self.confirmation_prompt.clone().render(prompt_chunk, buf);
        Widget::render(List::new(lines), response_chunk, buf);
    }
}
