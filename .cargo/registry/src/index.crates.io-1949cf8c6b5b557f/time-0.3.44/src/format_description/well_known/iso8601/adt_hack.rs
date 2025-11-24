//! Hackery to work around not being able to use ADTs in const generics on stable.

use core::num::NonZero;

#[cfg(feature = "formatting")]
use super::Iso8601;
use super::{Config, DateKind, FormattedComponents as FC, OffsetPrecision, TimePrecision};

// This provides a way to include `EncodedConfig` in documentation without displaying the type it is
// aliased to.
#[doc(hidden)]
pub type DoNotRelyOnWhatThisIs = u128;

/// An encoded [`Config`] that can be used as a const parameter to [`Iso8601`](super::Iso8601).
///
/// The type this is aliased to must not be relied upon. It can change in any release without
/// notice.
pub type EncodedConfig = DoNotRelyOnWhatThisIs;

#[cfg(feature = "formatting")]
impl<const CONFIG: EncodedConfig> Iso8601<CONFIG> {
    /// The user-provided configuration for the ISO 8601 format.
    const CONFIG: Config = Config::decode(CONFIG);
    /// Whether the date should be formatted.
    pub(crate) const FORMAT_DATE: bool = matches!(
        Self::CONFIG.formatted_components,
        FC::Date | FC::DateTime | FC::DateTimeOffset
    );
    /// Whether the time should be formatted.
    pub(crate) const FORMAT_TIME: bool = matches!(
        Self::CONFIG.formatted_components,
        FC::Time | FC::DateTime | FC::DateTimeOffset | FC::TimeOffset
    );
    /// Whether the UTC offset should be formatted.
    pub(crate) const FORMAT_OFFSET: bool = matches!(
        Self::CONFIG.formatted_components,
        FC::Offset | FC::DateTimeOffset | FC::TimeOffset
    );
    /// Whether the year is six digits.
    pub(crate) const YEAR_IS_SIX_DIGITS: bool = Self::CONFIG.year_is_six_digits;
    /// Whether the format contains separators (such as `-` or `:`).
    pub(crate) const USE_SEPARATORS: bool = Self::CONFIG.use_separators;
    /// Which format to use for the date.
    pub(crate) const DATE_KIND: DateKind = Self::CONFIG.date_kind;
    /// The precision and number of decimal digits to use for the time.
    pub(crate) const TIME_PRECISION: TimePrecision = Self::CONFIG.time_precision;
    /// The precision for the UTC offset.
    pub(crate) const OFFSET_PRECISION: OffsetPrecision = Self::CONFIG.offset_precision;
}

impl Config {
    /// Encode the configuration, permitting it to be used as a const parameter of [`Iso8601`].
    ///
    /// The value returned by this method must only be used as a const parameter to [`Iso8601`]. Any
    /// other usage is unspecified behavior.
    pub const fn encode(&self) -> EncodedConfig {
        let mut bytes = [0; EncodedConfig::BITS as usize / 8];

        bytes[0] = match self.formatted_components {
            FC::None => 0,
            FC::Date => 1,
            FC::Time => 2,
            FC::Offset => 3,
            FC::DateTime => 4,
            FC::DateTimeOffset => 5,
            FC::TimeOffset => 6,
        };
        bytes[1] = self.use_separators as u8;
        bytes[2] = self.year_is_six_digits as u8;
        bytes[3] = match self.date_kind {
            DateKind::Calendar => 0,
            DateKind::Week => 1,
            DateKind::Ordinal => 2,
        };
        bytes[4] = match self.time_precision {
            TimePrecision::Hour { .. } => 0,
            TimePrecision::Minute { .. } => 1,
            TimePrecision::Second { .. } => 2,
        };
        bytes[5] = match self.time_precision {
            TimePrecision::Hour { decimal_digits }
            | TimePrecision::Minute { decimal_digits }
            | TimePrecision::Second { decimal_digits } => match decimal_digits {
                None => 0,
                Some(decimal_digits) => decimal_digits.get(),
            },
        };
        bytes[6] = match self.offset_precision {
            OffsetPrecision::Hour => 0,
            OffsetPrecision::Minute => 1,
        };

        EncodedConfig::from_be_bytes(bytes)
    }

