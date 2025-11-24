//! Internal helper types for working with dates.

#![cfg_attr(feature = "__internal_bench", allow(missing_docs))]

use core::fmt;

/// Year flags (aka the dominical letter).
///
/// `YearFlags` are used as the last four bits of `NaiveDate`, `Mdf` and `IsoWeek`.
///
/// There are 14 possible classes of year in the Gregorian calendar:
/// common and leap years starting with Monday through Sunday.
///
/// The `YearFlags` stores this information into 4 bits `LWWW`. `L` is the leap year flag, with `1`
/// for the common year (this simplifies validating an ordinal in `NaiveDate`). `WWW` is a non-zero
/// `Weekday` of the last day in the preceding year.
#[allow(unreachable_pub)] // public as an alias for benchmarks only
#[derive(PartialEq, Eq, Copy, Clone, Hash)]
pub struct YearFlags(pub(super) u8);

// Weekday of the last day in the preceding year.
// Allows for quick day of week calculation from the 1-based ordinal.
const YEAR_STARTS_AFTER_MONDAY: u8 = 7; // non-zero to allow use with `NonZero*`.
const YEAR_STARTS_AFTER_THUESDAY: u8 = 1;
const YEAR_STARTS_AFTER_WEDNESDAY: u8 = 2;
const YEAR_STARTS_AFTER_THURSDAY: u8 = 3;
const YEAR_STARTS_AFTER_FRIDAY: u8 = 4;
const YEAR_STARTS_AFTER_SATURDAY: u8 = 5;
const YEAR_STARTS_AFTER_SUNDAY: u8 = 6;

const COMMON_YEAR: u8 = 1 << 3;
const LEAP_YEAR: u8 = 0 << 3;

pub(super) const A: YearFlags = YearFlags(COMMON_YEAR | YEAR_STARTS_AFTER_SATURDAY);
pub(super) const AG: YearFlags = YearFlags(LEAP_YEAR | YEAR_STARTS_AFTER_SATURDAY);
pub(super) const B: YearFlags = YearFlags(COMMON_YEAR | YEAR_STARTS_AFTER_FRIDAY);
pub(super) const BA: YearFlags = YearFlags(LEAP_YEAR | YEAR_STARTS_AFTER_FRIDAY);
pub(super) const C: YearFlags = YearFlags(COMMON_YEAR | YEAR_STARTS_AFTER_THURSDAY);
pub(super) const CB: YearFlags = YearFlags(LEAP_YEAR | YEAR_STARTS_AFTER_THURSDAY);
pub(super) const D: YearFlags = YearFlags(COMMON_YEAR | YEAR_STARTS_AFTER_WEDNESDAY);
pub(super) const DC: YearFlags = YearFlags(LEAP_YEAR | YEAR_STARTS_AFTER_WEDNESDAY);
pub(super) const E: YearFlags = YearFlags(COMMON_YEAR | YEAR_STARTS_AFTER_THUESDAY);
pub(super) const ED: YearFlags = YearFlags(LEAP_YEAR | YEAR_STARTS_AFTER_THUESDAY);
pub(super) const F: YearFlags = YearFlags(COMMON_YEAR | YEAR_STARTS_AFTER_MONDAY);
pub(super) const FE: YearFlags = YearFlags(LEAP_YEAR | YEAR_STARTS_AFTER_MONDAY);
pub(super) const G: YearFlags = YearFlags(COMMON_YEAR | YEAR_STARTS_AFTER_SUNDAY);
pub(super) const GF: YearFlags = YearFlags(LEAP_YEAR | YEAR_STARTS_AFTER_SUNDAY);

