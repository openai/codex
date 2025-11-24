use core::fmt::Write;

use crate::{
    civil::Weekday,
    error::{err, ErrorContext},
    fmt::{
        offset,
        strtime::{BrokenDownTime, Extension, Flag, Meridiem},
        Parsed,
    },
    tz::Offset,
    util::{
        escape, parse,
        rangeint::{ri8, RFrom},
        t::{self, C},
    },
    Error, Timestamp,
};

pub(super) struct Parser<'f, 'i, 't> {
    pub(super) fmt: &'f [u8],
    pub(super) inp: &'i [u8],
    pub(super) tm: &'t mut BrokenDownTime,
}

impl<'f, 'i, 't> Parser<'f, 'i, 't> {
    pub(super) fn parse(&mut self) -> Result<(), Error> {
        while !self.fmt.is_empty() {
            if self.f() != b'%' {
                self.parse_literal()?;
                continue;
            }
            if !self.bump_fmt() {
                return Err(err!(
                    "invalid format string, expected byte after '%', \
                     but found end of format string",
                ));
            }
            // We don't check this for `%.` since that currently always
            // must lead to `%.f` which can actually parse the empty string!
            if self.inp.is_empty() && self.f() != b'.' {
                return Err(err!(
                    "expected non-empty input for directive %{directive}, \
                     but found end of input",
                    directive = escape::Byte(self.f()),
                ));
            }
            // Parse extensions like padding/case options and padding width.
            let ext = self.parse_extension()?;
            match self.f() {
                b'%' => self.parse_percent().context("%% failed")?,
                b'A' => self.parse_weekday_full().context("%A failed")?,
                b'a' => self.parse_weekday_abbrev().context("%a failed")?,
                b'B' => self.parse_month_name_full().context("%B failed")?,
                b'b' => self.parse_month_name_abbrev().context("%b failed")?,
                b'C' => self.parse_century(ext).context("%C failed")?,
                b'D' => self.parse_american_date().context("%D failed")?,
                b'd' => self.parse_day(ext).context("%d failed")?,
                b'e' => self.parse_day(ext).context("%e failed")?,
                b'F' => self.parse_iso_date().context("%F failed")?,
                b'f' => self.parse_fractional(ext).context("%f failed")?,
                b'G' => self.parse_iso_week_year(ext).context("%G failed")?,
                b'g' => self.parse_iso_week_year2(ext).context("%g failed")?,
                b'H' => self.parse_hour24(ext).context("%H failed")?,
                b'h' => self.parse_month_name_abbrev().context("%h failed")?,
                b'I' => self.parse_hour12(ext).context("%I failed")?,
                b'j' => self.parse_day_of_year(ext).context("%j failed")?,
                b'k' => self.parse_hour24(ext).context("%k failed")?,
                b'l' => self.parse_hour12(ext).context("%l failed")?,
                b'M' => self.parse_minute(ext).context("%M failed")?,
                b'm' => self.parse_month(ext).context("%m failed")?,
                b'N' => self.parse_fractional(ext).context("%N failed")?,
                b'n' => self.parse_whitespace().context("%n failed")?,
                b'P' => self.parse_ampm().context("%P failed")?,
                b'p' => self.parse_ampm().context("%p failed")?,
                b'Q' => match ext.colons {
                    0 => self.parse_iana_nocolon().context("%Q failed")?,
                    1 => self.parse_iana_colon().context("%:Q failed")?,
                    _ => {
                        return Err(err!(
                            "invalid number of `:` in `%Q` directive"
                        ))
                    }
                },
                b'R' => self.parse_clock_nosecs().context("%R failed")?,
                b'S' => self.parse_second(ext).context("%S failed")?,
                b's' => self.parse_timestamp(ext).context("%s failed")?,
                b'T' => self.parse_clock_secs().context("%T failed")?,
                b't' => self.parse_whitespace().context("%t failed")?,
                b'U' => self.parse_week_sun(ext).context("%U failed")?,
                b'u' => self.parse_weekday_mon(ext).context("%u failed")?,
                b'V' => self.parse_week_iso(ext).context("%V failed")?,
                b'W' => self.parse_week_mon(ext).context("%W failed")?,
                b'w' => self.parse_weekday_sun(ext).context("%w failed")?,
                b'Y' => self.parse_year(ext).context("%Y failed")?,
                b'y' => self.parse_year2(ext).context("%y failed")?,
                b'z' => match ext.colons {
                    0 => self.parse_offset_nocolon().context("%z failed")?,
                    1 => self.parse_offset_colon().context("%:z failed")?,
                    2 => self.parse_offset_colon2().context("%::z failed")?,
                    3 => self.parse_offset_colon3().context("%:::z failed")?,
                    _ => {
                        return Err(err!(
                            "invalid number of `:` in `%z` directive"
                        ))
                    }
                },
                b'c' => {
                    return Err(err!("cannot parse locale date and time"));
                }
                b'r' => {
                    return Err(err!(
                        "cannot parse locale 12-hour clock time"
                    ));
                }
                b'X' => {
                    return Err(err!("cannot parse locale clock time"));
                }
                b'x' => {
                    return Err(err!("cannot parse locale date"));
                }
                b'Z' => {
                    return Err(err!("cannot parse time zone abbreviations"));
                }
                b'.' => {
                    if !self.bump_fmt() {
                        return Err(err!(
                            "invalid format string, expected directive \
                             after '%.'",
                        ));
                    }
                    // Skip over any precision settings that might be here.
                    // This is a specific special format supported by `%.f`.
                    let (width, fmt) = Extension::parse_width(self.fmt)?;
                    let ext = Extension { width, ..ext };
                    self.fmt = fmt;
                    match self.f() {
                        b'f' => self
                            .parse_dot_fractional(ext)
                            .context("%.f failed")?,
                        unk => {
                            return Err(err!(
                                "found unrecognized directive %{unk} \
                                 following %.",
                                unk = escape::Byte(unk),
                            ));
                        }
                    }
                }
                unk => {
                    return Err(err!(
                        "found unrecognized directive %{unk}",
                        unk = escape::Byte(unk),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Returns the byte at the current position of the format string.
    ///
    /// # Panics
    ///
    /// This panics when the entire format string has been consumed.
    fn f(&self) -> u8 {
        self.fmt[0]
    }

    /// Returns the byte at the current position of the input string.
    ///
    /// # Panics
    ///
    /// This panics when the entire input string has been consumed.
    fn i(&self) -> u8 {
        self.inp[0]
    }

    /// Bumps the position of the format string.
    ///
    /// This returns true in precisely the cases where `self.f()` will not
    /// panic. i.e., When the end of the format string hasn't been reached yet.
    fn bump_fmt(&mut self) -> bool {
        self.fmt = &self.fmt[1..];
        !self.fmt.is_empty()
    }

    /// Bumps the position of the input string.
    ///
    /// This returns true in precisely the cases where `self.i()` will not
    /// panic. i.e., When the end of the input string hasn't been reached yet.
    fn bump_input(&mut self) -> bool {
        self.inp = &self.inp[1..];
        !self.inp.is_empty()
    }

    /// Parses optional extensions before a specifier directive. That is, right
    /// after the `%`. If any extensions are parsed, the parser is bumped
    /// to the next byte. (If no next byte exists, then an error is returned.)
    fn parse_extension(&mut self) -> Result<Extension, Error> {
        let (flag, fmt) = Extension::parse_flag(self.fmt)?;
        let (width, fmt) = Extension::parse_width(fmt)?;
        let (colons, fmt) = Extension::parse_colons(fmt);
        self.fmt = fmt;
        Ok(Extension { flag, width, colons })
    }

    // We write out a parsing routine for each directive below. Each parsing
    // routine assumes that the parser is positioned immediately after the
    // `%` for the current directive, and that there is at least one unconsumed
    // byte in the input.

    /// Parses a literal from the input that matches the current byte in the
    /// format string.
    ///
    /// This may consume multiple bytes from the input, for example, a single
    /// whitespace byte in the format string can match zero or more whitespace
    /// in the input.
    fn parse_literal(&mut self) -> Result<(), Error> {
        if self.f().is_ascii_whitespace() {
            if !self.inp.is_empty() {
                while self.i().is_ascii_whitespace() && self.bump_input() {}
            }
        } else if self.inp.is_empty() {
            return Err(err!(
                "expected to match literal byte {byte:?} from \
                 format string, but found end of input",
                byte = escape::Byte(self.fmt[0]),
            ));
        } else if self.f() != self.i() {
            return Err(err!(
                "expected to match literal byte {expect:?} from \
                 format string, but found byte {found:?} in input",
                expect = escape::Byte(self.f()),
                found = escape::Byte(self.i()),
            ));
        } else {
            self.bump_input();
        }
        self.bump_fmt();
        Ok(())
    }

    /// Parses an arbitrary (zero or more) amount ASCII whitespace.
    ///
    /// This is for `%n` and `%t`.
    fn parse_whitespace(&mut self) -> Result<(), Error> {
        if !self.inp.is_empty() {
            while self.i().is_ascii_whitespace() && self.bump_input() {}
        }
        self.bump_fmt();
        Ok(())
    }

    /// Parses a literal '%' from the input.
    fn parse_percent(&mut self) -> Result<(), Error> {
        if self.i() != b'%' {
            return Err(err!(
                "expected '%' due to '%%' in format string, \
                 but found {byte:?} in input",
                byte = escape::Byte(self.inp[0]),
            ));
        }
        self.bump_fmt();
        self.bump_input();
        Ok(())
    }

    /// Parses `%D`, which is equivalent to `%m/%d/%y`.
    fn parse_american_date(&mut self) -> Result<(), Error> {
        let mut p = Parser { fmt: b"%m/%d/%y", inp: self.inp, tm: self.tm };
        p.parse()?;
        self.inp = p.inp;
        self.bump_fmt();
        Ok(())
    }

    /// Parse `%p`, which indicates whether the time is AM or PM.
    ///
    /// This is generally only useful with `%I`. If, say, `%H` is used, then
    /// the AM/PM moniker will be validated, but it doesn't actually influence
    /// the clock time.
    fn parse_ampm(&mut self) -> Result<(), Error> {
        let (index, inp) = parse_ampm(self.inp)?;
        self.inp = inp;

        self.tm.meridiem = Some(match index {
            0 => Meridiem::AM,
            1 => Meridiem::PM,
            // OK because 0 <= index <= 1.
            index => unreachable!("unknown AM/PM index {index}"),
        });
        self.bump_fmt();
        Ok(())
    }

    /// Parses `%T`, which is equivalent to `%H:%M:%S`.
    fn parse_clock_secs(&mut self) -> Result<(), Error> {
        let mut p = Parser { fmt: b"%H:%M:%S", inp: self.inp, tm: self.tm };
        p.parse()?;
        self.inp = p.inp;
        self.bump_fmt();
        Ok(())
    }

    /// Parses `%R`, which is equivalent to `%H:%M`.
    fn parse_clock_nosecs(&mut self) -> Result<(), Error> {
        let mut p = Parser { fmt: b"%H:%M", inp: self.inp, tm: self.tm };
        p.parse()?;
        self.inp = p.inp;
        self.bump_fmt();
        Ok(())
    }

    /// Parses `%d` and `%e`, which is equivalent to the day of the month.
    ///
    /// We merely require that it is in the range 1-31 here.
    fn parse_day(&mut self, ext: Extension) -> Result<(), Error> {
        let (day, inp) = ext
            .parse_number(2, Flag::PadZero, self.inp)
            .context("failed to parse day")?;
        self.inp = inp;

        let day =
            t::Day::try_new("day", day).context("day number is invalid")?;
        self.tm.day = Some(day);
        self.bump_fmt();
        Ok(())
    }

    /// Parses `%j`, which is equivalent to the day of the year.
    ///
    /// We merely require that it is in the range 1-366 here.
    fn parse_day_of_year(&mut self, ext: Extension) -> Result<(), Error> {
        let (day, inp) = ext
            .parse_number(3, Flag::PadZero, self.inp)
            .context("failed to parse day of year")?;
        self.inp = inp;

        let day = t::DayOfYear::try_new("day-of-year", day)
            .context("day of year number is invalid")?;
        self.tm.day_of_year = Some(day);
        self.bump_fmt();
        Ok(())
    }

    /// Parses `%H`, which is equivalent to the hour.
    fn parse_hour24(&mut self, ext: Extension) -> Result<(), Error> {
        let (hour, inp) = ext
            .parse_number(2, Flag::PadZero, self.inp)
            .context("failed to parse hour")?;
        self.inp = inp;

        let hour = t::Hour::try_new("hour", hour)
            .context("hour number is invalid")?;
        self.tm.hour = Some(hour);
        self.bump_fmt();
        Ok(())
    }

    /// Parses `%I`, which is equivalent to the hour on a 12-hour clock.
    fn parse_hour12(&mut self, ext: Extension) -> Result<(), Error> {
        type Hour12 = ri8<1, 12>;

        let (hour, inp) = ext
            .parse_number(2, Flag::PadZero, self.inp)
            .context("failed to parse hour")?;
        self.inp = inp;

        let hour =
            Hour12::try_new("hour", hour).context("hour number is invalid")?;
        self.tm.hour = Some(t::Hour::rfrom(hour));
        self.bump_fmt();
        Ok(())
    }

    /// Parses `%F`, which is equivalent to `%Y-%m-%d`.
    fn parse_iso_date(&mut self) -> Result<(), Error> {
        let mut p = Parser { fmt: b"%Y-%m-%d", inp: self.inp, tm: self.tm };
        p.parse()?;
        self.inp = p.inp;
        self.bump_fmt();
        Ok(())
    }

    /// Parses `%M`, which is equivalent to the minute.
    fn parse_minute(&mut self, ext: Extension) -> Result<(), Error> {
        let (minute, inp) = ext
            .parse_number(2, Flag::PadZero, self.inp)
            .context("failed to parse minute")?;
        self.inp = inp;

        let minute = t::Minute::try_new("minute", minute)
            .context("minute number is invalid")?;
        self.tm.minute = Some(minute);
        self.bump_fmt();
        Ok(())
    }

    /// Parse `%Q`, which is the IANA time zone identifier or an offset without
    /// colons.
    fn parse_iana_nocolon(&mut self) -> Result<(), Error> {
        #[cfg(not(feature = "alloc"))]
        {
            Err(err!(
                "cannot parse `%Q` without Jiff's `alloc` feature enabled"
            ))
        }
        #[cfg(feature = "alloc")]
        {
            use alloc::string::ToString;

            if !self.inp.is_empty() && matches!(self.inp[0], b'+' | b'-') {
                return self.parse_offset_nocolon();
            }
            let (iana, inp) = parse_iana(self.inp)?;
            self.inp = inp;
            self.tm.iana = Some(iana.to_string());
            self.bump_fmt();
            Ok(())
        }
    }

    /// Parse `%:Q`, which is the IANA time zone identifier or an offset with
    /// colons.
    fn parse_iana_colon(&mut self) -> Result<(), Error> {
        #[cfg(not(feature = "alloc"))]
        {
            Err(err!(
                "cannot parse `%:Q` without Jiff's `alloc` feature enabled"
            ))
        }
        #[cfg(feature = "alloc")]
        {
            use alloc::string::ToString;

            if !self.inp.is_empty() && matches!(self.inp[0], b'+' | b'-') {
                return self.parse_offset_colon();
            }
            let (iana, inp) = parse_iana(self.inp)?;
            self.inp = inp;
            self.tm.iana = Some(iana.to_string());
            self.bump_fmt();
            Ok(())
        }
    }

    /// Parse `%z`, which is a time zone offset without colons that requires
    /// a minutes component but will parse a second component if it's there.
    fn parse_offset_nocolon(&mut self) -> Result<(), Error> {
        static PARSER: offset::Parser = offset::Parser::new()
            .zulu(false)
            .require_minute(true)
            .subminute(true)
            .subsecond(false)
            .colon(offset::Colon::Absent);

        let Parsed { value, input } = PARSER.parse(self.inp)?;
        self.tm.offset = Some(value.to_offset()?);
        self.inp = input;
        self.bump_fmt();

        Ok(())
    }

    /// Parse `%:z`, which is a time zone offset with colons that requires
    /// a minutes component but will parse a second component if it's there.
    fn parse_offset_colon(&mut self) -> Result<(), Error> {
        static PARSER: offset::Parser = offset::Parser::new()
            .zulu(false)
            .require_minute(true)
            .subminute(true)
            .subsecond(false)
            .colon(offset::Colon::Required);

        let Parsed { value, input } = PARSER.parse(self.inp)?;
        self.tm.offset = Some(value.to_offset()?);
        self.inp = input;
        self.bump_fmt();

        Ok(())
    }

    /// Parse `%::z`, which is a time zone offset with colons that requires
    /// a seconds component.
    fn parse_offset_colon2(&mut self) -> Result<(), Error> {
        static PARSER: offset::Parser = offset::Parser::new()
            .zulu(false)
            .require_minute(true)
            .require_second(true)
            .subminute(true)
            .subsecond(false)
            .colon(offset::Colon::Required);

        let Parsed { value, input } = PARSER.parse(self.inp)?;
        self.tm.offset = Some(value.to_offset()?);
        self.inp = input;
        self.bump_fmt();

        Ok(())
    }

    /// Parse `%:::z`, which is a time zone offset with colons that only
    /// requires an hour component, but will parse minute/second components
    /// if they are there.
    fn parse_offset_colon3(&mut self) -> Result<(), Error> {
        static PARSER: offset::Parser = offset::Parser::new()
            .zulu(false)
            .require_minute(false)
            .require_second(false)
            .subminute(true)
            .subsecond(false)
            .colon(offset::Colon::Required);

        let Parsed { value, input } = PARSER.parse(self.inp)?;
        self.tm.offset = Some(value.to_offset()?);
        self.inp = input;
        self.bump_fmt();

        Ok(())
    }

    /// Parses `%S`, which is equivalent to the second.
    fn parse_second(&mut self, ext: Extension) -> Result<(), Error> {
        let (mut second, inp) = ext
            .parse_number(2, Flag::PadZero, self.inp)
            .context("failed to parse second")?;
        self.inp = inp;

        // As with other parses in Jiff, and like Temporal,
        // we constrain `60` seconds to `59` because we don't
        // support leap seconds.
        if second == 60 {
            second = 59;
        }
        let second = t::Second::try_new("second", second)
            .context("second number is invalid")?;
        self.tm.second = Some(second);
        self.bump_fmt();
        Ok(())
    }

    /// Parses `%s`, which is equivalent to a Unix timestamp.
    fn parse_timestamp(&mut self, ext: Extension) -> Result<(), Error> {
        let (sign, inp) = parse_optional_sign(self.inp);
        let (timestamp, inp) = ext
            // 19 comes from `i64::MAX.to_string().len()`.
            .parse_number(19, Flag::PadSpace, inp)
            .context("failed to parse Unix timestamp (in seconds)")?;
        // I believe this error case is actually impossible. Since `timestamp`
        // is guaranteed to be positive, and negating any positive `i64` will
        // always result in a valid `i64`.
        let timestamp = timestamp.checked_mul(sign).ok_or_else(|| {
            err!(
                "parsed Unix timestamp `{timestamp}` with a \
                 leading `-` sign, which causes overflow",
            )
        })?;
        let timestamp =
            Timestamp::from_second(timestamp).with_context(|| {
                err!(
                    "parsed Unix timestamp `{timestamp}`, \
                     but out of range of valid Jiff `Timestamp`",
                )
            })?;
        self.inp = inp;

        // This is basically just repeating the
        // `From<Timestamp> for BrokenDownTime`
        // trait implementation.
        let dt = Offset::UTC.to_datetime(timestamp);
        let (d, t) = (dt.date(), dt.time());
        self.tm.offset = Some(Offset::UTC);
        self.tm.year = Some(d.year_ranged());
        self.tm.month = Some(d.month_ranged());
        self.tm.day = Some(d.day_ranged());
        self.tm.hour = Some(t.hour_ranged());
        self.tm.minute = Some(t.minute_ranged());
        self.tm.second = Some(t.second_ranged());
        self.tm.subsec = Some(t.subsec_nanosecond_ranged());
        self.tm.meridiem = Some(Meridiem::from(t));

        self.bump_fmt();
        Ok(())
    }

    /// Parses `%f` (or `%N`, which is an alias for `%9f`), which is equivalent
    /// to a fractional second up to nanosecond precision. This must always
    /// parse at least one decimal digit and does not parse any leading dot.
    ///
    /// At present, we don't use any flags/width/precision settings to
    /// influence parsing. That is, `%3f` will parse the fractional component
    /// in `0.123456789`.
    fn parse_fractional(&mut self, _ext: Extension) -> Result<(), Error> {
        let mkdigits = parse::slicer(self.inp);
        while mkdigits(self.inp).len() < 9
            && self.inp.first().map_or(false, u8::is_ascii_digit)
        {
            self.inp = &self.inp[1..];
        }
        let digits = mkdigits(self.inp);
        if digits.is_empty() {
            return Err(err!(
                "expected at least one fractional decimal digit, \
                 but did not find any",
            ));
        }
        // I believe this error can never happen, since we know we have no more
        // than 9 ASCII digits. Any sequence of 9 ASCII digits can be parsed
        // into an `i64`.
        let nanoseconds = parse::fraction(digits, 9).map_err(|err| {
            err!(
                "failed to parse {digits:?} as fractional second component \
                 (up to 9 digits, nanosecond precision): {err}",
                digits = escape::Bytes(digits),
            )
        })?;
        // I believe this is also impossible to fail, since the maximal
        // fractional nanosecond is 999_999_999, and which also corresponds
        // to the maximal expressible number with 9 ASCII digits. So every
        // possible expressible value here is in range.
        let nanoseconds =
            t::SubsecNanosecond::try_new("nanoseconds", nanoseconds).map_err(
                |err| err!("fractional nanoseconds are not valid: {err}"),
            )?;
        self.tm.subsec = Some(nanoseconds);
        self.bump_fmt();
        Ok(())
    }

    /// Parses `%f`, which is equivalent to a dot followed by a fractional
    /// second up to nanosecond precision. Note that if there is no leading
    /// dot, then this successfully parses the empty string.
    fn parse_dot_fractional(&mut self, ext: Extension) -> Result<(), Error> {
        if !self.inp.starts_with(b".") {
            self.bump_fmt();
            return Ok(());
        }
        self.inp = &self.inp[1..];
        self.parse_fractional(ext)
    }

    /// Parses `%m`, which is equivalent to the month.
    fn parse_month(&mut self, ext: Extension) -> Result<(), Error> {
        let (month, inp) = ext
            .parse_number(2, Flag::PadZero, self.inp)
            .context("failed to parse month")?;
        self.inp = inp;

        let month = t::Month::try_new("month", month)
            .context("month number is invalid")?;
        self.tm.month = Some(month);
        self.bump_fmt();
        Ok(())
    }

    /// Parse `%b` or `%h`, which is an abbreviated month name.
    fn parse_month_name_abbrev(&mut self) -> Result<(), Error> {
        let (index, inp) = parse_month_name_abbrev(self.inp)?;
        self.inp = inp;

        // Both are OK because 0 <= index <= 11.
        let index = i8::try_from(index).unwrap();
        self.tm.month = Some(t::Month::new(index + 1).unwrap());
        self.bump_fmt();
        Ok(())
    }

    /// Parse `%B`, which is a full month name.
    fn parse_month_name_full(&mut self) -> Result<(), Error> {
        static CHOICES: &'static [&'static [u8]] = &[
            b"January",
            b"February",
            b"March",
            b"April",
            b"May",
            b"June",
            b"July",
            b"August",
            b"September",
            b"October",
            b"November",
            b"December",
        ];

        let (index, inp) = parse_choice(self.inp, CHOICES)
            .context("unrecognized month name")?;
        self.inp = inp;

        // Both are OK because 0 <= index <= 11.
        let index = i8::try_from(index).unwrap();
        self.tm.month = Some(t::Month::new(index + 1).unwrap());
        self.bump_fmt();
        Ok(())
    }

    /// Parse `%a`, which is an abbreviated weekday.
    fn parse_weekday_abbrev(&mut self) -> Result<(), Error> {
        let (index, inp) = parse_weekday_abbrev(self.inp)?;
        self.inp = inp;

        // Both are OK because 0 <= index <= 6.
        let index = i8::try_from(index).unwrap();
        self.tm.weekday =
            Some(Weekday::from_sunday_zero_offset(index).unwrap());
        self.bump_fmt();
        Ok(())
    }

    /// Parse `%A`, which is a full weekday name.
    fn parse_weekday_full(&mut self) -> Result<(), Error> {
        static CHOICES: &'static [&'static [u8]] = &[
            b"Sunday",
            b"Monday",
            b"Tuesday",
            b"Wednesday",
            b"Thursday",
            b"Friday",
            b"Saturday",
        ];

        let (index, inp) = parse_choice(self.inp, CHOICES)
            .context("unrecognized weekday abbreviation")?;
        self.inp = inp;

        // Both are OK because 0 <= index <= 6.
        let index = i8::try_from(index).unwrap();
        self.tm.weekday =
            Some(Weekday::from_sunday_zero_offset(index).unwrap());
        self.bump_fmt();
        Ok(())
    }

    /// Parse `%u`, which is a weekday number with Monday being `1` and
    /// Sunday being `7`.
    fn parse_weekday_mon(&mut self, ext: Extension) -> Result<(), Error> {
        let (weekday, inp) = ext
            .parse_number(1, Flag::NoPad, self.inp)
            .context("failed to parse weekday number")?;
        self.inp = inp;

        let weekday = i8::try_from(weekday).map_err(|_| {
            err!("parsed weekday number `{weekday}` is invalid")
        })?;
        let weekday = Weekday::from_monday_one_offset(weekday)
            .context("weekday number is invalid")?;
        self.tm.weekday = Some(weekday);
        self.bump_fmt();
        Ok(())
    }

    /// Parse `%w`, which is a weekday number with Sunday being `0`.
    fn parse_weekday_sun(&mut self, ext: Extension) -> Result<(), Error> {
        let (weekday, inp) = ext
            .parse_number(1, Flag::NoPad, self.inp)
            .context("failed to parse weekday number")?;
        self.inp = inp;

        let weekday = i8::try_from(weekday).map_err(|_| {
            err!("parsed weekday number `{weekday}` is invalid")
        })?;
        let weekday = Weekday::from_sunday_zero_offset(weekday)
            .context("weekday number is invalid")?;
        self.tm.weekday = Some(weekday);
        self.bump_fmt();
        Ok(())
    }

    /// Parse `%U`, which is a week number with Sunday being the first day
    /// in the first week numbered `01`.
    fn parse_week_sun(&mut self, ext: Extension) -> Result<(), Error> {
        let (week, inp) = ext
            .parse_number(2, Flag::PadZero, self.inp)
            .context("failed to parse Sunday-based week number")?;
        self.inp = inp;

        let week = t::WeekNum::try_new("week", week)
            .context("Sunday-based week number is invalid")?;
        self.tm.week_sun = Some(week);
        self.bump_fmt();
        Ok(())
    }

    /// Parse `%V`, which is an ISO 8601 week number.
    fn parse_week_iso(&mut self, ext: Extension) -> Result<(), Error> {
        let (week, inp) = ext
            .parse_number(2, Flag::PadZero, self.inp)
            .context("failed to parse ISO 8601 week number")?;
        self.inp = inp;

        let week = t::ISOWeek::try_new("week", week)
            .context("ISO 8601 week number is invalid")?;
        self.tm.iso_week = Some(week);
        self.bump_fmt();
        Ok(())
    }

    /// Parse `%W`, which is a week number with Monday being the first day
    /// in the first week numbered `01`.
    fn parse_week_mon(&mut self, ext: Extension) -> Result<(), Error> {
        let (week, inp) = ext
            .parse_number(2, Flag::PadZero, self.inp)
            .context("failed to parse Monday-based week number")?;
        self.inp = inp;

        let week = t::WeekNum::try_new("week", week)
            .context("Monday-based week number is invalid")?;
        self.tm.week_mon = Some(week);
        self.bump_fmt();
        Ok(())
    }

    /// Parses `%Y`, which we permit to be any year, including a negative year.
    fn parse_year(&mut self, ext: Extension) -> Result<(), Error> {
        let (sign, inp) = parse_optional_sign(self.inp);
        let (year, inp) = ext
            .parse_number(4, Flag::PadZero, inp)
            .context("failed to parse year")?;
        self.inp = inp;

        // OK because sign=={1,-1} and year can't be bigger than 4 digits
        // so overflow isn't possible.
        let year = sign.checked_mul(year).unwrap();
        let year = t::Year::try_new("year", year)
            .context("year number is invalid")?;
        self.tm.year = Some(year);
        self.bump_fmt();
        Ok(())
    }

    /// Parses `%y`, which is equivalent to a 2-digit year.
    ///
    /// The numbers 69-99 refer to 1969-1999, while 00-68 refer to 2000-2068.
    fn parse_year2(&mut self, ext: Extension) -> Result<(), Error> {
        type Year2Digit = ri8<0, 99>;

        let (year, inp) = ext
            .parse_number(2, Flag::PadZero, self.inp)
            .context("failed to parse 2-digit year")?;
        self.inp = inp;

        let year = Year2Digit::try_new("year (2 digits)", year)
            .context("year number is invalid")?;
        let mut year = t::Year::rfrom(year);
        if year <= C(68) {
            year += C(2000);
        } else {
            year += C(1900);
        }
        self.tm.year = Some(year);
        self.bump_fmt();
        Ok(())
    }

    /// Parses `%C`, which we permit to just be a century, including a negative
    /// century.
    fn parse_century(&mut self, ext: Extension) -> Result<(), Error> {
        let (sign, inp) = parse_optional_sign(self.inp);
        let (century, inp) = ext
            .parse_number(2, Flag::NoPad, inp)
            .context("failed to parse century")?;
        self.inp = inp;

        // OK because sign=={1,-1} and century can't be bigger than 2 digits
        // so overflow isn't possible.
        let century = sign.checked_mul(century).unwrap();
        // Similarly, we have 64-bit integers here. Two digits multiplied by
        // 100 will never overflow.
        let year = century.checked_mul(100).unwrap();
        // I believe the error condition here is impossible.
        let year = t::Year::try_new("year", year)
            .context("year number (from century) is invalid")?;
        self.tm.year = Some(year);
        self.bump_fmt();
        Ok(())
    }

    /// Parses `%G`, which we permit to be any year, including a negative year.
    fn parse_iso_week_year(&mut self, ext: Extension) -> Result<(), Error> {
        let (sign, inp) = parse_optional_sign(self.inp);
        let (year, inp) = ext
            .parse_number(4, Flag::PadZero, inp)
            .context("failed to parse ISO 8601 week-based year")?;
        self.inp = inp;

        // OK because sign=={1,-1} and year can't be bigger than 4 digits
        // so overflow isn't possible.
        let year = sign.checked_mul(year).unwrap();
        let year = t::ISOYear::try_new("year", year)
            .context("ISO 8601 week-based year number is invalid")?;
        self.tm.iso_week_year = Some(year);
        self.bump_fmt();
        Ok(())
    }

    /// Parses `%g`, which is equivalent to a 2-digit ISO 8601 week-based year.
    ///
    /// The numbers 69-99 refer to 1969-1999, while 00-68 refer to 2000-2068.
    fn parse_iso_week_year2(&mut self, ext: Extension) -> Result<(), Error> {
        type Year2Digit = ri8<0, 99>;

        let (year, inp) = ext
            .parse_number(2, Flag::PadZero, self.inp)
            .context("failed to parse 2-digit ISO 8601 week-based year")?;
        self.inp = inp;

        let year = Year2Digit::try_new("year (2 digits)", year)
            .context("ISO 8601 week-based year number is invalid")?;
        let mut year = t::ISOYear::rfrom(year);
        if year <= C(68) {
            year += C(2000);
        } else {
            year += C(1900);
        }
        self.tm.iso_week_year = Some(year);
        self.bump_fmt();
        Ok(())
    }
}

impl Extension {
    /// Parse an integer with the given default padding and flag settings.
    ///
    /// The default padding is usually 2 (4 for %Y) and the default flag is
    /// usually Flag::PadZero (there are no cases where the default flag is
    /// different at time of writing). But both the padding and the flag can be
    /// overridden by the settings on this extension.
    ///
    /// Generally speaking, parsing ignores everything in an extension except
    /// for padding. When padding is set, then parsing will limit itself to a
    /// number of digits equal to the greater of the default padding size or
    /// the configured padding size. This permits `%Y%m%d` to parse `20240730`
    /// successfully, for example.
    ///
    /// The remaining input is returned. This returns an error if the given
    /// input is empty.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn parse_number<'i>(
        &self,
        default_pad_width: usize,
        default_flag: Flag,
        mut inp: &'i [u8],
    ) -> Result<(i64, &'i [u8]), Error> {
        let flag = self.flag.unwrap_or(default_flag);
        let zero_pad_width = match flag {
            Flag::PadSpace | Flag::NoPad => 0,
            _ => self.width.map(usize::from).unwrap_or(default_pad_width),
        };
        let max_digits = default_pad_width.max(zero_pad_width);

        // Strip and ignore any whitespace we might see here.
        while inp.get(0).map_or(false, |b| b.is_ascii_whitespace()) {
            inp = &inp[1..];
        }
        let mut digits = 0;
        while digits < inp.len()
            && digits < zero_pad_width
            && inp[digits] == b'0'
        {
            digits += 1;
        }
        let mut n: i64 = 0;
        while digits < inp.len()
            && digits < max_digits
            && inp[digits].is_ascii_digit()
        {
            let byte = inp[digits];
            digits += 1;
            // This is manually inlined from `crate::util::parse::i64` to avoid
            // repeating this loop, and with some error cases removed since we
            // know that `byte` is an ASCII digit.
            let digit = i64::from(byte - b'0');
            n = n
                .checked_mul(10)
                .and_then(|n| n.checked_add(digit))
                .ok_or_else(|| {
                    err!(
                        "number '{}' too big to parse into 64-bit integer",
                        escape::Bytes(&inp[..digits]),
                    )
                })?;
        }
        if digits == 0 {
            return Err(err!("invalid number, no digits found"));
        }
        Ok((n, &inp[digits..]))
    }
}

/// Parses an optional sign from the beginning of the input. If one isn't
/// found, then the sign returned is positive.
///
/// This also returns the remaining unparsed input.
#[cfg_attr(feature = "perf-inline", inline(always))]
fn parse_optional_sign<'i>(input: &'i [u8]) -> (i64, &'i [u8]) {
    if input.is_empty() {
        (1, input)
    } else if input[0] == b'-' {
        (-1, &input[1..])
    } else if input[0] == b'+' {
        (1, &input[1..])
    } else {
        (1, input)
    }
}

/// Parses the input such that, on success, the index of the first matching
/// choice (via ASCII case insensitive comparisons) is returned, along with
/// any remaining unparsed input.
///
/// If no choice given is a prefix of the input, then an error is returned.
/// The error includes the possible allowed choices.
fn parse_choice<'i>(
    input: &'i [u8],
    choices: &[&'static [u8]],
) -> Result<(usize, &'i [u8]), Error> {
    for (i, choice) in choices.into_iter().enumerate() {
        if input.len() < choice.len() {
            continue;
        }
        let (candidate, input) = input.split_at(choice.len());
        if candidate.eq_ignore_ascii_case(choice) {
            return Ok((i, input));
        }
    }
    #[cfg(feature = "alloc")]
    {
        let mut err = alloc::format!(
            "failed to find expected choice at beginning of {input:?}, \
             available choices are: ",
            input = escape::Bytes(input),
        );
        for (i, choice) in choices.iter().enumerate() {
            if i > 0 {
                write!(err, ", ").unwrap();
            }
            write!(err, "{}", escape::Bytes(choice)).unwrap();
        }
        Err(Error::adhoc(err))
    }
    #[cfg(not(feature = "alloc"))]
    {
        Err(err!(
            "failed to find expected value from a set of allowed choices"
        ))
    }
}

/// Like `parse_choice`, but specialized for AM/PM.
///
/// This exists because AM/PM is common and we can take advantage of the fact
/// that they are both exactly two bytes.
#[cfg_attr(feature = "perf-inline", inline(always))]
fn parse_ampm<'i>(input: &'i [u8]) -> Result<(usize, &'i [u8]), Error> {
    if input.len() < 2 {
        return Err(err!(
            "expected to find AM or PM, \
             but the remaining input, {input:?}, is too short \
             to contain one",
            input = escape::Bytes(input),
        ));
    }
    let (x, input) = input.split_at(2);
    let candidate = &[x[0].to_ascii_lowercase(), x[1].to_ascii_lowercase()];
    let index = match candidate {
        b"am" => 0,
        b"pm" => 1,
        _ => {
            return Err(err!(
                "expected to find AM or PM, but found \
                {candidate:?} instead",
                candidate = escape::Bytes(x),
            ))
        }
    };
    Ok((index, input))
}

/// Like `parse_choice`, but specialized for weekday abbreviation.
///
/// This exists because weekday abbreviations are common and we can take
/// advantage of the fact that they are all exactly three bytes.
#[cfg_attr(feature = "perf-inline", inline(always))]
fn parse_weekday_abbrev<'i>(
    input: &'i [u8],
) -> Result<(usize, &'i [u8]), Error> {
    if input.len() < 3 {
        return Err(err!(
            "expected to find a weekday abbreviation, \
             but the remaining input, {input:?}, is too short \
             to contain one",
            input = escape::Bytes(input),
        ));
    }
    let (x, input) = input.split_at(3);
    let candidate = &[
        x[0].to_ascii_lowercase(),
        x[1].to_ascii_lowercase(),
        x[2].to_ascii_lowercase(),
    ];
    let index = match candidate {
        b"sun" => 0,
        b"mon" => 1,
        b"tue" => 2,
        b"wed" => 3,
        b"thu" => 4,
        b"fri" => 5,
        b"sat" => 6,
        _ => {
            return Err(err!(
                "expected to find weekday abbreviation, but found \
                {candidate:?} instead",
                candidate = escape::Bytes(x),
            ))
        }
    };
    Ok((index, input))
}

