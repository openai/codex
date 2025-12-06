#![allow(dead_code)]
use std::error::Error;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TermwizBackend;
use termwiz::input::{InputEvent, KeyCode};
use termwiz::terminal::Terminal as TermwizTerminal;

use crate::app::App;
use crate::ui;

pub fn run(tick_rate: Duration, enhanced_graphics: bool) -> Result<(), Box<dyn Error>> {
    let backend = TermwizBackend::new()?;
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    // create app and run it
    let app = App::new("Termwiz Demo", enhanced_graphics);
    let app_result = run_app(&mut terminal, app, tick_rate);

    terminal.show_cursor()?;
    terminal.flush()?;

    if let Err(err) = app_result {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<TermwizBackend>,
    mut app: App,
    tick_rate: Duration,
) -> Result<(), Box<dyn Error>> {
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|frame| ui::render(frame, &mut app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if let Some(input) = terminal
            .backend_mut()
            .buffered_terminal_mut()
            .terminal()
            .poll_input(Some(timeout))?
        {
            match input {
                InputEvent::Key(key_code) => match key_code.key {
                    KeyCode::UpArrow | KeyCode::Char('k') => app.on_up(),
                    KeyCode::DownArrow | KeyCode::Char('j') => app.on_down(),
                    KeyCode::LeftArrow | KeyCode::Char('h') => app.on_left(),
                    KeyCode::RightArrow | KeyCode::Char('l') => app.on_right(),
                    KeyCode::Char(c) => app.on_key(c),
                    _ => {}
                },
                InputEvent::Resized { cols, rows } => {
                    terminal
                        .backend_mut()
                        .buffered_terminal_mut()
                        .resize(cols, rows);
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }
        if app.should_quit {
            return Ok(());
        }
    }
}
