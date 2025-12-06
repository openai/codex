//! A Ratatui example that demonstrates how to handle input form focus
//!
//! This example demonstrates how to handle cursor and input focus between multiple fields in a
//! form. You can navigate between fields using the Tab key.
//!
//! This does not handle cursor movement etc. This is just a simple example. In a real application,
//! consider using [`tui-input`], or [`tui-prompts`], or [`tui-textarea`].
//!
//! This example runs with the Ratatui library code in the branch that you are currently reading.
//! See the [`latest`] branch for the code which works with the most recent Ratatui release.
//!
//! [`latest`]: https://github.com/ratatui/ratatui/tree/latest
//! [`tui-input`]: https://crates.io/crates/tui-input
//! [`tui-prompts`]: https://crates.io/crates/tui-prompts
//! [`tui-textarea`]: https://crates.io/crates/tui-textarea

use color_eyre::Result;
use crossterm::event::{self, KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Offset, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;
use ratatui::{DefaultTerminal, Frame};
use serde::Serialize;

fn main() -> Result<()> {
    color_eyre::install()?;
    // serialize the form to JSON if the user submitted it, otherwise print "Canceled"
    match ratatui::run(|terminal| App::default().run(terminal)) {
        Ok(Some(form)) => println!("{}", serde_json::to_string_pretty(&form)?),
        Ok(None) => println!("Canceled"),
        Err(err) => eprintln!("{err}"),
    }
    Ok(())
}

#[derive(Default)]
struct App {
    state: AppState,
    form: InputForm,
}

#[derive(Default, PartialEq, Eq)]
enum AppState {
    #[default]
    Running,
    Cancelled,
    Submitted,
}

impl App {
    fn run(mut self, terminal: &mut DefaultTerminal) -> Result<Option<InputForm>> {
        while self.state == AppState::Running {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }
        match self.state {
            AppState::Cancelled => Ok(None),
            AppState::Submitted => Ok(Some(self.form)),
            AppState::Running => unreachable!(),
        }
    }

    fn render(&self, frame: &mut Frame) {
        self.form.render(frame);
    }

    fn handle_events(&mut self) -> Result<()> {
        if let Some(key) = event::read()?.as_key_press_event() {
            match key.code {
                KeyCode::Esc => self.state = AppState::Cancelled,
                KeyCode::Enter => self.state = AppState::Submitted,
                _ => self.form.on_key_press(key),
            }
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct InputForm {
    #[serde(skip)]
    focus: Focus,
    first_name: StringField,
    last_name: StringField,
    age: AgeField,
}

impl Default for InputForm {
    fn default() -> Self {
        Self {
            focus: Focus::FirstName,
            first_name: StringField::new("First Name"),
            last_name: StringField::new("Last Name"),
            age: AgeField::new("Age"),
        }
    }
}

impl InputForm {
    // Handle focus navigation or pass the event to the focused field.
    fn on_key_press(&mut self, event: KeyEvent) {
        match event.code {
            KeyCode::Tab => self.focus = self.focus.next(),
            _ => match self.focus {
                Focus::FirstName => self.first_name.on_key_press(event),
                Focus::LastName => self.last_name.on_key_press(event),
                Focus::Age => self.age.on_key_press(event),
            },
        }
    }

    /// Render the form with the current focus.
    ///
    /// The cursor is placed at the end of the focused field.
    fn render(&self, frame: &mut Frame) {
        let layout = Layout::vertical(Constraint::from_lengths([1, 1, 1]));
        let [first_name_area, last_name_area, age_area] = frame.area().layout(&layout);

        frame.render_widget(&self.first_name, first_name_area);
        frame.render_widget(&self.last_name, last_name_area);
        frame.render_widget(&self.age, age_area);

        let cursor_position = match self.focus {
            Focus::FirstName => first_name_area + self.first_name.cursor_offset(),
            Focus::LastName => last_name_area + self.last_name.cursor_offset(),
            Focus::Age => age_area + self.age.cursor_offset(),
        };
        frame.set_cursor_position(cursor_position);
    }
}

#[derive(Default, PartialEq, Eq)]
enum Focus {
    #[default]
    FirstName,
    LastName,
    Age,
}

impl Focus {
    // Round-robin focus order.
    const fn next(&self) -> Self {
        match self {
            Self::FirstName => Self::LastName,
            Self::LastName => Self::Age,
            Self::Age => Self::FirstName,
        }
    }
}

/// A new-type representing a string field with a label.
#[derive(Debug, Serialize)]
struct StringField {
    #[serde(skip)]
    label: &'static str,
    value: String,
}

impl StringField {
    const fn new(label: &'static str) -> Self {
        Self {
            label,
            value: String::new(),
        }
    }

    /// Handle input events for the string input.
    fn on_key_press(&mut self, event: KeyEvent) {
        match event.code {
            KeyCode::Char(c) => self.value.push(c),
            KeyCode::Backspace => {
                self.value.pop();
            }
            _ => {}
        }
    }

    fn cursor_offset(&self) -> Offset {
        let x = (self.label.len() + self.value.len() + 2) as i32;
        Offset::new(x, 0)
    }
}

impl Widget for &StringField {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::horizontal([
            Constraint::Length(self.label.len() as u16 + 2),
            Constraint::Fill(1),
        ]);
        let [label_area, value_area] = area.layout(&layout);
        let label = Line::from_iter([self.label, ": "]).bold();
        label.render(label_area, buf);
        self.value.clone().render(value_area, buf);
    }
}

/// A new-type representing a person's age in years (0-130).
#[derive(Default, Clone, Copy, Serialize)]
struct AgeField {
    #[serde(skip)]
    label: &'static str,
    value: u8,
}

impl AgeField {
    const MAX: u8 = 130;

    const fn new(label: &'static str) -> Self {
        Self { label, value: 0 }
    }

    /// Handle input events for the age input.
    ///
    /// Digits are accepted as input, with any input which would exceed the maximum age being
    /// ignored. The up/down arrow keys and 'j'/'k' keys can be used to increment/decrement the
    /// age.
    fn on_key_press(&mut self, event: KeyEvent) {
        match event.code {
            KeyCode::Char(digit @ '0'..='9') => {
                let value = self
                    .value
                    .saturating_mul(10)
                    .saturating_add(digit as u8 - b'0');
                if value <= Self::MAX {
                    self.value = value;
                }
            }
            KeyCode::Backspace => self.value /= 10,
            KeyCode::Up | KeyCode::Char('k') => self.increment(),
            KeyCode::Down | KeyCode::Char('j') => self.decrement(),
            _ => {}
        }
    }

    fn increment(&mut self) {
        self.value = self.value.saturating_add(1).min(Self::MAX);
    }

    const fn decrement(&mut self) {
        self.value = self.value.saturating_sub(1);
    }

    fn cursor_offset(&self) -> Offset {
        let x = (self.label.len() + self.value.to_string().len() + 2) as i32;
        Offset::new(x, 0)
    }
}

impl Widget for &AgeField {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::horizontal([
            Constraint::Length(self.label.len() as u16 + 2),
            Constraint::Fill(1),
        ]);
        let [label_area, value_area] = area.layout(&layout);
        let label = Line::from_iter([self.label, ": "]).bold();
        let value = self.value.to_string();
        label.render(label_area, buf);
        value.render(value_area, buf);
    }
}