/// Like `parse_choice`, but specialized for month name abbreviation.
///
/// This exists because month name abbreviations are common and we can take
/// advantage of the fact that they are all exactly three bytes.
#[cfg_attr(feature = "perf-inline", inline(always))]
fn parse_month_name_abbrev<'i>(
    input: &'i [u8],
) -> Result<(usize, &'i [u8]), Error> {
    if input.len() < 3 {
        return Err(err!(
            "expected to find a month name abbreviation, \
             but the remaining input, {input:?}, is too short \
             to contain one",
            input = escape::Bytes(input),
        ));
    }
    let (x, input) = input.split_at(3);
    let candidate = &[
        x[0].to_ascii_lowercase(),
        x[1].to_ascii_lowercase(),
        x[2].to_ascii_lowercase(),
    ];
    let index = match candidate {
        b"jan" => 0,
        b"feb" => 1,
        b"mar" => 2,
        b"apr" => 3,
        b"may" => 4,
        b"jun" => 5,
        b"jul" => 6,
        b"aug" => 7,
        b"sep" => 8,
        b"oct" => 9,
        b"nov" => 10,
        b"dec" => 11,
        _ => {
            return Err(err!(
                "expected to find month name abbreviation, but found \
                 {candidate:?} instead",
                candidate = escape::Bytes(x),
            ))
        }
    };
    Ok((index, input))
}

