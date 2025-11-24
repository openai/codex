//! Days of the week.

use core::fmt;
use core::str::FromStr;

use powerfmt::smart_display::{FormatterOptions, Metadata, SmartDisplay};

use self::Weekday::*;
use crate::error;

/// Days of the week.
///
/// As order is dependent on context (Sunday could be either two days after or five days before
/// Friday), this type does not implement `PartialOrd` or `Ord`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weekday {
    #[expect(missing_docs)]
    Monday,
    #[expect(missing_docs)]
    Tuesday,
    #[expect(missing_docs)]
    Wednesday,
    #[expect(missing_docs)]
    Thursday,
    #[expect(missing_docs)]
    Friday,
    #[expect(missing_docs)]
    Saturday,
    #[expect(missing_docs)]
    Sunday,
}

impl Weekday {
    /// Get the previous weekday.
    ///
    /// ```rust
    /// # use time::Weekday;
    /// assert_eq!(Weekday::Tuesday.previous(), Weekday::Monday);
    /// ```
    #[inline]
    pub const fn previous(self) -> Self {
        match self {
            Monday => Sunday,
            Tuesday => Monday,
            Wednesday => Tuesday,
            Thursday => Wednesday,
            Friday => Thursday,
            Saturday => Friday,
            Sunday => Saturday,
        }
    }

    /// Get the next weekday.
    ///
    /// ```rust
    /// # use time::Weekday;
    /// assert_eq!(Weekday::Monday.next(), Weekday::Tuesday);
    /// ```
    #[inline]
    pub const fn next(self) -> Self {
        match self {
            Monday => Tuesday,
            Tuesday => Wednesday,
            Wednesday => Thursday,
            Thursday => Friday,
            Friday => Saturday,
            Saturday => Sunday,
            Sunday => Monday,
        }
    }

    /// Get n-th next day.
    ///
    /// ```rust
    /// # use time::Weekday;
    /// assert_eq!(Weekday::Monday.nth_next(1), Weekday::Tuesday);
    /// assert_eq!(Weekday::Sunday.nth_next(10), Weekday::Wednesday);
    /// ```
    #[inline]
    pub const fn nth_next(self, n: u8) -> Self {
        match (self.number_days_from_monday() + n % 7) % 7 {
            0 => Monday,
            1 => Tuesday,
            2 => Wednesday,
            3 => Thursday,
            4 => Friday,
            5 => Saturday,
            val => {
                debug_assert!(val == 6);
                Sunday
            }
        }
    }

    /// Get n-th previous day.
    ///
    /// ```rust
    /// # use time::Weekday;
    /// assert_eq!(Weekday::Monday.nth_prev(1), Weekday::Sunday);
    /// assert_eq!(Weekday::Sunday.nth_prev(10), Weekday::Thursday);
    /// ```
    #[inline]
    pub const fn nth_prev(self, n: u8) -> Self {
        match self.number_days_from_monday() as i8 - (n % 7) as i8 {
            1 | -6 => Tuesday,
            2 | -5 => Wednesday,
            3 | -4 => Thursday,
            4 | -3 => Friday,
            5 | -2 => Saturday,
            6 | -1 => Sunday,
            val => {
                debug_assert!(val == 0);
                Monday
            }
        }
    }

    /// Get the one-indexed number of days from Monday.
    ///
    /// ```rust
    /// # use time::Weekday;
    /// assert_eq!(Weekday::Monday.number_from_monday(), 1);
    /// ```
    #[doc(alias = "iso_weekday_number")]
    #[inline]
    pub const fn number_from_monday(self) -> u8 {
        self.number_days_from_monday() + 1
    }

    /// Get the one-indexed number of days from Sunday.
    ///
    /// ```rust
    /// # use time::Weekday;
    /// assert_eq!(Weekday::Monday.number_from_sunday(), 2);
    /// ```
    #[inline]
    pub const fn number_from_sunday(self) -> u8 {
        self.number_days_from_sunday() + 1
    }

    /// Get the zero-indexed number of days from Monday.
    ///
    /// ```rust
    /// # use time::Weekday;
    /// assert_eq!(Weekday::Monday.number_days_from_monday(), 0);
    /// ```
    #[inline]
    pub const fn number_days_from_monday(self) -> u8 {
        self as u8
    }

    /// Get the zero-indexed number of days from Sunday.
    ///
    /// ```rust
    /// # use time::Weekday;
    /// assert_eq!(Weekday::Monday.number_days_from_sunday(), 1);
    /// ```
    #[inline]
    pub const fn number_days_from_sunday(self) -> u8 {
        match self {
            Monday => 1,
            Tuesday => 2,
            Wednesday => 3,
            Thursday => 4,
            Friday => 5,
            Saturday => 6,
            Sunday => 0,
        }
    }
}

mod private {
    #[non_exhaustive]
    #[derive(Debug, Clone, Copy)]
    pub struct WeekdayMetadata;
}
use private::WeekdayMetadata;

impl SmartDisplay for Weekday {
    type Metadata = WeekdayMetadata;

    #[inline]
    fn metadata(&self, _: FormatterOptions) -> Metadata<'_, Self> {
        match self {
            Monday => Metadata::new(6, self, WeekdayMetadata),
            Tuesday => Metadata::new(7, self, WeekdayMetadata),
            Wednesday => Metadata::new(9, self, WeekdayMetadata),
            Thursday => Metadata::new(8, self, WeekdayMetadata),
            Friday => Metadata::new(6, self, WeekdayMetadata),
            Saturday => Metadata::new(8, self, WeekdayMetadata),
            Sunday => Metadata::new(6, self, WeekdayMetadata),
        }
    }

    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(match self {
            Monday => "Monday",
            Tuesday => "Tuesday",
            Wednesday => "Wednesday",
            Thursday => "Thursday",
            Friday => "Friday",
            Saturday => "Saturday",
            Sunday => "Sunday",
        })
    }
}

impl fmt::Display for Weekday {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        SmartDisplay::fmt(self, f)
    }
}

impl FromStr for Weekday {
    type Err = error::InvalidVariant;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Monday" => Ok(Monday),
            "Tuesday" => Ok(Tuesday),
            "Wednesday" => Ok(Wednesday),
            "Thursday" => Ok(Thursday),
            "Friday" => Ok(Friday),
            "Saturday" => Ok(Saturday),
            "Sunday" => Ok(Sunday),
            _ => Err(error::InvalidVariant),
        }
    }
}
