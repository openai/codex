// This is a part of Chrono.
// See README.md and LICENSE.txt for details.

//! ISO 8601 week.

use core::fmt;

use super::internals::YearFlags;

#[cfg(any(feature = "rkyv", feature = "rkyv-16", feature = "rkyv-32", feature = "rkyv-64"))]
use rkyv::{Archive, Deserialize, Serialize};

/// ISO 8601 week.
///
/// This type, combined with [`Weekday`](../enum.Weekday.html),
/// constitutes the ISO 8601 [week date](./struct.NaiveDate.html#week-date).
/// One can retrieve this type from the existing [`Datelike`](../trait.Datelike.html) types
/// via the [`Datelike::iso_week`](../trait.Datelike.html#tymethod.iso_week) method.
#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash)]
#[cfg_attr(
    any(feature = "rkyv", feature = "rkyv-16", feature = "rkyv-32", feature = "rkyv-64"),
    derive(Archive, Deserialize, Serialize),
    archive(compare(PartialEq, PartialOrd)),
    archive_attr(derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash))
)]
#[cfg_attr(feature = "rkyv-validation", archive(check_bytes))]
pub struct IsoWeek {
    // Note that this allows for larger year range than `NaiveDate`.
    // This is crucial because we have an edge case for the first and last week supported,
    // which year number might not match the calendar year number.
    ywf: i32, // (year << 10) | (week << 4) | flag
}

impl IsoWeek {
    /// Returns the corresponding `IsoWeek` from the year and the `Of` internal value.
    //
    // Internal use only. We don't expose the public constructor for `IsoWeek` for now
    // because the year range for the week date and the calendar date do not match, and
    // it is confusing to have a date that is out of range in one and not in another.
    // Currently we sidestep this issue by making `IsoWeek` fully dependent of `Datelike`.
    pub(super) fn from_yof(year: i32, ordinal: u32, year_flags: YearFlags) -> Self {
        let rawweek = (ordinal + year_flags.isoweek_delta()) / 7;
        let (year, week) = if rawweek < 1 {
            // previous year
            let prevlastweek = YearFlags::from_year(year - 1).nisoweeks();
            (year - 1, prevlastweek)
        } else {
            let lastweek = year_flags.nisoweeks();
            if rawweek > lastweek {
                // next year
                (year + 1, 1)
            } else {
                (year, rawweek)
            }
        };
        let flags = YearFlags::from_year(year);
        IsoWeek { ywf: (year << 10) | (week << 4) as i32 | i32::from(flags.0) }
    }

    /// Returns the year number for this ISO week.
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate, Weekday};
    ///
    /// let d = NaiveDate::from_isoywd_opt(2015, 1, Weekday::Mon).unwrap();
    /// assert_eq!(d.iso_week().year(), 2015);
    /// ```
    ///
    /// This year number might not match the calendar year number.
    /// Continuing the example...
    ///
    /// ```
    /// # use chrono::{NaiveDate, Datelike, Weekday};
    /// # let d = NaiveDate::from_isoywd_opt(2015, 1, Weekday::Mon).unwrap();
    /// assert_eq!(d.year(), 2014);
    /// assert_eq!(d, NaiveDate::from_ymd_opt(2014, 12, 29).unwrap());
    /// ```
    #[inline]
    pub const fn year(&self) -> i32 {
        self.ywf >> 10
    }

    /// Returns the ISO week number starting from 1.
    ///
    /// The return value ranges from 1 to 53. (The last week of year differs by years.)
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate, Weekday};
    ///
    /// let d = NaiveDate::from_isoywd_opt(2015, 15, Weekday::Mon).unwrap();
    /// assert_eq!(d.iso_week().week(), 15);
    /// ```
    #[inline]
    pub const fn week(&self) -> u32 {
        ((self.ywf >> 4) & 0x3f) as u32
    }

    /// Returns the ISO week number starting from 0.
    ///
    /// The return value ranges from 0 to 52. (The last week of year differs by years.)
    ///
    /// # Example
    ///
    /// ```
    /// use chrono::{Datelike, NaiveDate, Weekday};
    ///
    /// let d = NaiveDate::from_isoywd_opt(2015, 15, Weekday::Mon).unwrap();
    /// assert_eq!(d.iso_week().week0(), 14);
    /// ```
    #[inline]
    pub const fn week0(&self) -> u32 {
        ((self.ywf >> 4) & 0x3f) as u32 - 1
    }
}

