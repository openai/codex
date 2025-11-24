use core::fmt;
use core::str::{self, FromStr};

/// A parsed TOML datetime value
///
/// This structure is intended to represent the datetime primitive type that can
/// be encoded into TOML documents. This type is a parsed version that contains
/// all metadata internally.
///
/// Currently this type is intentionally conservative and only supports
/// `to_string` as an accessor. Over time though it's intended that it'll grow
/// more support!
///
/// Note that if you're using `Deserialize` to deserialize a TOML document, you
/// can use this as a placeholder for where you're expecting a datetime to be
/// specified.
///
/// Also note though that while this type implements `Serialize` and
/// `Deserialize` it's only recommended to use this type with the TOML format,
/// otherwise encoded in other formats it may look a little odd.
///
/// Depending on how the option values are used, this struct will correspond
/// with one of the following four datetimes from the [TOML v1.0.0 spec]:
///
/// | `date`    | `time`    | `offset`  | TOML type          |
/// | --------- | --------- | --------- | ------------------ |
/// | `Some(_)` | `Some(_)` | `Some(_)` | [Offset Date-Time] |
/// | `Some(_)` | `Some(_)` | `None`    | [Local Date-Time]  |
/// | `Some(_)` | `None`    | `None`    | [Local Date]       |
/// | `None`    | `Some(_)` | `None`    | [Local Time]       |
///
/// **1. Offset Date-Time**: If all the optional values are used, `Datetime`
/// corresponds to an [Offset Date-Time]. From the TOML v1.0.0 spec:
///
/// > To unambiguously represent a specific instant in time, you may use an
/// > RFC 3339 formatted date-time with offset.
/// >
/// > ```toml
/// > odt1 = 1979-05-27T07:32:00Z
/// > odt2 = 1979-05-27T00:32:00-07:00
/// > odt3 = 1979-05-27T00:32:00.999999-07:00
/// > ```
/// >
/// > For the sake of readability, you may replace the T delimiter between date
/// > and time with a space character (as permitted by RFC 3339 section 5.6).
/// >
/// > ```toml
/// > odt4 = 1979-05-27 07:32:00Z
/// > ```
///
/// **2. Local Date-Time**: If `date` and `time` are given but `offset` is
/// `None`, `Datetime` corresponds to a [Local Date-Time]. From the spec:
///
/// > If you omit the offset from an RFC 3339 formatted date-time, it will
/// > represent the given date-time without any relation to an offset or
/// > timezone. It cannot be converted to an instant in time without additional
/// > information. Conversion to an instant, if required, is implementation-
/// > specific.
/// >
/// > ```toml
/// > ldt1 = 1979-05-27T07:32:00
/// > ldt2 = 1979-05-27T00:32:00.999999
/// > ```
///
/// **3. Local Date**: If only `date` is given, `Datetime` corresponds to a
/// [Local Date]; see the docs for [`Date`].
///
/// **4. Local Time**: If only `time` is given, `Datetime` corresponds to a
/// [Local Time]; see the docs for [`Time`].
///
/// [TOML v1.0.0 spec]: https://toml.io/en/v1.0.0
/// [Offset Date-Time]: https://toml.io/en/v1.0.0#offset-date-time
/// [Local Date-Time]: https://toml.io/en/v1.0.0#local-date-time
/// [Local Date]: https://toml.io/en/v1.0.0#local-date
/// [Local Time]: https://toml.io/en/v1.0.0#local-time
#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Debug)]
pub struct Datetime {
    /// Optional date.
    /// Required for: *Offset Date-Time*, *Local Date-Time*, *Local Date*.
    pub date: Option<Date>,

    /// Optional time.
    /// Required for: *Offset Date-Time*, *Local Date-Time*, *Local Time*.
    pub time: Option<Time>,

    /// Optional offset.
    /// Required for: *Offset Date-Time*.
    pub offset: Option<Offset>,
}

