// See: https://github.com/rust-lang/rust/pull/121364
#![allow(unknown_lints, ambiguous_negative_literals)]

mod init;
mod procmacro;
mod tc39_262;

/// The simplest possible logger that logs to stderr.
///
/// This logger does no filtering. Instead, it relies on the `log` crates
/// filtering via its global max_level setting.
///
/// This provides a super simple logger that works with the `log` crate.
/// We don't need anything fancy; just basic log levels and the ability to
/// print to stderr. We therefore avoid bringing in extra dependencies just
/// for this functionality.
#[cfg(all(test))]
#[derive(Debug)]
pub(crate) struct Logger(());

#[cfg(all(test, feature = "std", feature = "logging"))]
const LOGGER: &'static Logger = &Logger(());

#[cfg(all(test))]
impl Logger {
    /// Create a new logger that logs to stderr and initialize it as the
    /// global logger. If there was a problem setting the logger, then an
    /// error is returned.
    pub(crate) fn init() -> Result<(), log::SetLoggerError> {
        #[cfg(all(feature = "std", feature = "logging"))]
        {
            log::set_logger(LOGGER)?;
            log::set_max_level(log::LevelFilter::Trace);
            Ok(())
        }
        #[cfg(not(all(feature = "std", feature = "logging")))]
        {
            Ok(())
        }
    }
}

#[cfg(all(test, feature = "std", feature = "logging"))]
impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        // We set the log level via log::set_max_level, so we don't need to
        // implement filtering here.
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        let now = jiff::Timestamp::now();
        match (record.file(), record.line()) {
            (Some(file), Some(line)) => {
                std::eprintln!(
                    "{}|{}|{}|{}:{}: {}",
                    now,
                    record.level(),
                    record.target(),
                    file,
                    line,
                    record.args()
                );
            }
            (Some(file), None) => {
                std::eprintln!(
                    "{}|{}|{}|{}: {}",
                    now,
                    record.level(),
                    record.target(),
                    file,
                    record.args()
                );
            }
            _ => {
                std::eprintln!(
                    "{}|{}|{}: {}",
                    now,
                    record.level(),
                    record.target(),
                    record.args()
                );
            }
        }
    }

    fn flush(&self) {
        // We use eprintln! which is flushed on every call.
    }
}
