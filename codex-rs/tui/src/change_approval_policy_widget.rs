use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPane;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::Op;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::prelude::Color;
use ratatui::prelude::Widget;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::List;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;

const HEADER_TEXT: &str = "Switch approval policy";

const PLAIN: Style = Style::new();
const GREEN_STYLE: Style = Style::new().fg(Color::Green);
const BLUE_STYLE: Style = Style::new().fg(Color::Blue);

const APPROVAL_POLICY_OPTIONS: &[AskForApproval] = &[
    AskForApproval::UnlessTrusted,
    AskForApproval::OnFailure,
    AskForApproval::Never,
];

/// Widget for changing the approval policy in the bottom pane.
/// This widget allows the user to select a new approval policy from a list of options.
/// The user can navigate the options using the up/down arrow keys, and select an option
/// by pressing Enter. The selected option is sent as an event to the application.
/// The widget can be closed by pressing Esc.
pub(crate) struct ChangeApprovalPolicyWidget<'a> {
    app_event_tx: AppEventSender,
    change_prompt: Paragraph<'a>,
    selected_option: usize,
    complete: bool,
}
impl<'a> ChangeApprovalPolicyWidget<'a> {
    pub(crate) fn new(
        app_event_tx: AppEventSender,
        current_approval_policy: AskForApproval,
    ) -> Self {
        let lines = vec![
            Line::from(vec![
                Span::from("Current policy: "),
                Span::from(current_approval_policy.to_string()).style(GREEN_STYLE),
            ]),
            Line::from(""),
        ];
        let header = Paragraph::new(lines);

        Self {
            app_event_tx,
            change_prompt: header,
            selected_option: 0,
            complete: false,
        }
    }

    fn send_decision(&mut self) {
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::ChangeApprovalPolicy {
                approval_policy: APPROVAL_POLICY_OPTIONS[self.selected_option],
            }));
    }

    pub(crate) fn handle_key_event(&mut self, _pane: &mut BottomPane<'_>, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Up => {
                if self.selected_option == 0 {
                    self.selected_option = APPROVAL_POLICY_OPTIONS.len() - 1;
                } else {
                    self.selected_option -= 1;
                }
            }
            KeyCode::Down => {
                self.selected_option = (self.selected_option + 1) % APPROVAL_POLICY_OPTIONS.len();
            }
            KeyCode::Esc => {
                self.complete = true;
            }
            KeyCode::Enter => {
                self.send_decision();
                self.complete = true;
            }
            _ => {}
        };
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.complete
    }
}

impl WidgetRef for ChangeApprovalPolicyWidget<'_> {
    /// Render the widget to the given area and buffer.
    /// Implementation similar to the `UserApprovalWidget`
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let outer = Block::default()
            .title(HEADER_TEXT)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);
        let inner = outer.inner(area);

        let full_prompt_height = self.change_prompt.line_count(inner.width) as u16;
        let min_response_rows = APPROVAL_POLICY_OPTIONS.len() as u16;

        let prompt_height = full_prompt_height.min(inner.height.saturating_sub(min_response_rows));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(prompt_height), Constraint::Min(0)])
            .split(inner);
        let prompt_chunk = chunks[0];
        let response_chunk = chunks[1];

        let lines: Vec<Line> = APPROVAL_POLICY_OPTIONS
            .iter()
            .enumerate()
            .map(|(idx, opt)| {
                let (prefix, style) = if idx == self.selected_option {
                    ("▶", BLUE_STYLE)
                } else {
                    (" ", PLAIN)
                };
                Line::styled(format!("  {prefix} {opt}"), style)
            })
            .collect();

        outer.render(area, buf);
        self.change_prompt.clone().render(prompt_chunk, buf);
        Widget::render(List::new(lines), response_chunk, buf);
    }
}