// Currently serde itself doesn't have a datetime type, so we map our `Datetime`
// to a special value in the serde data model. Namely one with these special
// fields/struct names.
//
// In general the TOML encoder/decoder will catch this and not literally emit
// these strings but rather emit datetimes as they're intended.
#[cfg(feature = "serde")]
pub(crate) const FIELD: &str = "$__toml_private_datetime";
#[cfg(feature = "serde")]
pub(crate) const NAME: &str = "$__toml_private_Datetime";
#[cfg(feature = "serde")]
pub(crate) fn is_datetime(name: &'static str) -> bool {
    name == NAME
}

/// A parsed TOML date value
///
/// May be part of a [`Datetime`]. Alone, `Date` corresponds to a [Local Date].
/// From the TOML v1.0.0 spec:
///
/// > If you include only the date portion of an RFC 3339 formatted date-time,
/// > it will represent that entire day without any relation to an offset or
/// > timezone.
/// >
/// > ```toml
/// > ld1 = 1979-05-27
/// > ```
///
/// [Local Date]: https://toml.io/en/v1.0.0#local-date
#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Debug)]
pub struct Date {
    /// Year: four digits
    pub year: u16,
    /// Month: 1 to 12
    pub month: u8,
    /// Day: 1 to {28, 29, 30, 31} (based on month/year)
    pub day: u8,
}

/// A parsed TOML time value
///
/// May be part of a [`Datetime`]. Alone, `Time` corresponds to a [Local Time].
/// From the TOML v1.0.0 spec:
///
/// > If you include only the time portion of an RFC 3339 formatted date-time,
/// > it will represent that time of day without any relation to a specific
/// > day or any offset or timezone.
/// >
/// > ```toml
/// > lt1 = 07:32:00
/// > lt2 = 00:32:00.999999
/// > ```
/// >
/// > Millisecond precision is required. Further precision of fractional
/// > seconds is implementation-specific. If the value contains greater
/// > precision than the implementation can support, the additional precision
/// > must be truncated, not rounded.
///
/// [Local Time]: https://toml.io/en/v1.0.0#local-time
#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Debug)]
pub struct Time {
    /// Hour: 0 to 23
    pub hour: u8,
    /// Minute: 0 to 59
    pub minute: u8,
    /// Second: 0 to {58, 59, 60} (based on leap second rules)
    pub second: u8,
    /// Nanosecond: 0 to `999_999_999`
    pub nanosecond: u32,
}

/// A parsed TOML time offset
///
#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Debug)]
pub enum Offset {
    /// > A suffix which, when applied to a time, denotes a UTC offset of 00:00;
    /// > often spoken "Zulu" from the ICAO phonetic alphabet representation of
    /// > the letter "Z". --- [RFC 3339 section 2]
    ///
    /// [RFC 3339 section 2]: https://datatracker.ietf.org/doc/html/rfc3339#section-2
    Z,

    /// Offset between local time and UTC
    Custom {
        /// Minutes: -`1_440..1_440`
        minutes: i16,
    },
}

impl Datetime {
    #[cfg(feature = "serde")]
    fn type_name(&self) -> &'static str {
        match (
            self.date.is_some(),
            self.time.is_some(),
            self.offset.is_some(),
        ) {
            (true, true, true) => "offset datetime",
            (true, true, false) => "local datetime",
            (true, false, false) => Date::type_name(),
            (false, true, false) => Time::type_name(),
            _ => unreachable!("unsupported datetime combination"),
        }
    }
}

impl Date {
    #[cfg(feature = "serde")]
    fn type_name() -> &'static str {
        "local date"
    }
}

impl Time {
    #[cfg(feature = "serde")]
    fn type_name() -> &'static str {
        "local time"
    }
}

impl From<Date> for Datetime {
    fn from(other: Date) -> Self {
        Self {
            date: Some(other),
            time: None,
            offset: None,
        }
    }
}

