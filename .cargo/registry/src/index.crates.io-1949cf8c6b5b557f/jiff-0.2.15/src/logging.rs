// Some feature combinations result in some of these macros never being used.
// Which is fine. Just squash the warnings.
#![allow(dead_code, unused_macros)]

macro_rules! log {
    ($($tt:tt)*) => {
        #[cfg(feature = "logging")]
        {
            $($tt)*
        }
    }
}

macro_rules! error {
    ($($tt:tt)*) => { log!(log::error!($($tt)*)) }
}

macro_rules! warn {
    ($($tt:tt)*) => { log!(log::warn!($($tt)*)) }
}

macro_rules! info {
    ($($tt:tt)*) => { log!(log::info!($($tt)*)) }
}

macro_rules! debug {
    ($($tt:tt)*) => { log!(log::debug!($($tt)*)) }
}

macro_rules! trace {
    ($($tt:tt)*) => { log!(log::trace!($($tt)*)) }
}

/// A copy of std's `dbg!` macro that doesn't do pretty printing.
///
/// This is nice because we usually want more compact output in this crate.
/// Also, because we don't import std's prelude, we have to use `std::dbg!`.
/// This macro definition makes it available as `dbg!`.
#[cfg(feature = "std")]
macro_rules! dbg {
    () => {
        std::eprintln!(
            "[{}:{}:{}]",
            $crate::file!(),
            $crate::line!(),
            $crate::column!(),
        )
    };
    ($val:expr $(,)?) => {
        match $val {
            tmp => {
                std::eprintln!(
                    "[{}:{}:{}] {} = {:?}",
                    std::file!(),
                    std::line!(),
                    std::column!(),
                    std::stringify!($val),
                    &tmp,
                );
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($(dbg!($val)),+,)
    };
}

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
    pub(crate) fn init() -> Result<(), crate::Error> {
        #[cfg(all(feature = "std", feature = "logging"))]
        {
            log::set_logger(LOGGER).map_err(crate::Error::adhoc)?;
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
        match (record.file(), record.line()) {
            (Some(file), Some(line)) => {
                std::eprintln!(
                    "{}|{}|{}:{}: {}",
                    record.level(),
                    record.target(),
                    file,
                    line,
                    record.args()
                );
            }
            (Some(file), None) => {
                std::eprintln!(
                    "{}|{}|{}: {}",
                    record.level(),
                    record.target(),
                    file,
                    record.args()
                );
            }
            _ => {
                std::eprintln!(
                    "{}|{}: {}",
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
