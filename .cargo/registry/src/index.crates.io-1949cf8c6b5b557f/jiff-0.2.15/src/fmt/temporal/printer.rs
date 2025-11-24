use crate::{
    civil::{Date, DateTime, Time},
    error::{err, Error},
    fmt::{
        temporal::{Pieces, PiecesOffset, TimeZoneAnnotationKind},
        util::{DecimalFormatter, FractionalFormatter},
        Write, WriteExt,
    },
    span::Span,
    tz::{Offset, TimeZone},
    util::{
        rangeint::RFrom,
        t::{self, C},
    },
    SignedDuration, Timestamp, Zoned,
};

#[derive(Clone, Debug)]
pub(super) struct DateTimePrinter {
    lowercase: bool,
    separator: u8,
    rfc9557: bool,
    precision: Option<u8>,
}

impl DateTimePrinter {
    pub(super) const fn new() -> DateTimePrinter {
        DateTimePrinter {
            lowercase: false,
            separator: b'T',
            rfc9557: true,
            precision: None,
        }
    }

    pub(super) const fn lowercase(self, yes: bool) -> DateTimePrinter {
        DateTimePrinter { lowercase: yes, ..self }
    }

    pub(super) const fn separator(self, ascii_char: u8) -> DateTimePrinter {
        assert!(ascii_char.is_ascii(), "RFC3339 separator must be ASCII");
        DateTimePrinter { separator: ascii_char, ..self }
    }

    pub(super) const fn precision(
        self,
        precision: Option<u8>,
    ) -> DateTimePrinter {
        DateTimePrinter { precision, ..self }
    }

    pub(super) fn print_zoned<W: Write>(
        &self,
        zdt: &Zoned,
        mut wtr: W,
    ) -> Result<(), Error> {
        let timestamp = zdt.timestamp();
        let tz = zdt.time_zone();
        let offset = tz.to_offset(timestamp);
        let dt = offset.to_datetime(timestamp);
        self.print_datetime(&dt, &mut wtr)?;
        if tz.is_unknown() {
            wtr.write_str("Z[Etc/Unknown]")?;
        } else {
            self.print_offset_rounded(&offset, &mut wtr)?;
            self.print_time_zone_annotation(&tz, &offset, &mut wtr)?;
        }
        Ok(())
    }

    pub(super) fn print_timestamp<W: Write>(
        &self,
        timestamp: &Timestamp,
        offset: Option<Offset>,
        mut wtr: W,
    ) -> Result<(), Error> {
        let Some(offset) = offset else {
            let dt = TimeZone::UTC.to_datetime(*timestamp);
            self.print_datetime(&dt, &mut wtr)?;
            self.print_zulu(&mut wtr)?;
            return Ok(());
        };
        let dt = offset.to_datetime(*timestamp);
        self.print_datetime(&dt, &mut wtr)?;
        self.print_offset_rounded(&offset, &mut wtr)?;
        Ok(())
    }

    /// Formats the given datetime into the writer given.
    pub(super) fn print_datetime<W: Write>(
        &self,
        dt: &DateTime,
        mut wtr: W,
    ) -> Result<(), Error> {
        self.print_date(&dt.date(), &mut wtr)?;
        wtr.write_char(char::from(if self.lowercase {
            self.separator.to_ascii_lowercase()
        } else {
            self.separator
        }))?;
        self.print_time(&dt.time(), &mut wtr)?;
        Ok(())
    }

    /// Formats the given date into the writer given.
    pub(super) fn print_date<W: Write>(
        &self,
        date: &Date,
        mut wtr: W,
    ) -> Result<(), Error> {
        static FMT_YEAR_POSITIVE: DecimalFormatter =
            DecimalFormatter::new().padding(4);
        static FMT_YEAR_NEGATIVE: DecimalFormatter =
            DecimalFormatter::new().padding(6);
        static FMT_TWO: DecimalFormatter = DecimalFormatter::new().padding(2);

        if date.year() >= 0 {
            wtr.write_int(&FMT_YEAR_POSITIVE, date.year())?;
        } else {
            wtr.write_int(&FMT_YEAR_NEGATIVE, date.year())?;
        }
        wtr.write_str("-")?;
        wtr.write_int(&FMT_TWO, date.month())?;
        wtr.write_str("-")?;
        wtr.write_int(&FMT_TWO, date.day())?;
        Ok(())
    }

