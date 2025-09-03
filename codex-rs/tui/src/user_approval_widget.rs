//! A modal widget that prompts the user to approve or deny an action
//! requested by the agent.
//!
//! This is a (very) rough port of
//! `src/components/chat/terminal-chat-command-review.tsx` from the TypeScript
//! UI to Rust using [`ratatui`]. The goal is feature‑parity for the keyboard
//! driven workflow – a fully‑fledged visual match is not required.

use std::path::PathBuf;
use std::sync::LazyLock;

use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::*;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Scrollbar;
use ratatui::widgets::ScrollbarOrientation;
use ratatui::widgets::ScrollbarState;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::history_cell;

/// Request coming from the agent that needs user approval.
pub(crate) enum ApprovalRequest {
    Exec {
        id: String,
        command: Vec<String>,
        reason: Option<String>,
    },
    ApplyPatch {
        id: String,
        reason: Option<String>,
        grant_root: Option<PathBuf>,
    },
}

/// Options displayed in the *select* mode.
///
/// The `key` is matched case-insensitively.
struct SelectOption {
    label: Line<'static>,
    description: &'static str,
    key: KeyCode,
    decision: ReviewDecision,
}

static COMMAND_SELECT_OPTIONS: LazyLock<Vec<SelectOption>> = LazyLock::new(|| {
    vec![
        SelectOption {
            label: Line::from(vec!["Y".underlined(), "es".into()]),
            description: "Approve and run the command",
            key: KeyCode::Char('y'),
            decision: ReviewDecision::Approved,
        },
        SelectOption {
            label: Line::from(vec!["A".underlined(), "lways".into()]),
            description: "Approve the command for the remainder of this session",
            key: KeyCode::Char('a'),
            decision: ReviewDecision::ApprovedForSession,
        },
        SelectOption {
            label: Line::from(vec!["N".underlined(), "o, provide feedback".into()]),
            description: "Do not run the command; provide feedback",
            key: KeyCode::Char('n'),
            decision: ReviewDecision::Abort,
        },
    ]
});

static PATCH_SELECT_OPTIONS: LazyLock<Vec<SelectOption>> = LazyLock::new(|| {
    vec![
        SelectOption {
            label: Line::from(vec!["Y".underlined(), "es".into()]),
            description: "Approve and apply the changes",
            key: KeyCode::Char('y'),
            decision: ReviewDecision::Approved,
        },
        SelectOption {
            label: Line::from(vec!["N".underlined(), "o, provide feedback".into()]),
            description: "Do not apply the changes; provide feedback",
            key: KeyCode::Char('n'),
            decision: ReviewDecision::Abort,
        },
    ]
});

/// A modal prompting the user to approve or deny the pending request.
pub(crate) struct UserApprovalWidget {
    approval_request: ApprovalRequest,
    app_event_tx: AppEventSender,
    confirmation_prompt: Paragraph<'static>,
    select_options: &'static Vec<SelectOption>,

    /// Currently selected index in *select* mode.
    selected_option: usize,

    /// Set to `true` once a decision has been sent – the parent view can then
    /// remove this widget from its queue.
    done: bool,

    /// Current vertical scroll offset for the prompt content.
    /// Stored in a Cell so render_ref can clamp it to bounds without &mut self.
    prompt_scroll: std::cell::Cell<u16>,
}

fn to_command_display<'a>(
    first_line: Vec<Span<'a>>,
    cmd: String,
    last_line: Vec<Span<'a>>,
) -> Vec<Line<'a>> {
    let command_lines: Vec<Span> = cmd.lines().map(|line| line.to_string().dim()).collect();

    let mut lines: Vec<Line<'a>> = vec![];

    let mut first_line = first_line.clone();
    if command_lines.len() == 1 {
        first_line.push(command_lines[0].clone());
        first_line.extend(last_line);
    } else {
        for line in command_lines {
            lines.push(vec!["    ".into(), line].into());
        }
        let last_line = last_line.clone();
        lines.push(Line::from(last_line));
    }
    lines.insert(0, Line::from(first_line));

    lines
}

