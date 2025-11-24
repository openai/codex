//! Utility functions.

use crate::hint;

/// Returns if the provided year is a leap year in the proleptic Gregorian calendar. Uses
/// [astronomical year numbering](https://en.wikipedia.org/wiki/Astronomical_year_numbering).
///
/// ```rust
/// # use time::util::is_leap_year;
/// assert!(!is_leap_year(1900));
/// assert!(is_leap_year(2000));
/// assert!(is_leap_year(2004));
/// assert!(!is_leap_year(2005));
/// assert!(!is_leap_year(2100));
/// ```
#[inline]
pub const fn is_leap_year(year: i32) -> bool {
    let d = if year % 100 == 0 { 15 } else { 3 };
    year & d == 0
}

/// Get the number of calendar days in a given year.
///
/// The returned value will always be either 365 or 366.
///
/// ```rust
/// # use time::util::days_in_year;
/// assert_eq!(days_in_year(1900), 365);
/// assert_eq!(days_in_year(2000), 366);
/// assert_eq!(days_in_year(2004), 366);
/// assert_eq!(days_in_year(2005), 365);
/// assert_eq!(days_in_year(2100), 365);
/// ```
#[inline]
pub const fn days_in_year(year: i32) -> u16 {
    if is_leap_year(year) {
        366
    } else {
        365
    }
}

/// Get the number of weeks in the ISO year.
///
/// The returned value will always be either 52 or 53.
///
/// ```rust
/// # use time::util::weeks_in_year;
/// assert_eq!(weeks_in_year(2019), 52);
/// assert_eq!(weeks_in_year(2020), 53);
/// ```
#[inline]
pub const fn weeks_in_year(year: i32) -> u8 {
    match year % 400 {
        -396 | -391 | -385 | -380 | -374 | -368 | -363 | -357 | -352 | -346 | -340 | -335
        | -329 | -324 | -318 | -312 | -307 | -301 | -295 | -289 | -284 | -278 | -272 | -267
        | -261 | -256 | -250 | -244 | -239 | -233 | -228 | -222 | -216 | -211 | -205 | -199
        | -193 | -188 | -182 | -176 | -171 | -165 | -160 | -154 | -148 | -143 | -137 | -132
        | -126 | -120 | -115 | -109 | -104 | -97 | -92 | -86 | -80 | -75 | -69 | -64 | -58
        | -52 | -47 | -41 | -36 | -30 | -24 | -19 | -13 | -8 | -2 | 4 | 9 | 15 | 20 | 26 | 32
        | 37 | 43 | 48 | 54 | 60 | 65 | 71 | 76 | 82 | 88 | 93 | 99 | 105 | 111 | 116 | 122
        | 128 | 133 | 139 | 144 | 150 | 156 | 161 | 167 | 172 | 178 | 184 | 189 | 195 | 201
        | 207 | 212 | 218 | 224 | 229 | 235 | 240 | 246 | 252 | 257 | 263 | 268 | 274 | 280
        | 285 | 291 | 296 | 303 | 308 | 314 | 320 | 325 | 331 | 336 | 342 | 348 | 353 | 359
        | 364 | 370 | 376 | 381 | 387 | 392 | 398 => 53,
        _ => 52,
    }
}

/// Get the number of days in the month of a given year.
///
/// ```rust
/// # use time_core::util::days_in_month;
/// assert_eq!(days_in_month(2, 2020), 29);
/// ```
///
/// Note: This function is not exposed by the `time` crate. It is an implementation detail.
#[inline]
pub const fn days_in_month(month: u8, year: i32) -> u8 {
    debug_assert!(month >= 1);
    debug_assert!(month <= 12);

    if hint::unlikely(month == 2) {
        if is_leap_year(year) {
            29
        } else {
            28
        }
    } else {
        30 | month ^ (month >> 3)
    }
}