    /// Formats the given time into the writer given.
    pub(super) fn print_time<W: Write>(
        &self,
        time: &Time,
        mut wtr: W,
    ) -> Result<(), Error> {
        static FMT_TWO: DecimalFormatter = DecimalFormatter::new().padding(2);
        static FMT_FRACTION: FractionalFormatter = FractionalFormatter::new();

        wtr.write_int(&FMT_TWO, time.hour())?;
        wtr.write_str(":")?;
        wtr.write_int(&FMT_TWO, time.minute())?;
        wtr.write_str(":")?;
        wtr.write_int(&FMT_TWO, time.second())?;
        let fractional_nanosecond = time.subsec_nanosecond();
        if self.precision.map_or(fractional_nanosecond != 0, |p| p > 0) {
            wtr.write_str(".")?;
            wtr.write_fraction(
                &FMT_FRACTION.precision(self.precision),
                fractional_nanosecond,
            )?;
        }
        Ok(())
    }

    /// Formats the given time zone into the writer given.
    pub(super) fn print_time_zone<W: Write>(
        &self,
        tz: &TimeZone,
        mut wtr: W,
    ) -> Result<(), Error> {
        if let Some(iana_name) = tz.iana_name() {
            return wtr.write_str(iana_name);
        }
        if tz.is_unknown() {
            return wtr.write_str("Etc/Unknown");
        }
        if let Ok(offset) = tz.to_fixed_offset() {
            return self.print_offset_full_precision(&offset, wtr);
        }
        // We get this on `alloc` because we format the POSIX time zone into a
        // `String` first. See the note below.
        //
        // This is generally okay because there is no current (2025-02-28) way
        // to create a `TimeZone` that is *only* a POSIX time zone in core-only
        // environments. (All you can do is create a TZif time zone, which may
        // contain a POSIX time zone, but `tz.posix_tz()` would still return
        // `None` in that case.)
        #[cfg(feature = "alloc")]
        {
            if let Some(posix_tz) = tz.posix_tz() {
                // This is pretty unfortunate, but at time of writing, I
                // didn't see an easy way to make the `Display` impl for
                // `PosixTimeZone` automatically work with
                // `jiff::fmt::Write` without allocating a new string. As
                // far as I can see, I either have to duplicate the code or
                // make it generic in some way. I judged neither to be worth
                // doing for such a rare case. ---AG
                let s = alloc::string::ToString::to_string(posix_tz);
                return wtr.write_str(&s);
            }
        }
        // Ideally this never actually happens, but it can, and there
        // are likely system configurations out there in which it does.
        // I can imagine "lightweight" installations that just have a
        // `/etc/localtime` as a TZif file that doesn't point to any IANA time
        // zone. In which case, serializing a time zone probably doesn't make
        // much sense.
        //
        // Anyway, if you're seeing this error and think there should be a
        // different behavior, please file an issue.
        Err(err!(
            "time zones without IANA identifiers that aren't either \
             fixed offsets or a POSIX time zone can't be serialized \
             (this typically occurs when this is a system time zone \
              derived from `/etc/localtime` on Unix systems that \
              isn't symlinked to an entry in `/usr/share/zoneinfo`)",
        ))
    }

