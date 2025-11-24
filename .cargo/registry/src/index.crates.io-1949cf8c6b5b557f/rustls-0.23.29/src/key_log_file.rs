use alloc::vec::Vec;
use core::fmt::{Debug, Formatter};
use std::env::var_os;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::Write;
use std::sync::Mutex;

use crate::KeyLog;
use crate::log::warn;

// Internal mutable state for KeyLogFile
struct KeyLogFileInner {
    file: Option<File>,
    buf: Vec<u8>,
}

impl KeyLogFileInner {
    fn new(var: Option<OsString>) -> Self {
        let Some(path) = &var else {
            return Self {
                file: None,
                buf: Vec::new(),
            };
        };

        #[cfg_attr(not(feature = "logging"), allow(unused_variables))]
        let file = match OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
        {
            Ok(f) => Some(f),
            Err(e) => {
                warn!("unable to create key log file {path:?}: {e}");
                None
            }
        };

        Self {
            file,
            buf: Vec::new(),
        }
    }

    fn try_write(&mut self, label: &str, client_random: &[u8], secret: &[u8]) -> io::Result<()> {
        let Some(file) = &mut self.file else {
            return Ok(());
        };

        self.buf.truncate(0);
        write!(self.buf, "{label} ")?;
        for b in client_random.iter() {
            write!(self.buf, "{b:02x}")?;
        }
        write!(self.buf, " ")?;
        for b in secret.iter() {
            write!(self.buf, "{b:02x}")?;
        }
        writeln!(self.buf)?;
        file.write_all(&self.buf)
    }
}

impl Debug for KeyLogFileInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KeyLogFileInner")
            // Note: we omit self.buf deliberately as it may contain key data.
            .field("file", &self.file)
            .finish()
    }
}

/// [`KeyLog`] implementation that opens a file whose name is
/// given by the `SSLKEYLOGFILE` environment variable, and writes
/// keys into it.
///
/// If `SSLKEYLOGFILE` is not set, this does nothing.
///
/// If such a file cannot be opened, or cannot be written then
/// this does nothing but logs errors at warning-level.
pub struct KeyLogFile(Mutex<KeyLogFileInner>);

impl KeyLogFile {
    /// Makes a new `KeyLogFile`.  The environment variable is
    /// inspected and the named file is opened during this call.
    pub fn new() -> Self {
        let var = var_os("SSLKEYLOGFILE");
        Self(Mutex::new(KeyLogFileInner::new(var)))
    }
}

impl KeyLog for KeyLogFile {
    fn log(&self, label: &str, client_random: &[u8], secret: &[u8]) {
        #[cfg_attr(not(feature = "logging"), allow(unused_variables))]
        match self
            .0
            .lock()
            .unwrap()
            .try_write(label, client_random, secret)
        {
            Ok(()) => {}
            Err(e) => {
                warn!("error writing to key log file: {e}");
            }
        }
    }
}

impl Debug for KeyLogFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self.0.try_lock() {
            Ok(key_log_file) => write!(f, "{key_log_file:?}"),
            Err(_) => write!(f, "KeyLogFile {{ <locked> }}"),
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    fn init() {
        let _ = env_logger::builder()
            .is_test(true)
            .try_init();
    }

    #[test]
    fn test_env_var_is_not_set() {
        init();
        let mut inner = KeyLogFileInner::new(None);
        assert!(
            inner
                .try_write("label", b"random", b"secret")
                .is_ok()
        );
    }

    #[test]
    fn test_env_var_cannot_be_opened() {
        init();
        let mut inner = KeyLogFileInner::new(Some("/dev/does-not-exist".into()));
        assert!(
            inner
                .try_write("label", b"random", b"secret")
                .is_ok()
        );
    }

    #[test]
    fn test_env_var_cannot_be_written() {
        init();
        let mut inner = KeyLogFileInner::new(Some("/dev/full".into()));
        assert!(
            inner
                .try_write("label", b"random", b"secret")
                .is_err()
        );
    }
}
