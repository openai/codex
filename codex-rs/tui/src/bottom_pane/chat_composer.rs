use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Alignment;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use tui_textarea::Input;
use tui_textarea::Key;
use tui_textarea::TextArea;

/// Minimum number of visible text rows inside the textarea.
const MIN_TEXTAREA_ROWS: usize = 1;
/// Rows consumed by the border.
const BORDER_LINES: u16 = 2;

/// Result returned when the user interacts with the text area.
pub enum InputResult {
    Submitted(String),
    None,
}

pub(crate) struct ChatComposer<'a> {
    textarea: TextArea<'a>,
}

impl ChatComposer<'_> {
    pub fn new(has_input_focus: bool) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_placeholder_text("send a message");
        textarea.set_cursor_line_style(ratatui::style::Style::default());

        let mut this = Self { textarea };
        this.update_border(has_input_focus);
        this
    }

    pub fn set_input_focus(&mut self, has_focus: bool) {
        self.update_border(has_focus);
    }

    /// Handle key event when no overlay is present.
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> (InputResult, bool) {
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
                (InputResult::Submitted(text), true)
            }
            Input {
                key: Key::Enter, ..
            }
            | Input {
                key: Key::Char('j'),
                ctrl: true,
                alt: false,
                shift: false,
            } => {
                self.textarea.insert_newline();
                (InputResult::None, true)
            }
            input => {
                self.textarea.input(input);
                (InputResult::None, true)
            }
        }
    }

    pub fn calculate_required_height(&self, _area: &Rect) -> u16 {
        let rows = self.textarea.lines().len().max(MIN_TEXTAREA_ROWS);
        rows as u16 + BORDER_LINES
    }

    fn update_border(&mut self, has_focus: bool) {
        struct BlockState {
            right_title: Line<'static>,
            border_style: Style,
        }

        let bs = if has_focus {
            BlockState {
                right_title: Line::from("Enter to send | Ctrl+D to quit | Ctrl+J for newline")
                    .alignment(Alignment::Right),
                border_style: Style::default(),
            }
        } else {
            BlockState {
                right_title: Line::from(""),
                border_style: Style::default().dim(),
            }
        };

        self.textarea.set_block(
            ratatui::widgets::Block::default()
                .title_bottom(bs.right_title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(bs.border_style),
        );
    }
}

impl WidgetRef for &ChatComposer<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        self.textarea.render(area, buf);
    }
}