    pub(super) fn print_pieces<W: Write>(
        &self,
        pieces: &Pieces,
        mut wtr: W,
    ) -> Result<(), Error> {
        if let Some(time) = pieces.time() {
            let dt = DateTime::from_parts(pieces.date(), time);
            self.print_datetime(&dt, &mut wtr)?;
            if let Some(poffset) = pieces.offset() {
                self.print_pieces_offset(&poffset, &mut wtr)?;
            }
        } else if let Some(poffset) = pieces.offset() {
            // In this case, we have an offset but no time component. Since
            // `2025-01-02-05:00` isn't valid, we forcefully write out the
            // default time (which is what would be assumed anyway).
            let dt = DateTime::from_parts(pieces.date(), Time::midnight());
            self.print_datetime(&dt, &mut wtr)?;
            self.print_pieces_offset(&poffset, &mut wtr)?;
        } else {
            // We have no time and no offset, so we can just write the date.
            // It's okay to write this followed by an annotation, e.g.,
            // `2025-01-02[America/New_York]` or even `2025-01-02[-05:00]`.
            self.print_date(&pieces.date(), &mut wtr)?;
        }
        // For the time zone annotation, a `Pieces` gives us the annotation
        // name or offset directly, where as with `Zoned`, we have a
        // `TimeZone`. So we hand-roll our own formatter directly from the
        // annotation.
        if let Some(ann) = pieces.time_zone_annotation() {
            // Note that we explicitly ignore `self.rfc9557` here, since with
            // `Pieces`, the annotation has been explicitly provided. Also,
            // at time of writing, `self.rfc9557` is always enabled anyway.
            wtr.write_str("[")?;
            if ann.is_critical() {
                wtr.write_str("!")?;
            }
            match *ann.kind() {
                TimeZoneAnnotationKind::Named(ref name) => {
                    wtr.write_str(name.as_str())?
                }
                TimeZoneAnnotationKind::Offset(offset) => {
                    self.print_offset_rounded(&offset, &mut wtr)?
                }
            }
            wtr.write_str("]")?;
        }
        Ok(())
    }

    /// Formats the given "pieces" offset into the writer given.
    fn print_pieces_offset<W: Write>(
        &self,
        poffset: &PiecesOffset,
        mut wtr: W,
    ) -> Result<(), Error> {
        match *poffset {
            PiecesOffset::Zulu => self.print_zulu(wtr),
            PiecesOffset::Numeric(ref noffset) => {
                if noffset.offset().is_zero() && noffset.is_negative() {
                    wtr.write_str("-00:00")
                } else {
                    self.print_offset_rounded(&noffset.offset(), wtr)
                }
            }
        }
    }

    /// Formats the given offset into the writer given.
    ///
    /// If the given offset has non-zero seconds, then they are rounded to
    /// the nearest minute.
    fn print_offset_rounded<W: Write>(
        &self,
        offset: &Offset,
        mut wtr: W,
    ) -> Result<(), Error> {
        static FMT_TWO: DecimalFormatter = DecimalFormatter::new().padding(2);

        wtr.write_str(if offset.is_negative() { "-" } else { "+" })?;
        let mut hours = offset.part_hours_ranged().abs().get();
        let mut minutes = offset.part_minutes_ranged().abs().get();
        // RFC 3339 requires that time zone offsets are an integral number
        // of minutes. While rounding based on seconds doesn't seem clearly
        // indicated, the `1937-01-01T12:00:27.87+00:20` example seems
        // to suggest that the number of minutes should be "as close as
        // possible" to the actual offset. So we just do basic rounding
        // here.
        if offset.part_seconds_ranged().abs() >= C(30) {
            if minutes == 59 {
                hours = hours.saturating_add(1);
                minutes = 0;
            } else {
                minutes = minutes.saturating_add(1);
            }
        }
        wtr.write_int(&FMT_TWO, hours)?;
        wtr.write_str(":")?;
        wtr.write_int(&FMT_TWO, minutes)?;
        Ok(())
    }

    /// Formats the given offset into the writer given.
    ///
    /// If the given offset has non-zero seconds, then they are emitted as a
    /// third `:`-delimited component of the offset. If seconds are zero, then
    /// only the hours and minute components are emitted.
    fn print_offset_full_precision<W: Write>(
        &self,
        offset: &Offset,
        mut wtr: W,
    ) -> Result<(), Error> {
        static FMT_TWO: DecimalFormatter = DecimalFormatter::new().padding(2);

        wtr.write_str(if offset.is_negative() { "-" } else { "+" })?;
        let hours = offset.part_hours_ranged().abs().get();
        let minutes = offset.part_minutes_ranged().abs().get();
        let seconds = offset.part_seconds_ranged().abs().get();
        wtr.write_int(&FMT_TWO, hours)?;
        wtr.write_str(":")?;
        wtr.write_int(&FMT_TWO, minutes)?;
        if seconds > 0 {
            wtr.write_str(":")?;
            wtr.write_int(&FMT_TWO, seconds)?;
        }
        Ok(())
    }