const YEAR_TO_FLAGS: &[YearFlags; 400] = &[
    BA, G, F, E, DC, B, A, G, FE, D, C, B, AG, F, E, D, CB, A, G, F, ED, C, B, A, GF, E, D, C, BA,
    G, F, E, DC, B, A, G, FE, D, C, B, AG, F, E, D, CB, A, G, F, ED, C, B, A, GF, E, D, C, BA, G,
    F, E, DC, B, A, G, FE, D, C, B, AG, F, E, D, CB, A, G, F, ED, C, B, A, GF, E, D, C, BA, G, F,
    E, DC, B, A, G, FE, D, C, B, AG, F, E, D, // 100
    C, B, A, G, FE, D, C, B, AG, F, E, D, CB, A, G, F, ED, C, B, A, GF, E, D, C, BA, G, F, E, DC,
    B, A, G, FE, D, C, B, AG, F, E, D, CB, A, G, F, ED, C, B, A, GF, E, D, C, BA, G, F, E, DC, B,
    A, G, FE, D, C, B, AG, F, E, D, CB, A, G, F, ED, C, B, A, GF, E, D, C, BA, G, F, E, DC, B, A,
    G, FE, D, C, B, AG, F, E, D, CB, A, G, F, // 200
    E, D, C, B, AG, F, E, D, CB, A, G, F, ED, C, B, A, GF, E, D, C, BA, G, F, E, DC, B, A, G, FE,
    D, C, B, AG, F, E, D, CB, A, G, F, ED, C, B, A, GF, E, D, C, BA, G, F, E, DC, B, A, G, FE, D,
    C, B, AG, F, E, D, CB, A, G, F, ED, C, B, A, GF, E, D, C, BA, G, F, E, DC, B, A, G, FE, D, C,
    B, AG, F, E, D, CB, A, G, F, ED, C, B, A, // 300
    G, F, E, D, CB, A, G, F, ED, C, B, A, GF, E, D, C, BA, G, F, E, DC, B, A, G, FE, D, C, B, AG,
    F, E, D, CB, A, G, F, ED, C, B, A, GF, E, D, C, BA, G, F, E, DC, B, A, G, FE, D, C, B, AG, F,
    E, D, CB, A, G, F, ED, C, B, A, GF, E, D, C, BA, G, F, E, DC, B, A, G, FE, D, C, B, AG, F, E,
    D, CB, A, G, F, ED, C, B, A, GF, E, D, C, // 400
];

impl YearFlags {
    #[allow(unreachable_pub)] // public as an alias for benchmarks only
    #[doc(hidden)] // for benchmarks only
    #[inline]
    #[must_use]
    pub const fn from_year(year: i32) -> YearFlags {
        let year = year.rem_euclid(400);
        YearFlags::from_year_mod_400(year)
    }

    #[inline]
    pub(super) const fn from_year_mod_400(year: i32) -> YearFlags {
        YEAR_TO_FLAGS[year as usize]
    }

    #[inline]
    pub(super) const fn ndays(&self) -> u32 {
        let YearFlags(flags) = *self;
        366 - (flags >> 3) as u32
    }

    #[inline]
    pub(super) const fn isoweek_delta(&self) -> u32 {
        let YearFlags(flags) = *self;
        let mut delta = (flags & 0b0111) as u32;
        if delta < 3 {
            delta += 7;
        }
        delta
    }

    #[inline]
    pub(super) const fn nisoweeks(&self) -> u32 {
        let YearFlags(flags) = *self;
        52 + ((0b0000_0100_0000_0110 >> flags as usize) & 1)
    }
}

impl fmt::Debug for YearFlags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let YearFlags(flags) = *self;
        match flags {
            0o15 => "A".fmt(f),
            0o05 => "AG".fmt(f),
            0o14 => "B".fmt(f),
            0o04 => "BA".fmt(f),
            0o13 => "C".fmt(f),
            0o03 => "CB".fmt(f),
            0o12 => "D".fmt(f),
            0o02 => "DC".fmt(f),
            0o11 => "E".fmt(f),
            0o01 => "ED".fmt(f),
            0o10 => "F?".fmt(f),
            0o00 => "FE?".fmt(f), // non-canonical
            0o17 => "F".fmt(f),
            0o07 => "FE".fmt(f),
            0o16 => "G".fmt(f),
            0o06 => "GF".fmt(f),
            _ => write!(f, "YearFlags({flags})"),
        }
    }
}

// OL: (ordinal << 1) | leap year flag
const MAX_OL: u32 = 366 << 1; // `(366 << 1) | 1` would be day 366 in a non-leap year
const MAX_MDL: u32 = (12 << 6) | (31 << 1) | 1;