impl From<Time> for Datetime {
    fn from(other: Time) -> Self {
        Self {
            date: None,
            time: Some(other),
            offset: None,
        }
    }
}

#[cfg(feature = "alloc")]
impl fmt::Display for Datetime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref date) = self.date {
            write!(f, "{date}")?;
        }
        if let Some(ref time) = self.time {
            if self.date.is_some() {
                write!(f, "T")?;
            }
            write!(f, "{time}")?;
        }
        if let Some(ref offset) = self.offset {
            write!(f, "{offset}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[cfg(feature = "alloc")]
impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.hour, self.minute, self.second)?;
        if self.nanosecond != 0 {
            let s = alloc::format!("{:09}", self.nanosecond);
            write!(f, ".{}", s.trim_end_matches('0'))?;
        }
        Ok(())
    }
}

impl fmt::Display for Offset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Z => write!(f, "Z"),
            Self::Custom { mut minutes } => {
                let mut sign = '+';
                if minutes < 0 {
                    minutes *= -1;
                    sign = '-';
                }
                let hours = minutes / 60;
                let minutes = minutes % 60;
                write!(f, "{sign}{hours:02}:{minutes:02}")
            }
        }
    }
}

impl FromStr for Datetime {
    type Err = DatetimeParseError;

    fn from_str(date: &str) -> Result<Self, DatetimeParseError> {
        // Accepted formats:
        //
        // 0000-00-00T00:00:00.00Z
        // 0000-00-00T00:00:00.00
        // 0000-00-00
        // 00:00:00.00
        //
        // ```abnf
        // ;; Date and Time (as defined in RFC 3339)
        //
        // date-time      = offset-date-time / local-date-time / local-date / local-time
        //
        // date-fullyear  = 4DIGIT
        // date-month     = 2DIGIT  ; 01-12
        // date-mday      = 2DIGIT  ; 01-28, 01-29, 01-30, 01-31 based on month/year
        // time-delim     = "T" / %x20 ; T, t, or space
        // time-hour      = 2DIGIT  ; 00-23
        // time-minute    = 2DIGIT  ; 00-59
        // time-second    = 2DIGIT  ; 00-58, 00-59, 00-60 based on leap second rules
        // time-secfrac   = "." 1*DIGIT
        // time-numoffset = ( "+" / "-" ) time-hour ":" time-minute
        // time-offset    = "Z" / time-numoffset
        //
        // partial-time   = time-hour ":" time-minute ":" time-second [ time-secfrac ]
        // full-date      = date-fullyear "-" date-month "-" date-mday
        // full-time      = partial-time time-offset
        //
        // ;; Offset Date-Time
        //
        // offset-date-time = full-date time-delim full-time
        //
        // ;; Local Date-Time
        //
        // local-date-time = full-date time-delim partial-time
        //
        // ;; Local Date
        //
        // local-date = full-date
        //
        // ;; Local Time
        //
        // local-time = partial-time
        // ```
        let mut result = Self {
            date: None,
            time: None,
            offset: None,
        };

        let mut lexer = Lexer::new(date);

        let digits = lexer
            .next()
            .ok_or(DatetimeParseError::new().expected("year or hour"))?;
        digits
            .is(TokenKind::Digits)
            .map_err(|err| err.expected("year or hour"))?;
        let sep = lexer
            .next()
            .ok_or(DatetimeParseError::new().expected("`-` (YYYY-MM) or `:` (HH:MM)"))?;
        match sep.kind {
            TokenKind::Dash => {
                let year = digits;
                let month = lexer
                    .next()
                    .ok_or_else(|| DatetimeParseError::new().what("date").expected("month"))?;
                month
                    .is(TokenKind::Digits)
                    .map_err(|err| err.what("date").expected("month"))?;
                let sep = lexer.next().ok_or(
                    DatetimeParseError::new()
                        .what("date")
                        .expected("`-` (MM-DD)"),
                )?;
                sep.is(TokenKind::Dash)
                    .map_err(|err| err.what("date").expected("`-` (MM-DD)"))?;
                let day = lexer
                    .next()
                    .ok_or(DatetimeParseError::new().what("date").expected("day"))?;
                day.is(TokenKind::Digits)
                    .map_err(|err| err.what("date").expected("day"))?;

                if year.raw.len() != 4 {
                    return Err(DatetimeParseError::new()
                        .what("date")
                        .expected("a four-digit year (YYYY)"));
                }
                if month.raw.len() != 2 {
                    return Err(DatetimeParseError::new()
                        .what("date")
                        .expected("a two-digit month (MM)"));
                }
                if day.raw.len() != 2 {
                    return Err(DatetimeParseError::new()
                        .what("date")
                        .expected("a two-digit day (DD)"));
                }
                let date = Date {
                    year: year.raw.parse().map_err(|_err| DatetimeParseError::new())?,
                    month: month
                        .raw
                        .parse()
                        .map_err(|_err| DatetimeParseError::new())?,
                    day: day.raw.parse().map_err(|_err| DatetimeParseError::new())?,
                };
                if date.month < 1 || date.month > 12 {
                    return Err(DatetimeParseError::new()
                        .what("date")
                        .expected("month between 01 and 12"));
                }
                let is_leap_year =
                    (date.year % 4 == 0) && ((date.year % 100 != 0) || (date.year % 400 == 0));
                let (max_days_in_month, expected_day) = match date.month {
                    2 if is_leap_year => (29, "day between 01 and 29"),
                    2 => (28, "day between 01 and 28"),
                    4 | 6 | 9 | 11 => (30, "day between 01 and 30"),
                    _ => (31, "day between 01 and 31"),
                };
                if date.day < 1 || date.day > max_days_in_month {
                    return Err(DatetimeParseError::new()
                        .what("date")
                        .expected(expected_day));
                }

                result.date = Some(date);
            }
            TokenKind::Colon => lexer = Lexer::new(date),
            _ => {
                return Err(DatetimeParseError::new().expected("`-` (YYYY-MM) or `:` (HH:MM)"));
            }
        }

        // Next parse the "partial-time" if available
        let partial_time = if result.date.is_some() {
            let sep = lexer.next();
            match sep {
                Some(token) if matches!(token.kind, TokenKind::T | TokenKind::Space) => true,
                Some(_token) => {
                    return Err(DatetimeParseError::new()
                        .what("date-time")
                        .expected("`T` between date and time"));
                }
                None => false,
            }
        } else {
            result.date.is_none()
        };

        if partial_time {
            let hour = lexer
                .next()
                .ok_or_else(|| DatetimeParseError::new().what("time").expected("hour"))?;
            hour.is(TokenKind::Digits)
                .map_err(|err| err.what("time").expected("hour"))?;
            let sep = lexer.next().ok_or(
                DatetimeParseError::new()
                    .what("time")
                    .expected("`:` (HH:MM)"),
            )?;
            sep.is(TokenKind::Colon)
                .map_err(|err| err.what("time").expected("`:` (HH:MM)"))?;
            let minute = lexer
                .next()
                .ok_or(DatetimeParseError::new().what("time").expected("minute"))?;
            minute
                .is(TokenKind::Digits)
                .map_err(|err| err.what("time").expected("minute"))?;
            let sep = lexer.next().ok_or(
                DatetimeParseError::new()
                    .what("time")
                    .expected("`:` (MM:SS)"),
            )?;
            sep.is(TokenKind::Colon)
                .map_err(|err| err.what("time").expected("`:` (MM:SS)"))?;
            let second = lexer
                .next()
                .ok_or(DatetimeParseError::new().what("time").expected("second"))?;
            second
                .is(TokenKind::Digits)
                .map_err(|err| err.what("time").expected("second"))?;

            let nanosecond = if lexer.clone().next().map(|t| t.kind) == Some(TokenKind::Dot) {
                let sep = lexer.next().ok_or(DatetimeParseError::new())?;
                sep.is(TokenKind::Dot)?;
                let nanosecond = lexer.next().ok_or(
                    DatetimeParseError::new()
                        .what("time")
                        .expected("nanosecond"),
                )?;
                nanosecond
                    .is(TokenKind::Digits)
                    .map_err(|err| err.what("time").expected("nanosecond"))?;
                Some(nanosecond)
            } else {
                None
            };

            if hour.raw.len() != 2 {
                return Err(DatetimeParseError::new()
                    .what("time")
                    .expected("a two-digit hour (HH)"));
            }
            if minute.raw.len() != 2 {
                return Err(DatetimeParseError::new()
                    .what("time")
                    .expected("a two-digit minute (MM)"));
            }
            if second.raw.len() != 2 {
                return Err(DatetimeParseError::new()
                    .what("time")
                    .expected("a two-digit second (SS)"));
            }

            let time = Time {
                hour: hour.raw.parse().map_err(|_err| DatetimeParseError::new())?,
                minute: minute
                    .raw
                    .parse()
                    .map_err(|_err| DatetimeParseError::new())?,
                second: second
                    .raw
                    .parse()
                    .map_err(|_err| DatetimeParseError::new())?,
                nanosecond: nanosecond.map(|t| s_to_nanoseconds(t.raw)).unwrap_or(0),
            };

            if time.hour > 23 {
                return Err(DatetimeParseError::new()
                    .what("time")
                    .expected("hour between 00 and 23"));
            }
            if time.minute > 59 {
                return Err(DatetimeParseError::new()
                    .what("time")
                    .expected("minute between 00 and 59"));
            }
            // 00-58, 00-59, 00-60 based on leap second rules
            if time.second > 60 {
                return Err(DatetimeParseError::new()
                    .what("time")
                    .expected("second between 00 and 60"));
            }
            if time.nanosecond > 999_999_999 {
                return Err(DatetimeParseError::new()
                    .what("time")
                    .expected("nanoseconds overflowed"));
            }

            result.time = Some(time);
        }

        // And finally, parse the offset
        if result.date.is_some() && result.time.is_some() {
            match lexer.next() {
                Some(token) if token.kind == TokenKind::Z => {
                    result.offset = Some(Offset::Z);
                }
                Some(token) if matches!(token.kind, TokenKind::Plus | TokenKind::Dash) => {
                    let sign = if token.kind == TokenKind::Plus { 1 } else { -1 };
                    let hours = lexer
                        .next()
                        .ok_or(DatetimeParseError::new().what("offset").expected("hour"))?;
                    hours
                        .is(TokenKind::Digits)
                        .map_err(|err| err.what("offset").expected("hour"))?;
                    let sep = lexer.next().ok_or(
                        DatetimeParseError::new()
                            .what("offset")
                            .expected("`:` (HH:MM)"),
                    )?;
                    sep.is(TokenKind::Colon)
                        .map_err(|err| err.what("offset").expected("`:` (HH:MM)"))?;
                    let minutes = lexer
                        .next()
                        .ok_or(DatetimeParseError::new().what("offset").expected("minute"))?;
                    minutes
                        .is(TokenKind::Digits)
                        .map_err(|err| err.what("offset").expected("minute"))?;

                    if hours.raw.len() != 2 {
                        return Err(DatetimeParseError::new()
                            .what("offset")
                            .expected("a two-digit hour (HH)"));
                    }
                    if minutes.raw.len() != 2 {
                        return Err(DatetimeParseError::new()
                            .what("offset")
                            .expected("a two-digit minute (MM)"));
                    }

                    let hours = hours
                        .raw
                        .parse::<u8>()
                        .map_err(|_err| DatetimeParseError::new())?;
                    let minutes = minutes
                        .raw
                        .parse::<u8>()
                        .map_err(|_err| DatetimeParseError::new())?;

                    if hours > 23 {
                        return Err(DatetimeParseError::new()
                            .what("offset")
                            .expected("hours between 00 and 23"));
                    }
                    if minutes > 59 {
                        return Err(DatetimeParseError::new()
                            .what("offset")
                            .expected("minutes between 00 and 59"));
                    }

                    let total_minutes = sign * (hours as i16 * 60 + minutes as i16);

                    if !((-24 * 60)..=(24 * 60)).contains(&total_minutes) {
                        return Err(DatetimeParseError::new().what("offset"));
                    }

                    result.offset = Some(Offset::Custom {
                        minutes: total_minutes,
                    });
                }
                Some(_token) => {
                    return Err(DatetimeParseError::new()
                        .what("offset")
                        .expected("`Z`, +OFFSET, -OFFSET"));
                }
                None => {}
            }
        }

        // Return an error if we didn't hit eof, otherwise return our parsed
        // date
        if lexer.unknown().is_some() {
            return Err(DatetimeParseError::new());
        }

        Ok(result)
    }
}

