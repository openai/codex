/*!
A bespoke but easy to read format for [`Span`](crate::Span) and
[`SignedDuration`](crate::SignedDuration).

The "friendly" duration format is meant to be an alternative to [Temporal's
ISO 8601 duration format](super::temporal) that is both easier to read and can
losslessly serialize and deserialize all `Span` values.

Here are a variety of examples showing valid friendly durations for `Span`:

```
use jiff::{Span, ToSpan};

let spans = [
    ("40d", 40.days()),
    ("40 days", 40.days()),
    ("1y1d", 1.year().days(1)),
    ("1yr 1d", 1.year().days(1)),
    ("3d4h59m", 3.days().hours(4).minutes(59)),
    ("3 days, 4 hours, 59 minutes", 3.days().hours(4).minutes(59)),
    ("3d 4h 59m", 3.days().hours(4).minutes(59)),
    ("2h30m", 2.hours().minutes(30)),
    ("2h 30m", 2.hours().minutes(30)),
    ("1mo", 1.month()),
    ("1w", 1.week()),
    ("1 week", 1.week()),
    ("1w4d", 1.week().days(4)),
    ("1 wk 4 days", 1.week().days(4)),
    ("1m", 1.minute()),
    ("0.0021s", 2.milliseconds().microseconds(100)),
    ("0s", 0.seconds()),
    ("0d", 0.seconds()),
    ("0 days", 0.seconds()),
    ("3 mins 34s 123ms", 3.minutes().seconds(34).milliseconds(123)),
    ("3 mins 34.123 secs", 3.minutes().seconds(34).milliseconds(123)),
    ("3 mins 34,123s", 3.minutes().seconds(34).milliseconds(123)),
    (
        "1y1mo1d1h1m1.1s",
        1.year().months(1).days(1).hours(1).minutes(1).seconds(1).milliseconds(100),
    ),
    (
        "1yr 1mo 1day 1hr 1min 1.1sec",
        1.year().months(1).days(1).hours(1).minutes(1).seconds(1).milliseconds(100),
    ),
    (
        "1 year, 1 month, 1 day, 1 hour, 1 minute 1.1 seconds",
        1.year().months(1).days(1).hours(1).minutes(1).seconds(1).milliseconds(100),
    ),
    (
        "1 year, 1 month, 1 day, 01:01:01.1",
        1.year().months(1).days(1).hours(1).minutes(1).seconds(1).milliseconds(100),
    ),
];
for (string, span) in spans {
    let parsed: Span = string.parse()?;
    assert_eq!(
        span.fieldwise(),
        parsed.fieldwise(),
        "result of parsing {string:?}",
    );
}

# Ok::<(), Box<dyn std::error::Error>>(())
```

Note that for a `SignedDuration`, only units up to hours are supported. If you
need to support bigger units, then you'll need to convert it to a `Span` before
printing to the friendly format (or parse into a `Span` and then convert to a
`SignedDuration`).

# Integration points

While this module can of course be used to parse and print durations in the
friendly format, in most cases, you don't have to. Namely, it is already
integrated into the `Span` and `SignedDuration` types.

For example, the friendly format can be used by invoking the "alternate"
format when using the `std::fmt::Display` trait implementation:

```
use jiff::{SignedDuration, ToSpan};

let span = 2.months().days(35).hours(2).minutes(30);
assert_eq!(format!("{span}"), "P2M35DT2H30M");      // ISO 8601
assert_eq!(format!("{span:#}"), "2mo 35d 2h 30m");  // "friendly"

let sdur = SignedDuration::new(2 * 60 * 60 + 30 * 60, 123_456_789);
assert_eq!(format!("{sdur}"), "PT2H30M0.123456789S");         // ISO 8601
assert_eq!(format!("{sdur:#}"), "2h 30m 123ms 456µs 789ns");  // "friendly"
```

Both `Span` and `SignedDuration` use the "friendly" format for its
`std::fmt::Debug` trait implementation:

```
use jiff::{SignedDuration, ToSpan};

let span = 2.months().days(35).hours(2).minutes(30);
assert_eq!(format!("{span:?}"), "2mo 35d 2h 30m");

let sdur = SignedDuration::new(2 * 60 * 60 + 30 * 60, 123_456_789);
assert_eq!(format!("{sdur:?}"), "2h 30m 123ms 456µs 789ns");
```

Both `Span` and `SignedDuration` support parsing the ISO 8601 _and_ friendly
formats via its `std::str::FromStr` trait:

```
use jiff::{SignedDuration, Span, ToSpan};

let expected = 2.months().days(35).hours(2).minutes(30);
let span: Span = "2 months, 35 days, 02:30:00".parse()?;
assert_eq!(span, expected.fieldwise());
let span: Span = "P2M35DT2H30M".parse()?;
assert_eq!(span, expected.fieldwise());

let expected = SignedDuration::new(2 * 60 * 60 + 30 * 60, 123_456_789);
let sdur: SignedDuration = "2h 30m 0,123456789s".parse()?;
assert_eq!(sdur, expected);
let sdur: SignedDuration = "PT2h30m0.123456789s".parse()?;
assert_eq!(sdur, expected);

# Ok::<(), Box<dyn std::error::Error>>(())
```

If you need to parse _only_ the friendly format, then that would be a good use
case for using [`SpanParser`] in this module.

Finally, when the `serde` crate feature is enabled, the friendly format is
automatically supported via the `serde::Deserialize` trait implementation, just
like for the `std::str::FromStr` trait above. However, for `serde::Serialize`,
both types use ISO 8601. In order to serialize the friendly format,
you'll need to write your own serialization function or use one of the
[`fmt::serde`](crate::fmt::serde) helpers provided by Jiff. For example:

```
use jiff::{ToSpan, Span};

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Record {
    #[serde(
        serialize_with = "jiff::fmt::serde::span::friendly::compact::required"
    )]
    span: Span,
}

let json = r#"{"span":"1 year 2 months 36 hours 1100ms"}"#;
let got: Record = serde_json::from_str(&json)?;
assert_eq!(
    got.span.fieldwise(),
    1.year().months(2).hours(36).milliseconds(1100),
);

let expected = r#"{"span":"1y 2mo 36h 1100ms"}"#;
assert_eq!(serde_json::to_string(&got).unwrap(), expected);

# Ok::<(), Box<dyn std::error::Error>>(())
```

The ISO 8601 format is used by default since it is part of a standard and is
more widely accepted. That is, if you need an interoperable interchange format,
then ISO 8601 is probably the right choice.

# Rounding

The printer in this module has no options for rounding. Instead, it is intended
for users to round a [`Span`](crate::Span) first, and then print it. The idea
is that printing a `Span` is a relatively "dumb" operation that just emits
whatever units are non-zero in the `Span`. This is possible with a `Span`
because it represents each unit distinctly. (With a [`std::time::Duration`] or
a [`jiff::SignedDuration`](crate::SignedDuration), more functionality would
need to be coupled with the printing logic to achieve a similar result.)

For example, if you want to print the duration since someone posted a comment
to an English speaking end user, precision below one half hour might be "too
much detail." You can remove this by rounding the `Span` to the nearest half
hour before printing:

```
use jiff::{civil, RoundMode, ToSpan, Unit, ZonedDifference};

let commented_at = civil::date(2024, 8, 1).at(19, 29, 13, 123_456_789).in_tz("US/Eastern")?;
let now = civil::date(2024, 12, 26).at(12, 49, 0, 0).in_tz("US/Eastern")?;

// The default, with units permitted up to years.
let span = now.since((Unit::Year, &commented_at))?;
assert_eq!(format!("{span:#}"), "4mo 24d 17h 19m 46s 876ms 543µs 211ns");

// The same subtraction, but with more options to control
// rounding the result. We could also do this with `Span::round`
// directly by providing `now` as our relative zoned datetime.
let rounded = now.since(
    ZonedDifference::new(&commented_at)
        .smallest(Unit::Minute)
        .largest(Unit::Year)
        .mode(RoundMode::HalfExpand)
        .increment(30),
)?;
assert_eq!(format!("{rounded:#}"), "4mo 24d 17h 30m");

# Ok::<(), Box<dyn std::error::Error>>(())
```

# Comparison with the [`humantime`] crate

To a first approximation, Jiff should cover all `humantime` use cases,
including [`humantime-serde`] for serialization support.

To a second approximation, it was a design point of the friendly format to be
mostly interoperable with what `humantime` supports. For example, any duration
string formatted by `humantime` at time of writing is also a valid friendly
duration:

```
use std::time::Duration;

use jiff::{Span, ToSpan};

// Just a duration that includes as many unit designator labels as possible.
let dur = Duration::new(
    2 * 31_557_600 + 1 * 2_630_016 + 15 * 86400 + 5 * 3600 + 59 * 60 + 1,
    123_456_789,
);
let formatted = humantime::format_duration(dur).to_string();
assert_eq!(formatted, "2years 1month 15days 5h 59m 1s 123ms 456us 789ns");

let span: Span = formatted.parse()?;
let expected =
    2.years()
        .months(1)
        .days(15)
        .hours(5)
        .minutes(59)
        .seconds(1)
        .milliseconds(123)
        .microseconds(456)
        .nanoseconds(789);
assert_eq!(span, expected.fieldwise());

# Ok::<(), Box<dyn std::error::Error>>(())
```

The above somewhat relies on the implementation details of `humantime`. Namely,
not everything parseable by `humantime` is also parseable by the friendly
format (and vice versa). For example, `humantime` parses `M` as a label for
months, but the friendly format specifically eschews `M` because of its
confusability with minutes:

```
use std::time::Duration;

let dur = humantime::parse_duration("1M")?;
// The +38,016 is because `humantime` assigns 30.44 24-hour days to all months.
assert_eq!(dur, Duration::new(30 * 24 * 60 * 60 + 38_016, 0));

// In contrast, Jiff will reject `1M`:
assert_eq!(
    "1M".parse::<jiff::Span>().unwrap_err().to_string(),
    "failed to parse \"1M\" in the \"friendly\" format: expected to find unit designator suffix (e.g., 'years' or 'secs'), but found input beginning with \"M\" instead",
);

# Ok::<(), Box<dyn std::error::Error>>(())
```

In the other direction, Jiff's default formatting for the friendly duration
isn't always parsable by `humantime`. This is because, for example, depending
on the configuration, Jiff may use `mo` and `mos` for months, and `µs` for
microseconds, none of which are supported by `humantime`. If you need it, to
ensure `humantime` can parse a Jiff formatted friendly duration, Jiff provides
a special mode that attempts compatibility with `humantime`:

```
use jiff::{fmt::friendly::{Designator, SpanPrinter}, ToSpan};


let span =
    2.years()
        .months(1)
        .days(15)
        .hours(5)
        .minutes(59)
        .seconds(1)
        .milliseconds(123)
        .microseconds(456)
        .nanoseconds(789);

let printer = SpanPrinter::new().designator(Designator::HumanTime);
assert_eq!(
    printer.span_to_string(&span),
    "2y 1month 15d 5h 59m 1s 123ms 456us 789ns",
);
```

It's hard to provide solid guarantees here because `humantime`'s behavior could
change, but at time of writing, `humantime` has not changed much in quite a
long time (its last release is almost 4 years ago at time of writing). So the
current behavior is likely pretty safe to rely upon.

More generally, the friendly format is more flexible than what `humantime`
supports. For example, the friendly format incorporates `HH:MM:SS` and
fractional time units. It also supports more unit labels and permits commas
to separate units.

```
use jiff::SignedDuration;

// 10 hours and 30 minutes
let expected = SignedDuration::new(10 * 60 * 60 + 30 * 60, 0);
assert_eq!(expected, "10h30m".parse()?);
assert_eq!(expected, "10hrs 30mins".parse()?);
assert_eq!(expected, "10 hours 30 minutes".parse()?);
assert_eq!(expected, "10 hours, 30 minutes".parse()?);
assert_eq!(expected, "10:30:00".parse()?);
assert_eq!(expected, "10.5 hours".parse()?);

# Ok::<(), Box<dyn std::error::Error>>(())
```

Finally, it's important to point out that `humantime` only supports parsing
variable width units like years, months and days by virtue of assigning fixed
static values to them that aren't always correct. In contrast, Jiff always
gets this right and specifically prevents you from getting it wrong.

To begin, Jiff returns an error if you try to parse a varying unit into a
[`SignedDuration`](crate::SignedDuration):

```
use jiff::SignedDuration;

// works fine
assert_eq!(
    "1 hour".parse::<SignedDuration>().unwrap(),
    SignedDuration::from_hours(1),
);
// Jiff is saving you from doing something wrong
assert_eq!(
    "1 day".parse::<SignedDuration>().unwrap_err().to_string(),
    "failed to parse \"1 day\" in the \"friendly\" format: parsing day units into a `SignedDuration` is not supported (perhaps try parsing into a `Span` instead)",
);
```

As the error message suggests, parsing into a [`Span`](crate::Span) works fine:

```
use jiff::Span;

assert_eq!("1 day".parse::<Span>().unwrap(), Span::new().days(1).fieldwise());
```

Jiff has this behavior because it's not possible to determine, in general,
how long "1 day" (or "1 month" or "1 year") is without a reference date.
Since a `SignedDuration` (along with a [`std::time::Duration`]) does not
support expressing durations in anything other than a 96-bit integer number of
nanoseconds, it's not possible to represent concepts like "1 month." But a
[`Span`](crate::Span) can.

To see this more concretely, consider the different behavior resulting from
using `humantime` to parse durations and adding them to a date:

```
use jiff::{civil, Span};

let span: Span = "1 month".parse()?;
let dur = humantime::parse_duration("1 month")?;

let datetime = civil::date(2024, 5, 1).at(0, 0, 0, 0);

// Adding 1 month using a `Span` gives one possible expected result. That is,
// 2024-06-01T00:00:00 is exactly one month later than 2024-05-01T00:00:00.
assert_eq!(datetime + span, civil::date(2024, 6, 1).at(0, 0, 0, 0));
// But if we add the duration representing "1 month" as interpreted by
// humantime, we get a very odd result. This is because humantime uses
// a duration of 30.44 days (where every day is 24 hours exactly) for
// all months.
assert_eq!(datetime + dur, civil::date(2024, 5, 31).at(10, 33, 36, 0));

# Ok::<(), Box<dyn std::error::Error>>(())
```

The same is true for days when dealing with zoned date times:

```
use jiff::{civil, Span};

let span: Span = "1 day".parse()?;
let dur = humantime::parse_duration("1 day")?;

let zdt = civil::date(2024, 3, 9).at(17, 0, 0, 0).in_tz("US/Eastern")?;

// Adding 1 day gives the generally expected result of the same clock
// time on the following day when adding a `Span`.
assert_eq!(&zdt + span, civil::date(2024, 3, 10).at(17, 0, 0, 0).in_tz("US/Eastern")?);
// But with humantime, all days are assumed to be exactly 24 hours. So
// you get an instant in time that is 24 hours later, even when some
// days are shorter and some are longer.
assert_eq!(&zdt + dur, civil::date(2024, 3, 10).at(18, 0, 0, 0).in_tz("US/Eastern")?);

// Notice also that this inaccuracy can occur merely by a duration that
// _crosses_ a time zone transition boundary (like DST) at any point. It
// doesn't require your datetimes to be "close" to when DST occurred.
let dur = humantime::parse_duration("20 day")?;
let zdt = civil::date(2024, 3, 1).at(17, 0, 0, 0).in_tz("US/Eastern")?;
assert_eq!(&zdt + dur, civil::date(2024, 3, 21).at(18, 0, 0, 0).in_tz("US/Eastern")?);

# Ok::<(), Box<dyn std::error::Error>>(())
```

It's worth pointing out that in some applications, the fixed values assigned
by `humantime` might be perfectly acceptable. Namely, they introduce error
into calculations, but the error might be small enough to be a non-issue in
some applications. But this error _can_ be avoided and `humantime` commits
it silently. Indeed, `humantime`'s API is itself not possible without either
rejecting varying length units or assuming fixed values for them. This is
because it parses varying length units but returns a duration expressed as a
single 96-bit integer number of nanoseconds. In order to do this, you _must_
assume a definite length for those varying units. To do this _correctly_, you
really need to provide a reference date.

For example, Jiff can parse `1 month` into a `std::time::Duration` too, but
it requires parsing into a `Span` and then converting into a `Duration` by
providing a reference date:

```
use std::time::Duration;

use jiff::{civil, Span};

let span: Span = "1 month".parse()?;
// converts to signed duration
let sdur = span.to_duration(civil::date(2024, 5, 1))?;
// converts to standard library unsigned duration
let dur = Duration::try_from(sdur)?;
// exactly 31 days where each day is 24 hours long.
assert_eq!(dur, Duration::from_secs(31 * 24 * 60 * 60));

// Now change the reference date and notice that the
// resulting duration is changed but still correct.
let sdur = span.to_duration(civil::date(2024, 6, 1))?;
let dur = Duration::try_from(sdur)?;
// exactly 30 days where each day is 24 hours long.
assert_eq!(dur, Duration::from_secs(30 * 24 * 60 * 60));

# Ok::<(), Box<dyn std::error::Error>>(())
```

# Motivation

This format was devised, in part, because the standard duration interchange
format specified by [Temporal's ISO 8601 definition](super::temporal) is
sub-optimal in two important respects:

1. It doesn't support individual sub-second components.
2. It is difficult to read.

In the first case, ISO 8601 durations do support sub-second components, but are
only expressible as fractional seconds. For example:

```text
PT1.100S
```

This is problematic in some cases because it doesn't permit distinguishing
between some spans. For example, `1.second().milliseconds(100)` and
`1100.milliseconds()` both serialize to the same ISO 8601 duration as shown
above. At deserialization time, it's impossible to know what the span originally
looked like. Thus, using the ISO 8601 format means the serialization and
deserialization of [`Span`](crate::Span) values is lossy.

In the second case, ISO 8601 durations appear somewhat difficult to quickly
read. For example:

```text
P1Y2M3DT4H59M1.1S
P1y2m3dT4h59m1.1S
```

When all of the unit designators are capital letters in particular (which
is the default), everything runs together and it's hard for the eye to
distinguish where digits stop and letters begin. Using lowercase letters for
unit designators helps somewhat, but this is an extension to ISO 8601 that
isn't broadly supported.

The "friendly" format resolves both of these problems by permitting sub-second
components and allowing the use of whitespace and longer unit designator labels
to improve readability. For example, all of the following are equivalent and
will parse to the same `Span`:

```text
1y 2mo 3d 4h 59m 1100ms
1 year 2 months 3 days 4h59m1100ms
1 year, 2 months, 3 days, 4h59m1100ms
1 year, 2 months, 3 days, 4 hours 59 minutes 1100 milliseconds
```

At the same time, the friendly format continues to support fractional
time components since they may be desirable in some cases. For example, all
of the following are equivalent:

```text
1h 1m 1.5s
1h 1m 1,5s
01:01:01.5
01:01:01,5
```

The idea with the friendly format is that end users who know how to write
English durations are happy to both read and write durations in this format.
And moreover, the format is flexible enough that end users generally don't need
to stare at a grammar to figure out how to write a valid duration. Most of the
intuitive things you'd expect to work will work.

# Internationalization

Currently, only US English unit designator labels are supported. In general,
Jiff resists trying to solve the internationalization problem in favor
of punting it to another crate, such as [`icu`] via [`jiff-icu`]. Jiff
_could_ adopt unit designator labels for other languages, but it's not
totally clear whether that's the right path to follow given the complexity
of internationalization. If you'd like to discuss it, please
[file an issue](https://github.com/BurntSushi/jiff/issues).

# Grammar

This section gives a more precise description of the "friendly" duration format
in the form of a grammar.

```text
format =
    format-signed-hms
    | format-signed-designator

format-signed-hms =
    sign? format-hms

format-hms =
    [0-9]+ ':' [0-9]+ ':' [0-9]+ fractional?

format-signed-designator =
    sign? format-designator-units
    | format-designator-units direction?
format-designator-units =
    years
    | months
    | weeks
    | days
    | hours
    | minutes
    | seconds
    | milliseconds
    | microseconds
    | nanoseconds

# This dance below is basically to ensure a few things:
# First, that at least one unit appears. That is, that
# we don't accept the empty string. Secondly, when a
# fractional component appears in a time value, we don't
# allow any subsequent units to appear. Thirdly, that
# `HH:MM:SS[.f{1,9}]?` is allowed after years, months,
# weeks or days.
years =
    unit-value unit-years comma? ws* format-hms
    | unit-value unit-years comma? ws* months
    | unit-value unit-years comma? ws* weeks
    | unit-value unit-years comma? ws* days
    | unit-value unit-years comma? ws* hours
    | unit-value unit-years comma? ws* minutes
    | unit-value unit-years comma? ws* seconds
    | unit-value unit-years comma? ws* milliseconds
    | unit-value unit-years comma? ws* microseconds
    | unit-value unit-years comma? ws* nanoseconds
    | unit-value unit-years
months =
    unit-value unit-months comma? ws* format-hms
    | unit-value unit-months comma? ws* weeks
    | unit-value unit-months comma? ws* days
    | unit-value unit-months comma? ws* hours
    | unit-value unit-months comma? ws* minutes
    | unit-value unit-months comma? ws* seconds
    | unit-value unit-months comma? ws* milliseconds
    | unit-value unit-months comma? ws* microseconds
    | unit-value unit-months comma? ws* nanoseconds
    | unit-value unit-months
weeks =
    unit-value unit-weeks comma? ws* format-hms
    | unit-value unit-weeks comma? ws* days
    | unit-value unit-weeks comma? ws* hours
    | unit-value unit-weeks comma? ws* minutes
    | unit-value unit-weeks comma? ws* seconds
    | unit-value unit-weeks comma? ws* milliseconds
    | unit-value unit-weeks comma? ws* microseconds
    | unit-value unit-weeks comma? ws* nanoseconds
    | unit-value unit-weeks
days =
    unit-value unit-days comma? ws* format-hms
    | unit-value unit-days comma? ws* hours
    | unit-value unit-days comma? ws* minutes
    | unit-value unit-days comma? ws* seconds
    | unit-value unit-days comma? ws* milliseconds
    | unit-value unit-days comma? ws* microseconds
    | unit-value unit-days comma? ws* nanoseconds
    | unit-value unit-days
hours =
    unit-value unit-hours comma? ws* minutes
    | unit-value unit-hours comma? ws* seconds
    | unit-value unit-hours comma? ws* milliseconds
    | unit-value unit-hours comma? ws* microseconds
    | unit-value unit-hours comma? ws* nanoseconds
    | unit-value fractional? ws* unit-hours
minutes =
    unit-value unit-minutes comma? ws* seconds
    | unit-value unit-minutes comma? ws* milliseconds
    | unit-value unit-minutes comma? ws* microseconds
    | unit-value unit-minutes comma? ws* nanoseconds
    | unit-value fractional? ws* unit-minutes
seconds =
    unit-value unit-seconds comma? ws* milliseconds
    | unit-value unit-seconds comma? ws* microseconds
    | unit-value unit-seconds comma? ws* nanoseconds
    | unit-value fractional? ws* unit-seconds
milliseconds =
    unit-value unit-milliseconds comma? ws* microseconds
    | unit-value unit-milliseconds comma? ws* nanoseconds
    | unit-value fractional? ws* unit-milliseconds
microseconds =
    unit-value unit-microseconds comma? ws* nanoseconds
    | unit-value fractional? ws* unit-microseconds
nanoseconds =
    unit-value fractional? ws* unit-nanoseconds

unit-value = [0-9]+ [ws*]
unit-years = 'years' | 'year' | 'yrs' | 'yr' | 'y'
unit-months = 'months' | 'month' | 'mos' | 'mo'
unit-weeks = 'weeks' | 'week' | 'wks' | 'wk' | 'w'
unit-days = 'days' | 'day' | 'd'
unit-hours = 'hours' | 'hour' | 'hrs' | 'hr' | 'h'
unit-minutes = 'minutes' | 'minute' | 'mins' | 'min' | 'm'
unit-seconds = 'seconds' | 'second' | 'secs' | 'sec' | 's'
unit-milliseconds =
    'milliseconds'
    | 'millisecond'
    | 'millis'
    | 'milli'
    | 'msecs'
    | 'msec'
    | 'ms'
unit-microseconds =
    'microseconds'
    | 'microsecond'
    | 'micros'
    | 'micro'
    | 'usecs'
    | 'usec'
    | 'µ' (U+00B5 MICRO SIGN) 'secs'
    | 'µ' (U+00B5 MICRO SIGN) 'sec'
    | 'us'
    | 'µ' (U+00B5 MICRO SIGN) 's'
unit-nanoseconds =
    'nanoseconds' | 'nanosecond' | 'nanos' | 'nano' | 'nsecs' | 'nsec' | 'ns'

fractional = decimal-separator decimal-fraction
decimal-separator = '.' | ','
decimal-fraction = [0-9]{1,9}

sign = '+' | '-'
direction = ws 'ago'
comma = ',' ws
ws =
    U+0020 SPACE
    | U+0009 HORIZONTAL TAB
    | U+000A LINE FEED
    | U+000C FORM FEED
    | U+000D CARRIAGE RETURN
```

One thing not specified by the grammar above are maximum values. Namely,
there are no specific maximum values imposed for each individual unit, nor
a maximum value for the entire duration (say, when converted to nanoseconds).
Instead, implementations are expected to impose their own limitations.

For Jiff, a `Span` is more limited than a `SignedDuration`. For example, a the
year component of a `Span` is limited to `[-19,999, 19,999]`. In contrast,
a `SignedDuration` is a 96-bit signed integer number of nanoseconds with no
particular limits on the individual units. They just can't combine to something
that overflows a 96-bit signed integer number of nanoseconds. (And parsing into
a `SignedDuration` directly only supports units of hours or smaller, since
bigger units do not have an invariant length.) In general, implementations
should support a "reasonable" range of values.

[`humantime`]: https://docs.rs/humantime
[`humantime-serde`]: https://docs.rs/humantime-serde
[`icu`]: https://docs.rs/icu
[`jiff-icu`]: https://docs.rs/jiff-icu
*/

pub use self::{
    parser::SpanParser,
    printer::{Designator, Direction, FractionalUnit, Spacing, SpanPrinter},
};

/// The default span/duration parser that we use.
pub(crate) static DEFAULT_SPAN_PARSER: SpanParser = SpanParser::new();

/// The default span/duration printer that we use.
pub(crate) static DEFAULT_SPAN_PRINTER: SpanPrinter = SpanPrinter::new();

mod parser;
mod parser_label;
mod printer;
