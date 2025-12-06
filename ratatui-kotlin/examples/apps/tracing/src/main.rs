/// A simple example demonstrating how to use the [tracing] with Ratatui to log to a file.
///
/// This example demonstrates how to use the [tracing] crate with Ratatui to log to a file. The
/// example sets up a simple logger that logs to a file named `tracing.log` in the current
/// directory.
///
/// Run the example with `cargo run --example tracing` and then view the `tracing.log` file to
/// see the logs. To see more logs, you can run the example with `RUST_LOG=tracing=debug cargo
/// run --example`
///
/// For a helpful widget that handles logging, see the [tui-logger] crate.
///
/// [tracing]: https://crates.io/crates/tracing
/// [tui-logger]: https://crates.io/crates/tui-logger
///
/// This example runs with the Ratatui library code in the branch that you are currently
/// reading. See the [`latest`] branch for the code which works with the most recent Ratatui
/// release.
///
/// [`latest`]: https://github.com/ratatui/ratatui/tree/latest
use std::{fs::File, time::Duration};

use color_eyre::Result;
use color_eyre::eyre::Context;
use crossterm::event::{self, Event, KeyCode};
use ratatui::Frame;
use ratatui::widgets::{Block, Paragraph};
use tracing::{Level, debug, info, instrument, trace};
use tracing_appender::non_blocking;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    color_eyre::install()?;

    let _guard = init_tracing()?;
    info!("Starting tracing example");

    let mut terminal = ratatui::init();
    let mut events = vec![]; // a buffer to store the recent events to display in the UI
    while !should_exit(&events) {
        handle_events(&mut events)?;
        terminal.draw(|frame| render(frame, &events))?;
    }
    ratatui::restore();

    info!("Exiting tracing example");
    println!("See the tracing.log file for the logs");
    Ok(())
}

fn should_exit(events: &[Event]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, Event::Key(key) if key.code == KeyCode::Char('q')))
}

/// Handle events and insert them into the events vector keeping only the last 10 events
#[instrument(skip(events))]
fn handle_events(events: &mut Vec<Event>) -> Result<()> {
    // Render the UI at least once every 100ms
    if event::poll(Duration::from_millis(100))? {
        let event = event::read()?;
        debug!(?event);
        events.insert(0, event);
    }
    events.truncate(10);
    Ok(())
}

#[instrument(skip_all)]
fn render(frame: &mut Frame, events: &[Event]) {
    // To view this event, run the example with `RUST_LOG=tracing=debug cargo run --example tracing`
    trace!(frame_count = frame.count(), event_count = events.len());
    let events = events.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>();
    let paragraph = Paragraph::new(events.join("\n"))
        .block(Block::bordered().title("Tracing example. Press 'q' to quit."));
    frame.render_widget(paragraph, frame.area());
}

/// Initialize the tracing subscriber to log to a file
///
/// This function initializes the tracing subscriber to log to a file named `tracing.log` in the
/// current directory. The function returns a [`WorkerGuard`] that must be kept alive for the
/// duration of the program to ensure that logs are flushed to the file on shutdown. The logs are
/// written in a non-blocking fashion to ensure that the logs do not block the main thread.
fn init_tracing() -> Result<WorkerGuard> {
    let file = File::create("tracing.log").wrap_err("failed to create tracing.log")?;
    let (non_blocking, guard) = non_blocking(file);

    // By default, the subscriber is configured to log all events with a level of `DEBUG` or higher,
    // but this can be changed by setting the `RUST_LOG` environment variable.
    let env_filter = EnvFilter::builder()
        .with_default_directive(Level::DEBUG.into())
        .from_env_lossy();

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter(env_filter)
        .init();
    Ok(guard)
}
