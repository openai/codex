/// A Ratatui example that demonstrates how to implement the `Widget` trait.
///
/// This example demonstrates various ways to implement `Widget` traits in Ratatui on a type, a
/// reference, and a mutable reference. It also shows how to use the `WidgetRef` trait to
/// render boxed widgets.
///
/// This example runs with the Ratatui library code in the branch that you are currently
/// reading. See the [`latest`] branch for the code which works with the most recent Ratatui
/// release.
///
/// [`latest`]: https://github.com/ratatui/ratatui/tree/latest
use std::time::{Duration, Instant};

use color_eyre::Result;
use crossterm::event;
use ratatui::DefaultTerminal;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Position, Rect, Size};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Widget, WidgetRef};

fn main() -> Result<()> {
    color_eyre::install()?;
    ratatui::run(|terminal| App::default().run(terminal))
}

#[derive(Default)]
struct App {
    should_quit: bool,
    timer: Timer,
    boxed_squares: BoxedSquares,
    green_square: RightAlignedSquare,
}

impl App {
    fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            self.render(terminal)?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn render(&mut self, tui: &mut DefaultTerminal) -> Result<()> {
        tui.draw(|frame| frame.render_widget(self, frame.area()))?;
        Ok(())
    }

    fn handle_events(&mut self) -> Result<()> {
        // Handle events at least 50 frames per second (gifs are usually 50fps)
        let timeout = Duration::from_secs_f64(1.0 / 50.0);
        if !event::poll(timeout)? {
            return Ok(());
        }
        if event::read()?.is_key_press() {
            self.should_quit = true;
        }
        Ok(())
    }
}

/// Implement the `Widget` trait on a mutable reference to the `App` type.
///
/// This allows the `App` type to be rendered as a widget. The `App` type owns several other widgets
/// that are rendered as part of the app. The `Widget` trait is implemented on a mutable reference
/// to the `App` type, which allows this to be rendered without consuming the `App` type, and allows
/// the sub-widgets to be mutable.
impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let constraints = Constraint::from_lengths([1, 1, 2, 1]);
        let [greeting, timer, squares, position] = area.layout(&Layout::vertical(constraints));

        // render an ephemeral greeting widget
        Greeting::new("Ratatui!").render(greeting, buf);

        // render a reference to the timer widget
        self.timer.render(timer, buf);

        // render a boxed widget containing red and blue squares
        self.boxed_squares.render(squares, buf);

        // render a mutable reference to the green square widget
        self.green_square.render(squares, buf);
        // Display the dynamically updated position of the green square
        let square_position = format!("Green square is at {}", self.green_square.last_position);
        square_position.render(position, buf);
    }
}

/// An ephemeral greeting widget.
///
/// This widget is implemented on the type itself, which means that it is consumed when it is
/// rendered. This is useful for widgets that are cheap to create, don't need to be reused, and
/// don't need to store any state between renders. This is the simplest way to implement a widget in
/// Ratatui, but in most cases, it is better to implement the `Widget` trait on a reference to the
/// type, as shown in the other examples below.
///
/// This was the way most widgets were implemented in Ratatui before `Widget` was implemented on
/// references in [PR #903] (merged in Ratatui 0.26.0).
///
/// [PR #903]: https://github.com/ratatui/ratatui/pull/903
struct Greeting {
    name: String,
}

impl Greeting {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl Widget for Greeting {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let greeting = format!("Hello, {}!", self.name);
        greeting.render(area, buf);
    }
}

/// A timer widget that displays the elapsed time since the timer was started.
#[derive(Debug)]
struct Timer {
    start: Instant,
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

/// This implements `Widget` on a reference to the type, which means that it can be reused and
/// doesn't need to be consumed when it is rendered. This is useful for widgets that need to store
/// state and be updated over time.
///
/// This approach was probably always available in Ratatui, but it wasn't widely used until `Widget`
/// was implemented on references in [PR #903] (merged in Ratatui 0.26.0). This is because all the
/// built-in widgets previously would consume themselves when rendered.
impl Widget for &Timer {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let elapsed = self.start.elapsed().as_secs_f32();
        let message = format!("Elapsed: {elapsed:.1?}s");
        message.render(area, buf);
    }
}

/// A widget that contains a list of several different widgets.
struct BoxedSquares {
    squares: Vec<Box<dyn WidgetRef>>,
}

impl Default for BoxedSquares {
    fn default() -> Self {
        let red_square: Box<dyn WidgetRef> = Box::new(RedSquare);
        let blue_square: Box<dyn WidgetRef> = Box::new(BlueSquare);
        Self {
            squares: vec![red_square, blue_square],
        }
    }
}

/// A widget that renders a red square.
struct RedSquare;

/// A widget that renders a blue square.
struct BlueSquare;

/// This implements the `Widget` trait on a reference to the type. It contains a list of boxed
/// widgets that implement the `WidgetRef` trait. This is useful for widgets that contain a list of
/// other widgets that can be different types.
impl Widget for &BoxedSquares {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let constraints = vec![Constraint::Length(4); self.squares.len()];
        let areas = area.layout_vec(&Layout::horizontal(constraints));
        for (widget, area) in self.squares.iter().zip(areas) {
            widget.render_ref(area, buf);
        }
    }
}

/// `RedSquare` and `BlueSquare` are widgets that render a red and blue square, respectively. They
/// implement the `WidgetRef` trait instead of the `Widget` trait, which which allows them to be
/// rendered as boxed widgets. It's not possible to use Widget for this as a dynamic reference to a
/// widget cannot generally be moved out of the box.
impl WidgetRef for RedSquare {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        fill(area, buf, "█", Color::Red);
    }
}

impl WidgetRef for BlueSquare {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        fill(area, buf, "█", Color::Blue);
    }
}

/// A widget that renders a green square aligned to the right of the area.
#[derive(Default)]
struct RightAlignedSquare {
    last_position: Position,
}

/// This widget is implemented on a mutable reference to the type, which means that it can store
/// state and update it when it is rendered. This is useful for widgets that need to store the
/// result of some calculation that can only be done when the widget is rendered.
///
/// The x and y coordinates of the square are stored in the widget and updated when the widget is
/// rendered. This allows the square to be aligned to the right of the area. These coordinates could
/// be used to perform hit testing (e.g. checking if a mouse click is inside the square). This app
/// just displays the coordinates as a string.
///
/// This approach was probably always available in Ratatui, but it wasn't widely used either. This
/// is an alternative to implementing the `StatefulWidget` trait, for situations where you want to
/// store the state in the widget itself instead of a separate struct.
impl Widget for &mut RightAlignedSquare {
    /// Render a green square aligned to the right of the area and store the position.
    fn render(self, area: Rect, buf: &mut Buffer) {
        const WIDTH: u16 = 4;
        let x = area.right() - WIDTH; // Align to the right
        self.last_position = Position { x, y: area.y };
        let size = Size::new(WIDTH, area.height);
        let area = Rect::from((self.last_position, size));
        fill(area, buf, "█", Color::Green);
    }
}

/// Fill the area with the specified symbol and style.
///
/// This probably should be a method on the `Buffer` type, but it is defined here for simplicity.
/// <https://github.com/ratatui/ratatui/issues/1146>
fn fill<S: Into<Style>>(area: Rect, buf: &mut Buffer, symbol: &str, style: S) {
    let style = style.into();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            buf[(x, y)].set_symbol(symbol).set_style(style);
        }
    }
}