// The next table are adjustment values to convert a date encoded as month-day-leapyear to
// ordinal-leapyear. OL = MDL - adjustment.
// Dates that do not exist are encoded as `XX`.
const XX: i8 = 0;
const MDL_TO_OL: &[i8; MAX_MDL as usize + 1] = &[
    XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX,
    XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX,
    XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, // 0
    XX, XX, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64,
    64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64,
    64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, // 1
    XX, XX, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66,
    66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66,
    66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, XX, XX, XX, XX, XX, // 2
    XX, XX, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74,
    72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74,
    72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, // 3
    XX, XX, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76,
    74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76,
    74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, XX, XX, // 4
    XX, XX, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80,
    78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80,
    78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, // 5
    XX, XX, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82,
    80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82,
    80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, XX, XX, // 6
    XX, XX, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86,
    84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86,
    84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, // 7
    XX, XX, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88,
    86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88,
    86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, // 8
    XX, XX, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90,
    88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90,
    88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, XX, XX, // 9
    XX, XX, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94,
    92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94,
    92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, // 10
    XX, XX, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96,
    94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96,
    94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, XX, XX, // 11
    XX, XX, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98,
    100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100,
    98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98,
    100, // 12
];

const OL_TO_MDL: &[u8; MAX_OL as usize + 1] = &[
    0, 0, // 0
    64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64,
    64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64,
    64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, // 1
    66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66,
    66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66,
    66, 66, 66, 66, 66, 66, 66, 66, 66, // 2
    74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72,
    74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72,
    74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, 74, 72, // 3
    76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74,
    76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74,
    76, 74, 76, 74, 76, 74, 76, 74, 76, 74, 76, 74, // 4
    80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78,
    80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78,
    80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, 80, 78, // 5
    82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80,
    82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80,
    82, 80, 82, 80, 82, 80, 82, 80, 82, 80, 82, 80, // 6
    86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84,
    86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84,
    86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, 86, 84, // 7
    88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86,
    88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86,
    88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, 88, 86, // 8
    90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88,
    90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88,
    90, 88, 90, 88, 90, 88, 90, 88, 90, 88, 90, 88, // 9
    94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92,
    94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92,
    94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, 94, 92, // 10
    96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94,
    96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94,
    96, 94, 96, 94, 96, 94, 96, 94, 96, 94, 96, 94, // 11
    100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100,
    98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98,
    100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100, 98, 100,
    98, // 12
];

/// Month, day of month and year flags: `(month << 9) | (day << 4) | flags`
/// `M_MMMD_DDDD_LFFF`
///
/// The whole bits except for the least 3 bits are referred as `Mdl` (month, day of month, and leap
/// year flag), which is an index to the `MDL_TO_OL` lookup table.
///
/// The conversion between the packed calendar date (`Mdf`) and the ordinal date (`NaiveDate`) is
/// based on the moderately-sized lookup table (~1.5KB) and the packed representation is chosen for
/// efficient lookup.
///
/// The methods of `Mdf` validate their inputs as late as possible. Dates that can't exist, like
/// February 30, can still be represented. This allows the validation to be combined with the final
/// table lookup, which is good for performance.
#[derive(PartialEq, PartialOrd, Copy, Clone)]
pub(super) struct Mdf(u32);

impl Mdf {
    /// Makes a new `Mdf` value from month, day and `YearFlags`.
    ///
    /// This method doesn't fully validate the range of the `month` and `day` parameters, only as
    /// much as what can't be deferred until later. The year `flags` are trusted to be correct.
    ///
    /// # Errors
    ///
    /// Returns `None` if `month > 12` or `day > 31`.
    #[inline]
    pub(super) const fn new(month: u32, day: u32, YearFlags(flags): YearFlags) -> Option<Mdf> {
        match month <= 12 && day <= 31 {
            true => Some(Mdf((month << 9) | (day << 4) | flags as u32)),
            false => None,
        }
    }

    /// Makes a new `Mdf` value from an `i32` with an ordinal and a leap year flag, and year
    /// `flags`.
    ///
    /// The `ol` is trusted to be valid, and the `flags` are trusted to match it.
    #[inline]
    pub(super) const fn from_ol(ol: i32, YearFlags(flags): YearFlags) -> Mdf {
        debug_assert!(ol > 1 && ol <= MAX_OL as i32);
        // Array is indexed from `[2..=MAX_OL]`, with a `0` index having a meaningless value.
        Mdf(((ol as u32 + OL_TO_MDL[ol as usize] as u32) << 3) | flags as u32)
    }