impl UserApprovalWidget {
    pub(crate) fn new(approval_request: ApprovalRequest, app_event_tx: AppEventSender) -> Self {
        let confirmation_prompt = match &approval_request {
            ApprovalRequest::Exec {
                command, reason, ..
            } => {
                let cmd = strip_bash_lc_and_escape(command);
                let mut contents: Vec<Line> = to_command_display(
                    vec!["? ".fg(Color::Cyan), "Codex wants to run ".bold()],
                    cmd,
                    vec![],
                );

                contents.push(Line::from(""));
                if let Some(reason) = reason {
                    contents.push(Line::from(reason.clone().italic()));
                    contents.push(Line::from(""));
                }
                Paragraph::new(contents).wrap(Wrap { trim: false })
            }
            ApprovalRequest::ApplyPatch {
                reason, grant_root, ..
            } => {
                let mut contents: Vec<Line> = vec![];

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

                Paragraph::new(contents).wrap(Wrap { trim: false })
            }
        };

        Self {
            select_options: match &approval_request {
                ApprovalRequest::Exec { .. } => &COMMAND_SELECT_OPTIONS,
                ApprovalRequest::ApplyPatch { .. } => &PATCH_SELECT_OPTIONS,
            },
            approval_request,
            app_event_tx,
            confirmation_prompt,
            selected_option: 0,
            done: false,
            prompt_scroll: std::cell::Cell::new(0),
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
        if key.kind == KeyEventKind::Press {
            self.handle_select_key(key);
        }
    }

    /// Normalize a key for comparison.
    /// - For `KeyCode::Char`, converts to lowercase for case-insensitive matching.
    /// - Other key codes are returned unchanged.
    fn normalize_keycode(code: KeyCode) -> KeyCode {
        match code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            other => other,
        }
    }

    /// Handle Ctrl-C pressed by the user while the modal is visible.
    /// Behaves like pressing Escape: abort the request and close the modal.
    pub(crate) fn on_ctrl_c(&mut self) {
        self.send_decision(ReviewDecision::Abort);
    }

    fn handle_select_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            // Scroll prompt content
            KeyCode::Up => {
                let v = self.prompt_scroll.get();
                self.prompt_scroll.set(v.saturating_sub(1));
            }
            KeyCode::Down => {
                let v = self.prompt_scroll.get();
                self.prompt_scroll.set(v.saturating_add(1));
            }
            KeyCode::PageUp => {
                // Large step; precise clamp applied at render time
                let v = self.prompt_scroll.get();
                self.prompt_scroll.set(v.saturating_sub(10));
            }
            KeyCode::PageDown => {
                let v = self.prompt_scroll.get();
                self.prompt_scroll.set(v.saturating_add(10));
            }
            KeyCode::Home => {
                self.prompt_scroll.set(0);
            }
            KeyCode::End => {
                // Set to a large value; will be clamped at render time based on height
                self.prompt_scroll.set(u16::MAX);
            }
            KeyCode::Left => {
                self.selected_option = (self.selected_option + self.select_options.len() - 1)
                    % self.select_options.len();
            }
            KeyCode::Right => {
                self.selected_option = (self.selected_option + 1) % self.select_options.len();
            }
            KeyCode::Enter => {
                let opt = &self.select_options[self.selected_option];
                self.send_decision(opt.decision);
            }
            KeyCode::Esc => {
                self.send_decision(ReviewDecision::Abort);
            }
            other => {
                let normalized = Self::normalize_keycode(other);
                if let Some(opt) = self
                    .select_options
                    .iter()
                    .find(|opt| Self::normalize_keycode(opt.key) == normalized)
                {
                    self.send_decision(opt.decision);
                }
            }
        }
    }

    fn send_decision(&mut self, decision: ReviewDecision) {
        self.send_decision_with_feedback(decision, String::new())
    }

    fn send_decision_with_feedback(&mut self, decision: ReviewDecision, feedback: String) {
        match &self.approval_request {
            ApprovalRequest::Exec { command, .. } => {
                let cmd = strip_bash_lc_and_escape(command);
                // TODO: move this rendering into history_cell.
                let mut lines: Vec<Line<'static>> = vec![];

                // Result line based on decision.
                match decision {
                    ReviewDecision::Approved => {
                        lines.extend(to_command_display(
                            vec![
                                "✔ ".fg(Color::Green),
                                "You ".into(),
                                "approved".bold(),
                                " codex to run ".into(),
                            ],
                            cmd,
                            vec![" this time".bold()],
                        ));
                    }
                    ReviewDecision::ApprovedForSession => {
                        lines.extend(to_command_display(
                            vec![
                                "✔ ".fg(Color::Green),
                                "You ".into(),
                                "approved".bold(),
                                " codex to run ".into(),
                            ],
                            cmd,
                            vec![" every time this session".bold()],
                        ));
                    }
                    ReviewDecision::Denied => {
                        lines.extend(to_command_display(
                            vec![
                                "✗ ".fg(Color::Red),
                                "You ".into(),
                                "did not approve".bold(),
                                " codex to run ".into(),
                            ],
                            cmd,
                            vec![],
                        ));
                    }
                    ReviewDecision::Abort => {
                        lines.extend(to_command_display(
                            vec![
                                "✗ ".fg(Color::Red),
                                "You ".into(),
                                "canceled".bold(),
                                " the request to run ".into(),
                            ],
                            cmd,
                            vec![],
                        ));
                    }
                }

                if !feedback.trim().is_empty() {
                    lines.push(Line::from("feedback:"));
                    for l in feedback.lines() {
                        lines.push(Line::from(l.to_string()));
                    }
                }

                self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                    history_cell::new_user_approval_decision(lines),
                )));
            }
            ApprovalRequest::ApplyPatch { .. } => {
                // No history line for patch approval decisions.
            }
        }

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

        self.app_event_tx.send(AppEvent::CodexOp(op));
        self.done = true;
    }

    /// Returns `true` once the user has made a decision and the widget no
    /// longer needs to be displayed.
    pub(crate) fn is_complete(&self) -> bool {
        self.done
    }

    pub(crate) fn desired_height(&self, width: u16) -> u16 {
        // Desired height equals prompt content plus footer rows.
        // Footer rows: 1 title, 1 buttons, 1 description.
        self.get_confirmation_prompt_height(width) + 3
    }
}

