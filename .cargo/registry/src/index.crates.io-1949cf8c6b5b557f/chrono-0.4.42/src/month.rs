use core::fmt;

#[cfg(any(feature = "rkyv", feature = "rkyv-16", feature = "rkyv-32", feature = "rkyv-64"))]
use rkyv::{Archive, Deserialize, Serialize};

use crate::OutOfRange;
use crate::naive::NaiveDate;

/// The month of the year.
///
/// This enum is just a convenience implementation.
/// The month in dates created by DateLike objects does not return this enum.
///
/// It is possible to convert from a date to a month independently
/// ```
/// use chrono::prelude::*;
/// let date = Utc.with_ymd_and_hms(2019, 10, 28, 9, 10, 11).unwrap();
/// // `2019-10-28T09:10:11Z`
/// let month = Month::try_from(u8::try_from(date.month()).unwrap()).ok();
/// assert_eq!(month, Some(Month::October))
/// ```
/// Or from a Month to an integer usable by dates
/// ```
/// # use chrono::prelude::*;
/// let month = Month::January;
/// let dt = Utc.with_ymd_and_hms(2019, month.number_from_month(), 28, 9, 10, 11).unwrap();
/// assert_eq!((dt.year(), dt.month(), dt.day()), (2019, 1, 28));
/// ```
/// Allows mapping from and to month, from 1-January to 12-December.
/// Can be Serialized/Deserialized with serde
// Actual implementation is zero-indexed, API intended as 1-indexed for more intuitive behavior.
#[derive(PartialEq, Eq, Copy, Clone, Debug, Hash, PartialOrd, Ord)]
#[cfg_attr(
    any(feature = "rkyv", feature = "rkyv-16", feature = "rkyv-32", feature = "rkyv-64"),
    derive(Archive, Deserialize, Serialize),
    archive(compare(PartialEq, PartialOrd)),
    archive_attr(derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash))
)]
#[cfg_attr(feature = "rkyv-validation", archive(check_bytes))]
#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(arbitrary::Arbitrary))]
pub enum Month {
    /// January
    January = 0,
    /// February
    February = 1,
    /// March
    March = 2,
    /// April
    April = 3,
    /// May
    May = 4,
    /// June
    June = 5,
    /// July
    July = 6,
    /// August
    August = 7,
    /// September
    September = 8,
    /// October
    October = 9,
    /// November
    November = 10,
    /// December
    December = 11,
}

impl Month {
    /// The next month.
    ///
    /// `m`:        | `January`  | `February` | `...` | `December`
    /// ----------- | ---------  | ---------- | --- | ---------
    /// `m.succ()`: | `February` | `March`    | `...` | `January`
    #[inline]
    #[must_use]
    pub const fn succ(&self) -> Month {
        match *self {
            Month::January => Month::February,
            Month::February => Month::March,
            Month::March => Month::April,
            Month::April => Month::May,
            Month::May => Month::June,
            Month::June => Month::July,
            Month::July => Month::August,
            Month::August => Month::September,
            Month::September => Month::October,
            Month::October => Month::November,
            Month::November => Month::December,
            Month::December => Month::January,
        }
    }

    /// The previous month.
    ///
    /// `m`:        | `January`  | `February` | `...` | `December`
    /// ----------- | ---------  | ---------- | --- | ---------
    /// `m.pred()`: | `December` | `January`  | `...` | `November`
    #[inline]
    #[must_use]
    pub const fn pred(&self) -> Month {
        match *self {
            Month::January => Month::December,
            Month::February => Month::January,
            Month::March => Month::February,
            Month::April => Month::March,
            Month::May => Month::April,
            Month::June => Month::May,
            Month::July => Month::June,
            Month::August => Month::July,
            Month::September => Month::August,
            Month::October => Month::September,
            Month::November => Month::October,
            Month::December => Month::November,
        }
    }

    /// Returns a month-of-year number starting from January = 1.
    ///
    /// `m`:                     | `January` | `February` | `...` | `December`
    /// -------------------------| --------- | ---------- | --- | -----
    /// `m.number_from_month()`: | 1         | 2          | `...` | 12
    #[inline]
    #[must_use]
    pub const fn number_from_month(&self) -> u32 {
        match *self {
            Month::January => 1,
            Month::February => 2,
            Month::March => 3,
            Month::April => 4,
            Month::May => 5,
            Month::June => 6,
            Month::July => 7,
            Month::August => 8,
            Month::September => 9,
            Month::October => 10,
            Month::November => 11,
            Month::December => 12,
        }
    }