fn s_to_nanoseconds(input: &str) -> u32 {
    let mut nanosecond = 0;
    for (i, byte) in input.bytes().enumerate() {
        if byte.is_ascii_digit() {
            if i < 9 {
                let p = 10_u32.pow(8 - i as u32);
                nanosecond += p * u32::from(byte - b'0');
            }
        } else {
            panic!("invalid nanoseconds {input:?}");
        }
    }
    nanosecond
}

#[derive(Copy, Clone)]
struct Token<'s> {
    kind: TokenKind,
    raw: &'s str,
}

impl Token<'_> {
    fn is(&self, kind: TokenKind) -> Result<(), DatetimeParseError> {
        if self.kind == kind {
            Ok(())
        } else {
            Err(DatetimeParseError::new())
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum TokenKind {
    Digits,
    Dash,
    Colon,
    Dot,
    T,
    Space,
    Z,
    Plus,
    Unknown,
}

#[derive(Copy, Clone)]
struct Lexer<'s> {
    stream: &'s str,
}

impl<'s> Lexer<'s> {
    fn new(input: &'s str) -> Self {
        Self { stream: input }
    }

    fn unknown(&mut self) -> Option<Token<'s>> {
        let remaining = self.stream.len();
        if remaining == 0 {
            return None;
        }
        let raw = self.stream;
        self.stream = &self.stream[remaining..remaining];
        Some(Token {
            kind: TokenKind::Unknown,
            raw,
        })
    }
}

