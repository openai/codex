//! A Ratatui example that shows the full range of RGB colors that can be displayed in the terminal.
//!
//! Requires a terminal that supports 24-bit color (true color) and unicode.
//!
//! This example also demonstrates how implementing the Widget trait on a mutable reference
//! allows the widget to update its state while it is being rendered. This allows the fps
//! widget to update the fps calculation and the colors widget to update a cached version of
//! the colors to render instead of recalculating them every frame.
//!
//! This is an alternative to using the `StatefulWidget` trait and a separate state struct. It
//! is useful when the state is only used by the widget and doesn't need to be shared with
//! other widgets.
//!
//! This example runs with the Ratatui library code in the branch that you are currently reading.
//! See the [`latest`] branch for the code which works with the most recent Ratatui release.
//!
//! [`latest`]: https://github.com/ratatui/ratatui/tree/latest

use std::time::{Duration, Instant};

use color_eyre::Result;
use crossterm::event;
use palette::convert::FromColorUnclamped;
use palette::{Okhsv, Srgb};
use ratatui::DefaultTerminal;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::Color;
use ratatui::text::Text;
use ratatui::widgets::Widget;

fn main() -> Result<()> {
    color_eyre::install()?;
    ratatui::run(|terminal| App::default().run(terminal))
}

#[derive(Debug, Default)]
struct App {
    /// The current state of the app (running or quit)
    state: AppState,

    /// A widget that displays the current frames per second
    fps_widget: FpsWidget,

    /// A widget that displays the full range of RGB colors that can be displayed in the terminal.
    colors_widget: ColorsWidget,
}

#[derive(Debug, Default, PartialEq, Eq)]
enum AppState {
    /// The app is running
    #[default]
    Running,

    /// The user has requested the app to quit
    Quit,
}

/// A widget that displays the current frames per second
#[derive(Debug)]
struct FpsWidget {
    /// The number of elapsed frames that have passed - used to calculate the fps
    frame_count: usize,

    /// The last instant that the fps was calculated
    last_instant: Instant,

    /// The current frames per second
    fps: Option<f32>,
}

/// A widget that displays the full range of RGB colors that can be displayed in the terminal.
///
/// This widget is animated and will change colors over time.
#[derive(Debug, Default)]
struct ColorsWidget {
    /// The colors to render - should be double the height of the area as we render two rows of
    /// pixels for each row of the widget using the half block character. This is computed any time
    /// the size of the widget changes.
    colors: Vec<Vec<Color>>,

    /// the number of elapsed frames that have passed - used to animate the colors by shifting the
    /// x index by the frame number
    frame_count: usize,
}

impl App {
    /// Run the app
    ///
    /// This is the main event loop for the app.
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while self.is_running() {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            self.handle_events()?;
        }
        Ok(())
    }

    const fn is_running(&self) -> bool {
        matches!(self.state, AppState::Running)
    }

    /// Handle any events that have occurred since the last time the app was rendered.
    fn handle_events(&mut self) -> Result<()> {
        // Ensure that the app only blocks for a period that allows the app to render at
        // approximately 60 FPS (this doesn't account for the time to render the frame, and will
        // also update the app immediately any time an event occurs)
        let timeout = Duration::from_secs_f32(1.0 / 60.0);
        if !event::poll(timeout)? {
            return Ok(());
        }
        if event::read()?.is_key_press() {
            self.state = AppState::Quit;
        }
        Ok(())
    }
}

/// Implement the Widget trait for &mut App so that it can be rendered
///
/// This is implemented on a mutable reference so that the app can update its state while it is
/// being rendered. This allows the fps widget to update the fps calculation and the colors widget
/// to update the colors to render.
impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        use Constraint::{Length, Min};
        let [top, colors] = area.layout(&Layout::vertical([Length(1), Min(0)]));
        let [title, fps] = top.layout(&Layout::horizontal([Min(0), Length(8)]));
        Text::from("colors_rgb example. Press q to quit")
            .centered()
            .render(title, buf);
        self.fps_widget.render(fps, buf);
        self.colors_widget.render(colors, buf);
    }
}

/// Default impl for `FpsWidget`
///
/// Manual impl is required because we need to initialize the `last_instant` field to the current
/// instant.
impl Default for FpsWidget {
    fn default() -> Self {
        Self {
            frame_count: 0,
            last_instant: Instant::now(),
            fps: None,
        }
    }
}

/// Widget impl for `FpsWidget`
///
/// This is implemented on a mutable reference so that we can update the frame count and fps
/// calculation while rendering.
impl Widget for &mut FpsWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.calculate_fps();
        if let Some(fps) = self.fps {
            let text = format!("{fps:.1} fps");
            Text::from(text).render(area, buf);
        }
    }
}

impl FpsWidget {
    /// Update the fps calculation.
    ///
    /// This updates the fps once a second, but only if the widget has rendered at least 2 frames
    /// since the last calculation. This avoids noise in the fps calculation when rendering on slow
    /// machines that can't render at least 2 frames per second.
    #[expect(clippy::cast_precision_loss)]
    fn calculate_fps(&mut self) {
        self.frame_count += 1;
        let elapsed = self.last_instant.elapsed();
        if elapsed > Duration::from_secs(1) && self.frame_count > 2 {
            self.fps = Some(self.frame_count as f32 / elapsed.as_secs_f32());
            self.frame_count = 0;
            self.last_instant = Instant::now();
        }
    }
}

/// Widget impl for `ColorsWidget`
///
/// This is implemented on a mutable reference so that we can update the frame count and store a
/// cached version of the colors to render instead of recalculating them every frame.
impl Widget for &mut ColorsWidget {
    /// Render the widget
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.setup_colors(area);
        let colors = &self.colors;
        for (xi, x) in (area.left()..area.right()).enumerate() {
            // animate the colors by shifting the x index by the frame number
            let xi = (xi + self.frame_count) % (area.width as usize);
            for (yi, y) in (area.top()..area.bottom()).enumerate() {
                // render a half block character for each row of pixels with the foreground color
                // set to the color of the pixel and the background color set to the color of the
                // pixel below it
                let fg = colors[yi * 2][xi];
                let bg = colors[yi * 2 + 1][xi];
                buf[Position::new(x, y)].set_char('▀').set_fg(fg).set_bg(bg);
            }
        }
        self.frame_count += 1;
    }
}

impl ColorsWidget {
    /// Setup the colors to render.
    ///
    /// This is called once per frame to setup the colors to render. It caches the colors so that
    /// they don't need to be recalculated every frame.
    #[expect(clippy::cast_precision_loss)]
    fn setup_colors(&mut self, size: Rect) {
        let Rect { width, height, .. } = size;
        // double the height because each screen row has two rows of half block pixels
        let height = height as usize * 2;
        let width = width as usize;
        // only update the colors if the size has changed since the last time we rendered
        if self.colors.len() == height && self.colors[0].len() == width {
            return;
        }
        self.colors = Vec::with_capacity(height);
        for y in 0..height {
            let mut row = Vec::with_capacity(width);
            for x in 0..width {
                let hue = x as f32 * 360.0 / width as f32;
                let value = (height - y) as f32 / height as f32;
                let saturation = Okhsv::max_saturation();
                let color = Okhsv::new(hue, saturation, value);
                let color = Srgb::<f32>::from_color_unclamped(color);
                let color: Srgb<u8> = color.into_format();
                let color = Color::Rgb(color.red, color.green, color.blue);
                row.push(color);
            }
            self.colors.push(row);
        }
    }
}