    /// Prints the "zulu" indicator.
    ///
    /// This should only be used when the offset is not known. For example,
    /// when printing a `Timestamp`.
    fn print_zulu<W: Write>(&self, mut wtr: W) -> Result<(), Error> {
        wtr.write_str(if self.lowercase { "z" } else { "Z" })
    }

    /// Formats the given time zone name into the writer given as an RFC 9557
    /// time zone annotation.
    ///
    /// This is a no-op when RFC 9557 support isn't enabled. And when the given
    /// time zone is not an IANA time zone name, then the offset is printed
    /// instead. (This means the offset will be printed twice, which is indeed
    /// an intended behavior of RFC 9557 for cases where a time zone name is
    /// not used or unavailable.)
    fn print_time_zone_annotation<W: Write>(
        &self,
        time_zone: &TimeZone,
        offset: &Offset,
        mut wtr: W,
    ) -> Result<(), Error> {
        if !self.rfc9557 {
            return Ok(());
        }
        wtr.write_str("[")?;
        if let Some(iana_name) = time_zone.iana_name() {
            wtr.write_str(iana_name)?;
        } else {
            self.print_offset_rounded(offset, &mut wtr)?;
        }
        wtr.write_str("]")?;
        Ok(())
    }
}

impl Default for DateTimePrinter {
    fn default() -> DateTimePrinter {
        DateTimePrinter::new()
    }
}

/// A printer for Temporal spans.
///
/// Note that in Temporal, a "span" is called a "duration."
#[derive(Debug)]
pub(super) struct SpanPrinter {
    /// Whether to use lowercase unit designators.
    lowercase: bool,
}

impl SpanPrinter {
    /// Create a new Temporal span printer with the default configuration.
    pub(super) const fn new() -> SpanPrinter {
        SpanPrinter { lowercase: false }
    }

    /// Use lowercase for unit designator labels.
    ///
    /// By default, unit designator labels are written in uppercase.
    pub(super) const fn lowercase(self, yes: bool) -> SpanPrinter {
        SpanPrinter { lowercase: yes }
    }