#[cfg_attr(feature = "perf-inline", inline(always))]
fn parse_iana<'i>(input: &'i [u8]) -> Result<(&'i str, &'i [u8]), Error> {
    let mkiana = parse::slicer(input);
    let (_, mut input) = parse_iana_component(input)?;
    while input.starts_with(b"/") {
        input = &input[1..];
        let (_, unconsumed) = parse_iana_component(input)?;
        input = unconsumed;
    }
    // This is OK because all bytes in a IANA TZ annotation are guaranteed
    // to be ASCII, or else we wouldn't be here. If this turns out to be
    // a perf issue, we can do an unchecked conversion here. But I figured
    // it would be better to start conservative.
    let iana = core::str::from_utf8(mkiana(input)).expect("ASCII");
    Ok((iana, input))
}

/// Parses a single IANA name component. That is, the thing that leads all IANA
/// time zone identifiers and the thing that must always come after a `/`. This
/// returns an error if no component could be found.
#[cfg_attr(feature = "perf-inline", inline(always))]
fn parse_iana_component<'i>(
    mut input: &'i [u8],
) -> Result<(&'i [u8], &'i [u8]), Error> {
    let mkname = parse::slicer(input);
    if input.is_empty() {
        return Err(err!(
            "expected the start of an IANA time zone identifier \
             name or component, but found end of input instead",
        ));
    }
    if !matches!(input[0], b'_' | b'.' | b'A'..=b'Z' | b'a'..=b'z') {
        return Err(err!(
            "expected the start of an IANA time zone identifier \
             name or component, but found {:?} instead",
            escape::Byte(input[0]),
        ));
    }
    input = &input[1..];

    let is_iana_char = |byte| {
        matches!(
            byte,
            b'_' | b'.' | b'+' | b'-' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z',
        )
    };
    while !input.is_empty() && is_iana_char(input[0]) {
        input = &input[1..];
    }
    Ok((mkname(input), input))
}

