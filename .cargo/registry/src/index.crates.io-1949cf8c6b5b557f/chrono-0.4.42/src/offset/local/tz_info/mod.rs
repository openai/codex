#![deny(missing_docs)]
#![allow(dead_code)]
#![warn(unreachable_pub)]

use std::num::ParseIntError;
use std::str::Utf8Error;
use std::time::SystemTimeError;
use std::{error, fmt, io};

mod timezone;
pub(crate) use timezone::TimeZone;

mod parser;
mod rule;

/// Unified error type for everything in the crate
#[derive(Debug)]
pub(crate) enum Error {
    /// Date time error
    DateTime(&'static str),
    /// Local time type search error
    FindLocalTimeType(&'static str),
    /// Local time type error
    LocalTimeType(&'static str),
    /// Invalid slice for integer conversion
    InvalidSlice(&'static str),
    /// Invalid Tzif file
    InvalidTzFile(&'static str),
    /// Invalid TZ string
    InvalidTzString(&'static str),
    /// I/O error
    Io(io::Error),
    /// Out of range error
    OutOfRange(&'static str),
    /// Integer parsing error
    ParseInt(ParseIntError),
    /// Date time projection error
    ProjectDateTime(&'static str),
    /// System time error
    SystemTime(SystemTimeError),
    /// Time zone error
    TimeZone(&'static str),
    /// Transition rule error
    TransitionRule(&'static str),
    /// Unsupported Tzif file
    UnsupportedTzFile(&'static str),
    /// Unsupported TZ string
    UnsupportedTzString(&'static str),
    /// UTF-8 error
    Utf8(Utf8Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;
        match self {
            DateTime(error) => write!(f, "invalid date time: {error}"),
            FindLocalTimeType(error) => error.fmt(f),
            LocalTimeType(error) => write!(f, "invalid local time type: {error}"),
            InvalidSlice(error) => error.fmt(f),
            InvalidTzString(error) => write!(f, "invalid TZ string: {error}"),
            InvalidTzFile(error) => error.fmt(f),
            Io(error) => error.fmt(f),
            OutOfRange(error) => error.fmt(f),
            ParseInt(error) => error.fmt(f),
            ProjectDateTime(error) => error.fmt(f),
            SystemTime(error) => error.fmt(f),
            TransitionRule(error) => write!(f, "invalid transition rule: {error}"),
            TimeZone(error) => write!(f, "invalid time zone: {error}"),
            UnsupportedTzFile(error) => error.fmt(f),
            UnsupportedTzString(error) => write!(f, "unsupported TZ string: {error}"),
            Utf8(error) => error.fmt(f),
        }
    }
}

impl error::Error for Error {}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::Io(error)
    }
}

impl From<ParseIntError> for Error {
    fn from(error: ParseIntError) -> Self {
        Error::ParseInt(error)
    }
}

impl From<SystemTimeError> for Error {
    fn from(error: SystemTimeError) -> Self {
        Error::SystemTime(error)
    }
}

impl From<Utf8Error> for Error {
    fn from(error: Utf8Error) -> Self {
        Error::Utf8(error)
    }
}

/// Number of hours in one day
const HOURS_PER_DAY: i64 = 24;
/// Number of seconds in one hour
const SECONDS_PER_HOUR: i64 = 3600;
/// Number of seconds in one day
const SECONDS_PER_DAY: i64 = SECONDS_PER_HOUR * HOURS_PER_DAY;
/// Number of days in one week
const DAYS_PER_WEEK: i64 = 7;

/// Month days in a normal year
const DAY_IN_MONTHS_NORMAL_YEAR: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
/// Cumulated month days in a normal year
const CUMUL_DAY_IN_MONTHS_NORMAL_YEAR: [i64; 12] =
    [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
