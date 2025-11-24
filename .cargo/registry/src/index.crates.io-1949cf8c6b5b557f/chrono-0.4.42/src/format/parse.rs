// This is a part of Chrono.
// Portions copyright (c) 2015, John Nagle.
// See README.md and LICENSE.txt for details.

//! Date and time parsing routines.

use core::borrow::Borrow;
use core::str;

use super::scan;
use super::{BAD_FORMAT, INVALID, OUT_OF_RANGE, TOO_LONG, TOO_SHORT};
use super::{Fixed, InternalFixed, InternalInternal, Item, Numeric, Pad, Parsed};
use super::{ParseError, ParseResult};
use crate::{DateTime, FixedOffset, Weekday};

fn set_weekday_with_num_days_from_sunday(p: &mut Parsed, v: i64) -> ParseResult<()> {
    p.set_weekday(match v {
        0 => Weekday::Sun,
        1 => Weekday::Mon,
        2 => Weekday::Tue,
        3 => Weekday::Wed,
        4 => Weekday::Thu,
        5 => Weekday::Fri,
        6 => Weekday::Sat,
        _ => return Err(OUT_OF_RANGE),
    })
}

fn set_weekday_with_number_from_monday(p: &mut Parsed, v: i64) -> ParseResult<()> {
    p.set_weekday(match v {
        1 => Weekday::Mon,
        2 => Weekday::Tue,
        3 => Weekday::Wed,
        4 => Weekday::Thu,
        5 => Weekday::Fri,
        6 => Weekday::Sat,
        7 => Weekday::Sun,
        _ => return Err(OUT_OF_RANGE),
    })
}

fn parse_rfc2822<'a>(parsed: &mut Parsed, mut s: &'a str) -> ParseResult<(&'a str, ())> {
    macro_rules! try_consume {
        ($e:expr) => {{
            let (s_, v) = $e?;
            s = s_;
            v
        }};
    }

    // an adapted RFC 2822 syntax from Section 3.3 and 4.3:
    //
    // c-char      = <any char except '(', ')' and '\\'>
    // c-escape    = "\" <any char>
    // comment     = "(" *(comment / c-char / c-escape) ")" *S
    // date-time   = [ day-of-week "," ] date 1*S time *S *comment
    // day-of-week = *S day-name *S
    // day-name    = "Mon" / "Tue" / "Wed" / "Thu" / "Fri" / "Sat" / "Sun"
    // date        = day month year
    // day         = *S 1*2DIGIT *S
    // month       = 1*S month-name 1*S
    // month-name  = "Jan" / "Feb" / "Mar" / "Apr" / "May" / "Jun" /
    //               "Jul" / "Aug" / "Sep" / "Oct" / "Nov" / "Dec"
    // year        = *S 2*DIGIT *S
    // time        = time-of-day 1*S zone
    // time-of-day = hour ":" minute [ ":" second ]
    // hour        = *S 2DIGIT *S
    // minute      = *S 2DIGIT *S
    // second      = *S 2DIGIT *S
    // zone        = ( "+" / "-" ) 4DIGIT /
    //               "UT" / "GMT" /                  ; same as +0000
    //               "EST" / "CST" / "MST" / "PST" / ; same as -0500 to -0800
    //               "EDT" / "CDT" / "MDT" / "PDT" / ; same as -0400 to -0700
    //               1*(%d65-90 / %d97-122)          ; same as -0000
    //
    // some notes:
    //
    // - quoted characters can be in any mixture of lower and upper cases.
    //
    // - we do not recognize a folding white space (FWS) or comment (CFWS).
    //   for our purposes, instead, we accept any sequence of Unicode
    //   white space characters (denoted here to `S`). For comments, we accept
    //   any text within parentheses while respecting escaped parentheses.
    //   Any actual RFC 2822 parser is expected to parse FWS and/or CFWS themselves
    //   and replace it with a single SP (`%x20`); this is legitimate.
    //
    // - two-digit year < 50 should be interpreted by adding 2000.
    //   two-digit year >= 50 or three-digit year should be interpreted
    //   by adding 1900. note that four-or-more-digit years less than 1000
    //   are *never* affected by this rule.
    //
    // - mismatching day-of-week is always an error, which is consistent to
    //   Chrono's own rules.
    //
    // - zones can range from `-9959` to `+9959`, but `FixedOffset` does not
    //   support offsets larger than 24 hours. this is not *that* problematic
    //   since we do not directly go to a `DateTime` so one can recover
    //   the offset information from `Parsed` anyway.

    s = s.trim_start();

    if let Ok((s_, weekday)) = scan::short_weekday(s) {
        if !s_.starts_with(',') {
            return Err(INVALID);
        }
        s = &s_[1..];
        parsed.set_weekday(weekday)?;
    }

    s = s.trim_start();
    parsed.set_day(try_consume!(scan::number(s, 1, 2)))?;
    s = scan::space(s)?; // mandatory
    parsed.set_month(1 + i64::from(try_consume!(scan::short_month0(s))))?;
    s = scan::space(s)?; // mandatory

    // distinguish two- and three-digit years from four-digit years
    let prevlen = s.len();
    let mut year = try_consume!(scan::number(s, 2, usize::MAX));
    let yearlen = prevlen - s.len();
    match (yearlen, year) {
        (2, 0..=49) => {
            year += 2000;
        } //   47 -> 2047,   05 -> 2005
        (2, 50..=99) => {
            year += 1900;
        } //   79 -> 1979
        (3, _) => {
            year += 1900;
        } //  112 -> 2012,  009 -> 1909
        (_, _) => {} // 1987 -> 1987, 0654 -> 0654
    }
    parsed.set_year(year)?;

    s = scan::space(s)?; // mandatory
    parsed.set_hour(try_consume!(scan::number(s, 2, 2)))?;
    s = scan::char(s.trim_start(), b':')?.trim_start(); // *S ":" *S
    parsed.set_minute(try_consume!(scan::number(s, 2, 2)))?;
    if let Ok(s_) = scan::char(s.trim_start(), b':') {
        // [ ":" *S 2DIGIT ]
        parsed.set_second(try_consume!(scan::number(s_, 2, 2)))?;
    }

    s = scan::space(s)?; // mandatory
    parsed.set_offset(i64::from(try_consume!(scan::timezone_offset_2822(s))))?;

    // optional comments
    while let Ok((s_out, ())) = scan::comment_2822(s) {
        s = s_out;
    }

    Ok((s, ()))
}

pub(crate) fn parse_rfc3339<'a>(parsed: &mut Parsed, mut s: &'a str) -> ParseResult<(&'a str, ())> {
    macro_rules! try_consume {
        ($e:expr) => {{
            let (s_, v) = $e?;
            s = s_;
            v
        }};
    }

    // an adapted RFC 3339 syntax from Section 5.6:
    //
    // date-fullyear  = 4DIGIT
    // date-month     = 2DIGIT ; 01-12
    // date-mday      = 2DIGIT ; 01-28, 01-29, 01-30, 01-31 based on month/year
    // time-hour      = 2DIGIT ; 00-23
    // time-minute    = 2DIGIT ; 00-59
    // time-second    = 2DIGIT ; 00-58, 00-59, 00-60 based on leap second rules
    // time-secfrac   = "." 1*DIGIT
    // time-numoffset = ("+" / "-") time-hour ":" time-minute
    // time-offset    = "Z" / time-numoffset
    // partial-time   = time-hour ":" time-minute ":" time-second [time-secfrac]
    // full-date      = date-fullyear "-" date-month "-" date-mday
    // full-time      = partial-time time-offset
    // date-time      = full-date "T" full-time
    //
    // some notes:
    //
    // - quoted characters can be in any mixture of lower and upper cases.
    //
    // - it may accept any number of fractional digits for seconds.
    //   for Chrono, this means that we should skip digits past first 9 digits.
    //
    // - unlike RFC 2822, the valid offset ranges from -23:59 to +23:59.
    //   note that this restriction is unique to RFC 3339 and not ISO 8601.
    //   since this is not a typical Chrono behavior, we check it earlier.
    //
    // - For readability a full-date and a full-time may be separated by a space character.

    parsed.set_year(try_consume!(scan::number(s, 4, 4)))?;
    s = scan::char(s, b'-')?;
    parsed.set_month(try_consume!(scan::number(s, 2, 2)))?;
    s = scan::char(s, b'-')?;
    parsed.set_day(try_consume!(scan::number(s, 2, 2)))?;

    s = match s.as_bytes().first() {
        Some(&b't' | &b'T' | &b' ') => &s[1..],
        Some(_) => return Err(INVALID),
        None => return Err(TOO_SHORT),
    };

    parsed.set_hour(try_consume!(scan::number(s, 2, 2)))?;
    s = scan::char(s, b':')?;
    parsed.set_minute(try_consume!(scan::number(s, 2, 2)))?;
    s = scan::char(s, b':')?;
    parsed.set_second(try_consume!(scan::number(s, 2, 2)))?;
    if s.starts_with('.') {
        let nanosecond = try_consume!(scan::nanosecond(&s[1..]));
        parsed.set_nanosecond(nanosecond)?;
    }

    let offset = try_consume!(scan::timezone_offset(s, |s| scan::char(s, b':'), true, false, true));
    // This range check is similar to the one in `FixedOffset::east_opt`, so it would be redundant.
    // But it is possible to read the offset directly from `Parsed`. We want to only successfully
    // populate `Parsed` if the input is fully valid RFC 3339.
    // Max for the hours field is `23`, and for the minutes field `59`.
    const MAX_RFC3339_OFFSET: i32 = (23 * 60 + 59) * 60;
    if !(-MAX_RFC3339_OFFSET..=MAX_RFC3339_OFFSET).contains(&offset) {
        return Err(OUT_OF_RANGE);
    }
    parsed.set_offset(i64::from(offset))?;

    Ok((s, ()))
}

/// Tries to parse given string into `parsed` with given formatting items.
/// Returns `Ok` when the entire string has been parsed (otherwise `parsed` should not be used).
/// There should be no trailing string after parsing;
/// use a stray [`Item::Space`](./enum.Item.html#variant.Space) to trim whitespaces.
///
/// This particular date and time parser is:
///
/// - Greedy. It will consume the longest possible prefix.
///   For example, `April` is always consumed entirely when the long month name is requested;
///   it equally accepts `Apr`, but prefers the longer prefix in this case.
///
/// - Padding-agnostic (for numeric items).
///   The [`Pad`](./enum.Pad.html) field is completely ignored,
///   so one can prepend any number of whitespace then any number of zeroes before numbers.
///
/// - (Still) obeying the intrinsic parsing width. This allows, for example, parsing `HHMMSS`.
pub fn parse<'a, I, B>(parsed: &mut Parsed, s: &str, items: I) -> ParseResult<()>
where
    I: Iterator<Item = B>,
    B: Borrow<Item<'a>>,
{
    match parse_internal(parsed, s, items) {
        Ok("") => Ok(()),
        Ok(_) => Err(TOO_LONG), // if there are trailing chars it is an error
        Err(e) => Err(e),
    }
}

/// Tries to parse given string into `parsed` with given formatting items.
/// Returns `Ok` with a slice of the unparsed remainder.
///
/// This particular date and time parser is:
///
/// - Greedy. It will consume the longest possible prefix.
///   For example, `April` is always consumed entirely when the long month name is requested;
///   it equally accepts `Apr`, but prefers the longer prefix in this case.
///
/// - Padding-agnostic (for numeric items).
///   The [`Pad`](./enum.Pad.html) field is completely ignored,
///   so one can prepend any number of zeroes before numbers.
///
/// - (Still) obeying the intrinsic parsing width. This allows, for example, parsing `HHMMSS`.
pub fn parse_and_remainder<'a, 'b, I, B>(
    parsed: &mut Parsed,
    s: &'b str,
    items: I,
) -> ParseResult<&'b str>
where
    I: Iterator<Item = B>,
    B: Borrow<Item<'a>>,
{
    parse_internal(parsed, s, items)
}