impl<'s> Iterator for Lexer<'s> {
    type Item = Token<'s>;

    fn next(&mut self) -> Option<Self::Item> {
        let (kind, end) = match self.stream.as_bytes().first()? {
            b'0'..=b'9' => {
                let end = self
                    .stream
                    .as_bytes()
                    .iter()
                    .position(|b| !b.is_ascii_digit())
                    .unwrap_or(self.stream.len());
                (TokenKind::Digits, end)
            }
            b'-' => (TokenKind::Dash, 1),
            b':' => (TokenKind::Colon, 1),
            b'T' | b't' => (TokenKind::T, 1),
            b' ' => (TokenKind::Space, 1),
            b'Z' | b'z' => (TokenKind::Z, 1),
            b'+' => (TokenKind::Plus, 1),
            b'.' => (TokenKind::Dot, 1),
            _ => (TokenKind::Unknown, self.stream.len()),
        };
        let (raw, rest) = self.stream.split_at(end);
        self.stream = rest;
        Some(Token { kind, raw })
    }
}

/// Error returned from parsing a `Datetime` in the `FromStr` implementation.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DatetimeParseError {
    what: Option<&'static str>,
    expected: Option<&'static str>,
}

impl DatetimeParseError {
    fn new() -> Self {
        Self {
            what: None,
            expected: None,
        }
    }
    fn what(mut self, what: &'static str) -> Self {
        self.what = Some(what);
        self
    }
    fn expected(mut self, expected: &'static str) -> Self {
        self.expected = Some(expected);
        self
    }
}