    /// Get the name of the month
    ///
    /// ```
    /// use chrono::Month;
    ///
    /// assert_eq!(Month::January.name(), "January")
    /// ```
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match *self {
            Month::January => "January",
            Month::February => "February",
            Month::March => "March",
            Month::April => "April",
            Month::May => "May",
            Month::June => "June",
            Month::July => "July",
            Month::August => "August",
            Month::September => "September",
            Month::October => "October",
            Month::November => "November",
            Month::December => "December",
        }
    }

    /// Get the length in days of the month
    ///
    /// Yields `None` if `year` is out of range for `NaiveDate`.
    pub fn num_days(&self, year: i32) -> Option<u8> {
        Some(match *self {
            Month::January => 31,
            Month::February => match NaiveDate::from_ymd_opt(year, 2, 1)?.leap_year() {
                true => 29,
                false => 28,
            },
            Month::March => 31,
            Month::April => 30,
            Month::May => 31,
            Month::June => 30,
            Month::July => 31,
            Month::August => 31,
            Month::September => 30,
            Month::October => 31,
            Month::November => 30,
            Month::December => 31,
        })
    }
}

impl TryFrom<u8> for Month {
    type Error = OutOfRange;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Month::January),
            2 => Ok(Month::February),
            3 => Ok(Month::March),
            4 => Ok(Month::April),
            5 => Ok(Month::May),
            6 => Ok(Month::June),
            7 => Ok(Month::July),
            8 => Ok(Month::August),
            9 => Ok(Month::September),
            10 => Ok(Month::October),
            11 => Ok(Month::November),
            12 => Ok(Month::December),
            _ => Err(OutOfRange::new()),
        }
    }
}

impl num_traits::FromPrimitive for Month {
    /// Returns an `Option<Month>` from a i64, assuming a 1-index, January = 1.
    ///
    /// `Month::from_i64(n: i64)`: | `1`                  | `2`                   | ... | `12`
    /// ---------------------------| -------------------- | --------------------- | ... | -----
    /// ``:                        | Some(Month::January) | Some(Month::February) | ... | Some(Month::December)
    #[inline]
    fn from_u64(n: u64) -> Option<Month> {
        Self::from_u32(n as u32)
    }

    #[inline]
    fn from_i64(n: i64) -> Option<Month> {
        Self::from_u32(n as u32)
    }

    #[inline]
    fn from_u32(n: u32) -> Option<Month> {
        match n {
            1 => Some(Month::January),
            2 => Some(Month::February),
            3 => Some(Month::March),
            4 => Some(Month::April),
            5 => Some(Month::May),
            6 => Some(Month::June),
            7 => Some(Month::July),
            8 => Some(Month::August),
            9 => Some(Month::September),
            10 => Some(Month::October),
            11 => Some(Month::November),
            12 => Some(Month::December),
            _ => None,
        }
    }
}

/// A duration in calendar months
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[cfg_attr(all(feature = "arbitrary", feature = "std"), derive(arbitrary::Arbitrary))]
pub struct Months(pub(crate) u32);

impl Months {
    /// Construct a new `Months` from a number of months
    pub const fn new(num: u32) -> Self {
        Self(num)
    }

    /// Returns the total number of months in the `Months` instance.
    #[inline]
    pub const fn as_u32(&self) -> u32 {
        self.0
    }
}

/// An error resulting from reading `<Month>` value with `FromStr`.
#[derive(Clone, PartialEq, Eq)]
pub struct ParseMonthError {
    pub(crate) _dummy: (),
}

#[cfg(feature = "std")]
impl std::error::Error for ParseMonthError {}

#[cfg(all(not(feature = "std"), feature = "core-error"))]
impl core::error::Error for ParseMonthError {}

impl fmt::Display for ParseMonthError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ParseMonthError {{ .. }}")
    }
}

impl fmt::Debug for ParseMonthError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ParseMonthError {{ .. }}")
    }
}

#[cfg(feature = "serde")]
mod month_serde {
    use super::Month;
    use serde::{de, ser};

    use core::fmt;

    impl ser::Serialize for Month {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: ser::Serializer,
        {
            serializer.collect_str(self.name())
        }
    }

    struct MonthVisitor;