fn parse_internal<'a, 'b, I, B>(
    parsed: &mut Parsed,
    mut s: &'b str,
    items: I,
) -> Result<&'b str, ParseError>
where
    I: Iterator<Item = B>,
    B: Borrow<Item<'a>>,
{
    macro_rules! try_consume {
        ($e:expr) => {{
            match $e {
                Ok((s_, v)) => {
                    s = s_;
                    v
                }
                Err(e) => return Err(e),
            }
        }};
    }

    for item in items {
        match *item.borrow() {
            Item::Literal(prefix) => {
                if s.len() < prefix.len() {
                    return Err(TOO_SHORT);
                }
                if !s.starts_with(prefix) {
                    return Err(INVALID);
                }
                s = &s[prefix.len()..];
            }

            #[cfg(feature = "alloc")]
            Item::OwnedLiteral(ref prefix) => {
                if s.len() < prefix.len() {
                    return Err(TOO_SHORT);
                }
                if !s.starts_with(&prefix[..]) {
                    return Err(INVALID);
                }
                s = &s[prefix.len()..];
            }

            Item::Space(_) => {
                s = s.trim_start();
            }

            #[cfg(feature = "alloc")]
            Item::OwnedSpace(_) => {
                s = s.trim_start();
            }

            Item::Numeric(ref spec, ref _pad) => {
                use super::Numeric::*;
                type Setter = fn(&mut Parsed, i64) -> ParseResult<()>;

                let (width, signed, set): (usize, bool, Setter) = match *spec {
                    Year => (4, true, Parsed::set_year),
                    YearDiv100 => (2, false, Parsed::set_year_div_100),
                    YearMod100 => (2, false, Parsed::set_year_mod_100),
                    IsoYear => (4, true, Parsed::set_isoyear),
                    IsoYearDiv100 => (2, false, Parsed::set_isoyear_div_100),
                    IsoYearMod100 => (2, false, Parsed::set_isoyear_mod_100),
                    Quarter => (1, false, Parsed::set_quarter),
                    Month => (2, false, Parsed::set_month),
                    Day => (2, false, Parsed::set_day),
                    WeekFromSun => (2, false, Parsed::set_week_from_sun),
                    WeekFromMon => (2, false, Parsed::set_week_from_mon),
                    IsoWeek => (2, false, Parsed::set_isoweek),
                    NumDaysFromSun => (1, false, set_weekday_with_num_days_from_sunday),
                    WeekdayFromMon => (1, false, set_weekday_with_number_from_monday),
                    Ordinal => (3, false, Parsed::set_ordinal),
                    Hour => (2, false, Parsed::set_hour),
                    Hour12 => (2, false, Parsed::set_hour12),
                    Minute => (2, false, Parsed::set_minute),
                    Second => (2, false, Parsed::set_second),
                    Nanosecond => (9, false, Parsed::set_nanosecond),
                    Timestamp => (usize::MAX, false, Parsed::set_timestamp),

                    // for the future expansion
                    Internal(ref int) => match int._dummy {},
                };

                s = s.trim_start();
                let v = if signed {
                    if s.starts_with('-') {
                        let v = try_consume!(scan::number(&s[1..], 1, usize::MAX));
                        0i64.checked_sub(v).ok_or(OUT_OF_RANGE)?
                    } else if s.starts_with('+') {
                        try_consume!(scan::number(&s[1..], 1, usize::MAX))
                    } else {
                        // if there is no explicit sign, we respect the original `width`
                        try_consume!(scan::number(s, 1, width))
                    }
                } else {
                    try_consume!(scan::number(s, 1, width))
                };
                set(parsed, v)?;
            }

            Item::Fixed(ref spec) => {
                use super::Fixed::*;

                match spec {
                    &ShortMonthName => {
                        let month0 = try_consume!(scan::short_month0(s));
                        parsed.set_month(i64::from(month0) + 1)?;
                    }

                    &LongMonthName => {
                        let month0 = try_consume!(scan::short_or_long_month0(s));
                        parsed.set_month(i64::from(month0) + 1)?;
                    }

                    &ShortWeekdayName => {
                        let weekday = try_consume!(scan::short_weekday(s));
                        parsed.set_weekday(weekday)?;
                    }

                    &LongWeekdayName => {
                        let weekday = try_consume!(scan::short_or_long_weekday(s));
                        parsed.set_weekday(weekday)?;
                    }

                    &LowerAmPm | &UpperAmPm => {
                        if s.len() < 2 {
                            return Err(TOO_SHORT);
                        }
                        let ampm = match (s.as_bytes()[0] | 32, s.as_bytes()[1] | 32) {
                            (b'a', b'm') => false,
                            (b'p', b'm') => true,
                            _ => return Err(INVALID),
                        };
                        parsed.set_ampm(ampm)?;
                        s = &s[2..];
                    }

                    &Nanosecond => {
                        if s.starts_with('.') {
                            let nano = try_consume!(scan::nanosecond(&s[1..]));
                            parsed.set_nanosecond(nano)?;
                        }
                    }

                    &Nanosecond3 => {
                        if s.starts_with('.') {
                            let nano = try_consume!(scan::nanosecond_fixed(&s[1..], 3));
                            parsed.set_nanosecond(nano)?;
                        }
                    }

                    &Nanosecond6 => {
                        if s.starts_with('.') {
                            let nano = try_consume!(scan::nanosecond_fixed(&s[1..], 6));
                            parsed.set_nanosecond(nano)?;
                        }
                    }

                    &Nanosecond9 => {
                        if s.starts_with('.') {
                            let nano = try_consume!(scan::nanosecond_fixed(&s[1..], 9));
                            parsed.set_nanosecond(nano)?;
                        }
                    }

                    &Internal(InternalFixed { val: InternalInternal::Nanosecond3NoDot }) => {
                        if s.len() < 3 {
                            return Err(TOO_SHORT);
                        }
                        let nano = try_consume!(scan::nanosecond_fixed(s, 3));
                        parsed.set_nanosecond(nano)?;
                    }

                    &Internal(InternalFixed { val: InternalInternal::Nanosecond6NoDot }) => {
                        if s.len() < 6 {
                            return Err(TOO_SHORT);
                        }
                        let nano = try_consume!(scan::nanosecond_fixed(s, 6));
                        parsed.set_nanosecond(nano)?;
                    }

                    &Internal(InternalFixed { val: InternalInternal::Nanosecond9NoDot }) => {
                        if s.len() < 9 {
                            return Err(TOO_SHORT);
                        }
                        let nano = try_consume!(scan::nanosecond_fixed(s, 9));
                        parsed.set_nanosecond(nano)?;
                    }

                    &TimezoneName => {
                        try_consume!(Ok((s.trim_start_matches(|c: char| !c.is_whitespace()), ())));
                    }

                    &TimezoneOffsetColon
                    | &TimezoneOffsetDoubleColon
                    | &TimezoneOffsetTripleColon
                    | &TimezoneOffset => {
                        let offset = try_consume!(scan::timezone_offset(
                            s.trim_start(),
                            scan::colon_or_space,
                            false,
                            false,
                            true,
                        ));
                        parsed.set_offset(i64::from(offset))?;
                    }

                    &TimezoneOffsetColonZ | &TimezoneOffsetZ => {
                        let offset = try_consume!(scan::timezone_offset(
                            s.trim_start(),
                            scan::colon_or_space,
                            true,
                            false,
                            true,
                        ));
                        parsed.set_offset(i64::from(offset))?;
                    }
                    &Internal(InternalFixed {
                        val: InternalInternal::TimezoneOffsetPermissive,
                    }) => {
                        let offset = try_consume!(scan::timezone_offset(
                            s.trim_start(),
                            scan::colon_or_space,
                            true,
                            true,
                            true,
                        ));
                        parsed.set_offset(i64::from(offset))?;
                    }

                    &RFC2822 => try_consume!(parse_rfc2822(parsed, s)),
                    &RFC3339 => {
                        // Used for the `%+` specifier, which has the description:
                        // "Same as `%Y-%m-%dT%H:%M:%S%.f%:z` (...)
                        // This format also supports having a `Z` or `UTC` in place of `%:z`."
                        // Use the relaxed parser to match this description.
                        try_consume!(parse_rfc3339_relaxed(parsed, s))
                    }
                }
            }

            Item::Error => {
                return Err(BAD_FORMAT);
            }
        }
    }
    Ok(s)
}

/// Accepts a relaxed form of RFC3339.
/// A space or a 'T' are accepted as the separator between the date and time
/// parts. Additional spaces are allowed between each component.
///
/// All of these examples are equivalent:
/// ```
/// # use chrono::{DateTime, offset::FixedOffset};
/// "2012-12-12T12:12:12Z".parse::<DateTime<FixedOffset>>()?;
/// "2012-12-12 12:12:12Z".parse::<DateTime<FixedOffset>>()?;
/// "2012-  12-12T12:  12:12Z".parse::<DateTime<FixedOffset>>()?;
/// # Ok::<(), chrono::ParseError>(())
/// ```
impl str::FromStr for DateTime<FixedOffset> {
    type Err = ParseError;

    fn from_str(s: &str) -> ParseResult<DateTime<FixedOffset>> {
        let mut parsed = Parsed::new();
        let (s, _) = parse_rfc3339_relaxed(&mut parsed, s)?;
        if !s.trim_start().is_empty() {
            return Err(TOO_LONG);
        }
        parsed.to_datetime()
    }
}

/// Accepts a relaxed form of RFC3339.
///
/// Differences with RFC3339:
/// - Values don't require padding to two digits.
/// - Years outside the range 0...=9999 are accepted, but they must include a sign.
/// - `UTC` is accepted as a valid timezone name/offset (for compatibility with the debug format of
///   `DateTime<Utc>`.
/// - There can be spaces between any of the components.
/// - The colon in the offset may be missing.
fn parse_rfc3339_relaxed<'a>(parsed: &mut Parsed, mut s: &'a str) -> ParseResult<(&'a str, ())> {
    const DATE_ITEMS: &[Item<'static>] = &[
        Item::Numeric(Numeric::Year, Pad::Zero),
        Item::Space(""),
        Item::Literal("-"),
        Item::Numeric(Numeric::Month, Pad::Zero),
        Item::Space(""),
        Item::Literal("-"),
        Item::Numeric(Numeric::Day, Pad::Zero),
    ];
    const TIME_ITEMS: &[Item<'static>] = &[
        Item::Numeric(Numeric::Hour, Pad::Zero),
        Item::Space(""),
        Item::Literal(":"),
        Item::Numeric(Numeric::Minute, Pad::Zero),
        Item::Space(""),
        Item::Literal(":"),
        Item::Numeric(Numeric::Second, Pad::Zero),
        Item::Fixed(Fixed::Nanosecond),
        Item::Space(""),
    ];

    s = parse_internal(parsed, s, DATE_ITEMS.iter())?;

    s = match s.as_bytes().first() {
        Some(&b't' | &b'T' | &b' ') => &s[1..],
        Some(_) => return Err(INVALID),
        None => return Err(TOO_SHORT),
    };

    s = parse_internal(parsed, s, TIME_ITEMS.iter())?;
    s = s.trim_start();
    let (s, offset) = if s.len() >= 3 && "UTC".as_bytes().eq_ignore_ascii_case(&s.as_bytes()[..3]) {
        (&s[3..], 0)
    } else {
        scan::timezone_offset(s, scan::colon_or_space, true, false, true)?
    };
    parsed.set_offset(i64::from(offset))?;
    Ok((s, ()))
}