impl fmt::Display for DatetimeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(what) = self.what {
            write!(f, "invalid {what}")?;
        } else {
            "invalid datetime".fmt(f)?;
        }
        if let Some(expected) = self.expected {
            write!(f, ", expected {expected}")?;
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DatetimeParseError {}
#[cfg(all(not(feature = "std"), feature = "serde"))]
impl serde::de::StdError for DatetimeParseError {}

#[cfg(feature = "serde")]
#[cfg(feature = "alloc")]
impl serde::ser::Serialize for Datetime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        use crate::alloc::string::ToString as _;
        use serde::ser::SerializeStruct;

        let mut s = serializer.serialize_struct(NAME, 1)?;
        s.serialize_field(FIELD, &self.to_string())?;
        s.end()
    }
}

#[cfg(feature = "serde")]
#[cfg(feature = "alloc")]
impl serde::ser::Serialize for Date {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        Datetime::from(*self).serialize(serializer)
    }
}

#[cfg(feature = "serde")]
#[cfg(feature = "alloc")]
impl serde::ser::Serialize for Time {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        Datetime::from(*self).serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::de::Deserialize<'de> for Datetime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct DatetimeVisitor;

        impl<'de> serde::de::Visitor<'de> for DatetimeVisitor {
            type Value = Datetime;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a TOML datetime")
            }