    /// Print the given span to the writer given.
    ///
    /// This only returns an error when the given writer returns an error.
    pub(super) fn print_span<W: Write>(
        &self,
        span: &Span,
        mut wtr: W,
    ) -> Result<(), Error> {
        static FMT_INT: DecimalFormatter = DecimalFormatter::new();
        static FMT_FRACTION: FractionalFormatter = FractionalFormatter::new();

        if span.is_negative() {
            wtr.write_str("-")?;
        }
        wtr.write_str("P")?;

        let mut non_zero_greater_than_second = false;
        if span.get_years_ranged() != C(0) {
            wtr.write_int(&FMT_INT, span.get_years_ranged().get().abs())?;
            wtr.write_char(self.label('Y'))?;
            non_zero_greater_than_second = true;
        }
        if span.get_months_ranged() != C(0) {
            wtr.write_int(&FMT_INT, span.get_months_ranged().get().abs())?;
            wtr.write_char(self.label('M'))?;
            non_zero_greater_than_second = true;
        }
        if span.get_weeks_ranged() != C(0) {
            wtr.write_int(&FMT_INT, span.get_weeks_ranged().get().abs())?;
            wtr.write_char(self.label('W'))?;
            non_zero_greater_than_second = true;
        }
        if span.get_days_ranged() != C(0) {
            wtr.write_int(&FMT_INT, span.get_days_ranged().get().abs())?;
            wtr.write_char(self.label('D'))?;
            non_zero_greater_than_second = true;
        }

        let mut printed_time_prefix = false;
        if span.get_hours_ranged() != C(0) {
            if !printed_time_prefix {
                wtr.write_str("T")?;
                printed_time_prefix = true;
            }
            wtr.write_int(&FMT_INT, span.get_hours_ranged().get().abs())?;
            wtr.write_char(self.label('H'))?;
            non_zero_greater_than_second = true;
        }
        if span.get_minutes_ranged() != C(0) {
            if !printed_time_prefix {
                wtr.write_str("T")?;
                printed_time_prefix = true;
            }
            wtr.write_int(&FMT_INT, span.get_minutes_ranged().get().abs())?;
            wtr.write_char(self.label('M'))?;
            non_zero_greater_than_second = true;
        }

        // ISO 8601 (and Temporal) don't support writing out milliseconds,
        // microseconds or nanoseconds as separate components like for all
        // the other units. Instead, they must be incorporated as fractional
        // seconds. But we only want to do that work if we need to.
        let (seconds, millis, micros, nanos) = (
            span.get_seconds_ranged().abs(),
            span.get_milliseconds_ranged().abs(),
            span.get_microseconds_ranged().abs(),
            span.get_nanoseconds_ranged().abs(),
        );
        if (seconds != C(0) || !non_zero_greater_than_second)
            && millis == C(0)
            && micros == C(0)
            && nanos == C(0)
        {
            if !printed_time_prefix {
                wtr.write_str("T")?;
            }
            wtr.write_int(&FMT_INT, seconds.get())?;
            wtr.write_char(self.label('S'))?;
        } else if millis != C(0) || micros != C(0) || nanos != C(0) {
            if !printed_time_prefix {
                wtr.write_str("T")?;
            }
            // We want to combine our seconds, milliseconds, microseconds and
            // nanoseconds into one single value in terms of nanoseconds. Then
            // we can "balance" that out so that we have a number of seconds
            // and a number of nanoseconds not greater than 1 second. (Which is
            // our fraction.)
            let combined_as_nanos =
                t::SpanSecondsOrLowerNanoseconds::rfrom(nanos)
                    + (t::SpanSecondsOrLowerNanoseconds::rfrom(micros)
                        * t::NANOS_PER_MICRO)
                    + (t::SpanSecondsOrLowerNanoseconds::rfrom(millis)
                        * t::NANOS_PER_MILLI)
                    + (t::SpanSecondsOrLowerNanoseconds::rfrom(seconds)
                        * t::NANOS_PER_SECOND);
            let fraction_second = t::SpanSecondsOrLower::rfrom(
                combined_as_nanos / t::NANOS_PER_SECOND,
            );
            let fraction_nano = t::SubsecNanosecond::rfrom(
                combined_as_nanos % t::NANOS_PER_SECOND,
            );
            wtr.write_int(&FMT_INT, fraction_second.get())?;
            if fraction_nano != C(0) {
                wtr.write_str(".")?;
                wtr.write_fraction(&FMT_FRACTION, fraction_nano.get())?;
            }
            wtr.write_char(self.label('S'))?;
        }
        Ok(())
    }

    /// Print the given signed duration to the writer given.
    ///
    /// This only returns an error when the given writer returns an error.
    pub(super) fn print_duration<W: Write>(
        &self,
        dur: &SignedDuration,
        mut wtr: W,
    ) -> Result<(), Error> {
        static FMT_INT: DecimalFormatter = DecimalFormatter::new();
        static FMT_FRACTION: FractionalFormatter = FractionalFormatter::new();

        let mut non_zero_greater_than_second = false;
        if dur.is_negative() {
            wtr.write_str("-")?;
        }
        wtr.write_str("PT")?;

        let mut secs = dur.as_secs();
        // OK because subsec_nanos -999_999_999<=nanos<=999_999_999.
        let nanos = dur.subsec_nanos().abs();
        // OK because guaranteed to be bigger than i64::MIN.
        let hours = (secs / (60 * 60)).abs();
        secs %= 60 * 60;
        // OK because guaranteed to be bigger than i64::MIN.
        let minutes = (secs / 60).abs();
        // OK because guaranteed to be bigger than i64::MIN.
        secs = (secs % 60).abs();
        if hours != 0 {
            wtr.write_int(&FMT_INT, hours)?;
            wtr.write_char(self.label('H'))?;
            non_zero_greater_than_second = true;
        }
        if minutes != 0 {
            wtr.write_int(&FMT_INT, minutes)?;
            wtr.write_char(self.label('M'))?;
            non_zero_greater_than_second = true;
        }
        if (secs != 0 || !non_zero_greater_than_second) && nanos == 0 {
            wtr.write_int(&FMT_INT, secs)?;
            wtr.write_char(self.label('S'))?;
        } else if nanos != 0 {
            wtr.write_int(&FMT_INT, secs)?;
            wtr.write_str(".")?;
            wtr.write_fraction(&FMT_FRACTION, nanos)?;
            wtr.write_char(self.label('S'))?;
        }
        Ok(())
    }