#[cfg(test)]
mod tests {
    use crate::format::*;
    use crate::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Timelike, Utc};

    macro_rules! parsed {
        ($($k:ident: $v:expr),*) => (#[allow(unused_mut)] {
            let mut expected = Parsed::new();
            $(expected.$k = Some($v);)*
            Ok(expected)
        });
    }

    #[test]
    fn test_parse_whitespace_and_literal() {
        use crate::format::Item::{Literal, Space};

        // empty string
        parses("", &[]);
        check(" ", &[], Err(TOO_LONG));
        check("a", &[], Err(TOO_LONG));
        check("abc", &[], Err(TOO_LONG));
        check("🤠", &[], Err(TOO_LONG));

        // whitespaces
        parses("", &[Space("")]);
        parses(" ", &[Space(" ")]);
        parses("  ", &[Space("  ")]);
        parses("   ", &[Space("   ")]);
        parses(" ", &[Space("")]);
        parses("  ", &[Space(" ")]);
        parses("   ", &[Space("  ")]);
        parses("    ", &[Space("  ")]);
        parses("", &[Space(" ")]);
        parses(" ", &[Space("  ")]);
        parses("  ", &[Space("   ")]);
        parses("  ", &[Space("  "), Space("  ")]);
        parses("   ", &[Space("  "), Space("  ")]);
        parses("  ", &[Space(" "), Space(" ")]);
        parses("   ", &[Space("  "), Space(" ")]);
        parses("   ", &[Space(" "), Space("  ")]);
        parses("   ", &[Space(" "), Space(" "), Space(" ")]);
        parses("\t", &[Space("")]);
        parses(" \n\r  \n", &[Space("")]);
        parses("\t", &[Space("\t")]);
        parses("\t", &[Space(" ")]);
        parses(" ", &[Space("\t")]);
        parses("\t\r", &[Space("\t\r")]);
        parses("\t\r ", &[Space("\t\r ")]);
        parses("\t \r", &[Space("\t \r")]);
        parses(" \t\r", &[Space(" \t\r")]);
        parses(" \n\r  \n", &[Space(" \n\r  \n")]);
        parses(" \t\n", &[Space(" \t")]);
        parses(" \n\t", &[Space(" \t\n")]);
        parses("\u{2002}", &[Space("\u{2002}")]);
        // most unicode whitespace characters
        parses(
            "\u{00A0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{3000}",
            &[Space(
                "\u{00A0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{3000}",
            )],
        );
        // most unicode whitespace characters
        parses(
            "\u{00A0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{3000}",
            &[
                Space("\u{00A0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}"),
                Space("\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{3000}"),
            ],
        );
        check("a", &[Space("")], Err(TOO_LONG));
        check("a", &[Space(" ")], Err(TOO_LONG));
        // a Space containing a literal does not match a literal
        check("a", &[Space("a")], Err(TOO_LONG));
        check("abc", &[Space("")], Err(TOO_LONG));
        check("abc", &[Space(" ")], Err(TOO_LONG));
        check(" abc", &[Space("")], Err(TOO_LONG));
        check(" abc", &[Space(" ")], Err(TOO_LONG));

        // `\u{0363}` is combining diacritic mark "COMBINING LATIN SMALL LETTER A"

        // literal
        parses("", &[Literal("")]);
        check("", &[Literal("a")], Err(TOO_SHORT));
        check(" ", &[Literal("a")], Err(INVALID));
        parses("a", &[Literal("a")]);
        parses("+", &[Literal("+")]);
        parses("-", &[Literal("-")]);
        parses("−", &[Literal("−")]); // MINUS SIGN (U+2212)
        parses(" ", &[Literal(" ")]); // a Literal may contain whitespace and match whitespace
        check("aa", &[Literal("a")], Err(TOO_LONG));
        check("🤠", &[Literal("a")], Err(INVALID));
        check("A", &[Literal("a")], Err(INVALID));
        check("a", &[Literal("z")], Err(INVALID));
        check("a", &[Literal("🤠")], Err(TOO_SHORT));
        check("a", &[Literal("\u{0363}a")], Err(TOO_SHORT));
        check("\u{0363}a", &[Literal("a")], Err(INVALID));
        parses("\u{0363}a", &[Literal("\u{0363}a")]);
        check("a", &[Literal("ab")], Err(TOO_SHORT));
        parses("xy", &[Literal("xy")]);
        parses("xy", &[Literal("x"), Literal("y")]);
        parses("1", &[Literal("1")]);
        parses("1234", &[Literal("1234")]);
        parses("+1234", &[Literal("+1234")]);
        parses("-1234", &[Literal("-1234")]);
        parses("−1234", &[Literal("−1234")]); // MINUS SIGN (U+2212)
        parses("PST", &[Literal("PST")]);
        parses("🤠", &[Literal("🤠")]);
        parses("🤠a", &[Literal("🤠"), Literal("a")]);
        parses("🤠a🤠", &[Literal("🤠"), Literal("a🤠")]);
        parses("a🤠b", &[Literal("a"), Literal("🤠"), Literal("b")]);
        // literals can be together
        parses("xy", &[Literal("xy")]);
        parses("xyz", &[Literal("xyz")]);
        // or literals can be apart
        parses("xy", &[Literal("x"), Literal("y")]);
        parses("xyz", &[Literal("x"), Literal("yz")]);
        parses("xyz", &[Literal("xy"), Literal("z")]);
        parses("xyz", &[Literal("x"), Literal("y"), Literal("z")]);
        //
        check("x y", &[Literal("x"), Literal("y")], Err(INVALID));
        parses("xy", &[Literal("x"), Space(""), Literal("y")]);
        parses("x y", &[Literal("x"), Space(""), Literal("y")]);
        parses("x y", &[Literal("x"), Space(" "), Literal("y")]);

        // whitespaces + literals
        parses("a\n", &[Literal("a"), Space("\n")]);
        parses("\tab\n", &[Space("\t"), Literal("ab"), Space("\n")]);
        parses(
            "ab\tcd\ne",
            &[Literal("ab"), Space("\t"), Literal("cd"), Space("\n"), Literal("e")],
        );
        parses(
            "+1ab\tcd\r\n+,.",
            &[Literal("+1ab"), Space("\t"), Literal("cd"), Space("\r\n"), Literal("+,.")],
        );
        // whitespace and literals can be intermixed
        parses("a\tb", &[Literal("a\tb")]);
        parses("a\tb", &[Literal("a"), Space("\t"), Literal("b")]);
    }

    #[test]
    fn test_parse_numeric() {
        use crate::format::Item::{Literal, Space};
        use crate::format::Numeric::*;

        // numeric
        check("1987", &[num(Year)], parsed!(year: 1987));
        check("1987 ", &[num(Year)], Err(TOO_LONG));
        check("0x12", &[num(Year)], Err(TOO_LONG)); // `0` is parsed
        check("x123", &[num(Year)], Err(INVALID));
        check("o123", &[num(Year)], Err(INVALID));
        check("2015", &[num(Year)], parsed!(year: 2015));
        check("0000", &[num(Year)], parsed!(year: 0));
        check("9999", &[num(Year)], parsed!(year: 9999));
        check(" \t987", &[num(Year)], parsed!(year: 987));
        check(" \t987", &[Space(" \t"), num(Year)], parsed!(year: 987));
        check(" \t987🤠", &[Space(" \t"), num(Year), Literal("🤠")], parsed!(year: 987));
        check("987🤠", &[num(Year), Literal("🤠")], parsed!(year: 987));
        check("5", &[num(Year)], parsed!(year: 5));
        check("5\0", &[num(Year)], Err(TOO_LONG));
        check("\x005", &[num(Year)], Err(INVALID));
        check("", &[num(Year)], Err(TOO_SHORT));
        check("12345", &[num(Year), Literal("5")], parsed!(year: 1234));
        check("12345", &[nums(Year), Literal("5")], parsed!(year: 1234));
        check("12345", &[num0(Year), Literal("5")], parsed!(year: 1234));
        check("12341234", &[num(Year), num(Year)], parsed!(year: 1234));
        check("1234 1234", &[num(Year), num(Year)], parsed!(year: 1234));
        check("1234 1234", &[num(Year), Space(" "), num(Year)], parsed!(year: 1234));
        check("1234 1235", &[num(Year), num(Year)], Err(IMPOSSIBLE));
        check("1234 1234", &[num(Year), Literal("x"), num(Year)], Err(INVALID));
        check("1234x1234", &[num(Year), Literal("x"), num(Year)], parsed!(year: 1234));
        check("1234 x 1234", &[num(Year), Literal("x"), num(Year)], Err(INVALID));
        check("1234xx1234", &[num(Year), Literal("x"), num(Year)], Err(INVALID));
        check("1234xx1234", &[num(Year), Literal("xx"), num(Year)], parsed!(year: 1234));
        check(
            "1234 x 1234",
            &[num(Year), Space(" "), Literal("x"), Space(" "), num(Year)],
            parsed!(year: 1234),
        );
        check(
            "1234 x 1235",
            &[num(Year), Space(" "), Literal("x"), Space(" "), Literal("1235")],
            parsed!(year: 1234),
        );

        // signed numeric
        check("-42", &[num(Year)], parsed!(year: -42));
        check("+42", &[num(Year)], parsed!(year: 42));
        check("-0042", &[num(Year)], parsed!(year: -42));
        check("+0042", &[num(Year)], parsed!(year: 42));
        check("-42195", &[num(Year)], parsed!(year: -42195));
        check("−42195", &[num(Year)], Err(INVALID)); // MINUS SIGN (U+2212)
        check("+42195", &[num(Year)], parsed!(year: 42195));
        check("  -42195", &[num(Year)], parsed!(year: -42195));
        check(" +42195", &[num(Year)], parsed!(year: 42195));
        check("  -42195", &[num(Year)], parsed!(year: -42195));
        check("  +42195", &[num(Year)], parsed!(year: 42195));
        check("-42195 ", &[num(Year)], Err(TOO_LONG));
        check("+42195 ", &[num(Year)], Err(TOO_LONG));
        check("  -   42", &[num(Year)], Err(INVALID));
        check("  +   42", &[num(Year)], Err(INVALID));
        check("  -42195", &[Space("  "), num(Year)], parsed!(year: -42195));
        check("  −42195", &[Space("  "), num(Year)], Err(INVALID)); // MINUS SIGN (U+2212)
        check("  +42195", &[Space("  "), num(Year)], parsed!(year: 42195));
        check("  -   42", &[Space("  "), num(Year)], Err(INVALID));
        check("  +   42", &[Space("  "), num(Year)], Err(INVALID));
        check("-", &[num(Year)], Err(TOO_SHORT));
        check("+", &[num(Year)], Err(TOO_SHORT));

        // unsigned numeric
        check("345", &[num(Ordinal)], parsed!(ordinal: 345));
        check("+345", &[num(Ordinal)], Err(INVALID));
        check("-345", &[num(Ordinal)], Err(INVALID));
        check(" 345", &[num(Ordinal)], parsed!(ordinal: 345));
        check("−345", &[num(Ordinal)], Err(INVALID)); // MINUS SIGN (U+2212)
        check("345 ", &[num(Ordinal)], Err(TOO_LONG));
        check(" 345", &[Space(" "), num(Ordinal)], parsed!(ordinal: 345));
        check("345 ", &[num(Ordinal), Space(" ")], parsed!(ordinal: 345));
        check("345🤠 ", &[num(Ordinal), Literal("🤠"), Space(" ")], parsed!(ordinal: 345));
        check("345🤠", &[num(Ordinal)], Err(TOO_LONG));
        check("\u{0363}345", &[num(Ordinal)], Err(INVALID));
        check(" +345", &[num(Ordinal)], Err(INVALID));
        check(" -345", &[num(Ordinal)], Err(INVALID));
        check("\t345", &[Space("\t"), num(Ordinal)], parsed!(ordinal: 345));
        check(" +345", &[Space(" "), num(Ordinal)], Err(INVALID));
        check(" -345", &[Space(" "), num(Ordinal)], Err(INVALID));

        // various numeric fields
        check("1234 5678", &[num(Year), num(IsoYear)], parsed!(year: 1234, isoyear: 5678));
        check("1234 5678", &[num(Year), num(IsoYear)], parsed!(year: 1234, isoyear: 5678));
        check(
            "12 34 56 78",
            &[num(YearDiv100), num(YearMod100), num(IsoYearDiv100), num(IsoYearMod100)],
            parsed!(year_div_100: 12, year_mod_100: 34, isoyear_div_100: 56, isoyear_mod_100: 78),
        );
        check(
            "1 1 2 3 4 5",
            &[
                num(Quarter),
                num(Month),
                num(Day),
                num(WeekFromSun),
                num(NumDaysFromSun),
                num(IsoWeek),
            ],
            parsed!(quarter: 1, month: 1, day: 2, week_from_sun: 3, weekday: Weekday::Thu, isoweek: 5),
        );
        check(
            "6 7 89 01",
            &[num(WeekFromMon), num(WeekdayFromMon), num(Ordinal), num(Hour12)],
            parsed!(week_from_mon: 6, weekday: Weekday::Sun, ordinal: 89, hour_mod_12: 1),
        );
        check(
            "23 45 6 78901234 567890123",
            &[num(Hour), num(Minute), num(Second), num(Nanosecond), num(Timestamp)],
            parsed!(hour_div_12: 1, hour_mod_12: 11, minute: 45, second: 6, nanosecond: 78_901_234, timestamp: 567_890_123),
        );
    }

    #[test]
    fn test_parse_fixed() {
        use crate::format::Fixed::*;
        use crate::format::Item::{Literal, Space};

        // fixed: month and weekday names
        check("apr", &[fixed(ShortMonthName)], parsed!(month: 4));
        check("Apr", &[fixed(ShortMonthName)], parsed!(month: 4));
        check("APR", &[fixed(ShortMonthName)], parsed!(month: 4));
        check("ApR", &[fixed(ShortMonthName)], parsed!(month: 4));
        check("\u{0363}APR", &[fixed(ShortMonthName)], Err(INVALID));
        check("April", &[fixed(ShortMonthName)], Err(TOO_LONG)); // `Apr` is parsed
        check("A", &[fixed(ShortMonthName)], Err(TOO_SHORT));
        check("Sol", &[fixed(ShortMonthName)], Err(INVALID));
        check("Apr", &[fixed(LongMonthName)], parsed!(month: 4));
        check("Apri", &[fixed(LongMonthName)], Err(TOO_LONG)); // `Apr` is parsed
        check("April", &[fixed(LongMonthName)], parsed!(month: 4));
        check("Aprill", &[fixed(LongMonthName)], Err(TOO_LONG));
        check("Aprill", &[fixed(LongMonthName), Literal("l")], parsed!(month: 4));
        check("Aprl", &[fixed(LongMonthName), Literal("l")], parsed!(month: 4));
        check("April", &[fixed(LongMonthName), Literal("il")], Err(TOO_SHORT)); // do not backtrack
        check("thu", &[fixed(ShortWeekdayName)], parsed!(weekday: Weekday::Thu));
        check("Thu", &[fixed(ShortWeekdayName)], parsed!(weekday: Weekday::Thu));
        check("THU", &[fixed(ShortWeekdayName)], parsed!(weekday: Weekday::Thu));
        check("tHu", &[fixed(ShortWeekdayName)], parsed!(weekday: Weekday::Thu));
        check("Thursday", &[fixed(ShortWeekdayName)], Err(TOO_LONG)); // `Thu` is parsed
        check("T", &[fixed(ShortWeekdayName)], Err(TOO_SHORT));
        check("The", &[fixed(ShortWeekdayName)], Err(INVALID));
        check("Nop", &[fixed(ShortWeekdayName)], Err(INVALID));
        check("Thu", &[fixed(LongWeekdayName)], parsed!(weekday: Weekday::Thu));
        check("Thur", &[fixed(LongWeekdayName)], Err(TOO_LONG)); // `Thu` is parsed
        check("Thurs", &[fixed(LongWeekdayName)], Err(TOO_LONG)); // `Thu` is parsed
        check("Thursday", &[fixed(LongWeekdayName)], parsed!(weekday: Weekday::Thu));
        check("Thursdays", &[fixed(LongWeekdayName)], Err(TOO_LONG));
        check("Thursdays", &[fixed(LongWeekdayName), Literal("s")], parsed!(weekday: Weekday::Thu));
        check("Thus", &[fixed(LongWeekdayName), Literal("s")], parsed!(weekday: Weekday::Thu));
        check("Thursday", &[fixed(LongWeekdayName), Literal("rsday")], Err(TOO_SHORT)); // do not backtrack

        // fixed: am/pm
        check("am", &[fixed(LowerAmPm)], parsed!(hour_div_12: 0));
        check("pm", &[fixed(LowerAmPm)], parsed!(hour_div_12: 1));
        check("AM", &[fixed(LowerAmPm)], parsed!(hour_div_12: 0));
        check("PM", &[fixed(LowerAmPm)], parsed!(hour_div_12: 1));
        check("am", &[fixed(UpperAmPm)], parsed!(hour_div_12: 0));
        check("pm", &[fixed(UpperAmPm)], parsed!(hour_div_12: 1));
        check("AM", &[fixed(UpperAmPm)], parsed!(hour_div_12: 0));
        check("PM", &[fixed(UpperAmPm)], parsed!(hour_div_12: 1));
        check("Am", &[fixed(LowerAmPm)], parsed!(hour_div_12: 0));
        check(" Am", &[Space(" "), fixed(LowerAmPm)], parsed!(hour_div_12: 0));
        check("Am🤠", &[fixed(LowerAmPm), Literal("🤠")], parsed!(hour_div_12: 0));
        check("🤠Am", &[Literal("🤠"), fixed(LowerAmPm)], parsed!(hour_div_12: 0));
        check("\u{0363}am", &[fixed(LowerAmPm)], Err(INVALID));
        check("\u{0360}am", &[fixed(LowerAmPm)], Err(INVALID));
        check(" Am", &[fixed(LowerAmPm)], Err(INVALID));
        check("Am ", &[fixed(LowerAmPm)], Err(TOO_LONG));
        check("a.m.", &[fixed(LowerAmPm)], Err(INVALID));
        check("A.M.", &[fixed(LowerAmPm)], Err(INVALID));
        check("ame", &[fixed(LowerAmPm)], Err(TOO_LONG)); // `am` is parsed
        check("a", &[fixed(LowerAmPm)], Err(TOO_SHORT));
        check("p", &[fixed(LowerAmPm)], Err(TOO_SHORT));
        check("x", &[fixed(LowerAmPm)], Err(TOO_SHORT));
        check("xx", &[fixed(LowerAmPm)], Err(INVALID));
        check("", &[fixed(LowerAmPm)], Err(TOO_SHORT));
    }

    #[test]
    fn test_parse_fixed_nanosecond() {
        use crate::format::Fixed::{Nanosecond, Nanosecond3, Nanosecond6, Nanosecond9};
        use crate::format::InternalInternal::*;
        use crate::format::Item::Literal;
        use crate::format::Numeric::Second;

        // fixed: dot plus nanoseconds
        check("", &[fixed(Nanosecond)], parsed!()); // no field set, but not an error
        check(".", &[fixed(Nanosecond)], Err(TOO_SHORT));
        check("4", &[fixed(Nanosecond)], Err(TOO_LONG)); // never consumes `4`
        check("4", &[fixed(Nanosecond), num(Second)], parsed!(second: 4));
        check(".0", &[fixed(Nanosecond)], parsed!(nanosecond: 0));
        check(".4", &[fixed(Nanosecond)], parsed!(nanosecond: 400_000_000));
        check(".42", &[fixed(Nanosecond)], parsed!(nanosecond: 420_000_000));
        check(".421", &[fixed(Nanosecond)], parsed!(nanosecond: 421_000_000));
        check(".42195", &[fixed(Nanosecond)], parsed!(nanosecond: 421_950_000));
        check(".421951", &[fixed(Nanosecond)], parsed!(nanosecond: 421_951_000));
        check(".4219512", &[fixed(Nanosecond)], parsed!(nanosecond: 421_951_200));
        check(".42195123", &[fixed(Nanosecond)], parsed!(nanosecond: 421_951_230));
        check(".421950803", &[fixed(Nanosecond)], parsed!(nanosecond: 421_950_803));
        check(".4219508035", &[fixed(Nanosecond)], parsed!(nanosecond: 421_950_803));
        check(".42195080354", &[fixed(Nanosecond)], parsed!(nanosecond: 421_950_803));
        check(".421950803547", &[fixed(Nanosecond)], parsed!(nanosecond: 421_950_803));
        check(".000000003", &[fixed(Nanosecond)], parsed!(nanosecond: 3));
        check(".0000000031", &[fixed(Nanosecond)], parsed!(nanosecond: 3));
        check(".0000000035", &[fixed(Nanosecond)], parsed!(nanosecond: 3));
        check(".000000003547", &[fixed(Nanosecond)], parsed!(nanosecond: 3));
        check(".0000000009", &[fixed(Nanosecond)], parsed!(nanosecond: 0));
        check(".000000000547", &[fixed(Nanosecond)], parsed!(nanosecond: 0));
        check(".0000000009999999999999999999999999", &[fixed(Nanosecond)], parsed!(nanosecond: 0));
        check(".4🤠", &[fixed(Nanosecond), Literal("🤠")], parsed!(nanosecond: 400_000_000));
        check(".4x", &[fixed(Nanosecond)], Err(TOO_LONG));
        check(".  4", &[fixed(Nanosecond)], Err(INVALID));
        check("  .4", &[fixed(Nanosecond)], Err(TOO_LONG)); // no automatic trimming

        // fixed-length fractions of a second
        check("", &[fixed(Nanosecond3)], parsed!()); // no field set, but not an error
        check("4", &[fixed(Nanosecond3)], Err(TOO_LONG)); // never consumes `4`
        check(".12", &[fixed(Nanosecond3)], Err(TOO_SHORT));
        check(".123", &[fixed(Nanosecond3)], parsed!(nanosecond: 123_000_000));
        check(".1234", &[fixed(Nanosecond3)], Err(TOO_LONG));
        check(".1234", &[fixed(Nanosecond3), Literal("4")], parsed!(nanosecond: 123_000_000));

        check("", &[fixed(Nanosecond6)], parsed!()); // no field set, but not an error
        check("4", &[fixed(Nanosecond6)], Err(TOO_LONG)); // never consumes `4`
        check(".12345", &[fixed(Nanosecond6)], Err(TOO_SHORT));
        check(".123456", &[fixed(Nanosecond6)], parsed!(nanosecond: 123_456_000));
        check(".1234567", &[fixed(Nanosecond6)], Err(TOO_LONG));
        check(".1234567", &[fixed(Nanosecond6), Literal("7")], parsed!(nanosecond: 123_456_000));

        check("", &[fixed(Nanosecond9)], parsed!()); // no field set, but not an error
        check("4", &[fixed(Nanosecond9)], Err(TOO_LONG)); // never consumes `4`
        check(".12345678", &[fixed(Nanosecond9)], Err(TOO_SHORT));
        check(".123456789", &[fixed(Nanosecond9)], parsed!(nanosecond: 123_456_789));
        check(".1234567890", &[fixed(Nanosecond9)], Err(TOO_LONG));
        check(".1234567890", &[fixed(Nanosecond9), Literal("0")], parsed!(nanosecond: 123_456_789));

        // fixed: nanoseconds without the dot
        check("", &[internal_fixed(Nanosecond3NoDot)], Err(TOO_SHORT));
        check(".", &[internal_fixed(Nanosecond3NoDot)], Err(TOO_SHORT));
        check("0", &[internal_fixed(Nanosecond3NoDot)], Err(TOO_SHORT));
        check("4", &[internal_fixed(Nanosecond3NoDot)], Err(TOO_SHORT));
        check("42", &[internal_fixed(Nanosecond3NoDot)], Err(TOO_SHORT));
        check("421", &[internal_fixed(Nanosecond3NoDot)], parsed!(nanosecond: 421_000_000));
        check("4210", &[internal_fixed(Nanosecond3NoDot)], Err(TOO_LONG));
        check(
            "42143",
            &[internal_fixed(Nanosecond3NoDot), num(Second)],
            parsed!(nanosecond: 421_000_000, second: 43),
        );
        check(
            "421🤠",
            &[internal_fixed(Nanosecond3NoDot), Literal("🤠")],
            parsed!(nanosecond: 421_000_000),
        );
        check(
            "🤠421",
            &[Literal("🤠"), internal_fixed(Nanosecond3NoDot)],
            parsed!(nanosecond: 421_000_000),
        );
        check("42195", &[internal_fixed(Nanosecond3NoDot)], Err(TOO_LONG));
        check("123456789", &[internal_fixed(Nanosecond3NoDot)], Err(TOO_LONG));
        check("4x", &[internal_fixed(Nanosecond3NoDot)], Err(TOO_SHORT));
        check("  4", &[internal_fixed(Nanosecond3NoDot)], Err(INVALID));
        check(".421", &[internal_fixed(Nanosecond3NoDot)], Err(INVALID));

        check("", &[internal_fixed(Nanosecond6NoDot)], Err(TOO_SHORT));
        check(".", &[internal_fixed(Nanosecond6NoDot)], Err(TOO_SHORT));
        check("0", &[internal_fixed(Nanosecond6NoDot)], Err(TOO_SHORT));
        check("1234", &[internal_fixed(Nanosecond6NoDot)], Err(TOO_SHORT));
        check("12345", &[internal_fixed(Nanosecond6NoDot)], Err(TOO_SHORT));
        check("421950", &[internal_fixed(Nanosecond6NoDot)], parsed!(nanosecond: 421_950_000));
        check("000003", &[internal_fixed(Nanosecond6NoDot)], parsed!(nanosecond: 3000));
        check("000000", &[internal_fixed(Nanosecond6NoDot)], parsed!(nanosecond: 0));
        check("1234567", &[internal_fixed(Nanosecond6NoDot)], Err(TOO_LONG));
        check("123456789", &[internal_fixed(Nanosecond6NoDot)], Err(TOO_LONG));
        check("4x", &[internal_fixed(Nanosecond6NoDot)], Err(TOO_SHORT));
        check("     4", &[internal_fixed(Nanosecond6NoDot)], Err(INVALID));
        check(".42100", &[internal_fixed(Nanosecond6NoDot)], Err(INVALID));

        check("", &[internal_fixed(Nanosecond9NoDot)], Err(TOO_SHORT));
        check(".", &[internal_fixed(Nanosecond9NoDot)], Err(TOO_SHORT));
        check("42195", &[internal_fixed(Nanosecond9NoDot)], Err(TOO_SHORT));
        check("12345678", &[internal_fixed(Nanosecond9NoDot)], Err(TOO_SHORT));
        check("421950803", &[internal_fixed(Nanosecond9NoDot)], parsed!(nanosecond: 421_950_803));
        check("000000003", &[internal_fixed(Nanosecond9NoDot)], parsed!(nanosecond: 3));
        check(
            "42195080354",
            &[internal_fixed(Nanosecond9NoDot), num(Second)],
            parsed!(nanosecond: 421_950_803, second: 54),
        ); // don't skip digits that come after the 9
        check("1234567890", &[internal_fixed(Nanosecond9NoDot)], Err(TOO_LONG));
        check("000000000", &[internal_fixed(Nanosecond9NoDot)], parsed!(nanosecond: 0));
        check("00000000x", &[internal_fixed(Nanosecond9NoDot)], Err(INVALID));
        check("        4", &[internal_fixed(Nanosecond9NoDot)], Err(INVALID));
        check(".42100000", &[internal_fixed(Nanosecond9NoDot)], Err(INVALID));
    }

    #[test]
    fn test_parse_fixed_timezone_offset() {
        use crate::format::Fixed::*;
        use crate::format::InternalInternal::*;
        use crate::format::Item::Literal;

        // TimezoneOffset
        check("1", &[fixed(TimezoneOffset)], Err(INVALID));
        check("12", &[fixed(TimezoneOffset)], Err(INVALID));
        check("123", &[fixed(TimezoneOffset)], Err(INVALID));
        check("1234", &[fixed(TimezoneOffset)], Err(INVALID));
        check("12345", &[fixed(TimezoneOffset)], Err(INVALID));
        check("123456", &[fixed(TimezoneOffset)], Err(INVALID));
        check("1234567", &[fixed(TimezoneOffset)], Err(INVALID));
        check("+1", &[fixed(TimezoneOffset)], Err(TOO_SHORT));
        check("+12", &[fixed(TimezoneOffset)], Err(TOO_SHORT));
        check("+123", &[fixed(TimezoneOffset)], Err(TOO_SHORT));
        check("+1234", &[fixed(TimezoneOffset)], parsed!(offset: 45_240));
        check("+12345", &[fixed(TimezoneOffset)], Err(TOO_LONG));
        check("+123456", &[fixed(TimezoneOffset)], Err(TOO_LONG));
        check("+1234567", &[fixed(TimezoneOffset)], Err(TOO_LONG));
        check("+12345678", &[fixed(TimezoneOffset)], Err(TOO_LONG));
        check("+12:", &[fixed(TimezoneOffset)], Err(TOO_SHORT));
        check("+12:3", &[fixed(TimezoneOffset)], Err(TOO_SHORT));
        check("+12:34", &[fixed(TimezoneOffset)], parsed!(offset: 45_240));
        check("-12:34", &[fixed(TimezoneOffset)], parsed!(offset: -45_240));
        check("−12:34", &[fixed(TimezoneOffset)], parsed!(offset: -45_240)); // MINUS SIGN (U+2212)
        check("+12:34:", &[fixed(TimezoneOffset)], Err(TOO_LONG));
        check("+12:34:5", &[fixed(TimezoneOffset)], Err(TOO_LONG));
        check("+12:34:56", &[fixed(TimezoneOffset)], Err(TOO_LONG));
        check("+12:34:56:", &[fixed(TimezoneOffset)], Err(TOO_LONG));
        check("+12 34", &[fixed(TimezoneOffset)], parsed!(offset: 45_240));
        check("+12  34", &[fixed(TimezoneOffset)], parsed!(offset: 45_240));
        check("12:34", &[fixed(TimezoneOffset)], Err(INVALID));
        check("12:34:56", &[fixed(TimezoneOffset)], Err(INVALID));
        check("+12::34", &[fixed(TimezoneOffset)], parsed!(offset: 45_240));
        check("+12: :34", &[fixed(TimezoneOffset)], parsed!(offset: 45_240));
        check("+12:::34", &[fixed(TimezoneOffset)], parsed!(offset: 45_240));
        check("+12::::34", &[fixed(TimezoneOffset)], parsed!(offset: 45_240));
        check("+12::34", &[fixed(TimezoneOffset)], parsed!(offset: 45_240));
        check("+12:34:56", &[fixed(TimezoneOffset)], Err(TOO_LONG));
        check("+12:3456", &[fixed(TimezoneOffset)], Err(TOO_LONG));
        check("+1234:56", &[fixed(TimezoneOffset)], Err(TOO_LONG));
        check("+1234:567", &[fixed(TimezoneOffset)], Err(TOO_LONG));
        check("+00:00", &[fixed(TimezoneOffset)], parsed!(offset: 0));
        check("-00:00", &[fixed(TimezoneOffset)], parsed!(offset: 0));
        check("−00:00", &[fixed(TimezoneOffset)], parsed!(offset: 0)); // MINUS SIGN (U+2212)
        check("+00:01", &[fixed(TimezoneOffset)], parsed!(offset: 60));
        check("-00:01", &[fixed(TimezoneOffset)], parsed!(offset: -60));
        check("+00:30", &[fixed(TimezoneOffset)], parsed!(offset: 1_800));
        check("-00:30", &[fixed(TimezoneOffset)], parsed!(offset: -1_800));
        check("+24:00", &[fixed(TimezoneOffset)], parsed!(offset: 86_400));
        check("-24:00", &[fixed(TimezoneOffset)], parsed!(offset: -86_400));
        check("−24:00", &[fixed(TimezoneOffset)], parsed!(offset: -86_400)); // MINUS SIGN (U+2212)
        check("+99:59", &[fixed(TimezoneOffset)], parsed!(offset: 359_940));
        check("-99:59", &[fixed(TimezoneOffset)], parsed!(offset: -359_940));
        check("+00:60", &[fixed(TimezoneOffset)], Err(OUT_OF_RANGE));
        check("+00:99", &[fixed(TimezoneOffset)], Err(OUT_OF_RANGE));
        check("#12:34", &[fixed(TimezoneOffset)], Err(INVALID));
        check("+12:34 ", &[fixed(TimezoneOffset)], Err(TOO_LONG));
        check("+12 34 ", &[fixed(TimezoneOffset)], Err(TOO_LONG));
        check(" +12:34", &[fixed(TimezoneOffset)], parsed!(offset: 45_240));
        check(" -12:34", &[fixed(TimezoneOffset)], parsed!(offset: -45_240));
        check(" −12:34", &[fixed(TimezoneOffset)], parsed!(offset: -45_240)); // MINUS SIGN (U+2212)
        check("  +12:34", &[fixed(TimezoneOffset)], parsed!(offset: 45_240));
        check("  -12:34", &[fixed(TimezoneOffset)], parsed!(offset: -45_240));
        check("\t -12:34", &[fixed(TimezoneOffset)], parsed!(offset: -45_240));
        check("-12: 34", &[fixed(TimezoneOffset)], parsed!(offset: -45_240));
        check("-12 :34", &[fixed(TimezoneOffset)], parsed!(offset: -45_240));
        check("-12 : 34", &[fixed(TimezoneOffset)], parsed!(offset: -45_240));
        check("-12 :  34", &[fixed(TimezoneOffset)], parsed!(offset: -45_240));
        check("-12  : 34", &[fixed(TimezoneOffset)], parsed!(offset: -45_240));
        check("-12:  34", &[fixed(TimezoneOffset)], parsed!(offset: -45_240));
        check("-12  :34", &[fixed(TimezoneOffset)], parsed!(offset: -45_240));
        check("-12  :  34", &[fixed(TimezoneOffset)], parsed!(offset: -45_240));
        check("12:34 ", &[fixed(TimezoneOffset)], Err(INVALID));
        check(" 12:34", &[fixed(TimezoneOffset)], Err(INVALID));
        check("", &[fixed(TimezoneOffset)], Err(TOO_SHORT));
        check("+", &[fixed(TimezoneOffset)], Err(TOO_SHORT));
        check(
            "+12345",
            &[fixed(TimezoneOffset), num(Numeric::Day)],
            parsed!(offset: 45_240, day: 5),
        );
        check(
            "+12:345",
            &[fixed(TimezoneOffset), num(Numeric::Day)],
            parsed!(offset: 45_240, day: 5),
        );
        check("+12:34:", &[fixed(TimezoneOffset), Literal(":")], parsed!(offset: 45_240));
        check("Z12:34", &[fixed(TimezoneOffset)], Err(INVALID));
        check("X12:34", &[fixed(TimezoneOffset)], Err(INVALID));
        check("Z+12:34", &[fixed(TimezoneOffset)], Err(INVALID));
        check("X+12:34", &[fixed(TimezoneOffset)], Err(INVALID));
        check("X−12:34", &[fixed(TimezoneOffset)], Err(INVALID)); // MINUS SIGN (U+2212)
        check("🤠+12:34", &[fixed(TimezoneOffset)], Err(INVALID));
        check("+12:34🤠", &[fixed(TimezoneOffset)], Err(TOO_LONG));
        check("+12:🤠34", &[fixed(TimezoneOffset)], Err(INVALID));
        check("+1234🤠", &[fixed(TimezoneOffset), Literal("🤠")], parsed!(offset: 45_240));
        check("-1234🤠", &[fixed(TimezoneOffset), Literal("🤠")], parsed!(offset: -45_240));
        check("−1234🤠", &[fixed(TimezoneOffset), Literal("🤠")], parsed!(offset: -45_240)); // MINUS SIGN (U+2212)
        check("+12:34🤠", &[fixed(TimezoneOffset), Literal("🤠")], parsed!(offset: 45_240));
        check("-12:34🤠", &[fixed(TimezoneOffset), Literal("🤠")], parsed!(offset: -45_240));
        check("−12:34🤠", &[fixed(TimezoneOffset), Literal("🤠")], parsed!(offset: -45_240)); // MINUS SIGN (U+2212)
        check("🤠+12:34", &[Literal("🤠"), fixed(TimezoneOffset)], parsed!(offset: 45_240));
        check("Z", &[fixed(TimezoneOffset)], Err(INVALID));
        check("A", &[fixed(TimezoneOffset)], Err(INVALID));
        check("PST", &[fixed(TimezoneOffset)], Err(INVALID));
        check("#Z", &[fixed(TimezoneOffset)], Err(INVALID));
        check(":Z", &[fixed(TimezoneOffset)], Err(INVALID));
        check("+Z", &[fixed(TimezoneOffset)], Err(TOO_SHORT));
        check("+:Z", &[fixed(TimezoneOffset)], Err(INVALID));
        check("+Z:", &[fixed(TimezoneOffset)], Err(INVALID));
        check("z", &[fixed(TimezoneOffset)], Err(INVALID));
        check(" :Z", &[fixed(TimezoneOffset)], Err(INVALID));
        check(" Z", &[fixed(TimezoneOffset)], Err(INVALID));
        check(" z", &[fixed(TimezoneOffset)], Err(INVALID));

        // TimezoneOffsetColon
        check("1", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("12", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("123", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("1234", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("12345", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("123456", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("1234567", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("12345678", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("+1", &[fixed(TimezoneOffsetColon)], Err(TOO_SHORT));
        check("+12", &[fixed(TimezoneOffsetColon)], Err(TOO_SHORT));
        check("+123", &[fixed(TimezoneOffsetColon)], Err(TOO_SHORT));
        check("+1234", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("-1234", &[fixed(TimezoneOffsetColon)], parsed!(offset: -45_240));
        check("−1234", &[fixed(TimezoneOffsetColon)], parsed!(offset: -45_240)); // MINUS SIGN (U+2212)
        check("+12345", &[fixed(TimezoneOffsetColon)], Err(TOO_LONG));
        check("+123456", &[fixed(TimezoneOffsetColon)], Err(TOO_LONG));
        check("+1234567", &[fixed(TimezoneOffsetColon)], Err(TOO_LONG));
        check("+12345678", &[fixed(TimezoneOffsetColon)], Err(TOO_LONG));
        check("1:", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("12:", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("12:3", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("12:34", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("12:34:", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("12:34:5", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("12:34:56", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("+1:", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("+12:", &[fixed(TimezoneOffsetColon)], Err(TOO_SHORT));
        check("+12:3", &[fixed(TimezoneOffsetColon)], Err(TOO_SHORT));
        check("+12:34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("-12:34", &[fixed(TimezoneOffsetColon)], parsed!(offset: -45_240));
        check("−12:34", &[fixed(TimezoneOffsetColon)], parsed!(offset: -45_240)); // MINUS SIGN (U+2212)
        check("+12:34:", &[fixed(TimezoneOffsetColon)], Err(TOO_LONG));
        check("+12:34:5", &[fixed(TimezoneOffsetColon)], Err(TOO_LONG));
        check("+12:34:56", &[fixed(TimezoneOffsetColon)], Err(TOO_LONG));
        check("+12:34:56:", &[fixed(TimezoneOffsetColon)], Err(TOO_LONG));
        check("+12:34:56:7", &[fixed(TimezoneOffsetColon)], Err(TOO_LONG));
        check("+12:34:56:78", &[fixed(TimezoneOffsetColon)], Err(TOO_LONG));
        check("+12:3456", &[fixed(TimezoneOffsetColon)], Err(TOO_LONG));
        check("+1234:56", &[fixed(TimezoneOffsetColon)], Err(TOO_LONG));
        check("−12:34", &[fixed(TimezoneOffsetColon)], parsed!(offset: -45_240)); // MINUS SIGN (U+2212)
        check("−12 : 34", &[fixed(TimezoneOffsetColon)], parsed!(offset: -45_240)); // MINUS SIGN (U+2212)
        check("+12 :34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("+12: 34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("+12 34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("+12: 34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("+12 :34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("+12 : 34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("-12 : 34", &[fixed(TimezoneOffsetColon)], parsed!(offset: -45_240));
        check("+12  : 34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("+12 :  34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("+12  :  34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("+12::34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("+12: :34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("+12:::34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("+12::::34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("+12::34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("#1234", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("#12:34", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("+12:34 ", &[fixed(TimezoneOffsetColon)], Err(TOO_LONG));
        check(" +12:34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("\t+12:34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("\t\t+12:34", &[fixed(TimezoneOffsetColon)], parsed!(offset: 45_240));
        check("12:34 ", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check(" 12:34", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("", &[fixed(TimezoneOffsetColon)], Err(TOO_SHORT));
        check("+", &[fixed(TimezoneOffsetColon)], Err(TOO_SHORT));
        check(":", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check(
            "+12345",
            &[fixed(TimezoneOffsetColon), num(Numeric::Day)],
            parsed!(offset: 45_240, day: 5),
        );
        check(
            "+12:345",
            &[fixed(TimezoneOffsetColon), num(Numeric::Day)],
            parsed!(offset: 45_240, day: 5),
        );
        check("+12:34:", &[fixed(TimezoneOffsetColon), Literal(":")], parsed!(offset: 45_240));
        check("Z", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("A", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("PST", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("#Z", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check(":Z", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("+Z", &[fixed(TimezoneOffsetColon)], Err(TOO_SHORT));
        check("+:Z", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("+Z:", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check("z", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check(" :Z", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check(" Z", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        check(" z", &[fixed(TimezoneOffsetColon)], Err(INVALID));
        // testing `TimezoneOffsetColon` also tests same path as `TimezoneOffsetDoubleColon`
        // and `TimezoneOffsetTripleColon` for function `parse_internal`.
        // No need for separate tests for `TimezoneOffsetDoubleColon` and
        // `TimezoneOffsetTripleColon`.

        // TimezoneOffsetZ
        check("1", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("12", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("123", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("1234", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("12345", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("123456", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("1234567", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("12345678", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("+1", &[fixed(TimezoneOffsetZ)], Err(TOO_SHORT));
        check("+12", &[fixed(TimezoneOffsetZ)], Err(TOO_SHORT));
        check("+123", &[fixed(TimezoneOffsetZ)], Err(TOO_SHORT));
        check("+1234", &[fixed(TimezoneOffsetZ)], parsed!(offset: 45_240));
        check("-1234", &[fixed(TimezoneOffsetZ)], parsed!(offset: -45_240));
        check("−1234", &[fixed(TimezoneOffsetZ)], parsed!(offset: -45_240)); // MINUS SIGN (U+2212)
        check("+12345", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check("+123456", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check("+1234567", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check("+12345678", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check("1:", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("12:", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("12:3", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("12:34", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("12:34:", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("12:34:5", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("12:34:56", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("+1:", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("+12:", &[fixed(TimezoneOffsetZ)], Err(TOO_SHORT));
        check("+12:3", &[fixed(TimezoneOffsetZ)], Err(TOO_SHORT));
        check("+12:34", &[fixed(TimezoneOffsetZ)], parsed!(offset: 45_240));
        check("-12:34", &[fixed(TimezoneOffsetZ)], parsed!(offset: -45_240));
        check("−12:34", &[fixed(TimezoneOffsetZ)], parsed!(offset: -45_240)); // MINUS SIGN (U+2212)
        check("+12:34:", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check("+12:34:5", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check("+12:34:56", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check("+12:34:56:", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check("+12:34:56:7", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check("+12:34:56:78", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check("+12::34", &[fixed(TimezoneOffsetZ)], parsed!(offset: 45_240));
        check("+12:3456", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check("+1234:56", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check("+12 34", &[fixed(TimezoneOffsetZ)], parsed!(offset: 45_240));
        check("+12  34", &[fixed(TimezoneOffsetZ)], parsed!(offset: 45_240));
        check("+12: 34", &[fixed(TimezoneOffsetZ)], parsed!(offset: 45_240));
        check("+12 :34", &[fixed(TimezoneOffsetZ)], parsed!(offset: 45_240));
        check("+12 : 34", &[fixed(TimezoneOffsetZ)], parsed!(offset: 45_240));
        check("+12  : 34", &[fixed(TimezoneOffsetZ)], parsed!(offset: 45_240));
        check("+12 :  34", &[fixed(TimezoneOffsetZ)], parsed!(offset: 45_240));
        check("+12  :  34", &[fixed(TimezoneOffsetZ)], parsed!(offset: 45_240));
        check("12:34 ", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check(" 12:34", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("+12:34 ", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check("+12 34 ", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check(" +12:34", &[fixed(TimezoneOffsetZ)], parsed!(offset: 45_240));
        check(
            "+12345",
            &[fixed(TimezoneOffsetZ), num(Numeric::Day)],
            parsed!(offset: 45_240, day: 5),
        );
        check(
            "+12:345",
            &[fixed(TimezoneOffsetZ), num(Numeric::Day)],
            parsed!(offset: 45_240, day: 5),
        );
        check("+12:34:", &[fixed(TimezoneOffsetZ), Literal(":")], parsed!(offset: 45_240));
        check("Z12:34", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check("X12:34", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("Z", &[fixed(TimezoneOffsetZ)], parsed!(offset: 0));
        check("z", &[fixed(TimezoneOffsetZ)], parsed!(offset: 0));
        check(" Z", &[fixed(TimezoneOffsetZ)], parsed!(offset: 0));
        check(" z", &[fixed(TimezoneOffsetZ)], parsed!(offset: 0));
        check("\u{0363}Z", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("Z ", &[fixed(TimezoneOffsetZ)], Err(TOO_LONG));
        check("A", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("PST", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("#Z", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check(":Z", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check(":z", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("+Z", &[fixed(TimezoneOffsetZ)], Err(TOO_SHORT));
        check("-Z", &[fixed(TimezoneOffsetZ)], Err(TOO_SHORT));
        check("+A", &[fixed(TimezoneOffsetZ)], Err(TOO_SHORT));
        check("+🙃", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("+Z:", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check(" :Z", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check(" +Z", &[fixed(TimezoneOffsetZ)], Err(TOO_SHORT));
        check(" -Z", &[fixed(TimezoneOffsetZ)], Err(TOO_SHORT));
        check("+:Z", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("Y", &[fixed(TimezoneOffsetZ)], Err(INVALID));
        check("Zulu", &[fixed(TimezoneOffsetZ), Literal("ulu")], parsed!(offset: 0));
        check("zulu", &[fixed(TimezoneOffsetZ), Literal("ulu")], parsed!(offset: 0));
        check("+1234ulu", &[fixed(TimezoneOffsetZ), Literal("ulu")], parsed!(offset: 45_240));
        check("+12:34ulu", &[fixed(TimezoneOffsetZ), Literal("ulu")], parsed!(offset: 45_240));
        // Testing `TimezoneOffsetZ` also tests same path as `TimezoneOffsetColonZ`
        // in function `parse_internal`.
        // No need for separate tests for `TimezoneOffsetColonZ`.

        // TimezoneOffsetPermissive
        check("1", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("12", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("123", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("1234", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("12345", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("123456", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("1234567", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("12345678", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("+1", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_SHORT));
        check("+12", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 43_200));
        check("+123", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_SHORT));
        check("+1234", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("-1234", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: -45_240));
        check("−1234", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: -45_240)); // MINUS SIGN (U+2212)
        check("+12345", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_LONG));
        check("+123456", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_LONG));
        check("+1234567", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_LONG));
        check("+12345678", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_LONG));
        check("1:", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("12:", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("12:3", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("12:34", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("12:34:", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("12:34:5", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("12:34:56", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("+1:", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("+12:", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 43_200));
        check("+12:3", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_SHORT));
        check("+12:34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("-12:34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: -45_240));
        check("−12:34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: -45_240)); // MINUS SIGN (U+2212)
        check("+12:34:", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_LONG));
        check("+12:34:5", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_LONG));
        check("+12:34:56", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_LONG));
        check("+12:34:56:", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_LONG));
        check("+12:34:56:7", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_LONG));
        check("+12:34:56:78", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_LONG));
        check("+12 34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12  34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12 :34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12: 34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12 : 34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12  :34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12:  34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12  :  34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12::34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12 ::34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12: :34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12:: 34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12  ::34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12:  :34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12::  34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12:::34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("+12::::34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check("12:34 ", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check(" 12:34", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("+12:34 ", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_LONG));
        check(" +12:34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 45_240));
        check(" -12:34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: -45_240));
        check(" −12:34", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: -45_240)); // MINUS SIGN (U+2212)
        check(
            "+12345",
            &[internal_fixed(TimezoneOffsetPermissive), num(Numeric::Day)],
            parsed!(offset: 45_240, day: 5),
        );
        check(
            "+12:345",
            &[internal_fixed(TimezoneOffsetPermissive), num(Numeric::Day)],
            parsed!(offset: 45_240, day: 5),
        );
        check(
            "+12:34:",
            &[internal_fixed(TimezoneOffsetPermissive), Literal(":")],
            parsed!(offset: 45_240),
        );
        check("🤠+12:34", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("+12:34🤠", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_LONG));
        check("+12:🤠34", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check(
            "+12:34🤠",
            &[internal_fixed(TimezoneOffsetPermissive), Literal("🤠")],
            parsed!(offset: 45_240),
        );
        check(
            "🤠+12:34",
            &[Literal("🤠"), internal_fixed(TimezoneOffsetPermissive)],
            parsed!(offset: 45_240),
        );
        check("Z", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 0));
        check("A", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("PST", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("z", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 0));
        check(" Z", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 0));
        check(" z", &[internal_fixed(TimezoneOffsetPermissive)], parsed!(offset: 0));
        check("Z ", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_LONG));
        check("#Z", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check(":Z", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check(":z", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("+Z", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_SHORT));
        check("-Z", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_SHORT));
        check("+A", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_SHORT));
        check("+PST", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("+🙃", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("+Z:", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check(" :Z", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check(" +Z", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_SHORT));
        check(" -Z", &[internal_fixed(TimezoneOffsetPermissive)], Err(TOO_SHORT));
        check("+:Z", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));
        check("Y", &[internal_fixed(TimezoneOffsetPermissive)], Err(INVALID));

        // TimezoneName
        check("CEST", &[fixed(TimezoneName)], parsed!());
        check("cest", &[fixed(TimezoneName)], parsed!()); // lowercase
        check("XXXXXXXX", &[fixed(TimezoneName)], parsed!()); // not a real timezone name
        check("!!!!", &[fixed(TimezoneName)], parsed!()); // not a real timezone name!
        check("CEST 5", &[fixed(TimezoneName), Literal(" "), num(Numeric::Day)], parsed!(day: 5));
        check("CEST ", &[fixed(TimezoneName)], Err(TOO_LONG));
        check(" CEST", &[fixed(TimezoneName)], Err(TOO_LONG));
        check("CE ST", &[fixed(TimezoneName)], Err(TOO_LONG));
    }

    #[test]
    #[rustfmt::skip]
    fn test_parse_practical_examples() {
        use crate::format::InternalInternal::*;
        use crate::format::Item::{Literal, Space};
        use crate::format::Numeric::*;

        // some practical examples
        check(
            "2015-02-04T14:37:05+09:00",
            &[
                num(Year), Literal("-"), num(Month), Literal("-"), num(Day), Literal("T"),
                num(Hour), Literal(":"), num(Minute), Literal(":"), num(Second),
                fixed(Fixed::TimezoneOffset),
            ],
            parsed!(
                year: 2015, month: 2, day: 4, hour_div_12: 1, hour_mod_12: 2, minute: 37,
                second: 5, offset: 32400
            ),
        );
        check(
            "2015-02-04T14:37:05-09:00",
            &[
                num(Year), Literal("-"), num(Month), Literal("-"), num(Day), Literal("T"),
                num(Hour), Literal(":"), num(Minute), Literal(":"), num(Second),
                fixed(Fixed::TimezoneOffset),
            ],
            parsed!(
                year: 2015, month: 2, day: 4, hour_div_12: 1, hour_mod_12: 2, minute: 37,
                second: 5, offset: -32400
            ),
        );
        check(
            "2015-02-04T14:37:05−09:00", // timezone offset using MINUS SIGN (U+2212)
            &[
                num(Year), Literal("-"), num(Month), Literal("-"), num(Day), Literal("T"),
                num(Hour), Literal(":"), num(Minute), Literal(":"), num(Second),
                fixed(Fixed::TimezoneOffset)
            ],
            parsed!(
                year: 2015, month: 2, day: 4, hour_div_12: 1, hour_mod_12: 2, minute: 37,
                second: 5, offset: -32400
            ),
        );
        check(
            "20150204143705567",
            &[
                num(Year), num(Month), num(Day), num(Hour), num(Minute), num(Second),
                internal_fixed(Nanosecond3NoDot)
            ],
            parsed!(
                year: 2015, month: 2, day: 4, hour_div_12: 1, hour_mod_12: 2, minute: 37,
                second: 5, nanosecond: 567000000
            ),
        );
        check(
            "Mon, 10 Jun 2013 09:32:37 GMT",
            &[
                fixed(Fixed::ShortWeekdayName), Literal(","), Space(" "), num(Day), Space(" "),
                fixed(Fixed::ShortMonthName), Space(" "), num(Year), Space(" "), num(Hour),
                Literal(":"), num(Minute), Literal(":"), num(Second), Space(" "), Literal("GMT")
            ],
            parsed!(
                year: 2013, month: 6, day: 10, weekday: Weekday::Mon,
                hour_div_12: 0, hour_mod_12: 9, minute: 32, second: 37
            ),
        );
        check(
            "🤠Mon, 10 Jun🤠2013 09:32:37  GMT🤠",
            &[
                Literal("🤠"), fixed(Fixed::ShortWeekdayName), Literal(","), Space(" "), num(Day),
                Space(" "), fixed(Fixed::ShortMonthName), Literal("🤠"), num(Year), Space(" "),
                num(Hour), Literal(":"), num(Minute), Literal(":"), num(Second), Space("  "),
                Literal("GMT"), Literal("🤠")
            ],
            parsed!(
                year: 2013, month: 6, day: 10, weekday: Weekday::Mon,
                hour_div_12: 0, hour_mod_12: 9, minute: 32, second: 37
            ),
        );
        check(
            "Sun Aug 02 13:39:15 CEST 2020",
            &[
                fixed(Fixed::ShortWeekdayName), Space(" "), fixed(Fixed::ShortMonthName),
                Space(" "), num(Day), Space(" "), num(Hour), Literal(":"), num(Minute),
                Literal(":"), num(Second), Space(" "), fixed(Fixed::TimezoneName), Space(" "),
                num(Year)
            ],
            parsed!(
                year: 2020, month: 8, day: 2, weekday: Weekday::Sun,
                hour_div_12: 1, hour_mod_12: 1, minute: 39, second: 15
            ),
        );
        check(
            "20060102150405",
            &[num(Year), num(Month), num(Day), num(Hour), num(Minute), num(Second)],
            parsed!(
                year: 2006, month: 1, day: 2, hour_div_12: 1, hour_mod_12: 3, minute: 4, second: 5
            ),
        );
        check(
            "3:14PM",
            &[num(Hour12), Literal(":"), num(Minute), fixed(Fixed::LowerAmPm)],
            parsed!(hour_div_12: 1, hour_mod_12: 3, minute: 14),
        );
        check(
            "12345678901234.56789",
            &[num(Timestamp), Literal("."), num(Nanosecond)],
            parsed!(nanosecond: 56_789, timestamp: 12_345_678_901_234),
        );
        check(
            "12345678901234.56789",
            &[num(Timestamp), fixed(Fixed::Nanosecond)],
            parsed!(nanosecond: 567_890_000, timestamp: 12_345_678_901_234),
        );

        // docstring examples from `impl str::FromStr`
        check(
            "2000-01-02T03:04:05Z",
            &[
                num(Year), Literal("-"), num(Month), Literal("-"), num(Day), Literal("T"),
                num(Hour), Literal(":"), num(Minute), Literal(":"), num(Second),
                internal_fixed(TimezoneOffsetPermissive)
            ],
            parsed!(
                year: 2000, month: 1, day: 2, hour_div_12: 0, hour_mod_12: 3, minute: 4, second: 5,
                offset: 0
            ),
        );
        check(
            "2000-01-02 03:04:05Z",
            &[
                num(Year), Literal("-"), num(Month), Literal("-"), num(Day), Space(" "),
                num(Hour), Literal(":"), num(Minute), Literal(":"), num(Second),
                internal_fixed(TimezoneOffsetPermissive)
            ],
            parsed!(
                year: 2000, month: 1, day: 2, hour_div_12: 0, hour_mod_12: 3, minute: 4, second: 5,
                offset: 0
            ),
        );
    }

    #[track_caller]
    fn parses(s: &str, items: &[Item]) {
        let mut parsed = Parsed::new();
        assert!(parse(&mut parsed, s, items.iter()).is_ok());
    }

    #[track_caller]
    fn check(s: &str, items: &[Item], expected: ParseResult<Parsed>) {
        let mut parsed = Parsed::new();
        let result = parse(&mut parsed, s, items.iter());
        let parsed = result.map(|_| parsed);
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_rfc2822() {
        let ymd_hmsn = |y, m, d, h, n, s, nano, off| {
            FixedOffset::east_opt(off * 60 * 60)
                .unwrap()
                .with_ymd_and_hms(y, m, d, h, n, s)
                .unwrap()
                .with_nanosecond(nano)
                .unwrap()
        };

        // Test data - (input, Ok(expected result) or Err(error code))
        let testdates = [
            ("Tue, 20 Jan 2015 17:35:20 -0800", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -8))), // normal case
            ("Fri,  2 Jan 2015 17:35:20 -0800", Ok(ymd_hmsn(2015, 1, 2, 17, 35, 20, 0, -8))), // folding whitespace
            ("Fri, 02 Jan 2015 17:35:20 -0800", Ok(ymd_hmsn(2015, 1, 2, 17, 35, 20, 0, -8))), // leading zero
            ("Tue, 20 Jan 2015 17:35:20 -0800 (UTC)", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -8))), // trailing comment
            (
                r"Tue, 20 Jan 2015 17:35:20 -0800 ( (UTC ) (\( (a)\(( \t ) ) \\( \) ))",
                Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -8)),
            ), // complex trailing comment
            (r"Tue, 20 Jan 2015 17:35:20 -0800 (UTC\)", Err(TOO_LONG)), // incorrect comment, not enough closing parentheses
            (
                "Tue, 20 Jan 2015 17:35:20 -0800 (UTC)\t \r\n(Anothercomment)",
                Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -8)),
            ), // multiple comments
            ("Tue, 20 Jan 2015 17:35:20 -0800 (UTC) ", Err(TOO_LONG)), // trailing whitespace after comment
            ("20 Jan 2015 17:35:20 -0800", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -8))), // no day of week
            ("20 JAN 2015 17:35:20 -0800", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -8))), // upper case month
            ("Tue, 20 Jan 2015 17:35 -0800", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 0, 0, -8))), // no second
            ("11 Sep 2001 09:45:00 +0000", Ok(ymd_hmsn(2001, 9, 11, 9, 45, 0, 0, 0))),
            ("11 Sep 2001 09:45:00 EST", Ok(ymd_hmsn(2001, 9, 11, 9, 45, 0, 0, -5))),
            ("11 Sep 2001 09:45:00 GMT", Ok(ymd_hmsn(2001, 9, 11, 9, 45, 0, 0, 0))),
            ("30 Feb 2015 17:35:20 -0800", Err(OUT_OF_RANGE)), // bad day of month
            ("Tue, 20 Jan 2015", Err(TOO_SHORT)),              // omitted fields
            ("Tue, 20 Avr 2015 17:35:20 -0800", Err(INVALID)), // bad month name
            ("Tue, 20 Jan 2015 25:35:20 -0800", Err(OUT_OF_RANGE)), // bad hour
            ("Tue, 20 Jan 2015 7:35:20 -0800", Err(INVALID)),  // bad # of digits in hour
            ("Tue, 20 Jan 2015 17:65:20 -0800", Err(OUT_OF_RANGE)), // bad minute
            ("Tue, 20 Jan 2015 17:35:90 -0800", Err(OUT_OF_RANGE)), // bad second
            ("Tue, 20 Jan 2015 17:35:20 -0890", Err(OUT_OF_RANGE)), // bad offset
            ("6 Jun 1944 04:00:00Z", Err(INVALID)),            // bad offset (zulu not allowed)
            // named timezones that have specific timezone offsets
            // see https://www.rfc-editor.org/rfc/rfc2822#section-4.3
            ("Tue, 20 Jan 2015 17:35:20 GMT", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, 0))),
            ("Tue, 20 Jan 2015 17:35:20 UT", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, 0))),
            ("Tue, 20 Jan 2015 17:35:20 ut", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, 0))),
            ("Tue, 20 Jan 2015 17:35:20 EDT", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -4))),
            ("Tue, 20 Jan 2015 17:35:20 EST", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -5))),
            ("Tue, 20 Jan 2015 17:35:20 CDT", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -5))),
            ("Tue, 20 Jan 2015 17:35:20 CST", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -6))),
            ("Tue, 20 Jan 2015 17:35:20 MDT", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -6))),
            ("Tue, 20 Jan 2015 17:35:20 MST", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -7))),
            ("Tue, 20 Jan 2015 17:35:20 PDT", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -7))),
            ("Tue, 20 Jan 2015 17:35:20 PST", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -8))),
            ("Tue, 20 Jan 2015 17:35:20 pst", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -8))),
            // named single-letter military timezones must fallback to +0000
            ("Tue, 20 Jan 2015 17:35:20 Z", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, 0))),
            ("Tue, 20 Jan 2015 17:35:20 A", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, 0))),
            ("Tue, 20 Jan 2015 17:35:20 a", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, 0))),
            ("Tue, 20 Jan 2015 17:35:20 K", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, 0))),
            ("Tue, 20 Jan 2015 17:35:20 k", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, 0))),
            // named single-letter timezone "J" is specifically not valid
            ("Tue, 20 Jan 2015 17:35:20 J", Err(INVALID)),
            ("Tue, 20 Jan 2015 17:35:20 -0890", Err(OUT_OF_RANGE)), // bad offset minutes
            ("Tue, 20 Jan 2015 17:35:20Z", Err(INVALID)),           // bad offset: zulu not allowed
            ("Tue, 20 Jan 2015 17:35:20 Zulu", Err(INVALID)),       // bad offset: zulu not allowed
            ("Tue, 20 Jan 2015 17:35:20 ZULU", Err(INVALID)),       // bad offset: zulu not allowed
            ("Tue, 20 Jan 2015 17:35:20 −0800", Err(INVALID)), // bad offset: timezone offset using MINUS SIGN (U+2212), not specified for RFC 2822
            ("Tue, 20 Jan 2015 17:35:20 0800", Err(INVALID)),  // missing offset sign
            ("Tue, 20 Jan 2015 17:35:20 HAS", Err(INVALID)),   // bad named timezone
            ("Tue, 20 Jan 2015😈17:35:20 -0800", Err(INVALID)), // bad character!
        ];

        fn rfc2822_to_datetime(date: &str) -> ParseResult<DateTime<FixedOffset>> {
            let mut parsed = Parsed::new();
            parse(&mut parsed, date, [Item::Fixed(Fixed::RFC2822)].iter())?;
            parsed.to_datetime()
        }

        // Test against test data above
        for &(date, checkdate) in testdates.iter() {
            #[cfg(feature = "std")]
            eprintln!("Test input: {date:?}\n    Expect: {checkdate:?}");
            let dt = rfc2822_to_datetime(date); // parse a date
            if dt != checkdate {
                // check for expected result
                panic!(
                    "Date conversion failed for {date}\nReceived: {dt:?}\nExpected: {checkdate:?}"
                );
            }
        }
    }

    #[test]
    fn parse_rfc850() {
        static RFC850_FMT: &str = "%A, %d-%b-%y %T GMT";

        let dt = Utc.with_ymd_and_hms(1994, 11, 6, 8, 49, 37).unwrap();

        // Check that the format is what we expect
        #[cfg(feature = "alloc")]
        assert_eq!(dt.format(RFC850_FMT).to_string(), "Sunday, 06-Nov-94 08:49:37 GMT");

        // Check that it parses correctly
        assert_eq!(
            NaiveDateTime::parse_from_str("Sunday, 06-Nov-94 08:49:37 GMT", RFC850_FMT),
            Ok(dt.naive_utc())
        );

        // Check that the rest of the weekdays parse correctly (this test originally failed because
        // Sunday parsed incorrectly).
        let testdates = [
            (
                Utc.with_ymd_and_hms(1994, 11, 7, 8, 49, 37).unwrap(),
                "Monday, 07-Nov-94 08:49:37 GMT",
            ),
            (
                Utc.with_ymd_and_hms(1994, 11, 8, 8, 49, 37).unwrap(),
                "Tuesday, 08-Nov-94 08:49:37 GMT",
            ),
            (
                Utc.with_ymd_and_hms(1994, 11, 9, 8, 49, 37).unwrap(),
                "Wednesday, 09-Nov-94 08:49:37 GMT",
            ),
            (
                Utc.with_ymd_and_hms(1994, 11, 10, 8, 49, 37).unwrap(),
                "Thursday, 10-Nov-94 08:49:37 GMT",
            ),
            (
                Utc.with_ymd_and_hms(1994, 11, 11, 8, 49, 37).unwrap(),
                "Friday, 11-Nov-94 08:49:37 GMT",
            ),
            (
                Utc.with_ymd_and_hms(1994, 11, 12, 8, 49, 37).unwrap(),
                "Saturday, 12-Nov-94 08:49:37 GMT",
            ),
        ];

        for val in &testdates {
            assert_eq!(NaiveDateTime::parse_from_str(val.1, RFC850_FMT), Ok(val.0.naive_utc()));
        }

        let test_dates_fail = [
            "Saturday, 12-Nov-94 08:49:37",
            "Saturday, 12-Nov-94 08:49:37 Z",
            "Saturday, 12-Nov-94 08:49:37 GMTTTT",
            "Saturday, 12-Nov-94 08:49:37 gmt",
            "Saturday, 12-Nov-94 08:49:37 +08:00",
            "Caturday, 12-Nov-94 08:49:37 GMT",
            "Saturday, 99-Nov-94 08:49:37 GMT",
            "Saturday, 12-Nov-2000 08:49:37 GMT",
            "Saturday, 12-Mop-94 08:49:37 GMT",
            "Saturday, 12-Nov-94 28:49:37 GMT",
            "Saturday, 12-Nov-94 08:99:37 GMT",
            "Saturday, 12-Nov-94 08:49:99 GMT",
        ];

        for val in &test_dates_fail {
            assert!(NaiveDateTime::parse_from_str(val, RFC850_FMT).is_err());
        }
    }

    #[test]
    fn test_rfc3339() {
        let ymd_hmsn = |y, m, d, h, n, s, nano, off| {
            FixedOffset::east_opt(off * 60 * 60)
                .unwrap()
                .with_ymd_and_hms(y, m, d, h, n, s)
                .unwrap()
                .with_nanosecond(nano)
                .unwrap()
        };

        // Test data - (input, Ok(expected result) or Err(error code))
        let testdates = [
            ("2015-01-20T17:35:20-08:00", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -8))), // normal case
            ("2015-01-20T17:35:20−08:00", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -8))), // normal case with MINUS SIGN (U+2212)
            ("1944-06-06T04:04:00Z", Ok(ymd_hmsn(1944, 6, 6, 4, 4, 0, 0, 0))),           // D-day
            ("2001-09-11T09:45:00-08:00", Ok(ymd_hmsn(2001, 9, 11, 9, 45, 0, 0, -8))),
            ("2015-01-20T17:35:20.001-08:00", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 1_000_000, -8))),
            ("2015-01-20T17:35:20.001−08:00", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 1_000_000, -8))), // with MINUS SIGN (U+2212)
            ("2015-01-20T17:35:20.000031-08:00", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 31_000, -8))),
            ("2015-01-20T17:35:20.000000004-08:00", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 4, -8))),
            ("2015-01-20T17:35:20.000000004−08:00", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 4, -8))), // with MINUS SIGN (U+2212)
            (
                "2015-01-20T17:35:20.000000000452-08:00",
                Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -8)),
            ), // too small
            (
                "2015-01-20T17:35:20.000000000452−08:00",
                Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -8)),
            ), // too small with MINUS SIGN (U+2212)
            ("2015-01-20 17:35:20-08:00", Ok(ymd_hmsn(2015, 1, 20, 17, 35, 20, 0, -8))), // without 'T'
            ("2015/01/20T17:35:20.001-08:00", Err(INVALID)), // wrong separator char YMD
            ("2015-01-20T17-35-20.001-08:00", Err(INVALID)), // wrong separator char HMS
            ("-01-20T17:35:20-08:00", Err(INVALID)),         // missing year
            ("99-01-20T17:35:20-08:00", Err(INVALID)),       // bad year format
            ("99999-01-20T17:35:20-08:00", Err(INVALID)),    // bad year value
            ("-2000-01-20T17:35:20-08:00", Err(INVALID)),    // bad year value
            ("2015-02-30T17:35:20-08:00", Err(OUT_OF_RANGE)), // bad day of month value
            ("2015-01-20T25:35:20-08:00", Err(OUT_OF_RANGE)), // bad hour value
            ("2015-01-20T17:65:20-08:00", Err(OUT_OF_RANGE)), // bad minute value
            ("2015-01-20T17:35:90-08:00", Err(OUT_OF_RANGE)), // bad second value
            ("2015-01-20T17:35:20-24:00", Err(OUT_OF_RANGE)), // bad offset value
            ("15-01-20T17:35:20-08:00", Err(INVALID)),       // bad year format
            ("15-01-20T17:35:20-08:00:00", Err(INVALID)),    // bad year format, bad offset format
            ("2015-01-20T17:35:2008:00", Err(INVALID)),      // missing offset sign
            ("2015-01-20T17:35:20 08:00", Err(INVALID)),     // missing offset sign
            ("2015-01-20T17:35:20Zulu", Err(TOO_LONG)),      // bad offset format
            ("2015-01-20T17:35:20 Zulu", Err(INVALID)),      // bad offset format
            ("2015-01-20T17:35:20GMT", Err(INVALID)),        // bad offset format
            ("2015-01-20T17:35:20 GMT", Err(INVALID)),       // bad offset format
            ("2015-01-20T17:35:20+GMT", Err(INVALID)),       // bad offset format
            ("2015-01-20T17:35:20++08:00", Err(INVALID)),    // bad offset format
            ("2015-01-20T17:35:20--08:00", Err(INVALID)),    // bad offset format
            ("2015-01-20T17:35:20−−08:00", Err(INVALID)), // bad offset format with MINUS SIGN (U+2212)
            ("2015-01-20T17:35:20±08:00", Err(INVALID)),  // bad offset sign
            ("2015-01-20T17:35:20-08-00", Err(INVALID)),  // bad offset separator
            ("2015-01-20T17:35:20-08;00", Err(INVALID)),  // bad offset separator
            ("2015-01-20T17:35:20-0800", Err(INVALID)),   // bad offset separator
            ("2015-01-20T17:35:20-08:0", Err(TOO_SHORT)), // bad offset minutes
            ("2015-01-20T17:35:20-08:AA", Err(INVALID)),  // bad offset minutes
            ("2015-01-20T17:35:20-08:ZZ", Err(INVALID)),  // bad offset minutes
            ("2015-01-20T17:35:20.001-08 : 00", Err(INVALID)), // bad offset separator
            ("2015-01-20T17:35:20-08:00:00", Err(TOO_LONG)), // bad offset format
            ("2015-01-20T17:35:20+08:", Err(TOO_SHORT)),  // bad offset format
            ("2015-01-20T17:35:20-08:", Err(TOO_SHORT)),  // bad offset format
            ("2015-01-20T17:35:20−08:", Err(TOO_SHORT)), // bad offset format with MINUS SIGN (U+2212)
            ("2015-01-20T17:35:20-08", Err(TOO_SHORT)),  // bad offset format
            ("2015-01-20T", Err(TOO_SHORT)),             // missing HMS
            ("2015-01-20T00:00:1", Err(TOO_SHORT)),      // missing complete S
            ("2015-01-20T00:00:1-08:00", Err(INVALID)),  // missing complete S
        ];

        // Test against test data above
        for &(date, checkdate) in testdates.iter() {
            let dt = DateTime::<FixedOffset>::parse_from_rfc3339(date);
            if dt != checkdate {
                // check for expected result
                panic!(
                    "Date conversion failed for {date}\nReceived: {dt:?}\nExpected: {checkdate:?}"
                );
            }
        }
    }

    #[test]
    fn test_issue_1010() {
        let dt = crate::NaiveDateTime::parse_from_str(
            "\u{c}SUN\u{e}\u{3000}\0m@J\u{3000}\0\u{3000}\0m\u{c}!\u{c}\u{b}\u{c}\u{c}\u{c}\u{c}%A\u{c}\u{b}\0SU\u{c}\u{c}",
            "\u{c}\u{c}%A\u{c}\u{b}\0SUN\u{c}\u{c}\u{c}SUNN\u{c}\u{c}\u{c}SUN\u{c}\u{c}!\u{c}\u{b}\u{c}\u{c}\u{c}\u{c}%A\u{c}\u{b}%a",
        );
        assert_eq!(dt, Err(ParseError(ParseErrorKind::Invalid)));
    }
}