#[cfg(feature = "alloc")]
#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn ok_parse_zoned() {
        if crate::tz::db().is_definitively_empty() {
            return;
        }

        let p = |fmt: &str, input: &str| {
            BrokenDownTime::parse_mono(fmt.as_bytes(), input.as_bytes())
                .unwrap()
                .to_zoned()
                .unwrap()
        };

        insta::assert_debug_snapshot!(
            p("%h %d, %Y %H:%M:%S %z", "Apr 1, 2022 20:46:15 -0400"),
            @"2022-04-01T20:46:15-04:00[-04:00]",
        );
        insta::assert_debug_snapshot!(
            p("%h %d, %Y %H:%M:%S %Q", "Apr 1, 2022 20:46:15 -0400"),
            @"2022-04-01T20:46:15-04:00[-04:00]",
        );
        insta::assert_debug_snapshot!(
            p("%h %d, %Y %H:%M:%S [%Q]", "Apr 1, 2022 20:46:15 [America/New_York]"),
            @"2022-04-01T20:46:15-04:00[America/New_York]",
        );
        insta::assert_debug_snapshot!(
            p("%h %d, %Y %H:%M:%S %Q", "Apr 1, 2022 20:46:15 America/New_York"),
            @"2022-04-01T20:46:15-04:00[America/New_York]",
        );
        insta::assert_debug_snapshot!(
            p("%h %d, %Y %H:%M:%S %:z %:Q", "Apr 1, 2022 20:46:15 -08:00 -04:00"),
            @"2022-04-01T20:46:15-04:00[-04:00]",
        );
    }

    #[test]
    fn ok_parse_timestamp() {
        let p = |fmt: &str, input: &str| {
            BrokenDownTime::parse_mono(fmt.as_bytes(), input.as_bytes())
                .unwrap()
                .to_timestamp()
                .unwrap()
        };

        insta::assert_debug_snapshot!(
            p("%h %d, %Y %H:%M:%S %z", "Apr 1, 2022 20:46:15 -0400"),
            @"2022-04-02T00:46:15Z",
        );
        insta::assert_debug_snapshot!(
            p("%h %d, %Y %H:%M:%S %z", "Apr 1, 2022 20:46:15 +0400"),
            @"2022-04-01T16:46:15Z",
        );
        insta::assert_debug_snapshot!(
            p("%h %d, %Y %H:%M:%S %z", "Apr 1, 2022 20:46:15 -040059"),
            @"2022-04-02T00:47:14Z",
        );

        insta::assert_debug_snapshot!(
            p("%h %d, %Y %H:%M:%S %:z", "Apr 1, 2022 20:46:15 -04:00"),
            @"2022-04-02T00:46:15Z",
        );
        insta::assert_debug_snapshot!(
            p("%h %d, %Y %H:%M:%S %:z", "Apr 1, 2022 20:46:15 +04:00"),
            @"2022-04-01T16:46:15Z",
        );
        insta::assert_debug_snapshot!(
            p("%h %d, %Y %H:%M:%S %:z", "Apr 1, 2022 20:46:15 -04:00:59"),
            @"2022-04-02T00:47:14Z",
        );

        insta::assert_debug_snapshot!(
            p("%s", "0"),
            @"1970-01-01T00:00:00Z",
        );
        insta::assert_debug_snapshot!(
            p("%s", "-0"),
            @"1970-01-01T00:00:00Z",
        );
        insta::assert_debug_snapshot!(
            p("%s", "-1"),
            @"1969-12-31T23:59:59Z",
        );
        insta::assert_debug_snapshot!(
            p("%s", "1"),
            @"1970-01-01T00:00:01Z",
        );
        insta::assert_debug_snapshot!(
            p("%s", "+1"),
            @"1970-01-01T00:00:01Z",
        );
        insta::assert_debug_snapshot!(
            p("%s", "1737396540"),
            @"2025-01-20T18:09:00Z",
        );
        insta::assert_debug_snapshot!(
            p("%s", "-377705023201"),
            @"-009999-01-02T01:59:59Z",
        );
        insta::assert_debug_snapshot!(
            p("%s", "253402207200"),
            @"9999-12-30T22:00:00Z",
        );
    }

    #[test]
    fn ok_parse_datetime() {
        let p = |fmt: &str, input: &str| {
            BrokenDownTime::parse_mono(fmt.as_bytes(), input.as_bytes())
                .unwrap()
                .to_datetime()
                .unwrap()
        };

        insta::assert_debug_snapshot!(
            p("%h %d, %Y %H:%M:%S", "Apr 1, 2022 20:46:15"),
            @"2022-04-01T20:46:15",
        );
        insta::assert_debug_snapshot!(
            p("%h %05d, %Y %H:%M:%S", "Apr 1, 2022 20:46:15"),
            @"2022-04-01T20:46:15",
        );
    }

    #[test]
    fn ok_parse_date() {
        let p = |fmt: &str, input: &str| {
            BrokenDownTime::parse_mono(fmt.as_bytes(), input.as_bytes())
                .unwrap()
                .to_date()
                .unwrap()
        };

        insta::assert_debug_snapshot!(
            p("%m/%d/%y", "1/1/99"),
            @"1999-01-01",
        );
        insta::assert_debug_snapshot!(
            p("%m/%d/%04y", "1/1/0099"),
            @"1999-01-01",
        );
        insta::assert_debug_snapshot!(
            p("%D", "1/1/99"),
            @"1999-01-01",
        );
        insta::assert_debug_snapshot!(
            p("%m/%d/%Y", "1/1/0099"),
            @"0099-01-01",
        );
        insta::assert_debug_snapshot!(
            p("%m/%d/%Y", "1/1/1999"),
            @"1999-01-01",
        );
        insta::assert_debug_snapshot!(
            p("%m/%d/%Y", "12/31/9999"),
            @"9999-12-31",
        );
        insta::assert_debug_snapshot!(
            p("%m/%d/%Y", "01/01/-9999"),
            @"-009999-01-01",
        );
        insta::assert_snapshot!(
            p("%a %m/%d/%Y", "sun 7/14/2024"),
            @"2024-07-14",
        );
        insta::assert_snapshot!(
            p("%A %m/%d/%Y", "sUnDaY 7/14/2024"),
            @"2024-07-14",
        );
        insta::assert_snapshot!(
            p("%b %d %Y", "Jul 14 2024"),
            @"2024-07-14",
        );
        insta::assert_snapshot!(
            p("%B %d, %Y", "July 14, 2024"),
            @"2024-07-14",
        );
        insta::assert_snapshot!(
            p("%A, %B %d, %Y", "Wednesday, dEcEmBeR 25, 2024"),
            @"2024-12-25",
        );

        insta::assert_debug_snapshot!(
            p("%Y%m%d", "20240730"),
            @"2024-07-30",
        );
        insta::assert_debug_snapshot!(
            p("%Y%m%d", "09990730"),
            @"0999-07-30",
        );
        insta::assert_debug_snapshot!(
            p("%Y%m%d", "9990111"),
            @"9990-11-01",
        );
        insta::assert_debug_snapshot!(
            p("%3Y%m%d", "09990111"),
            @"0999-01-11",
        );
        insta::assert_debug_snapshot!(
            p("%5Y%m%d", "09990111"),
            @"9990-11-01",
        );
        insta::assert_debug_snapshot!(
            p("%5Y%m%d", "009990111"),
            @"0999-01-11",
        );

        insta::assert_debug_snapshot!(
            p("%C-%m-%d", "20-07-01"),
            @"2000-07-01",
        );
        insta::assert_debug_snapshot!(
            p("%C-%m-%d", "-20-07-01"),
            @"-002000-07-01",
        );
        insta::assert_debug_snapshot!(
            p("%C-%m-%d", "9-07-01"),
            @"0900-07-01",
        );
        insta::assert_debug_snapshot!(
            p("%C-%m-%d", "-9-07-01"),
            @"-000900-07-01",
        );
        insta::assert_debug_snapshot!(
            p("%C-%m-%d", "09-07-01"),
            @"0900-07-01",
        );
        insta::assert_debug_snapshot!(
            p("%C-%m-%d", "-09-07-01"),
            @"-000900-07-01",
        );
        insta::assert_debug_snapshot!(
            p("%C-%m-%d", "0-07-01"),
            @"0000-07-01",
        );
        insta::assert_debug_snapshot!(
            p("%C-%m-%d", "-0-07-01"),
            @"0000-07-01",
        );

        insta::assert_snapshot!(
            p("%u %m/%d/%Y", "7 7/14/2024"),
            @"2024-07-14",
        );
        insta::assert_snapshot!(
            p("%w %m/%d/%Y", "0 7/14/2024"),
            @"2024-07-14",
        );

        insta::assert_snapshot!(
            p("%Y-%U-%u", "2025-00-6"),
            @"2025-01-04",
        );
        insta::assert_snapshot!(
            p("%Y-%U-%u", "2025-01-7"),
            @"2025-01-05",
        );
        insta::assert_snapshot!(
            p("%Y-%U-%u", "2025-01-1"),
            @"2025-01-06",
        );

        insta::assert_snapshot!(
            p("%Y-%W-%u", "2025-00-6"),
            @"2025-01-04",
        );
        insta::assert_snapshot!(
            p("%Y-%W-%u", "2025-00-7"),
            @"2025-01-05",
        );
        insta::assert_snapshot!(
            p("%Y-%W-%u", "2025-01-1"),
            @"2025-01-06",
        );
        insta::assert_snapshot!(
            p("%Y-%W-%u", "2025-01-2"),
            @"2025-01-07",
        );
    }

    #[test]
    fn ok_parse_time() {
        let p = |fmt: &str, input: &str| {
            BrokenDownTime::parse_mono(fmt.as_bytes(), input.as_bytes())
                .unwrap()
                .to_time()
                .unwrap()
        };

        insta::assert_debug_snapshot!(
            p("%H:%M", "15:48"),
            @"15:48:00",
        );
        insta::assert_debug_snapshot!(
            p("%H:%M:%S", "15:48:59"),
            @"15:48:59",
        );
        insta::assert_debug_snapshot!(
            p("%H:%M:%S", "15:48:60"),
            @"15:48:59",
        );
        insta::assert_debug_snapshot!(
            p("%T", "15:48:59"),
            @"15:48:59",
        );
        insta::assert_debug_snapshot!(
            p("%R", "15:48"),
            @"15:48:00",
        );

        insta::assert_debug_snapshot!(
            p("%H %p", "5 am"),
            @"05:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%H%p", "5am"),
            @"05:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%H%p", "11pm"),
            @"23:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%I%p", "11pm"),
            @"23:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%I%p", "12am"),
            @"00:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%H%p", "23pm"),
            @"23:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%H%p", "23am"),
            @"11:00:00",
        );

        insta::assert_debug_snapshot!(
            p("%H:%M:%S%.f", "15:48:01.1"),
            @"15:48:01.1",
        );
        insta::assert_debug_snapshot!(
            p("%H:%M:%S%.255f", "15:48:01.1"),
            @"15:48:01.1",
        );
        insta::assert_debug_snapshot!(
            p("%H:%M:%S%255.255f", "15:48:01.1"),
            @"15:48:01.1",
        );
        insta::assert_debug_snapshot!(
            p("%H:%M:%S%.f", "15:48:01"),
            @"15:48:01",
        );
        insta::assert_debug_snapshot!(
            p("%H:%M:%S%.fa", "15:48:01a"),
            @"15:48:01",
        );
        insta::assert_debug_snapshot!(
            p("%H:%M:%S%.f", "15:48:01.123456789"),
            @"15:48:01.123456789",
        );
        insta::assert_debug_snapshot!(
            p("%H:%M:%S%.f", "15:48:01.000000001"),
            @"15:48:01.000000001",
        );

        insta::assert_debug_snapshot!(
            p("%H:%M:%S.%f", "15:48:01.1"),
            @"15:48:01.1",
        );
        insta::assert_debug_snapshot!(
            p("%H:%M:%S.%3f", "15:48:01.123"),
            @"15:48:01.123",
        );
        insta::assert_debug_snapshot!(
            p("%H:%M:%S.%3f", "15:48:01.123456"),
            @"15:48:01.123456",
        );

        insta::assert_debug_snapshot!(
            p("%H:%M:%S.%N", "15:48:01.1"),
            @"15:48:01.1",
        );
        insta::assert_debug_snapshot!(
            p("%H:%M:%S.%3N", "15:48:01.123"),
            @"15:48:01.123",
        );
        insta::assert_debug_snapshot!(
            p("%H:%M:%S.%3N", "15:48:01.123456"),
            @"15:48:01.123456",
        );

        insta::assert_debug_snapshot!(
            p("%H", "09"),
            @"09:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%H", " 9"),
            @"09:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%H", "15"),
            @"15:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%k", "09"),
            @"09:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%k", " 9"),
            @"09:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%k", "15"),
            @"15:00:00",
        );

        insta::assert_debug_snapshot!(
            p("%I", "09"),
            @"09:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%I", " 9"),
            @"09:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%l", "09"),
            @"09:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%l", " 9"),
            @"09:00:00",
        );
    }

    #[test]
    fn ok_parse_whitespace() {
        let p = |fmt: &str, input: &str| {
            BrokenDownTime::parse_mono(fmt.as_bytes(), input.as_bytes())
                .unwrap()
                .to_time()
                .unwrap()
        };

        insta::assert_debug_snapshot!(
            p("%H%M", "1548"),
            @"15:48:00",
        );
        insta::assert_debug_snapshot!(
            p("%H%M", "15\n48"),
            @"15:48:00",
        );
        insta::assert_debug_snapshot!(
            p("%H%M", "15\t48"),
            @"15:48:00",
        );
        insta::assert_debug_snapshot!(
            p("%H%n%M", "1548"),
            @"15:48:00",
        );
        insta::assert_debug_snapshot!(
            p("%H%n%M", "15\n48"),
            @"15:48:00",
        );
        insta::assert_debug_snapshot!(
            p("%H%n%M", "15\t48"),
            @"15:48:00",
        );
        insta::assert_debug_snapshot!(
            p("%H%t%M", "1548"),
            @"15:48:00",
        );
        insta::assert_debug_snapshot!(
            p("%H%t%M", "15\n48"),
            @"15:48:00",
        );
        insta::assert_debug_snapshot!(
            p("%H%t%M", "15\t48"),
            @"15:48:00",
        );
    }

    #[test]
    fn ok_parse_offset() {
        let p = |fmt: &str, input: &str| {
            BrokenDownTime::parse_mono(fmt.as_bytes(), input.as_bytes())
                .unwrap()
                .to_offset()
                .unwrap()
        };

        insta::assert_debug_snapshot!(
            p("%z", "+0530"),
            @"05:30:00",
        );
        insta::assert_debug_snapshot!(
            p("%z", "-0530"),
            @"-05:30:00",
        );
        insta::assert_debug_snapshot!(
            p("%z", "-0500"),
            @"-05:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%z", "+053015"),
            @"05:30:15",
        );
        insta::assert_debug_snapshot!(
            p("%z", "+050015"),
            @"05:00:15",
        );

        insta::assert_debug_snapshot!(
            p("%:z", "+05:30"),
            @"05:30:00",
        );
        insta::assert_debug_snapshot!(
            p("%:z", "-05:30"),
            @"-05:30:00",
        );
        insta::assert_debug_snapshot!(
            p("%:z", "-05:00"),
            @"-05:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%:z", "+05:30:15"),
            @"05:30:15",
        );
        insta::assert_debug_snapshot!(
            p("%:z", "-05:00:15"),
            @"-05:00:15",
        );

        insta::assert_debug_snapshot!(
            p("%::z", "+05:30:15"),
            @"05:30:15",
        );
        insta::assert_debug_snapshot!(
            p("%::z", "-05:30:15"),
            @"-05:30:15",
        );
        insta::assert_debug_snapshot!(
            p("%::z", "-05:00:00"),
            @"-05:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%::z", "-05:00:15"),
            @"-05:00:15",
        );

        insta::assert_debug_snapshot!(
            p("%:::z", "+05"),
            @"05:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%:::z", "-05"),
            @"-05:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%:::z", "+00"),
            @"00:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%:::z", "-00"),
            @"00:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%:::z", "+05:30"),
            @"05:30:00",
        );
        insta::assert_debug_snapshot!(
            p("%:::z", "-05:30"),
            @"-05:30:00",
        );
        insta::assert_debug_snapshot!(
            p("%:::z", "+05:30:15"),
            @"05:30:15",
        );
        insta::assert_debug_snapshot!(
            p("%:::z", "-05:30:15"),
            @"-05:30:15",
        );
        insta::assert_debug_snapshot!(
            p("%:::z", "-05:00:00"),
            @"-05:00:00",
        );
        insta::assert_debug_snapshot!(
            p("%:::z", "-05:00:15"),
            @"-05:00:15",
        );
    }

    #[test]
    fn err_parse() {
        let p = |fmt: &str, input: &str| {
            BrokenDownTime::parse_mono(fmt.as_bytes(), input.as_bytes())
                .unwrap_err()
                .to_string()
        };

        insta::assert_snapshot!(
            p("%M", ""),
            @"strptime parsing failed: expected non-empty input for directive %M, but found end of input",
        );
        insta::assert_snapshot!(
            p("%M", "a"),
            @"strptime parsing failed: %M failed: failed to parse minute: invalid number, no digits found",
        );
        insta::assert_snapshot!(
            p("%M%S", "15"),
            @"strptime parsing failed: expected non-empty input for directive %S, but found end of input",
        );
        insta::assert_snapshot!(
            p("%M%a", "Sun"),
            @"strptime parsing failed: %M failed: failed to parse minute: invalid number, no digits found",
        );

        insta::assert_snapshot!(
            p("%y", "999"),
            @r###"strptime expects to consume the entire input, but "9" remains unparsed"###,
        );
        insta::assert_snapshot!(
            p("%Y", "-10000"),
            @r###"strptime expects to consume the entire input, but "0" remains unparsed"###,
        );
        insta::assert_snapshot!(
            p("%Y", "10000"),
            @r###"strptime expects to consume the entire input, but "0" remains unparsed"###,
        );
        insta::assert_snapshot!(
            p("%A %m/%d/%y", "Mon 7/14/24"),
            @r#"strptime parsing failed: %A failed: unrecognized weekday abbreviation: failed to find expected choice at beginning of "Mon 7/14/24", available choices are: Sunday, Monday, Tuesday, Wednesday, Thursday, Friday, Saturday"#,
        );
        insta::assert_snapshot!(
            p("%b", "Bad"),
            @r###"strptime parsing failed: %b failed: expected to find month name abbreviation, but found "Bad" instead"###,
        );
        insta::assert_snapshot!(
            p("%h", "July"),
            @r###"strptime expects to consume the entire input, but "y" remains unparsed"###,
        );
        insta::assert_snapshot!(
            p("%B", "Jul"),
            @r###"strptime parsing failed: %B failed: unrecognized month name: failed to find expected choice at beginning of "Jul", available choices are: January, February, March, April, May, June, July, August, September, October, November, December"###,
        );
        insta::assert_snapshot!(
            p("%H", "24"),
            @"strptime parsing failed: %H failed: hour number is invalid: parameter 'hour' with value 24 is not in the required range of 0..=23",
        );
        insta::assert_snapshot!(
            p("%M", "60"),
            @"strptime parsing failed: %M failed: minute number is invalid: parameter 'minute' with value 60 is not in the required range of 0..=59",
        );
        insta::assert_snapshot!(
            p("%S", "61"),
            @"strptime parsing failed: %S failed: second number is invalid: parameter 'second' with value 61 is not in the required range of 0..=59",
        );
        insta::assert_snapshot!(
            p("%I", "0"),
            @"strptime parsing failed: %I failed: hour number is invalid: parameter 'hour' with value 0 is not in the required range of 1..=12",
        );
        insta::assert_snapshot!(
            p("%I", "13"),
            @"strptime parsing failed: %I failed: hour number is invalid: parameter 'hour' with value 13 is not in the required range of 1..=12",
        );
        insta::assert_snapshot!(
            p("%p", "aa"),
            @r###"strptime parsing failed: %p failed: expected to find AM or PM, but found "aa" instead"###,
        );

        insta::assert_snapshot!(
            p("%_", " "),
            @r###"strptime parsing failed: expected to find specifier directive after flag "_", but found end of format string"###,
        );
        insta::assert_snapshot!(
            p("%-", " "),
            @r###"strptime parsing failed: expected to find specifier directive after flag "-", but found end of format string"###,
        );
        insta::assert_snapshot!(
            p("%0", " "),
            @r###"strptime parsing failed: expected to find specifier directive after flag "0", but found end of format string"###,
        );
        insta::assert_snapshot!(
            p("%^", " "),
            @r###"strptime parsing failed: expected to find specifier directive after flag "^", but found end of format string"###,
        );
        insta::assert_snapshot!(
            p("%#", " "),
            @r###"strptime parsing failed: expected to find specifier directive after flag "#", but found end of format string"###,
        );
        insta::assert_snapshot!(
            p("%_1", " "),
            @"strptime parsing failed: expected to find specifier directive after width 1, but found end of format string",
        );
        insta::assert_snapshot!(
            p("%_23", " "),
            @"strptime parsing failed: expected to find specifier directive after width 23, but found end of format string",
        );

        insta::assert_snapshot!(
            p("%H:%M:%S%.f", "15:59:01."),
            @"strptime parsing failed: %.f failed: expected at least one fractional decimal digit, but did not find any",
        );
        insta::assert_snapshot!(
            p("%H:%M:%S%.f", "15:59:01.a"),
            @"strptime parsing failed: %.f failed: expected at least one fractional decimal digit, but did not find any",
        );
        insta::assert_snapshot!(
            p("%H:%M:%S%.f", "15:59:01.1234567891"),
            @r###"strptime expects to consume the entire input, but "1" remains unparsed"###,
        );
        insta::assert_snapshot!(
            p("%H:%M:%S.%f", "15:59:01."),
            @"strptime parsing failed: expected non-empty input for directive %f, but found end of input",
        );
        insta::assert_snapshot!(
            p("%H:%M:%S.%f", "15:59:01"),
            @r###"strptime parsing failed: expected to match literal byte "." from format string, but found end of input"###,
        );
        insta::assert_snapshot!(
            p("%H:%M:%S.%f", "15:59:01.a"),
            @"strptime parsing failed: %f failed: expected at least one fractional decimal digit, but did not find any",
        );
        insta::assert_snapshot!(
            p("%H:%M:%S.%N", "15:59:01."),
            @"strptime parsing failed: expected non-empty input for directive %N, but found end of input",
        );
        insta::assert_snapshot!(
            p("%H:%M:%S.%N", "15:59:01"),
            @r###"strptime parsing failed: expected to match literal byte "." from format string, but found end of input"###,
        );
        insta::assert_snapshot!(
            p("%H:%M:%S.%N", "15:59:01.a"),
            @"strptime parsing failed: %N failed: expected at least one fractional decimal digit, but did not find any",
        );

        insta::assert_snapshot!(
            p("%Q", "+America/New_York"),
            @r#"strptime parsing failed: %Q failed: failed to parse hours in UTC numeric offset "+America/New_York": failed to parse "Am" as hours (a two digit integer): invalid digit, expected 0-9 but got A"#,
        );
        insta::assert_snapshot!(
            p("%Q", "-America/New_York"),
            @r#"strptime parsing failed: %Q failed: failed to parse hours in UTC numeric offset "-America/New_York": failed to parse "Am" as hours (a two digit integer): invalid digit, expected 0-9 but got A"#,
        );
        insta::assert_snapshot!(
            p("%:Q", "+0400"),
            @r#"strptime parsing failed: %:Q failed: parsed hour component of time zone offset from "+0400", but could not find required colon separator"#,
        );
        insta::assert_snapshot!(
            p("%Q", "+04:00"),
            @r#"strptime parsing failed: %Q failed: parsed hour component of time zone offset from "+04:00", but found colon after hours which is not allowed"#,
        );
        insta::assert_snapshot!(
            p("%Q", "America/"),
            @"strptime parsing failed: %Q failed: expected the start of an IANA time zone identifier name or component, but found end of input instead",
        );
        insta::assert_snapshot!(
            p("%Q", "America/+"),
            @r###"strptime parsing failed: %Q failed: expected the start of an IANA time zone identifier name or component, but found "+" instead"###,
        );

        insta::assert_snapshot!(
            p("%s", "-377705023202"),
            @"strptime parsing failed: %s failed: parsed Unix timestamp `-377705023202`, but out of range of valid Jiff `Timestamp`: parameter 'second' with value -377705023202 is not in the required range of -377705023201..=253402207200",
        );
        insta::assert_snapshot!(
            p("%s", "253402207201"),
            @"strptime parsing failed: %s failed: parsed Unix timestamp `253402207201`, but out of range of valid Jiff `Timestamp`: parameter 'second' with value 253402207201 is not in the required range of -377705023201..=253402207200",
        );
        insta::assert_snapshot!(
            p("%s", "-9999999999999999999"),
            @"strptime parsing failed: %s failed: failed to parse Unix timestamp (in seconds): number '9999999999999999999' too big to parse into 64-bit integer",
        );
        insta::assert_snapshot!(
            p("%s", "9999999999999999999"),
            @"strptime parsing failed: %s failed: failed to parse Unix timestamp (in seconds): number '9999999999999999999' too big to parse into 64-bit integer",
        );

        insta::assert_snapshot!(
            p("%u", "0"),
            @"strptime parsing failed: %u failed: weekday number is invalid: parameter 'weekday' with value 0 is not in the required range of 1..=7",
        );
        insta::assert_snapshot!(
            p("%w", "7"),
            @"strptime parsing failed: %w failed: weekday number is invalid: parameter 'weekday' with value 7 is not in the required range of 0..=6",
        );
        insta::assert_snapshot!(
            p("%u", "128"),
            @r###"strptime expects to consume the entire input, but "28" remains unparsed"###,
        );
        insta::assert_snapshot!(
            p("%w", "128"),
            @r###"strptime expects to consume the entire input, but "28" remains unparsed"###,
        );
    }

    #[test]
    fn err_parse_date() {
        let p = |fmt: &str, input: &str| {
            BrokenDownTime::parse_mono(fmt.as_bytes(), input.as_bytes())
                .unwrap()
                .to_date()
                .unwrap_err()
                .to_string()
        };

        insta::assert_snapshot!(
            p("%Y", "2024"),
            @"a month/day, day-of-year or week date must be present to create a date, but none were found",
        );
        insta::assert_snapshot!(
            p("%m", "7"),
            @"missing year, date cannot be created",
        );
        insta::assert_snapshot!(
            p("%d", "25"),
            @"missing year, date cannot be created",
        );
        insta::assert_snapshot!(
            p("%Y-%m", "2024-7"),
            @"a month/day, day-of-year or week date must be present to create a date, but none were found",
        );
        insta::assert_snapshot!(
            p("%Y-%d", "2024-25"),
            @"a month/day, day-of-year or week date must be present to create a date, but none were found",
        );
        insta::assert_snapshot!(
            p("%m-%d", "7-25"),
            @"missing year, date cannot be created",
        );

        insta::assert_snapshot!(
            p("%m/%d/%y", "6/31/24"),
            @"invalid date: parameter 'day' with value 31 is not in the required range of 1..=30",
        );
        insta::assert_snapshot!(
            p("%m/%d/%y", "2/29/23"),
            @"invalid date: parameter 'day' with value 29 is not in the required range of 1..=28",
        );
        insta::assert_snapshot!(
            p("%a %m/%d/%y", "Mon 7/14/24"),
            @"parsed weekday Monday does not match weekday Sunday from parsed date 2024-07-14",
        );
        insta::assert_snapshot!(
            p("%A %m/%d/%y", "Monday 7/14/24"),
            @"parsed weekday Monday does not match weekday Sunday from parsed date 2024-07-14",
        );

        insta::assert_snapshot!(
            p("%Y-%U-%u", "2025-00-2"),
            @"weekday `Tuesday` is not valid for Sunday based week number `0` in year `2025`",
        );
        insta::assert_snapshot!(
            p("%Y-%W-%u", "2025-00-2"),
            @"weekday `Tuesday` is not valid for Monday based week number `0` in year `2025`",
        );
    }

    #[test]
    fn err_parse_time() {
        let p = |fmt: &str, input: &str| {
            BrokenDownTime::parse_mono(fmt.as_bytes(), input.as_bytes())
                .unwrap()
                .to_time()
                .unwrap_err()
                .to_string()
        };

        insta::assert_snapshot!(
            p("%M", "59"),
            @"parsing format did not include hour directive, but did include minute directive (cannot have smaller time units with bigger time units missing)",
        );
        insta::assert_snapshot!(
            p("%S", "59"),
            @"parsing format did not include hour directive, but did include second directive (cannot have smaller time units with bigger time units missing)",
        );
        insta::assert_snapshot!(
            p("%M:%S", "59:59"),
            @"parsing format did not include hour directive, but did include minute directive (cannot have smaller time units with bigger time units missing)",
        );
        insta::assert_snapshot!(
            p("%H:%S", "15:59"),
            @"parsing format did not include minute directive, but did include second directive (cannot have smaller time units with bigger time units missing)",
        );
    }

    #[test]
    fn err_parse_offset() {
        let p = |fmt: &str, input: &str| {
            BrokenDownTime::parse_mono(fmt.as_bytes(), input.as_bytes())
                .unwrap_err()
                .to_string()
        };

        insta::assert_snapshot!(
            p("%z", "+05:30"),
            @r#"strptime parsing failed: %z failed: parsed hour component of time zone offset from "+05:30", but found colon after hours which is not allowed"#,
        );
        insta::assert_snapshot!(
            p("%:z", "+0530"),
            @r#"strptime parsing failed: %:z failed: parsed hour component of time zone offset from "+0530", but could not find required colon separator"#,
        );
        insta::assert_snapshot!(
            p("%::z", "+0530"),
            @r#"strptime parsing failed: %::z failed: parsed hour component of time zone offset from "+0530", but could not find required colon separator"#,
        );
        insta::assert_snapshot!(
            p("%:::z", "+0530"),
            @r#"strptime parsing failed: %:::z failed: parsed hour component of time zone offset from "+0530", but could not find required colon separator"#,
        );

        insta::assert_snapshot!(
            p("%z", "+05"),
            @r#"strptime parsing failed: %z failed: parsed hour component of time zone offset from "+05", but could not find required minute component"#,
        );
        insta::assert_snapshot!(
            p("%:z", "+05"),
            @r#"strptime parsing failed: %:z failed: parsed hour component of time zone offset from "+05", but could not find required minute component"#,
        );
        insta::assert_snapshot!(
            p("%::z", "+05"),
            @r#"strptime parsing failed: %::z failed: parsed hour component of time zone offset from "+05", but could not find required minute component"#,
        );
        insta::assert_snapshot!(
            p("%::z", "+05:30"),
            @r#"strptime parsing failed: %::z failed: parsed hour and minute components of time zone offset from "+05:30", but could not find required second component"#,
        );
        insta::assert_snapshot!(
            p("%:::z", "+5"),
            @r#"strptime parsing failed: %:::z failed: failed to parse hours in UTC numeric offset "+5": expected two digit hour after sign, but found end of input"#,
        );

        insta::assert_snapshot!(
            p("%z", "+0530:15"),
            @r#"strptime expects to consume the entire input, but ":15" remains unparsed"#,
        );
        insta::assert_snapshot!(
            p("%:z", "+05:3015"),
            @r#"strptime expects to consume the entire input, but "15" remains unparsed"#,
        );
        insta::assert_snapshot!(
            p("%::z", "+05:3015"),
            @r#"strptime parsing failed: %::z failed: parsed hour and minute components of time zone offset from "+05:3015", but could not find required second component"#,
        );
        insta::assert_snapshot!(
            p("%:::z", "+05:3015"),
            @r#"strptime expects to consume the entire input, but "15" remains unparsed"#,
        );
    }
}