impl WidgetRef for &UserApprovalWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        // Always reserve footer rows so options remain visible.
        let footer_rows = 3u16.min(area.height);
        let mut prompt_height = area.height.saturating_sub(footer_rows);
        // If there's no room for prompt (very small viewport), keep at least 1 row for footer.
        if footer_rows == area.height && area.height > 0 {
            prompt_height = 0;
        }
        let [prompt_chunk, response_chunk] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(prompt_height),
                Constraint::Length(footer_rows),
            ])
            .areas(area);

        let lines: Vec<Line> = self
            .select_options
            .iter()
            .enumerate()
            .map(|(idx, opt)| {
                let style = if idx == self.selected_option {
                    Style::new().bg(Color::Cyan).fg(Color::Black)
                } else {
                    Style::new().add_modifier(Modifier::DIM)
                };
                opt.label.clone().alignment(Alignment::Center).style(style)
            })
            .collect();

        let [title_area, button_area, description_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(response_chunk.inner(Margin::new(1, 0)));
        let title = match &self.approval_request {
            ApprovalRequest::Exec { .. } => "Allow command?",
            ApprovalRequest::ApplyPatch { .. } => "Apply changes?",
        };
        Line::from(title).render(title_area, buf);

        // Compute scroll bounds based on current width and reserved prompt height.
        let total_lines = self.get_confirmation_prompt_height(prompt_chunk.width);
        let max_scroll = total_lines.saturating_sub(prompt_chunk.height);
        // Clamp stored scroll to current bounds so Up works immediately after hitting End
        // or overscrolling past the bottom.
        let clamped = self.prompt_scroll.get().min(max_scroll);
        self.prompt_scroll.set(clamped);
        let scroll = clamped;

        // Render prompt with vertical scroll if any. `Paragraph::scroll` takes (y, x).
        let mut prompt = self.confirmation_prompt.clone();
        prompt = prompt.scroll((scroll, 0));
        prompt.render(prompt_chunk, buf);

        // Draw a right-edge scrollbar when content overflows.
        if total_lines > prompt_chunk.height && prompt_chunk.height > 0 {
            // Work around ratatui Scrollbar behavior by setting content_length to the
            // scrollable range (max_scroll) so the thumb reaches the very end when at
            // the bottom. Also specify the viewport length for a correct thumb size.
            let mut sb_state = ScrollbarState::new(max_scroll as usize)
                .position(scroll as usize)
                .viewport_content_length(prompt_chunk.height as usize);
            let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            ratatui::widgets::StatefulWidget::render(sb, prompt_chunk, buf, &mut sb_state);
        }
        let areas = Layout::horizontal(
            lines
                .iter()
                .map(|l| Constraint::Length(l.width() as u16 + 2)),
        )
        .spacing(1)
        .split(button_area);
        for (idx, area) in areas.iter().enumerate() {
            let line = &lines[idx];
            line.render(*area, buf);
        }

        // Footer description + optional scroll hint when applicable.
        let mut desc = self.select_options[self.selected_option]
            .description
            .to_string();
        if total_lines > prompt_chunk.height {
            if !desc.is_empty() {
                // Use a subtle bullet divider consistent with other UI hints.
                desc.push_str("   •   ");
            }
            // Keyboard-only controls; no mouse hint.
            desc.push_str("↑/↓ PgUp/PgDn Home/End");
        }
        Line::from(desc)
            .style(Style::new().italic().add_modifier(Modifier::DIM))
            .render(description_area.inner(Margin::new(1, 0)), buf);

        Block::bordered()
            .border_type(BorderType::QuadrantOutside)
            .border_style(Style::default().fg(Color::Cyan))
            .borders(Borders::LEFT)
            .render_ref(
                Rect::new(0, response_chunk.y, 1, response_chunk.height),
                buf,
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn lowercase_shortcut_is_accepted() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let req = ApprovalRequest::Exec {
            id: "1".to_string(),
            command: vec!["echo".to_string()],
            reason: None,
        };
        let mut widget = UserApprovalWidget::new(req, tx);
        widget.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        assert!(widget.is_complete());
        let mut events: Vec<AppEvent> = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            events.push(ev);
        }
        assert!(events.iter().any(|e| matches!(
            e,
            AppEvent::CodexOp(Op::ExecApproval {
                decision: ReviewDecision::Approved,
                ..
            })
        )));
    }

    #[test]
    fn uppercase_shortcut_is_accepted() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let req = ApprovalRequest::Exec {
            id: "2".to_string(),
            command: vec!["echo".to_string()],
            reason: None,
        };
        let mut widget = UserApprovalWidget::new(req, tx);
        widget.handle_key_event(KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::NONE));
        assert!(widget.is_complete());
        let mut events: Vec<AppEvent> = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            events.push(ev);
        }
        assert!(events.iter().any(|e| matches!(
            e,
            AppEvent::CodexOp(Op::ExecApproval {
                decision: ReviewDecision::Approved,
                ..
            })
        )));
    }

    #[test]
    fn long_prompt_keeps_footer_visible_and_scrolls() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        // Build a long multi-line reason to force overflow.
        let reason = (0..50)
            .map(|i| format!("line {i}: this is a long reason to test scrolling"))
            .collect::<Vec<_>>()
            .join("\n");

        let mut widget = UserApprovalWidget::new(
            ApprovalRequest::Exec {
                id: "scroll".into(),
                command: vec!["echo".into(), "hello".into()],
                reason: Some(reason),
            },
            tx,
        );

        // Render into a small area to guarantee overflow.
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        (&widget).render_ref(area, &mut buf);

        // Collect buffer to a single string for simple contains checks.
        let mut all = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                all.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            all.push('\n');
        }

        // Footer title and buttons should be visible despite overflow.
        assert!(
            all.contains("Allow command?"),
            "expected footer title visible"
        );
        assert!(all.contains("Yes"), "expected 'Yes' option visible");

        // Scroll down and render again; footer should still be visible.
        widget.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let mut buf2 = Buffer::empty(area);
        (&widget).render_ref(area, &mut buf2);
        let mut all2 = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                all2.push(buf2[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            all2.push('\n');
        }
        assert!(
            all2.contains("Allow command?"),
            "footer remains visible after scroll"
        );

        // Additionally, verify that the visible prompt content actually shifted
        // after scrolling down by comparing the prompt region (top rows) before
        // and after the scroll.
        let footer_rows = 3u16.min(area.height);
        let prompt_height = area.height.saturating_sub(footer_rows) as usize;
        let before_prompt: String = all
            .lines()
            .take(prompt_height)
            .collect::<Vec<_>>()
            .join("\n");
        let after_prompt: String = all2
            .lines()
            .take(prompt_height)
            .collect::<Vec<_>>()
            .join("\n");
        assert_ne!(
            before_prompt, after_prompt,
            "expected prompt content to shift after scrolling"
        );
    }
}