            fn visit_map<V>(self, mut visitor: V) -> Result<Datetime, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let value = visitor.next_key::<DatetimeKey>()?;
                if value.is_none() {
                    return Err(serde::de::Error::custom("datetime key not found"));
                }
                let v: DatetimeFromString = visitor.next_value()?;
                Ok(v.value)
            }
        }

        static FIELDS: [&str; 1] = [FIELD];
        deserializer.deserialize_struct(NAME, &FIELDS, DatetimeVisitor)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::de::Deserialize<'de> for Date {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        match Datetime::deserialize(deserializer)? {
            Datetime {
                date: Some(date),
                time: None,
                offset: None,
            } => Ok(date),
            datetime => Err(serde::de::Error::invalid_type(
                serde::de::Unexpected::Other(datetime.type_name()),
                &Self::type_name(),
            )),
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::de::Deserialize<'de> for Time {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        match Datetime::deserialize(deserializer)? {
            Datetime {
                date: None,
                time: Some(time),
                offset: None,
            } => Ok(time),
            datetime => Err(serde::de::Error::invalid_type(
                serde::de::Unexpected::Other(datetime.type_name()),
                &Self::type_name(),
            )),
        }
    }
}

#[cfg(feature = "serde")]
struct DatetimeKey;

#[cfg(feature = "serde")]
impl<'de> serde::de::Deserialize<'de> for DatetimeKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct FieldVisitor;

        impl serde::de::Visitor<'_> for FieldVisitor {
            type Value = ();

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a valid datetime field")
            }

            fn visit_str<E>(self, s: &str) -> Result<(), E>
            where
                E: serde::de::Error,
            {
                if s == FIELD {
                    Ok(())
                } else {
                    Err(serde::de::Error::custom("expected field with custom name"))
                }
            }
        }

        deserializer.deserialize_identifier(FieldVisitor)?;
        Ok(Self)
    }
}

#[cfg(feature = "serde")]
pub(crate) struct DatetimeFromString {
    pub(crate) value: Datetime,
}

#[cfg(feature = "serde")]
impl<'de> serde::de::Deserialize<'de> for DatetimeFromString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = DatetimeFromString;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("string containing a datetime")
            }

            fn visit_str<E>(self, s: &str) -> Result<DatetimeFromString, E>
            where
                E: serde::de::Error,
            {
                match s.parse() {
                    Ok(date) => Ok(DatetimeFromString { value: date }),
                    Err(e) => Err(serde::de::Error::custom(e)),
                }
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}