    /// Returns the month of this `Mdf`.
    #[inline]
    pub(super) const fn month(&self) -> u32 {
        let Mdf(mdf) = *self;
        mdf >> 9
    }

    /// Replaces the month of this `Mdf`, keeping the day and flags.
    ///
    /// # Errors
    ///
    /// Returns `None` if `month > 12`.
    #[inline]
    pub(super) const fn with_month(&self, month: u32) -> Option<Mdf> {
        if month > 12 {
            return None;
        }

        let Mdf(mdf) = *self;
        Some(Mdf((mdf & 0b1_1111_1111) | (month << 9)))
    }

    /// Returns the day of this `Mdf`.
    #[inline]
    pub(super) const fn day(&self) -> u32 {
        let Mdf(mdf) = *self;
        (mdf >> 4) & 0b1_1111
    }

    /// Replaces the day of this `Mdf`, keeping the month and flags.
    ///
    /// # Errors
    ///
    /// Returns `None` if `day > 31`.
    #[inline]
    pub(super) const fn with_day(&self, day: u32) -> Option<Mdf> {
        if day > 31 {
            return None;
        }

        let Mdf(mdf) = *self;
        Some(Mdf((mdf & !0b1_1111_0000) | (day << 4)))
    }

    /// Replaces the flags of this `Mdf`, keeping the month and day.
    #[inline]
    pub(super) const fn with_flags(&self, YearFlags(flags): YearFlags) -> Mdf {
        let Mdf(mdf) = *self;
        Mdf((mdf & !0b1111) | flags as u32)
    }

    /// Returns the ordinal that corresponds to this `Mdf`.
    ///
    /// This does a table lookup to calculate the corresponding ordinal. It will return an error if
    /// the `Mdl` turns out not to be a valid date.
    ///
    /// # Errors
    ///
    /// Returns `None` if `month == 0` or `day == 0`, or if a the given day does not exist in the
    /// given month.
    #[inline]
    pub(super) const fn ordinal(&self) -> Option<u32> {
        let mdl = self.0 >> 3;
        match MDL_TO_OL[mdl as usize] {
            XX => None,
            v => Some((mdl - v as u8 as u32) >> 1),
        }
    }

    /// Returns the year flags of this `Mdf`.
    #[inline]
    pub(super) const fn year_flags(&self) -> YearFlags {
        YearFlags((self.0 & 0b1111) as u8)
    }

    /// Returns the ordinal that corresponds to this `Mdf`, encoded as a value including year flags.
    ///
    /// This does a table lookup to calculate the corresponding ordinal. It will return an error if
    /// the `Mdl` turns out not to be a valid date.
    ///
    /// # Errors
    ///
    /// Returns `None` if `month == 0` or `day == 0`, or if a the given day does not exist in the
    /// given month.
    #[inline]
    pub(super) const fn ordinal_and_flags(&self) -> Option<i32> {
        let mdl = self.0 >> 3;
        match MDL_TO_OL[mdl as usize] {
            XX => None,
            v => Some(self.0 as i32 - ((v as i32) << 3)),
        }
    }

    #[cfg(test)]
    fn valid(&self) -> bool {
        let mdl = self.0 >> 3;
        MDL_TO_OL[mdl as usize] > 0
    }
}