/// The `Debug` output of the ISO week `w` is the same as
/// [`d.format("%G-W%V")`](../format/strftime/index.html)
/// where `d` is any `NaiveDate` value in that week.
///
/// # Example
///
/// ```
/// use chrono::{Datelike, NaiveDate};
///
/// assert_eq!(
///     format!("{:?}", NaiveDate::from_ymd_opt(2015, 9, 5).unwrap().iso_week()),
///     "2015-W36"
/// );
/// assert_eq!(format!("{:?}", NaiveDate::from_ymd_opt(0, 1, 3).unwrap().iso_week()), "0000-W01");
/// assert_eq!(
///     format!("{:?}", NaiveDate::from_ymd_opt(9999, 12, 31).unwrap().iso_week()),
///     "9999-W52"
/// );
/// ```
///
/// ISO 8601 requires an explicit sign for years before 1 BCE or after 9999 CE.
///
/// ```
/// # use chrono::{NaiveDate, Datelike};
/// assert_eq!(format!("{:?}", NaiveDate::from_ymd_opt(0, 1, 2).unwrap().iso_week()), "-0001-W52");
/// assert_eq!(
///     format!("{:?}", NaiveDate::from_ymd_opt(10000, 12, 31).unwrap().iso_week()),
///     "+10000-W52"
/// );
/// ```
impl fmt::Debug for IsoWeek {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let year = self.year();
        let week = self.week();
        if (0..=9999).contains(&year) {
            write!(f, "{year:04}-W{week:02}")
        } else {
            // ISO 8601 requires the explicit sign for out-of-range years
            write!(f, "{year:+05}-W{week:02}")
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "rkyv-validation")]
    use super::IsoWeek;
    use crate::Datelike;
    use crate::naive::date::{self, NaiveDate};

    #[test]
    fn test_iso_week_extremes() {
        let minweek = NaiveDate::MIN.iso_week();
        let maxweek = NaiveDate::MAX.iso_week();

        assert_eq!(minweek.year(), date::MIN_YEAR);
        assert_eq!(minweek.week(), 1);
        assert_eq!(minweek.week0(), 0);
        #[cfg(feature = "alloc")]
        assert_eq!(format!("{minweek:?}"), NaiveDate::MIN.format("%G-W%V").to_string());

        assert_eq!(maxweek.year(), date::MAX_YEAR + 1);
        assert_eq!(maxweek.week(), 1);
        assert_eq!(maxweek.week0(), 0);
        #[cfg(feature = "alloc")]
        assert_eq!(format!("{maxweek:?}"), NaiveDate::MAX.format("%G-W%V").to_string());
    }

    #[test]
    fn test_iso_week_equivalence_for_first_week() {
        let monday = NaiveDate::from_ymd_opt(2024, 12, 30).unwrap();
        let friday = NaiveDate::from_ymd_opt(2025, 1, 3).unwrap();

        assert_eq!(monday.iso_week(), friday.iso_week());
    }

    #[test]
    fn test_iso_week_equivalence_for_last_week() {
        let monday = NaiveDate::from_ymd_opt(2026, 12, 28).unwrap();
        let friday = NaiveDate::from_ymd_opt(2027, 1, 1).unwrap();

        assert_eq!(monday.iso_week(), friday.iso_week());
    }

    #[test]
    fn test_iso_week_ordering_for_first_week() {
        let monday = NaiveDate::from_ymd_opt(2024, 12, 30).unwrap();
        let friday = NaiveDate::from_ymd_opt(2025, 1, 3).unwrap();

        assert!(monday.iso_week() >= friday.iso_week());
        assert!(monday.iso_week() <= friday.iso_week());
    }

    #[test]
    fn test_iso_week_ordering_for_last_week() {
        let monday = NaiveDate::from_ymd_opt(2026, 12, 28).unwrap();
        let friday = NaiveDate::from_ymd_opt(2027, 1, 1).unwrap();

        assert!(monday.iso_week() >= friday.iso_week());
        assert!(monday.iso_week() <= friday.iso_week());
    }

    #[test]
    #[cfg(feature = "rkyv-validation")]
    fn test_rkyv_validation() {
        let minweek = NaiveDate::MIN.iso_week();
        let bytes = rkyv::to_bytes::<_, 4>(&minweek).unwrap();
        assert_eq!(rkyv::from_bytes::<IsoWeek>(&bytes).unwrap(), minweek);

        let maxweek = NaiveDate::MAX.iso_week();
        let bytes = rkyv::to_bytes::<_, 4>(&maxweek).unwrap();
        assert_eq!(rkyv::from_bytes::<IsoWeek>(&bytes).unwrap(), maxweek);
    }
}
