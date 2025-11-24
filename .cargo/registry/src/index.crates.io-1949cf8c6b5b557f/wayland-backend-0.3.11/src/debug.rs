//! Debugging helpers to handle `WAYLAND_DEBUG` env variable.

use std::{
    fmt::Display,
    os::unix::io::AsRawFd,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::protocol::Argument;

/// The `WAYLAND_DEBUG` env variable is set to debug client.
pub fn has_debug_client_env() -> bool {
    matches!(std::env::var_os("WAYLAND_DEBUG"), Some(str) if str == "1" || str == "client")
}

/// Print the dispatched message to stderr in a following format:
///
/// `[timestamp] <- interface@id.msg_name(args)`
#[cfg_attr(unstable_coverage, coverage(off))]
pub fn print_dispatched_message<Id: Display, Fd: AsRawFd>(
    interface: &str,
    id: u32,
    msg_name: &str,
    args: &[Argument<Id, Fd>],
) {
    // Add timestamp to output.
    print_timestamp();

    eprint!(" <- {}@{}.{}, ({})", interface, id, msg_name, DisplaySlice(args));

    // Add a new line.
    eprintln!();
}

/// Print the send message to stderr in a following format:
///
/// `[timestamp] -> interface@id.msg_name(args)`
#[cfg_attr(unstable_coverage, coverage(off))]
pub fn print_send_message<Id: Display, Fd: AsRawFd>(
    interface: &str,
    id: u32,
    msg_name: &str,
    args: &[Argument<Id, Fd>],
    discarded: bool,
) {
    // Add timestamp to output.
    print_timestamp();

    if discarded {
        eprint!("[discarded]");
    }

    eprint!(" -> {}@{}.{}({})", interface, id, msg_name, DisplaySlice(args));

    // Add a new line.
    eprintln!();
}

pub(crate) struct DisplaySlice<'a, D>(pub &'a [D]);

impl<D: Display> Display for DisplaySlice<'_, D> {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut it = self.0.iter();
        if let Some(val) = it.next() {
            write!(f, "{val}")?;
        }
        for val in it {
            write!(f, ", {val}")?;
        }
        Ok(())
    }
}

/// Print timestamp in seconds.microseconds format.
#[cfg_attr(unstable_coverage, coverage(off))]
fn print_timestamp() {
    if let Ok(timestamp) = SystemTime::now().duration_since(UNIX_EPOCH) {
        // NOTE this is all to make timestamps the same with libwayland, so the log doesn't look
        // out of place when sys tries to log on their own.
        let time = (timestamp.as_secs() * 1000000 + timestamp.subsec_nanos() as u64 / 1000) as u32;
        // NOTE annotate timestamp so we know which library emmited the log entry.
        eprint!("[{:7}.{:03}][rs]", time / 1000, time % 1000);
    }
}