impl fmt::Debug for Mdf {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Mdf(mdf) = *self;
        write!(
            f,
            "Mdf(({} << 9) | ({} << 4) | {:#04o} /*{:?}*/)",
            mdf >> 9,
            (mdf >> 4) & 0b1_1111,
            mdf & 0b1111,
            YearFlags((mdf & 0b1111) as u8)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Mdf;
    use super::{A, AG, B, BA, C, CB, D, DC, E, ED, F, FE, G, GF, YearFlags};

    const NONLEAP_FLAGS: [YearFlags; 7] = [A, B, C, D, E, F, G];
    const LEAP_FLAGS: [YearFlags; 7] = [AG, BA, CB, DC, ED, FE, GF];
    const FLAGS: [YearFlags; 14] = [A, B, C, D, E, F, G, AG, BA, CB, DC, ED, FE, GF];

    #[test]
    fn test_year_flags_ndays_from_year() {
        assert_eq!(YearFlags::from_year(2014).ndays(), 365);
        assert_eq!(YearFlags::from_year(2012).ndays(), 366);
        assert_eq!(YearFlags::from_year(2000).ndays(), 366);
        assert_eq!(YearFlags::from_year(1900).ndays(), 365);
        assert_eq!(YearFlags::from_year(1600).ndays(), 366);
        assert_eq!(YearFlags::from_year(1).ndays(), 365);
        assert_eq!(YearFlags::from_year(0).ndays(), 366); // 1 BCE (proleptic Gregorian)
        assert_eq!(YearFlags::from_year(-1).ndays(), 365); // 2 BCE
        assert_eq!(YearFlags::from_year(-4).ndays(), 366); // 5 BCE
        assert_eq!(YearFlags::from_year(-99).ndays(), 365); // 100 BCE
        assert_eq!(YearFlags::from_year(-100).ndays(), 365); // 101 BCE
        assert_eq!(YearFlags::from_year(-399).ndays(), 365); // 400 BCE
        assert_eq!(YearFlags::from_year(-400).ndays(), 366); // 401 BCE
    }

    #[test]
    fn test_year_flags_nisoweeks() {
        assert_eq!(A.nisoweeks(), 52);
        assert_eq!(B.nisoweeks(), 52);
        assert_eq!(C.nisoweeks(), 52);
        assert_eq!(D.nisoweeks(), 53);
        assert_eq!(E.nisoweeks(), 52);
        assert_eq!(F.nisoweeks(), 52);
        assert_eq!(G.nisoweeks(), 52);
        assert_eq!(AG.nisoweeks(), 52);
        assert_eq!(BA.nisoweeks(), 52);
        assert_eq!(CB.nisoweeks(), 52);
        assert_eq!(DC.nisoweeks(), 53);
        assert_eq!(ED.nisoweeks(), 53);
        assert_eq!(FE.nisoweeks(), 52);
        assert_eq!(GF.nisoweeks(), 52);
    }

    #[test]
    fn test_mdf_valid() {
        fn check(expected: bool, flags: YearFlags, month1: u32, day1: u32, month2: u32, day2: u32) {
            for month in month1..=month2 {
                for day in day1..=day2 {
                    let mdf = match Mdf::new(month, day, flags) {
                        Some(mdf) => mdf,
                        None if !expected => continue,
                        None => panic!("Mdf::new({month}, {day}, {flags:?}) returned None"),
                    };

                    assert!(
                        mdf.valid() == expected,
                        "month {} day {} = {:?} should be {} for dominical year {:?}",
                        month,
                        day,
                        mdf,
                        if expected { "valid" } else { "invalid" },
                        flags
                    );
                }
            }
        }

        for &flags in NONLEAP_FLAGS.iter() {
            check(false, flags, 0, 0, 0, 1024);
            check(false, flags, 0, 0, 16, 0);
            check(true, flags, 1, 1, 1, 31);
            check(false, flags, 1, 32, 1, 1024);
            check(true, flags, 2, 1, 2, 28);
            check(false, flags, 2, 29, 2, 1024);
            check(true, flags, 3, 1, 3, 31);
            check(false, flags, 3, 32, 3, 1024);
            check(true, flags, 4, 1, 4, 30);
            check(false, flags, 4, 31, 4, 1024);
            check(true, flags, 5, 1, 5, 31);
            check(false, flags, 5, 32, 5, 1024);
            check(true, flags, 6, 1, 6, 30);
            check(false, flags, 6, 31, 6, 1024);
            check(true, flags, 7, 1, 7, 31);
            check(false, flags, 7, 32, 7, 1024);
            check(true, flags, 8, 1, 8, 31);
            check(false, flags, 8, 32, 8, 1024);
            check(true, flags, 9, 1, 9, 30);
            check(false, flags, 9, 31, 9, 1024);
            check(true, flags, 10, 1, 10, 31);
            check(false, flags, 10, 32, 10, 1024);
            check(true, flags, 11, 1, 11, 30);
            check(false, flags, 11, 31, 11, 1024);
            check(true, flags, 12, 1, 12, 31);
            check(false, flags, 12, 32, 12, 1024);
            check(false, flags, 13, 0, 16, 1024);
            check(false, flags, u32::MAX, 0, u32::MAX, 1024);
            check(false, flags, 0, u32::MAX, 16, u32::MAX);
            check(false, flags, u32::MAX, u32::MAX, u32::MAX, u32::MAX);
        }

        for &flags in LEAP_FLAGS.iter() {
            check(false, flags, 0, 0, 0, 1024);
            check(false, flags, 0, 0, 16, 0);
            check(true, flags, 1, 1, 1, 31);
            check(false, flags, 1, 32, 1, 1024);
            check(true, flags, 2, 1, 2, 29);
            check(false, flags, 2, 30, 2, 1024);
            check(true, flags, 3, 1, 3, 31);
            check(false, flags, 3, 32, 3, 1024);
            check(true, flags, 4, 1, 4, 30);
            check(false, flags, 4, 31, 4, 1024);
            check(true, flags, 5, 1, 5, 31);
            check(false, flags, 5, 32, 5, 1024);
            check(true, flags, 6, 1, 6, 30);
            check(false, flags, 6, 31, 6, 1024);
            check(true, flags, 7, 1, 7, 31);
            check(false, flags, 7, 32, 7, 1024);
            check(true, flags, 8, 1, 8, 31);
            check(false, flags, 8, 32, 8, 1024);
            check(true, flags, 9, 1, 9, 30);
            check(false, flags, 9, 31, 9, 1024);
            check(true, flags, 10, 1, 10, 31);
            check(false, flags, 10, 32, 10, 1024);
            check(true, flags, 11, 1, 11, 30);
            check(false, flags, 11, 31, 11, 1024);
            check(true, flags, 12, 1, 12, 31);
            check(false, flags, 12, 32, 12, 1024);
            check(false, flags, 13, 0, 16, 1024);
            check(false, flags, u32::MAX, 0, u32::MAX, 1024);
            check(false, flags, 0, u32::MAX, 16, u32::MAX);
            check(false, flags, u32::MAX, u32::MAX, u32::MAX, u32::MAX);
        }
    }

    #[test]
    fn test_mdf_fields() {
        for &flags in FLAGS.iter() {
            for month in 1u32..=12 {
                for day in 1u32..31 {
                    let mdf = match Mdf::new(month, day, flags) {
                        Some(mdf) => mdf,
                        None => continue,
                    };

                    if mdf.valid() {
                        assert_eq!(mdf.month(), month);
                        assert_eq!(mdf.day(), day);
                    }
                }
            }
        }
    }

    #[test]
    fn test_mdf_with_fields() {
        fn check(flags: YearFlags, month: u32, day: u32) {
            let mdf = Mdf::new(month, day, flags).unwrap();

            for month in 0u32..=16 {
                let mdf = match mdf.with_month(month) {
                    Some(mdf) => mdf,
                    None if month > 12 => continue,
                    None => panic!("failed to create Mdf with month {month}"),
                };

                if mdf.valid() {
                    assert_eq!(mdf.month(), month);
                    assert_eq!(mdf.day(), day);
                }
            }

            for day in 0u32..=1024 {
                let mdf = match mdf.with_day(day) {
                    Some(mdf) => mdf,
                    None if day > 31 => continue,
                    None => panic!("failed to create Mdf with month {month}"),
                };

                if mdf.valid() {
                    assert_eq!(mdf.month(), month);
                    assert_eq!(mdf.day(), day);
                }
            }
        }

        for &flags in NONLEAP_FLAGS.iter() {
            check(flags, 1, 1);
            check(flags, 1, 31);
            check(flags, 2, 1);
            check(flags, 2, 28);
            check(flags, 2, 29);
            check(flags, 12, 31);
        }
        for &flags in LEAP_FLAGS.iter() {
            check(flags, 1, 1);
            check(flags, 1, 31);
            check(flags, 2, 1);
            check(flags, 2, 29);
            check(flags, 2, 30);
            check(flags, 12, 31);
        }
    }

    #[test]
    fn test_mdf_new_range() {
        let flags = YearFlags::from_year(2023);
        assert!(Mdf::new(13, 1, flags).is_none());
        assert!(Mdf::new(1, 32, flags).is_none());
    }
}
