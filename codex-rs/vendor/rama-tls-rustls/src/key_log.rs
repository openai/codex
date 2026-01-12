use crate::dep::rustls::KeyLog;
use rama_core::error::OpaqueError;
use rama_net::tls::keylog::{KeyLogFileHandle, new_key_log_file_handle};
use std::fmt;

#[derive(Debug, Clone)]
/// [`KeyLog`] implementation that opens a file for the given path.
pub struct KeyLogFile(KeyLogFileHandle);

impl KeyLogFile {
    /// Makes a new [`KeyLogFile`].
    pub fn new(path: &str) -> Result<Self, OpaqueError> {
        let handle = new_key_log_file_handle(path)?;
        Ok(Self(handle))
    }
}

impl KeyLog for KeyLogFile {
    #[inline]
    fn log(&self, label: &str, client_random: &[u8], secret: &[u8]) {
        let line = format!(
            "{} {:02x} {:02x}\n",
            label,
            PlainHex {
                slice: client_random
            },
            PlainHex { slice: secret },
        );
        self.0.write_log_line(line);
    }
}

struct PlainHex<'a, T: 'a> {
    slice: &'a [T],
}

impl<T: fmt::LowerHex> fmt::LowerHex for PlainHex<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_inner_hex(self.slice, f, fmt::LowerHex::fmt)
    }
}

fn fmt_inner_hex<T, F: Fn(&T, &mut fmt::Formatter) -> fmt::Result>(
    slice: &[T],
    f: &mut fmt::Formatter,
    fmt_fn: F,
) -> fmt::Result {
    for val in slice.iter() {
        fmt_fn(val, f)?;
    }
    Ok(())
}
