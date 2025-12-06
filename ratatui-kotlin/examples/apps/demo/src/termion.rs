#![allow(dead_code)]
use std::error::Error;
use std::sync::mpsc;
use std::time::Duration;
use std::{io, thread};

use ratatui::Terminal;
use ratatui::backend::{Backend, TermionBackend};
use termion::event::Key;
use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;

use crate::app::App;
use crate::ui;

pub fn run(tick_rate: Duration, enhanced_graphics: bool) -> Result<(), Box<dyn Error>> {
    // setup terminal
    let stdout = io::stdout()
        .into_raw_mode()
        .unwrap()
        .into_alternate_screen()
        .unwrap();
    let stdout = MouseTerminal::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let app = App::new("Termion demo", enhanced_graphics);
    run_app(&mut terminal, app, tick_rate)?;

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
) -> Result<(), Box<dyn Error>>
where
    B::Error: 'static,
{
    let events = events(tick_rate);
    loop {
        terminal.draw(|frame| ui::render(frame, &mut app))?;

        match events.recv()? {
            Event::Input(key) => match key {
                Key::Up | Key::Char('k') => app.on_up(),
                Key::Down | Key::Char('j') => app.on_down(),
                Key::Left | Key::Char('h') => app.on_left(),
                Key::Right | Key::Char('l') => app.on_right(),
                Key::Char(c) => app.on_key(c),
                _ => {}
            },
            Event::Tick => app.on_tick(),
        }
        if app.should_quit {
            return Ok(());
        }
    }
}

enum Event {
    Input(Key),
    Tick,
}

fn events(tick_rate: Duration) -> mpsc::Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    let keys_tx = tx.clone();
    thread::spawn(move || {
        let stdin = io::stdin();
        for key in stdin.keys().flatten() {
            if let Err(err) = keys_tx.send(Event::Input(key)) {
                eprintln!("{err}");
                return;
            }
        }
    });
    thread::spawn(move || {
        loop {
            if let Err(err) = tx.send(Event::Tick) {
                eprintln!("{err}");
                break;
            }
            thread::sleep(tick_rate);
        }
    });
    rx
}
