use std::fmt;
use std::hash::Hash;
use std::num::ParseIntError;
use std::ops::{BitAnd, BitOr, Not};

/// Flags that can be set on a `SpanContext`.
///
/// The current version of the specification only supports a single flag
/// [`TraceFlags::SAMPLED`].
///
/// See the W3C TraceContext specification's [trace-flags] section for more
/// details.
///
/// [trace-flags]: https://www.w3.org/TR/trace-context/#trace-flags
#[derive(Clone, Debug, Default, PartialEq, Eq, Copy, Hash)]
pub struct TraceFlags(u8);

impl TraceFlags {
    /// Trace flags with the `sampled` flag set to `0`.
    ///
    /// Spans that are not sampled will be ignored by most tracing tools.
    /// See the `sampled` section of the [W3C TraceContext specification] for details.
    ///
    /// [W3C TraceContext specification]: https://www.w3.org/TR/trace-context/#sampled-flag
    pub const NOT_SAMPLED: TraceFlags = TraceFlags(0x00);

    /// Trace flags with the `sampled` flag set to `1`.
    ///
    /// Spans that are not sampled will be ignored by most tracing tools.
    /// See the `sampled` section of the [W3C TraceContext specification] for details.
    ///
    /// [W3C TraceContext specification]: https://www.w3.org/TR/trace-context/#sampled-flag
    pub const SAMPLED: TraceFlags = TraceFlags(0x01);

    /// Construct new trace flags
    pub const fn new(flags: u8) -> Self {
        TraceFlags(flags)
    }

    /// Returns `true` if the `sampled` flag is set
    pub fn is_sampled(&self) -> bool {
        (*self & TraceFlags::SAMPLED) == TraceFlags::SAMPLED
    }

    /// Returns copy of the current flags with the `sampled` flag set.
    pub fn with_sampled(&self, sampled: bool) -> Self {
        if sampled {
            *self | TraceFlags::SAMPLED
        } else {
            *self & !TraceFlags::SAMPLED
        }
    }

    /// Returns the flags as a `u8`
    pub fn to_u8(self) -> u8 {
        self.0
    }
}

impl BitAnd for TraceFlags {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitOr for TraceFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl Not for TraceFlags {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl fmt::LowerHex for TraceFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

/// A 16-byte value which identifies a given trace.
///
/// The id is valid if it contains at least one non-zero byte.
#[derive(Clone, PartialEq, Eq, Copy, Hash)]
pub struct TraceId(u128);

impl TraceId {
    /// Invalid trace id
    pub const INVALID: TraceId = TraceId(0);

    /// Create a trace id from its representation as a byte array.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        TraceId(u128::from_be_bytes(bytes))
    }

    /// Return the representation of this trace id as a byte array.
    pub const fn to_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }

    /// Converts a string in base 16 to a trace id.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::trace::TraceId;
    ///
    /// assert!(TraceId::from_hex("42").is_ok());
    /// assert!(TraceId::from_hex("58406520a006649127e371903a2de979").is_ok());
    ///
    /// assert!(TraceId::from_hex("not_hex").is_err());
    /// ```
    pub fn from_hex(hex: &str) -> Result<Self, ParseIntError> {
        u128::from_str_radix(hex, 16).map(TraceId)
    }
}

impl From<u128> for TraceId {
    fn from(value: u128) -> Self {
        TraceId(value)
    }
}

impl fmt::Debug for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{:032x}", self.0))
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{:032x}", self.0))
    }
}

impl fmt::LowerHex for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

/// An 8-byte value which identifies a given span.
///
/// The id is valid if it contains at least one non-zero byte.
#[derive(Clone, PartialEq, Eq, Copy, Hash)]
pub struct SpanId(u64);

impl SpanId {
    /// Invalid span id
    pub const INVALID: SpanId = SpanId(0);

    /// Create a span id from its representation as a byte array.
    pub const fn from_bytes(bytes: [u8; 8]) -> Self {
        SpanId(u64::from_be_bytes(bytes))
    }

    /// Return the representation of this span id as a byte array.
    pub const fn to_bytes(self) -> [u8; 8] {
        self.0.to_be_bytes()
    }

    /// Converts a string in base 16 to a span id.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::trace::SpanId;
    ///
    /// assert!(SpanId::from_hex("42").is_ok());
    /// assert!(SpanId::from_hex("58406520a0066491").is_ok());
    ///
    /// assert!(SpanId::from_hex("not_hex").is_err());
    /// ```
    pub fn from_hex(hex: &str) -> Result<Self, ParseIntError> {
        u64::from_str_radix(hex, 16).map(SpanId)
    }
}

impl From<u64> for SpanId {
    fn from(value: u64) -> Self {
        SpanId(value)
    }
}

impl fmt::Debug for SpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{:016x}", self.0))
    }
}

impl fmt::Display for SpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{:016x}", self.0))
    }
}

impl fmt::LowerHex for SpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rustfmt::skip]
    fn trace_id_test_data() -> Vec<(TraceId, &'static str, [u8; 16])> {
        vec![
            (TraceId(0), "00000000000000000000000000000000", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            (TraceId(42), "0000000000000000000000000000002a", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 42]),
            (TraceId(126642714606581564793456114182061442190), "5f467fe7bf42676c05e20ba4a90e448e", [95, 70, 127, 231, 191, 66, 103, 108, 5, 226, 11, 164, 169, 14, 68, 142])
        ]
    }

    #[rustfmt::skip]
    fn span_id_test_data() -> Vec<(SpanId, &'static str, [u8; 8])> {
        vec![
            (SpanId(0), "0000000000000000", [0, 0, 0, 0, 0, 0, 0, 0]),
            (SpanId(42), "000000000000002a", [0, 0, 0, 0, 0, 0, 0, 42]),
            (SpanId(5508496025762705295), "4c721bf33e3caf8f", [76, 114, 27, 243, 62, 60, 175, 143])
        ]
    }

    #[test]
    fn test_trace_id() {
        for test_case in trace_id_test_data() {
            assert_eq!(format!("{}", test_case.0), test_case.1);
            assert_eq!(format!("{:032x}", test_case.0), test_case.1);
            assert_eq!(test_case.0.to_bytes(), test_case.2);

            assert_eq!(test_case.0, TraceId::from_hex(test_case.1).unwrap());
            assert_eq!(test_case.0, TraceId::from_bytes(test_case.2));
        }
    }

    #[test]
    fn test_span_id() {
        for test_case in span_id_test_data() {
            assert_eq!(format!("{}", test_case.0), test_case.1);
            assert_eq!(format!("{:016x}", test_case.0), test_case.1);
            assert_eq!(test_case.0.to_bytes(), test_case.2);

            assert_eq!(test_case.0, SpanId::from_hex(test_case.1).unwrap());
            assert_eq!(test_case.0, SpanId::from_bytes(test_case.2));
        }
    }
}
