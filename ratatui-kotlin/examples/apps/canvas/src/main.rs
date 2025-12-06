/// A Ratatui example that demonstrates how to draw on a canvas.
///
/// This example demonstrates how to draw various shapes such as rectangles, circles, and lines
/// on a canvas. It also demonstrates how to draw a map.
///
/// This example runs with the Ratatui library code in the branch that you are currently
/// reading. See the [`latest`] branch for the code which works with the most recent Ratatui
/// release.
///
/// [`latest`]: https://github.com/ratatui/ratatui/tree/latest
use std::{
    io::stdout,
    time::{Duration, Instant},
};

use color_eyre::Result;
use crossterm::ExecutableCommand;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, MouseEventKind,
};
use itertools::Itertools;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Stylize};
use ratatui::symbols::Marker;
use ratatui::text::Text;
use ratatui::widgets::canvas::{Canvas, Circle, Map, MapResolution, Points, Rectangle};
use ratatui::widgets::{Block, Widget};
use ratatui::{DefaultTerminal, Frame};

fn main() -> Result<()> {
    color_eyre::install()?;
    stdout().execute(EnableMouseCapture)?;
    let terminal = ratatui::init();
    let app_result = App::new().run(terminal);
    ratatui::restore();
    stdout().execute(DisableMouseCapture)?;
    app_result
}

struct App {
    exit: bool,
    x: f64,
    y: f64,
    ball: Circle,
    playground: Rect,
    vx: f64,
    vy: f64,
    marker: Marker,
    points: Vec<Position>,
    is_drawing: bool,
}

impl App {
    const fn new() -> Self {
        Self {
            exit: false,
            x: 0.0,
            y: 0.0,
            ball: Circle {
                x: 20.0,
                y: 40.0,
                radius: 10.0,
                color: Color::Yellow,
            },
            playground: Rect::new(10, 10, 200, 100),
            vx: 1.0,
            vy: 1.0,
            marker: Marker::Dot,
            points: vec![],
            is_drawing: false,
        }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let tick_rate = Duration::from_millis(16);
        let mut last_tick = Instant::now();
        while !self.exit {
            terminal.draw(|frame| self.render(frame))?;
            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if !event::poll(timeout)? {
                self.on_tick();
                last_tick = Instant::now();
                continue;
            }
            match event::read()? {
                Event::Key(key) => self.handle_key_event(key),
                Event::Mouse(event) => self.handle_mouse_event(event),
                _ => (),
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        if !key.is_press() {
            return;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.exit = true,
            KeyCode::Char('j') | KeyCode::Down => self.y += 1.0,
            KeyCode::Char('k') | KeyCode::Up => self.y -= 1.0,
            KeyCode::Char('l') | KeyCode::Right => self.x += 1.0,
            KeyCode::Char('h') | KeyCode::Left => self.x -= 1.0,
            KeyCode::Enter => self.cycle_marker(),
            _ => {}
        }
    }

    fn handle_mouse_event(&mut self, event: event::MouseEvent) {
        match event.kind {
            MouseEventKind::Down(_) => self.is_drawing = true,
            MouseEventKind::Up(_) => self.is_drawing = false,
            MouseEventKind::Drag(_) => {
                self.points.push(Position::new(event.column, event.row));
            }
            _ => {}
        }
    }

    const fn cycle_marker(&mut self) {
        self.marker = match self.marker {
            Marker::Dot => Marker::Braille,
            Marker::Braille => Marker::Block,
            Marker::Block => Marker::HalfBlock,
            Marker::HalfBlock => Marker::Quadrant,
            Marker::Quadrant => Marker::Sextant,
            Marker::Sextant => Marker::Octant,
            Marker::Octant => Marker::Bar,
            Marker::Bar => Marker::Dot,
            _ => unreachable!(),
        };
    }

    fn on_tick(&mut self) {
        // bounce the ball by flipping the velocity vector
        let ball = &self.ball;
        let playground = self.playground;
        if ball.x - ball.radius < f64::from(playground.left())
            || ball.x + ball.radius > f64::from(playground.right())
        {
            self.vx = -self.vx;
        }
        if ball.y - ball.radius < f64::from(playground.top())
            || ball.y + ball.radius > f64::from(playground.bottom())
        {
            self.vy = -self.vy;
        }
        self.ball.x += self.vx;
        self.ball.y += self.vy;
    }

    fn render(&self, frame: &mut Frame) {
        let header = Text::from_iter([
            "Canvas Example".bold(),
            "<q> Quit | <enter> Change Marker | <hjkl> Move".into(),
        ]);

        let vertical = Layout::vertical([
            Constraint::Length(header.height() as u16),
            Constraint::Fill(1),
            Constraint::Fill(1),
        ]);
        let [text_area, up, down] = frame.area().layout(&vertical);
        frame.render_widget(header.centered(), text_area);

        let horizontal = Layout::horizontal([Constraint::Fill(1); 2]);
        let [draw, pong] = up.layout(&horizontal);
        let [map, boxes] = down.layout(&horizontal);

        frame.render_widget(self.map_canvas(), map);
        frame.render_widget(self.draw_canvas(draw), draw);
        frame.render_widget(self.pong_canvas(), pong);
        frame.render_widget(self.boxes_canvas(boxes), boxes);
    }

    fn map_canvas(&self) -> impl Widget + '_ {
        Canvas::default()
            .block(Block::bordered().title("World"))
            .marker(self.marker)
            .paint(|ctx| {
                ctx.draw(&Map {
                    color: Color::Green,
                    resolution: MapResolution::High,
                });
                ctx.print(self.x, -self.y, "You are here".yellow());
            })
            .x_bounds([-180.0, 180.0])
            .y_bounds([-90.0, 90.0])
    }

    fn draw_canvas(&self, area: Rect) -> impl Widget + '_ {
        Canvas::default()
            .block(Block::bordered().title("Draw here"))
            .marker(self.marker)
            .x_bounds([0.0, f64::from(area.width)])
            .y_bounds([0.0, f64::from(area.height)])
            .paint(move |ctx| {
                let points = self
                    .points
                    .iter()
                    .map(|p| {
                        (
                            f64::from(p.x) - f64::from(area.left()),
                            f64::from(area.bottom()) - f64::from(p.y),
                        )
                    })
                    .collect_vec();
                ctx.draw(&Points {
                    coords: &points,
                    color: Color::White,
                });
            })
    }