    /// Decode the configuration. The configuration must have been generated from
    /// [`Config::encode`].
    pub(super) const fn decode(encoded: EncodedConfig) -> Self {
        let bytes = encoded.to_be_bytes();

        let formatted_components = match bytes[0] {
            0 => FC::None,
            1 => FC::Date,
            2 => FC::Time,
            3 => FC::Offset,
            4 => FC::DateTime,
            5 => FC::DateTimeOffset,
            6 => FC::TimeOffset,
            _ => panic!("invalid configuration"),
        };
        let use_separators = match bytes[1] {
            0 => false,
            1 => true,
            _ => panic!("invalid configuration"),
        };
        let year_is_six_digits = match bytes[2] {
            0 => false,
            1 => true,
            _ => panic!("invalid configuration"),
        };
        let date_kind = match bytes[3] {
            0 => DateKind::Calendar,
            1 => DateKind::Week,
            2 => DateKind::Ordinal,
            _ => panic!("invalid configuration"),
        };
        let time_precision = match bytes[4] {
            0 => TimePrecision::Hour {
                decimal_digits: NonZero::new(bytes[5]),
            },
            1 => TimePrecision::Minute {
                decimal_digits: NonZero::new(bytes[5]),
            },
            2 => TimePrecision::Second {
                decimal_digits: NonZero::new(bytes[5]),
            },
            _ => panic!("invalid configuration"),
        };
        let offset_precision = match bytes[6] {
            0 => OffsetPrecision::Hour,
            1 => OffsetPrecision::Minute,
            _ => panic!("invalid configuration"),
        };

        // No `for` loops in `const fn`.
        let mut idx = 7; // first unused byte
        while idx < EncodedConfig::BITS as usize / 8 {
            assert!(bytes[idx] == 0, "invalid configuration");
            idx += 1;
        }

        Self {
            formatted_components,
            use_separators,
            year_is_six_digits,
            date_kind,
            time_precision,
            offset_precision,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! eq {
        ($a:expr, $b:expr) => {{
            let a = $a;
            let b = $b;
            a.formatted_components == b.formatted_components
                && a.use_separators == b.use_separators
                && a.year_is_six_digits == b.year_is_six_digits
                && a.date_kind == b.date_kind
                && a.time_precision == b.time_precision
                && a.offset_precision == b.offset_precision
        }};
    }

    #[test]
    fn encoding_roundtrip() {
        macro_rules! assert_roundtrip {
            ($config:expr) => {
                let config = $config;
                let encoded = config.encode();
                let decoded = Config::decode(encoded);
                assert!(eq!(config, decoded));
            };
        }

        assert_roundtrip!(Config::DEFAULT);
        assert_roundtrip!(Config::DEFAULT.set_formatted_components(FC::None));
        assert_roundtrip!(Config::DEFAULT.set_formatted_components(FC::Date));
        assert_roundtrip!(Config::DEFAULT.set_formatted_components(FC::Time));
        assert_roundtrip!(Config::DEFAULT.set_formatted_components(FC::Offset));
        assert_roundtrip!(Config::DEFAULT.set_formatted_components(FC::DateTime));
        assert_roundtrip!(Config::DEFAULT.set_formatted_components(FC::DateTimeOffset));
        assert_roundtrip!(Config::DEFAULT.set_formatted_components(FC::TimeOffset));
        assert_roundtrip!(Config::DEFAULT.set_use_separators(false));
        assert_roundtrip!(Config::DEFAULT.set_use_separators(true));
        assert_roundtrip!(Config::DEFAULT.set_year_is_six_digits(false));
        assert_roundtrip!(Config::DEFAULT.set_year_is_six_digits(true));
        assert_roundtrip!(Config::DEFAULT.set_date_kind(DateKind::Calendar));
        assert_roundtrip!(Config::DEFAULT.set_date_kind(DateKind::Week));
        assert_roundtrip!(Config::DEFAULT.set_date_kind(DateKind::Ordinal));
        assert_roundtrip!(Config::DEFAULT.set_time_precision(TimePrecision::Hour {
            decimal_digits: None,
        }));
        assert_roundtrip!(Config::DEFAULT.set_time_precision(TimePrecision::Minute {
            decimal_digits: None,
        }));
        assert_roundtrip!(Config::DEFAULT.set_time_precision(TimePrecision::Second {
            decimal_digits: None,
        }));
        assert_roundtrip!(Config::DEFAULT.set_time_precision(TimePrecision::Hour {
            decimal_digits: NonZero::new(1),
        }));
        assert_roundtrip!(Config::DEFAULT.set_time_precision(TimePrecision::Minute {
            decimal_digits: NonZero::new(1),
        }));
        assert_roundtrip!(Config::DEFAULT.set_time_precision(TimePrecision::Second {
            decimal_digits: NonZero::new(1),
        }));
        assert_roundtrip!(Config::DEFAULT.set_offset_precision(OffsetPrecision::Hour));
        assert_roundtrip!(Config::DEFAULT.set_offset_precision(OffsetPrecision::Minute));
    }

    macro_rules! assert_decode_fail {
        ($encoding:expr) => {
            assert!(std::panic::catch_unwind(|| {
                Config::decode($encoding);
            })
            .is_err());
        };
    }

    #[test]
    fn decode_fail() {
        assert_decode_fail!(0x07_00_00_00_00_00_00_00_00_00_00_00_00_00_00_00);
        assert_decode_fail!(0x00_02_00_00_00_00_00_00_00_00_00_00_00_00_00_00);
        assert_decode_fail!(0x00_00_02_00_00_00_00_00_00_00_00_00_00_00_00_00);
        assert_decode_fail!(0x00_00_00_03_00_00_00_00_00_00_00_00_00_00_00_00);
        assert_decode_fail!(0x00_00_00_00_03_00_00_00_00_00_00_00_00_00_00_00);
        assert_decode_fail!(0x00_00_00_00_00_00_02_00_00_00_00_00_00_00_00_00);
        assert_decode_fail!(0x00_00_00_00_00_00_00_01_00_00_00_00_00_00_00_00);
    }
}