    /// Converts the uppercase unit designator label to lowercase if this
    /// printer is configured to use lowercase. Otherwise the label is returned
    /// unchanged.
    fn label(&self, upper: char) -> char {
        debug_assert!(upper.is_ascii());
        if self.lowercase {
            upper.to_ascii_lowercase()
        } else {
            upper
        }
    }
}

#[cfg(feature = "alloc")]
#[cfg(test)]
mod tests {
    use alloc::string::String;

    use crate::{civil::date, span::ToSpan};

    use super::*;

    #[test]
    fn print_zoned() {
        if crate::tz::db().is_definitively_empty() {
            return;
        }

        let dt = date(2024, 3, 10).at(5, 34, 45, 0);
        let zoned: Zoned = dt.in_tz("America/New_York").unwrap();
        let mut buf = String::new();
        DateTimePrinter::new().print_zoned(&zoned, &mut buf).unwrap();
        assert_eq!(buf, "2024-03-10T05:34:45-04:00[America/New_York]");

        let dt = date(2024, 3, 10).at(5, 34, 45, 0);
        let zoned: Zoned = dt.in_tz("America/New_York").unwrap();
        let zoned = zoned.with_time_zone(TimeZone::UTC);
        let mut buf = String::new();
        DateTimePrinter::new().print_zoned(&zoned, &mut buf).unwrap();
        assert_eq!(buf, "2024-03-10T09:34:45+00:00[UTC]");
    }

    #[test]
    fn print_timestamp() {
        if crate::tz::db().is_definitively_empty() {
            return;
        }

        let dt = date(2024, 3, 10).at(5, 34, 45, 0);
        let zoned: Zoned = dt.in_tz("America/New_York").unwrap();
        let mut buf = String::new();
        DateTimePrinter::new()
            .print_timestamp(&zoned.timestamp(), None, &mut buf)
            .unwrap();
        assert_eq!(buf, "2024-03-10T09:34:45Z");

        let dt = date(-2024, 3, 10).at(5, 34, 45, 0);
        let zoned: Zoned = dt.in_tz("America/New_York").unwrap();
        let mut buf = String::new();
        DateTimePrinter::new()
            .print_timestamp(&zoned.timestamp(), None, &mut buf)
            .unwrap();
        assert_eq!(buf, "-002024-03-10T10:30:47Z");
    }

    #[test]
    fn print_span_basic() {
        let p = |span: Span| -> String {
            let mut buf = String::new();
            SpanPrinter::new().print_span(&span, &mut buf).unwrap();
            buf
        };

        insta::assert_snapshot!(p(Span::new()), @"PT0S");
        insta::assert_snapshot!(p(1.second()), @"PT1S");
        insta::assert_snapshot!(p(-1.second()), @"-PT1S");
        insta::assert_snapshot!(p(
            1.second().milliseconds(1).microseconds(1).nanoseconds(1),
        ), @"PT1.001001001S");
        insta::assert_snapshot!(p(
            0.second().milliseconds(999).microseconds(999).nanoseconds(999),
        ), @"PT0.999999999S");
        insta::assert_snapshot!(p(
            1.year().months(1).weeks(1).days(1)
            .hours(1).minutes(1).seconds(1)
            .milliseconds(1).microseconds(1).nanoseconds(1),
        ), @"P1Y1M1W1DT1H1M1.001001001S");
        insta::assert_snapshot!(p(
            -1.year().months(1).weeks(1).days(1)
            .hours(1).minutes(1).seconds(1)
            .milliseconds(1).microseconds(1).nanoseconds(1),
        ), @"-P1Y1M1W1DT1H1M1.001001001S");
    }