    fn pong_canvas(&self) -> impl Widget + '_ {
        Canvas::default()
            .block(Block::bordered().title("Pong"))
            .marker(self.marker)
            .paint(|ctx| {
                ctx.draw(&self.ball);
            })
            .x_bounds([10.0, 210.0])
            .y_bounds([10.0, 110.0])
    }

    fn boxes_canvas(&self, area: Rect) -> impl Widget {
        let left = 0.0;
        let right = f64::from(area.width);
        let bottom = 0.0;
        let top = f64::from(area.height).mul_add(2.0, -4.0);
        Canvas::default()
            .block(Block::bordered().title("Rects"))
            .marker(self.marker)
            .x_bounds([left, right])
            .y_bounds([bottom, top])
            .paint(|ctx| {
                for i in 0..=11 {
                    ctx.draw(&Rectangle {
                        x: f64::from(i * i + 3 * i) / 2.0 + 2.0,
                        y: 2.0,
                        width: f64::from(i),
                        height: f64::from(i),
                        color: Color::Red,
                    });
                    ctx.draw(&Rectangle {
                        x: f64::from(i * i + 3 * i) / 2.0 + 2.0,
                        y: 21.0,
                        width: f64::from(i),
                        height: f64::from(i),
                        color: Color::Blue,
                    });
                }
                for i in 0..100 {
                    if i % 10 != 0 {
                        ctx.print(f64::from(i) + 1.0, 0.0, format!("{i}", i = i % 10));
                    }
                    if i % 2 == 0 && i % 10 != 0 {
                        ctx.print(0.0, f64::from(i), format!("{i}", i = i % 10));
                    }
                }
            })
    }
}