    impl de::Visitor<'_> for MonthVisitor {
        type Value = Month;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("Month")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            value.parse().map_err(|_| E::custom("short (3-letter) or full month names expected"))
        }
    }

    impl<'de> de::Deserialize<'de> for Month {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            deserializer.deserialize_str(MonthVisitor)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Month;
    use crate::{Datelike, Months, OutOfRange, TimeZone, Utc};

    #[test]
    fn test_month_enum_try_from() {
        assert_eq!(Month::try_from(1), Ok(Month::January));
        assert_eq!(Month::try_from(2), Ok(Month::February));
        assert_eq!(Month::try_from(12), Ok(Month::December));
        assert_eq!(Month::try_from(13), Err(OutOfRange::new()));

        let date = Utc.with_ymd_and_hms(2019, 10, 28, 9, 10, 11).unwrap();
        assert_eq!(Month::try_from(date.month() as u8), Ok(Month::October));

        let month = Month::January;
        let dt = Utc.with_ymd_and_hms(2019, month.number_from_month(), 28, 9, 10, 11).unwrap();
        assert_eq!((dt.year(), dt.month(), dt.day()), (2019, 1, 28));
    }

    #[test]
    fn test_month_enum_primitive_parse() {
        use num_traits::FromPrimitive;

        let jan_opt = Month::from_u32(1);
        let feb_opt = Month::from_u64(2);
        let dec_opt = Month::from_i64(12);
        let no_month = Month::from_u32(13);
        assert_eq!(jan_opt, Some(Month::January));
        assert_eq!(feb_opt, Some(Month::February));
        assert_eq!(dec_opt, Some(Month::December));
        assert_eq!(no_month, None);

        let date = Utc.with_ymd_and_hms(2019, 10, 28, 9, 10, 11).unwrap();
        assert_eq!(Month::from_u32(date.month()), Some(Month::October));

        let month = Month::January;
        let dt = Utc.with_ymd_and_hms(2019, month.number_from_month(), 28, 9, 10, 11).unwrap();
        assert_eq!((dt.year(), dt.month(), dt.day()), (2019, 1, 28));
    }

    #[test]
    fn test_month_enum_succ_pred() {
        assert_eq!(Month::January.succ(), Month::February);
        assert_eq!(Month::December.succ(), Month::January);
        assert_eq!(Month::January.pred(), Month::December);
        assert_eq!(Month::February.pred(), Month::January);
    }

    #[test]
    fn test_month_partial_ord() {
        assert!(Month::January <= Month::January);
        assert!(Month::January < Month::February);
        assert!(Month::January < Month::December);
        assert!(Month::July >= Month::May);
        assert!(Month::September > Month::March);
    }

    #[test]
    fn test_months_as_u32() {
        assert_eq!(Months::new(0).as_u32(), 0);
        assert_eq!(Months::new(1).as_u32(), 1);
        assert_eq!(Months::new(u32::MAX).as_u32(), u32::MAX);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_serialize() {
        use Month::*;
        use serde_json::to_string;

        let cases: Vec<(Month, &str)> = vec![
            (January, "\"January\""),
            (February, "\"February\""),
            (March, "\"March\""),
            (April, "\"April\""),
            (May, "\"May\""),
            (June, "\"June\""),
            (July, "\"July\""),
            (August, "\"August\""),
            (September, "\"September\""),
            (October, "\"October\""),
            (November, "\"November\""),
            (December, "\"December\""),
        ];

        for (month, expected_str) in cases {
            let string = to_string(&month).unwrap();
            assert_eq!(string, expected_str);
        }
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_deserialize() {
        use Month::*;
        use serde_json::from_str;

        let cases: Vec<(&str, Month)> = vec![
            ("\"january\"", January),
            ("\"jan\"", January),
            ("\"FeB\"", February),
            ("\"MAR\"", March),
            ("\"mar\"", March),
            ("\"april\"", April),
            ("\"may\"", May),
            ("\"june\"", June),
            ("\"JULY\"", July),
            ("\"august\"", August),
            ("\"september\"", September),
            ("\"October\"", October),
            ("\"November\"", November),
            ("\"DECEmbEr\"", December),
        ];

        for (string, expected_month) in cases {
            let month = from_str::<Month>(string).unwrap();
            assert_eq!(month, expected_month);
        }

        let errors: Vec<&str> =
            vec!["\"not a month\"", "\"ja\"", "\"Dece\"", "Dec", "\"Augustin\""];

        for string in errors {
            from_str::<Month>(string).unwrap_err();
        }
    }

    #[test]
    #[cfg(feature = "rkyv-validation")]
    fn test_rkyv_validation() {
        let month = Month::January;
        let bytes = rkyv::to_bytes::<_, 1>(&month).unwrap();
        assert_eq!(rkyv::from_bytes::<Month>(&bytes).unwrap(), month);
    }

    #[test]
    fn num_days() {
        assert_eq!(Month::January.num_days(2020), Some(31));
        assert_eq!(Month::February.num_days(2020), Some(29));
        assert_eq!(Month::February.num_days(2019), Some(28));
    }
}