    #[test]
    fn print_span_subsecond_positive() {
        let p = |span: Span| -> String {
            let mut buf = String::new();
            SpanPrinter::new().print_span(&span, &mut buf).unwrap();
            buf
        };

        // These are all sub-second trickery tests.
        insta::assert_snapshot!(p(
            0.second().milliseconds(1000).microseconds(1000).nanoseconds(1000),
        ), @"PT1.001001S");
        insta::assert_snapshot!(p(
            1.second().milliseconds(1000).microseconds(1000).nanoseconds(1000),
        ), @"PT2.001001S");
        insta::assert_snapshot!(p(
            0.second()
            .milliseconds(t::SpanMilliseconds::MAX_REPR),
        ), @"PT631107417600S");
        insta::assert_snapshot!(p(
            0.second()
            .microseconds(t::SpanMicroseconds::MAX_REPR),
        ), @"PT631107417600S");
        insta::assert_snapshot!(p(
            0.second()
            .nanoseconds(t::SpanNanoseconds::MAX_REPR),
        ), @"PT9223372036.854775807S");

        insta::assert_snapshot!(p(
            0.second()
            .milliseconds(t::SpanMilliseconds::MAX_REPR)
            .microseconds(999_999),
        ), @"PT631107417600.999999S");
        // This is 1 microsecond more than the maximum number of seconds
        // representable in a span.
        insta::assert_snapshot!(p(
            0.second()
            .milliseconds(t::SpanMilliseconds::MAX_REPR)
            .microseconds(1_000_000),
        ), @"PT631107417601S");
        insta::assert_snapshot!(p(
            0.second()
            .milliseconds(t::SpanMilliseconds::MAX_REPR)
            .microseconds(1_000_001),
        ), @"PT631107417601.000001S");
        // This is 1 nanosecond more than the maximum number of seconds
        // representable in a span.
        insta::assert_snapshot!(p(
            0.second()
            .milliseconds(t::SpanMilliseconds::MAX_REPR)
            .nanoseconds(1_000_000_000),
        ), @"PT631107417601S");
        insta::assert_snapshot!(p(
            0.second()
            .milliseconds(t::SpanMilliseconds::MAX_REPR)
            .nanoseconds(1_000_000_001),
        ), @"PT631107417601.000000001S");

        // The max millis, micros and nanos, combined.
        insta::assert_snapshot!(p(
            0.second()
            .milliseconds(t::SpanMilliseconds::MAX_REPR)
            .microseconds(t::SpanMicroseconds::MAX_REPR)
            .nanoseconds(t::SpanNanoseconds::MAX_REPR),
        ), @"PT1271438207236.854775807S");
        // The max seconds, millis, micros and nanos, combined.
        insta::assert_snapshot!(p(
            Span::new()
            .seconds(t::SpanSeconds::MAX_REPR)
            .milliseconds(t::SpanMilliseconds::MAX_REPR)
            .microseconds(t::SpanMicroseconds::MAX_REPR)
            .nanoseconds(t::SpanNanoseconds::MAX_REPR),
        ), @"PT1902545624836.854775807S");
    }

    #[test]
    fn print_span_subsecond_negative() {
        let p = |span: Span| -> String {
            let mut buf = String::new();
            SpanPrinter::new().print_span(&span, &mut buf).unwrap();
            buf
        };

        // These are all sub-second trickery tests.
        insta::assert_snapshot!(p(
            -0.second().milliseconds(1000).microseconds(1000).nanoseconds(1000),
        ), @"-PT1.001001S");
        insta::assert_snapshot!(p(
            -1.second().milliseconds(1000).microseconds(1000).nanoseconds(1000),
        ), @"-PT2.001001S");
        insta::assert_snapshot!(p(
            0.second()
            .milliseconds(t::SpanMilliseconds::MIN_REPR),
        ), @"-PT631107417600S");
        insta::assert_snapshot!(p(
            0.second()
            .microseconds(t::SpanMicroseconds::MIN_REPR),
        ), @"-PT631107417600S");
        insta::assert_snapshot!(p(
            0.second()
            .nanoseconds(t::SpanNanoseconds::MIN_REPR),
        ), @"-PT9223372036.854775807S");

        insta::assert_snapshot!(p(
            0.second()
            .milliseconds(t::SpanMilliseconds::MIN_REPR)
            .microseconds(999_999),
        ), @"-PT631107417600.999999S");
        // This is 1 microsecond more than the maximum number of seconds
        // representable in a span.
        insta::assert_snapshot!(p(
            0.second()
            .milliseconds(t::SpanMilliseconds::MIN_REPR)
            .microseconds(1_000_000),
        ), @"-PT631107417601S");
        insta::assert_snapshot!(p(
            0.second()
            .milliseconds(t::SpanMilliseconds::MIN_REPR)
            .microseconds(1_000_001),
        ), @"-PT631107417601.000001S");
        // This is 1 nanosecond more than the maximum number of seconds
        // representable in a span.
        insta::assert_snapshot!(p(
            0.second()
            .milliseconds(t::SpanMilliseconds::MIN_REPR)
            .nanoseconds(1_000_000_000),
        ), @"-PT631107417601S");
        insta::assert_snapshot!(p(
            0.second()
            .milliseconds(t::SpanMilliseconds::MIN_REPR)
            .nanoseconds(1_000_000_001),
        ), @"-PT631107417601.000000001S");

        // The max millis, micros and nanos, combined.
        insta::assert_snapshot!(p(
            0.second()
            .milliseconds(t::SpanMilliseconds::MIN_REPR)
            .microseconds(t::SpanMicroseconds::MIN_REPR)
            .nanoseconds(t::SpanNanoseconds::MIN_REPR),
        ), @"-PT1271438207236.854775807S");
        // The max seconds, millis, micros and nanos, combined.
        insta::assert_snapshot!(p(
            Span::new()
            .seconds(t::SpanSeconds::MIN_REPR)
            .milliseconds(t::SpanMilliseconds::MIN_REPR)
            .microseconds(t::SpanMicroseconds::MIN_REPR)
            .nanoseconds(t::SpanNanoseconds::MIN_REPR),
        ), @"-PT1902545624836.854775807S");
    }

