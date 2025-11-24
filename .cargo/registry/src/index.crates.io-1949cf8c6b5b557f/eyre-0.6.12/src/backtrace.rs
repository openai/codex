#[cfg(backtrace)]
pub(crate) use std::backtrace::Backtrace;

#[cfg(not(backtrace))]
pub(crate) enum Backtrace {}

#[cfg(backtrace)]
macro_rules! backtrace_if_absent {
    ($err:expr) => {
        match $err.backtrace() {
            Some(_) => None,
            None => Some(Backtrace::capture()),
        }
    };
}

#[cfg(not(backtrace))]
macro_rules! backtrace_if_absent {
    ($err:expr) => {
        None
    };
}
