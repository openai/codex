use std::error;
use std::fmt;
use std::io;
use std::num;
use std::process;
use std::str;

/// A common error type for the `autocfg` crate.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}

impl error::Error for Error {
    fn description(&self) -> &str {
        "AutoCfg error"
    }

    fn cause(&self) -> Option<&error::Error> {
        match self.kind {
            ErrorKind::Io(ref e) => Some(e),
            ErrorKind::Num(ref e) => Some(e),
            ErrorKind::Utf8(ref e) => Some(e),
            ErrorKind::Process(_) | ErrorKind::Other(_) => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self.kind {
            ErrorKind::Io(ref e) => e.fmt(f),
            ErrorKind::Num(ref e) => e.fmt(f),
            ErrorKind::Utf8(ref e) => e.fmt(f),
            ErrorKind::Process(ref status) => {
                // Same message as the newer `ExitStatusError`
                write!(f, "process exited unsuccessfully: {}", status)
            }
            ErrorKind::Other(s) => s.fmt(f),
        }
    }
}

#[derive(Debug)]
enum ErrorKind {
    Io(io::Error),
    Num(num::ParseIntError),
    Process(process::ExitStatus),
    Utf8(str::Utf8Error),
    Other(&'static str),
}

pub fn from_exit(status: process::ExitStatus) -> Error {
    Error {
        kind: ErrorKind::Process(status),
    }
}

pub fn from_io(e: io::Error) -> Error {
    Error {
        kind: ErrorKind::Io(e),
    }
}

pub fn from_num(e: num::ParseIntError) -> Error {
    Error {
        kind: ErrorKind::Num(e),
    }
}

pub fn from_utf8(e: str::Utf8Error) -> Error {
    Error {
        kind: ErrorKind::Utf8(e),
    }
}

pub fn from_str(s: &'static str) -> Error {
    Error {
        kind: ErrorKind::Other(s),
    }
}
