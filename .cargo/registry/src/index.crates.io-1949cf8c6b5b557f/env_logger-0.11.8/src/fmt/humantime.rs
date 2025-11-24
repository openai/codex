use std::fmt;
use std::time::SystemTime;

use crate::fmt::{Formatter, TimestampPrecision};

impl Formatter {
    /// Get a [`Timestamp`] for the current date and time in UTC.
    ///
    /// # Examples
    ///
    /// Include the current timestamp with the log record:
    ///
    /// ```
    /// use std::io::Write;
    ///
    /// let mut builder = env_logger::Builder::new();
    ///
    /// builder.format(|buf, record| {
    ///     let ts = buf.timestamp();
    ///
    ///     writeln!(buf, "{}: {}: {}", ts, record.level(), record.args())
    /// });
    /// ```
    pub fn timestamp(&self) -> Timestamp {
        Timestamp {
            time: SystemTime::now(),
            precision: TimestampPrecision::Seconds,
        }
    }

    /// Get a [`Timestamp`] for the current date and time in UTC with full
    /// second precision.
    pub fn timestamp_seconds(&self) -> Timestamp {
        Timestamp {
            time: SystemTime::now(),
            precision: TimestampPrecision::Seconds,
        }
    }

    /// Get a [`Timestamp`] for the current date and time in UTC with
    /// millisecond precision.
    pub fn timestamp_millis(&self) -> Timestamp {
        Timestamp {
            time: SystemTime::now(),
            precision: TimestampPrecision::Millis,
        }
    }

    /// Get a [`Timestamp`] for the current date and time in UTC with
    /// microsecond precision.
    pub fn timestamp_micros(&self) -> Timestamp {
        Timestamp {
            time: SystemTime::now(),
            precision: TimestampPrecision::Micros,
        }
    }

    /// Get a [`Timestamp`] for the current date and time in UTC with
    /// nanosecond precision.
    pub fn timestamp_nanos(&self) -> Timestamp {
        Timestamp {
            time: SystemTime::now(),
            precision: TimestampPrecision::Nanos,
        }
    }
}

/// An [RFC3339] formatted timestamp.
///
/// The timestamp implements [`Display`] and can be written to a [`Formatter`].
///
/// [RFC3339]: https://www.ietf.org/rfc/rfc3339.txt
/// [`Display`]: std::fmt::Display
pub struct Timestamp {
    time: SystemTime,
    precision: TimestampPrecision,
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        /// A `Debug` wrapper for `Timestamp` that uses the `Display` implementation.
        struct TimestampValue<'a>(&'a Timestamp);

        impl fmt::Debug for TimestampValue<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        f.debug_tuple("Timestamp")
            .field(&TimestampValue(self))
            .finish()
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Ok(ts) = jiff::Timestamp::try_from(self.time) else {
            return Err(fmt::Error);
        };

        match self.precision {
            TimestampPrecision::Seconds => write!(f, "{ts:.0}"),
            TimestampPrecision::Millis => write!(f, "{ts:.3}"),
            TimestampPrecision::Micros => write!(f, "{ts:.6}"),
            TimestampPrecision::Nanos => write!(f, "{ts:.9}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Timestamp;
    use crate::TimestampPrecision;

    #[test]
    fn test_display_timestamp() {
        let mut ts = Timestamp {
            time: std::time::SystemTime::UNIX_EPOCH,
            precision: TimestampPrecision::Nanos,
        };

        assert_eq!("1970-01-01T00:00:00.000000000Z", format!("{ts}"));

        ts.precision = TimestampPrecision::Micros;
        assert_eq!("1970-01-01T00:00:00.000000Z", format!("{ts}"));

        ts.precision = TimestampPrecision::Millis;
        assert_eq!("1970-01-01T00:00:00.000Z", format!("{ts}"));

        ts.precision = TimestampPrecision::Seconds;
        assert_eq!("1970-01-01T00:00:00Z", format!("{ts}"));
    }
}
