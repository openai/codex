#![allow(dead_code, unused_imports)]

use log::{debug, error, info, trace, warn, Level, LevelFilter, Log, Metadata, Record};
use std::sync::{Arc, Mutex};

struct State {
    last_log_level: Mutex<Option<Level>>,
    last_log_location: Mutex<Option<u32>>,
}

struct Logger(Arc<State>);

impl Log for Logger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        *self.0.last_log_level.lock().unwrap() = Some(record.level());
        *self.0.last_log_location.lock().unwrap() = record.line();
    }
    fn flush(&self) {}
}

#[test]
fn test_integration() {
    // These tests don't really make sense when static
    // max level filtering is applied
    #[cfg(not(any(
        feature = "max_level_off",
        feature = "max_level_error",
        feature = "max_level_warn",
        feature = "max_level_info",
        feature = "max_level_debug",
        feature = "max_level_trace",
        feature = "release_max_level_off",
        feature = "release_max_level_error",
        feature = "release_max_level_warn",
        feature = "release_max_level_info",
        feature = "release_max_level_debug",
        feature = "release_max_level_trace",
    )))]
    {
        let me = Arc::new(State {
            last_log_level: Mutex::new(None),
            last_log_location: Mutex::new(None),
        });
        let a = me.clone();
        let logger = Logger(me);

        test_filter(&logger, &a, LevelFilter::Off);
        test_filter(&logger, &a, LevelFilter::Error);
        test_filter(&logger, &a, LevelFilter::Warn);
        test_filter(&logger, &a, LevelFilter::Info);
        test_filter(&logger, &a, LevelFilter::Debug);
        test_filter(&logger, &a, LevelFilter::Trace);

        test_line_numbers(&logger, &a);
    }
}

fn test_filter(logger: &dyn Log, a: &State, filter: LevelFilter) {
    // tests to ensure logs with a level beneath 'max_level' are filtered out
    log::set_max_level(filter);
    error!(logger: logger, "");
    last(a, t(Level::Error, filter));
    warn!(logger: logger, "");
    last(a, t(Level::Warn, filter));
    info!(logger: logger, "");
    last(a, t(Level::Info, filter));
    debug!(logger: logger, "");
    last(a, t(Level::Debug, filter));
    trace!(logger: logger, "");
    last(a, t(Level::Trace, filter));

    fn t(lvl: Level, filter: LevelFilter) -> Option<Level> {
        if lvl <= filter {
            Some(lvl)
        } else {
            None
        }
    }
    fn last(state: &State, expected: Option<Level>) {
        let lvl = state.last_log_level.lock().unwrap().take();
        assert_eq!(lvl, expected);
    }
}

fn test_line_numbers(logger: &dyn Log, state: &State) {
    log::set_max_level(LevelFilter::Trace);

    info!(logger: logger, ""); // ensure check_line function follows log macro
    check_log_location(state);

    #[track_caller]
    fn check_log_location(state: &State) {
        let location = std::panic::Location::caller().line(); // get function calling location
        let line_number = state.last_log_location.lock().unwrap().take().unwrap(); // get location of most recent log
        assert_eq!(line_number, location - 1);
    }
}