    #[test]
    fn print_duration() {
        let p = |secs, nanos| -> String {
            let dur = SignedDuration::new(secs, nanos);
            let mut buf = String::new();
            SpanPrinter::new().print_duration(&dur, &mut buf).unwrap();
            buf
        };

        insta::assert_snapshot!(p(0, 0), @"PT0S");
        insta::assert_snapshot!(p(0, 1), @"PT0.000000001S");
        insta::assert_snapshot!(p(1, 0), @"PT1S");
        insta::assert_snapshot!(p(59, 0), @"PT59S");
        insta::assert_snapshot!(p(60, 0), @"PT1M");
        insta::assert_snapshot!(p(60, 1), @"PT1M0.000000001S");
        insta::assert_snapshot!(p(61, 1), @"PT1M1.000000001S");
        insta::assert_snapshot!(p(3_600, 0), @"PT1H");
        insta::assert_snapshot!(p(3_600, 1), @"PT1H0.000000001S");
        insta::assert_snapshot!(p(3_660, 0), @"PT1H1M");
        insta::assert_snapshot!(p(3_660, 1), @"PT1H1M0.000000001S");
        insta::assert_snapshot!(p(3_661, 0), @"PT1H1M1S");
        insta::assert_snapshot!(p(3_661, 1), @"PT1H1M1.000000001S");

        insta::assert_snapshot!(p(0, -1), @"-PT0.000000001S");
        insta::assert_snapshot!(p(-1, 0), @"-PT1S");
        insta::assert_snapshot!(p(-59, 0), @"-PT59S");
        insta::assert_snapshot!(p(-60, 0), @"-PT1M");
        insta::assert_snapshot!(p(-60, -1), @"-PT1M0.000000001S");
        insta::assert_snapshot!(p(-61, -1), @"-PT1M1.000000001S");
        insta::assert_snapshot!(p(-3_600, 0), @"-PT1H");
        insta::assert_snapshot!(p(-3_600, -1), @"-PT1H0.000000001S");
        insta::assert_snapshot!(p(-3_660, 0), @"-PT1H1M");
        insta::assert_snapshot!(p(-3_660, -1), @"-PT1H1M0.000000001S");
        insta::assert_snapshot!(p(-3_661, 0), @"-PT1H1M1S");
        insta::assert_snapshot!(p(-3_661, -1), @"-PT1H1M1.000000001S");

        insta::assert_snapshot!(
            p(i64::MIN, -999_999_999),
            @"-PT2562047788015215H30M8.999999999S",
        );
        insta::assert_snapshot!(
            p(i64::MAX, 999_999_999),
            @"PT2562047788015215H30M7.999999999S",
        );
    }
}
