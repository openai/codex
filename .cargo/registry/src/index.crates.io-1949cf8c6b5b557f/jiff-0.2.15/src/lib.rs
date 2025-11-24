/*!
Jiff is a datetime library for Rust that encourages you to jump into the pit
of success. The focus of this library is providing high level datetime
primitives that are difficult to misuse and have reasonable performance.

Jiff takes enormous inspiration from [Temporal], which is a [TC39] proposal to
improve datetime handling in JavaScript.

Here is a quick example that shows how to parse a typical RFC 3339 instant,
convert it to a zone aware datetime, add a span of time and losslessly print
it:

```
use jiff::{Timestamp, ToSpan};

let time: Timestamp = "2024-07-11T01:14:00Z".parse()?;
let zoned = time.in_tz("America/New_York")?.checked_add(1.month().hours(2))?;
assert_eq!(zoned.to_string(), "2024-08-10T23:14:00-04:00[America/New_York]");
// Or, if you want an RFC3339 formatted string:
assert_eq!(zoned.timestamp().to_string(), "2024-08-11T03:14:00Z");

# Ok::<(), Box<dyn std::error::Error>>(())
```

[TC39]: https://tc39.es/
[Temporal]: https://tc39.es/proposal-temporal/docs/index.html

# Overview

The primary type in this crate is [`Zoned`]. A `Zoned` value is a datetime that
corresponds to a precise instant in time in a particular geographic region.
Users of this crate may find it helpful to think of a `Zoned` as a triple of
the following components:

* A [`Timestamp`] is a 96-bit integer of nanoseconds since the [Unix epoch].
A timestamp is a precise instant in time.
* A [`civil::DateTime`] is an inexact calendar date and clock time. The terms
"civil", "local", "plain" and "naive" are all used in various places to
describe this same concept.
* A [`tz::TimeZone`] is a set of rules for determining the civil time, via an
offset from [UTC], in a particular geographic region.

All three of these components are used to provide convenient high level
operations on `Zoned` such as computing durations, adding durations and
rounding.

A [`Span`] is this crate's primary duration type. It mixes calendar and clock
units into a single type. Jiff also provides [`SignedDuration`], which is like
[`std::time::Duration`], but signed. Users should default to a `Span` for
representing durations when using Jiff.

[Unix epoch]: https://en.wikipedia.org/wiki/Unix_time
[UTC]: https://en.wikipedia.org/wiki/Coordinated_Universal_Time

The remainder of this documentation is organized as follows:

* [Features](#features) gives a very brief summary of the features Jiff does
and does not support.
* [Usage](#usage) shows how to add Jiff to your Rust project.
* [Examples](#examples) shows a small cookbook of programs for common tasks.
* [Crate features](#crate-features) documents the Cargo features that can be
enabled or disabled for this crate.

Also, the `_documentation` sub-module serves to provide longer form
documentation:

* [Comparison with other Rust datetime crates](crate::_documentation::comparison)
* [The API design rationale for Jiff](crate::_documentation::design)
* [Platform support](crate::_documentation::platform)
* [CHANGELOG](crate::_documentation::changelog)

# Features

Here is a non-exhaustive list of the things that Jiff supports:

* Automatic and seamless integration with your system's copy of the
[IANA Time Zone Database]. When a platform doesn't have a time zone database,
Jiff automatically embeds a copy of it.
* A separation of datetime types between absolute times ([`Timestamp`] and
[`Zoned`]) and civil times ([`civil::DateTime`]).
* Nanosecond precision.
* Time zone and daylight saving time aware arithmetic.
* The primary duration type, [`Span`], mixes calendar and clock
units to provide an all-in-one human friendly experience that is time zone
aware.
* An "absolute" duration type, [`SignedDuration`], is like
[`std::time::Duration`] but signed.
* Datetime rounding.
* Span rounding, including calendar units and including taking time zones into
account.
* Formatting and parsing datetimes via a Temporal-specified hybrid format
that takes the best parts of [RFC 3339], [RFC 9557] and [ISO 8601]. This
includes lossless round tripping of zone aware datetimes and durations.
* Formatting and parsing according to [RFC 2822].
* Formatting and parsing via routines similar to [`strftime`] and [`strptime`].
* Formatting and parsing durations via a bespoke
["friendly" format](crate::fmt::friendly), with Serde support, that is meant
to service the [`humantime`](https://docs.rs/humantime) use cases.
* Opt-in Serde integration.
* Full support for dealing with ambiguous civil datetimes.
* Protection against deserializing datetimes in the future with an offset
different than what is possible with your copy of the Time Zone Database.
(This is done via [`tz::OffsetConflict`].)
* APIs that panic by design are clearly documented as such and few in number.
Otherwise, all operations that can fail, including because of overflow,
return a `Result`.

Here is also a list of things that Jiff doesn't currently support, along with
a link to a relevant issue (if one exists).

* [Leap seconds]. (Jiff will automatically constrain times like `23:59:60` to
`23:59:59`.)
* Time scales other than Unix.
* [Calendars other than Gregorian].
* [Localization].
* [Changing the representation size, precision or limits on the minimum and
maximum datetime values.][cppchrono]
* [Jiff aims to have reasonable performance and may not be capable of doing the
fastest possible thing.][perf]

At present, it is recommended to use the [`icu`] crate via [`jiff-icu`] for
localization and non-Gregorian use cases.

[Leap seconds]: https://github.com/BurntSushi/jiff/issues/7
[Calendars other than Gregorian]: https://github.com/BurntSushi/jiff/issues/6
[Localization]: https://github.com/BurntSushi/jiff/issues/4
[cppchrono]: https://github.com/BurntSushi/jiff/issues/3
[perf]: https://github.com/BurntSushi/jiff/issues/17
[`icu`]: https://docs.rs/icu
[`jiff-icu`]: https://docs.rs/jiff-icu

Please file an issue if you can think of more (substantial) things to add to
the above list.

[IANA Time Zone Database]: https://en.wikipedia.org/wiki/Tz_database
[RFC 3339]: https://www.rfc-editor.org/rfc/rfc3339
[RFC 9557]: https://www.rfc-editor.org/rfc/rfc9557.html
[ISO 8601]: https://www.iso.org/iso-8601-date-and-time-format.html

# Usage

Jiff is [on crates.io](https://crates.io/crates/jiff) and can be
used by adding `jiff` to your dependencies in your project's `Cargo.toml`.
Or more simply, just run `cargo add jiff`.

Here is a complete example that creates a new Rust project, adds a dependency
on `jiff`, creates the source code for a simple datetime program and then runs
it.

First, create the project in a new directory:

```text
$ cargo new jiff-example
$ cd jiff-example
```

Second, add a dependency on `jiff`:

```text
$ cargo add jiff
```

Third, edit `src/main.rs`. Delete what's there and replace it with this:

```
use jiff::{Unit, Zoned};

fn main() -> Result<(), jiff::Error> {
    let now = Zoned::now().round(Unit::Second)?;
    println!("{now}");
    Ok(())
}
```

Fourth, run it with `cargo run`:

```text
$ cargo run
   Compiling jiff v0.2.0 (/home/andrew/rust/jiff)
   Compiling jiff-play v0.2.0 (/home/andrew/tmp/scratch/rust/jiff-play)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.37s
     Running `target/debug/jiff-play`
2024-07-10T19:54:20-04:00[America/New_York]
```

The first time you run the program will show more output like above. But
subsequent runs shouldn't have to re-compile the dependencies.

# Examples

* [Get the current time in your system's time zone](#get-the-current-time-in-your-systems-time-zone)
* [Print the current time rounded to the nearest second](#print-the-current-time-rounded-to-the-nearest-second)
* [Print today's date at a specific time](#print-todays-date-at-a-specific-time)
* [Print the current Unix timestamp](#print-the-current-unix-timestamp)
* [Print the datetime for a timestamp](#print-the-datetime-for-a-timestamp)
* [Create a zoned datetime from civil time](#create-a-zoned-datetime-from-civil-time)
* [Change an instant from one time zone to another](#change-an-instant-from-one-time-zone-to-another)
* [Find the duration between two zoned datetimes](#find-the-duration-between-two-zoned-datetimes)
* [Add a duration to a zoned datetime](#add-a-duration-to-a-zoned-datetime)
* [Dealing with ambiguity](#dealing-with-ambiguity)
* [Parsing a span](#parsing-a-span)
* [Parsing an RFC 2822 datetime string](#parsing-an-rfc-2822-datetime-string)
* [Using `strftime` and `strptime` for formatting and parsing](#using-strftime-and-strptime-for-formatting-and-parsing)
* [Serializing and deserializing integer timestamps with Serde](#serializing-and-deserializing-integer-timestamps-with-serde)

### Get the current time in your system's time zone

The [`Zoned::now`] returns your system's time and also attempts
to automatically find your system's default time zone via
[`tz::TimeZone::system`]:

```
use jiff::Zoned;

let now = Zoned::now();
println!("{now}");
// Output: 2024-07-10T17:09:28.168146054-04:00[America/New_York]
```

### Print the current time rounded to the nearest second

This uses the [`Zoned::round`] API to round a zoned datetime to the nearest
second. This is useful, for example, if you don't care about fractional
seconds:

```
use jiff::{Unit, Zoned};

let now = Zoned::now().round(Unit::Second)?;
println!("{now}");
// Output: 2024-07-10T17:09:28-04:00[America/New_York]
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Print today's date at a specific time

Let's say you want to get the current date at 2pm. Here's one way of doing it
that makes use of [`Zoned::with`]:

```
use jiff::Zoned;

let zdt = Zoned::now().with()
    .hour(14)
    .minute(0)
    .second(0)
    .subsec_nanosecond(0)
    .build()?;
println!("{zdt}");
// Output: 2024-07-12T14:00:00-04:00[America/New_York]
# Ok::<(), Box<dyn std::error::Error>>(())
```

Or, if the time is known to be valid, you can use the infallibe
[`civil::time`](civil::time()) convenience constructor:

```
use jiff::{civil::time, Zoned};

let zdt = Zoned::now().with().time(time(14, 0, 0, 0)).build()?;
println!("{zdt}");
// Output: 2024-07-12T14:00:00-04:00[America/New_York]
# Ok::<(), Box<dyn std::error::Error>>(())
```

You can eliminate the possibility of a panic at runtime by using `time` in
a `const` block:

```
use jiff::{civil::time, Zoned};

let zdt = Zoned::now().with().time(const { time(14, 0, 0, 0) }).build()?;
println!("{zdt}");
// Output: 2024-07-12T14:00:00-04:00[America/New_York]
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Print the current Unix timestamp

This prints a Unix timestamp as the number of seconds since the Unix epoch
via [`Timestamp::now`]:

```
use jiff::Timestamp;

let now = Timestamp::now();
println!("{}", now.as_second());
// Output: 1720646365
```

Or print the current timestamp to nanosecond precision (which is the maximum
supported by Jiff):

```
use jiff::Timestamp;

let now = Timestamp::now();
println!("{}", now.as_nanosecond());
// Output: 1720646414218901664
```

### Print the datetime for a timestamp

This example shows how to convert a Unix timestamp, in milliseconds, to
a zoned datetime in the system's current time zone. This utilizes the
[`Timestamp::from_millisecond`] constructor, [`tz::TimeZone::system`] to get
the default time zone and the [`Timestamp::to_zoned`] routine to convert a
timestamp to a zoned datetime.

```
use jiff::{tz::TimeZone, Timestamp};

let ts = Timestamp::from_millisecond(1_720_646_365_567)?;
let zdt = ts.to_zoned(TimeZone::system());
println!("{zdt}");
// Output: 2024-07-10T17:19:25.567-04:00[America/New_York]
// Or if you just want the RFC 3339 time without bothering with time zones:
assert_eq!(ts.to_string(), "2024-07-10T21:19:25.567Z");

# Ok::<(), Box<dyn std::error::Error>>(())
```

### Create a zoned datetime from civil time

This example demonstrates the convenience constructor, [`civil::date`],
for a [`civil::Date`]. And use the [`civil::Date::at`] method to create
a [`civil::DateTime`]. Once we have a civil datetime, we can use
[`civil::DateTime::in_tz`] to do a time zone lookup and convert it to a precise
instant in time:

```
use jiff::civil::date;

let zdt = date(2023, 12, 31).at(18, 30, 0, 0).in_tz("America/New_York")?;
assert_eq!(zdt.to_string(), "2023-12-31T18:30:00-05:00[America/New_York]");

# Ok::<(), Box<dyn std::error::Error>>(())
```

Note that [`civil::date`] should only be used for inputs that are known to be
correct since it panics for an invalid date. If your date isn't known to be
valid, then use the fallible [`civil::Date::new`] constructor.

### Change an instant from one time zone to another

This shows how to find the civil time, in New York, when World War 1 ended:

```
use jiff::civil::date;

let zdt1 = date(1918, 11, 11).at(11, 0, 0, 0).in_tz("Europe/Paris")?;
let zdt2 = zdt1.in_tz("America/New_York")?;
assert_eq!(
    zdt2.to_string(),
    "1918-11-11T06:00:00-05:00[America/New_York]",
);

# Ok::<(), Box<dyn std::error::Error>>(())
```

### Find the duration between two zoned datetimes

This shows how to compute a span between two zoned datetimes. This utilizes
a [`Zoned`]'s implementation for `Sub`, permitting one to subtract two zoned
datetimes via the `-` operator:

```
use jiff::civil::date;

let zdt1 = date(2020, 8, 26).at(6, 27, 0, 0).in_tz("America/New_York")?;
let zdt2 = date(2023, 12, 31).at(18, 30, 0, 0).in_tz("America/New_York")?;
let span = &zdt2 - &zdt1;
assert_eq!(format!("{span:#}"), "29341h 3m");

# Ok::<(), Box<dyn std::error::Error>>(())
```

The above returns no units bigger than hours because it makes the operation
reversible in all cases. But if you don't need reversibility (i.e., adding the
span returned to `zdt1` gives you `zdt2`), then you can ask for bigger units
via [`Zoned::until`] to make the span more comprehensible:

```
use jiff::{civil::date, Unit};

let zdt1 = date(2020, 8, 26).at(6, 27, 0, 0).in_tz("America/New_York")?;
let zdt2 = date(2023, 12, 31).at(18, 30, 0, 0).in_tz("America/New_York")?;
let span = zdt1.until((Unit::Year, &zdt2))?;
assert_eq!(format!("{span:#}"), "3y 4mo 5d 12h 3m");

# Ok::<(), Box<dyn std::error::Error>>(())
```

### Add a duration to a zoned datetime

This example shows how one can add a [`Span`] to a [`Zoned`] via
[`Zoned::checked_add`] to get a new `Zoned` value. We utilize the [`ToSpan`]
trait for convenience construction of `Span` values.

```
use jiff::{civil::date, ToSpan};

let zdt1 = date(2020, 8, 26).at(6, 27, 0, 0).in_tz("America/New_York")?;
let span = 3.years().months(4).days(5).hours(12).minutes(3);
let zdt2 = zdt1.checked_add(span)?;
assert_eq!(zdt2.to_string(), "2023-12-31T18:30:00-05:00[America/New_York]");

# Ok::<(), Box<dyn std::error::Error>>(())
```

As with [`civil::date`], the [`ToSpan`] trait should only be used with inputs
that are known to be valid. If you aren't sure whether the inputs are valid,
then use [`Span::new`] and its fallible mutators like [`Span::try_years`].

### Dealing with ambiguity

In some cases, civil datetimes either don't exist in a particular time zone or
are repeated. By default, Jiff automatically uses the
[`tz::Disambiguation::Compatible`] strategy for choosing an instant in all
cases:

```
use jiff::civil::date;

// 2:30 on 2024-03-10 in New York didn't exist. It's a "gap."
// The compatible strategy selects the datetime after the gap.
let zdt = date(2024, 3, 10).at(2, 30, 0, 0).in_tz("America/New_York")?;
assert_eq!(zdt.to_string(), "2024-03-10T03:30:00-04:00[America/New_York]");

// 1:30 on 2024-11-03 in New York appeared twice. It's a "fold."
// The compatible strategy selects the datetime before the fold.
let zdt = date(2024, 11, 3).at(1, 30, 0, 0).in_tz("America/New_York")?;
assert_eq!(zdt.to_string(), "2024-11-03T01:30:00-04:00[America/New_York]");

# Ok::<(), Box<dyn std::error::Error>>(())
```

For more control over disambiguation, see
[`tz::TimeZone::to_ambiguous_zoned`]. Or
[`fmt::temporal::DateTimeParser::disambiguation`]
if you're parsing zoned datetimes.

### Parsing a span

Jiff supports parsing ISO 8601 duration strings:

```
use jiff::Span;

let span: Span = "P5y1w10dT5h59m".parse()?;
let expected = Span::new().years(5).weeks(1).days(10).hours(5).minutes(59);
assert_eq!(span, expected.fieldwise());

# Ok::<(), Box<dyn std::error::Error>>(())
```

The same format is used for serializing and deserializing `Span` values when
the `serde` feature is enabled.

Jiff also supports a bespoke ["friendly" format](crate::fmt::friendly) as
well:

```
use jiff::Span;

let expected = Span::new().years(5).weeks(1).days(10).hours(5).minutes(59);
let span: Span = "5 years, 1 week, 10 days, 5 hours, 59 minutes".parse()?;
assert_eq!(span, expected.fieldwise());
let span: Span = "5yrs 1wk 10d 5hrs 59mins".parse()?;
assert_eq!(span, expected.fieldwise());
let span: Span = "5y 1w 10d 5h 59m".parse()?;
assert_eq!(span, expected.fieldwise());

# Ok::<(), Box<dyn std::error::Error>>(())
```

### Parsing an RFC 2822 datetime string

While you probably shouldn't pick [RFC 2822] as a format for new things, it is
sometimes necessary to use it when something else requires it (like HTTP
or email). Parsing and printing of RFC 2822 datetimes is done via the
[`fmt::rfc2822`] module:

```
use jiff::fmt::rfc2822;

let zdt1 = rfc2822::parse("Thu, 29 Feb 2024 05:34 -0500")?;
let zdt2 = zdt1.in_tz("Australia/Tasmania")?;
assert_eq!(rfc2822::to_string(&zdt2)?, "Thu, 29 Feb 2024 21:34:00 +1100");
let zdt3 = zdt1.in_tz("Asia/Kolkata")?;
assert_eq!(rfc2822::to_string(&zdt3)?, "Thu, 29 Feb 2024 16:04:00 +0530");

# Ok::<(), Box<dyn std::error::Error>>(())
```

[RFC 2822]: https://datatracker.ietf.org/doc/html/rfc2822

### Using `strftime` and `strptime` for formatting and parsing

Jiff has support for the C style [`strftime`] and [`strptime`] functions for
formatting and parsing datetime types. All of Jiff's datetime types having a
`strptime` constructor for parsing, and a `strftime` method for formatting.
For example, this shows how to use [`Zoned::strptime`] to parsed a string in
a "odd" custom format into a zoned datetime:

```
use jiff::Zoned;

let zdt = Zoned::strptime(
    "%A, %B %d, %Y at %I:%M%p %Q",
    "Monday, July 15, 2024 at 5:30pm US/Eastern",
)?;
assert_eq!(zdt.to_string(), "2024-07-15T17:30:00-04:00[US/Eastern]");

# Ok::<(), Box<dyn std::error::Error>>(())
```

And this shows how to use [`Zoned::strftime`] to format a zoned datetime.
Note the use of `%Z`, which will print a time zone abbreviation (when one is
available) instead of an offset (`%Z` can't be used for parsing):

```
use jiff::civil::date;

let zdt = date(2024, 7, 15).at(17, 30, 59, 0).in_tz("Australia/Tasmania")?;
// %-I instead of %I means no padding.
let string = zdt.strftime("%A, %B %d, %Y at %-I:%M%P %Z").to_string();
assert_eq!(string, "Monday, July 15, 2024 at 5:30pm AEST");

# Ok::<(), Box<dyn std::error::Error>>(())
```

However, time zone abbreviations aren't parsable because they are ambiguous.
For example, `CST` can stand for `Central Standard Time`, `Cuba Standard Time`
or `China Standard Time`. Instead, it is recommended to use `%Q` to format an
IANA time zone identifier (which can be parsed, as shown above):

```
use jiff::civil::date;

let zdt = date(2024, 7, 15).at(17, 30, 59, 0).in_tz("Australia/Tasmania")?;
// %-I instead of %I means no padding.
let string = zdt.strftime("%A, %B %d, %Y at %-I:%M%P %Q").to_string();
assert_eq!(string, "Monday, July 15, 2024 at 5:30pm Australia/Tasmania");

# Ok::<(), Box<dyn std::error::Error>>(())
```

See the [`fmt::strtime`] module documentation for supported conversion
specifiers and other APIs.

[`strftime`]: https://pubs.opengroup.org/onlinepubs/009695399/functions/strftime.html
[`strptime`]: https://pubs.opengroup.org/onlinepubs/009695399/functions/strptime.html

### Serializing and deserializing integer timestamps with Serde

Sometimes you need to interact with external services that use integer timestamps
instead of something more civilized like RFC 3339. Since [`Timestamp`]'s
Serde integration uses RFC 3339, you'll need to override the default. While
you could hand-write this, Jiff provides convenience routines that do this
for you. But you do need to wire it up via [Serde's `with` attribute]:

```
use jiff::Timestamp;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Record {
    #[serde(with = "jiff::fmt::serde::timestamp::second::required")]
    timestamp: Timestamp,
}

let json = r#"{"timestamp":1517644800}"#;
let got: Record = serde_json::from_str(&json)?;
assert_eq!(got.timestamp, Timestamp::from_second(1517644800)?);
assert_eq!(serde_json::to_string(&got)?, json);

# Ok::<(), Box<dyn std::error::Error>>(())
```

If you need to support optional timestamps via `Option<Timestamp>`, then use
`jiff::fmt::serde::timestamp::second::optional` instead.

For more, see the [`fmt::serde`] sub-module. (This requires enabling Jiff's
`serde` crate feature.)

[Serde's `with` attribute]: https://serde.rs/field-attrs.html#with

# Crate features

### Ecosystem features

* **std** (enabled by default) -
  When enabled, Jiff will depend on Rust's standard library. This is needed
  for things that require interacting with your system, such as reading
  `/usr/share/zoneinfo` on Unix systems for time zone information, or for
  finding your system's default time zone. But if you don't need that (or can
  bundle the Time Zone Database), then Jiff has nearly full functionality
  without `std` enabled, excepting things like `std::error::Error` trait
  implementations and a global time zone database (which is required for
  things like [`Timestamp::in_tz`] to work).
* **alloc** (enabled by default) -
  When enabled, Jiff will depend on the `alloc` crate. In particular, this
  enables functionality that requires or greatly benefits from dynamic memory
  allocation. If you can enable this, it is strongly encouraged that you do so.
  Without it, only fixed time zones are supported and error messages are
  significantly degraded. Also, the sizes of some types get bigger. If you
  have use cases for Jiff in a no-std and no-alloc context, I would love
  feedback on the issue tracker about your use cases.
* **logging** -
  When enabled, the `log` crate is used to emit messages where appropriate.
  Generally speaking, this is reserved for system interaction points, such as
  finding the system copy of the Time Zone Database or finding the system's
  default time zone.
* **serde** -
  When enabled, all of the datetime and span types in Jiff implement
  serde's `Serialize` and `Deserialize` traits. The format used is specified by
  Temporal, but it's a mix of the "best" parts of RFC 3339, RFC 9557 and
  ISO 8601. See the [`fmt::temporal`] module for more details on the format
  used.
* **js** -
  On _only_ the `wasm32-unknown-unknown` and `wasm64-unknown-unknown` targets,
  the `js` feature will add dependencies on `js-sys` and `wasm-bindgen`.
  These dependencies are used to determine the current datetime and time
  zone from the web browser. On these targets without the `js` feature
  enabled, getting the current datetime will panic (because that's what
  `std::time::SystemTime::now()` does), and it won't be possible to determine
  the time zone. This feature is disabled by default because not all uses
  of `wasm{32,64}-unknown-unknown` are in a web context, although _many_ are
  (for example, when using `wasm-pack`). Only binary, tests and benchmarks
  should enable this feature. See
  [Platform support](crate::_documentation::platform) for more details.

### Time zone features

* **tz-system** (enabled by default) -
  When enabled, Jiff will include code that attempts to determine the "system"
  time zone. For example, on Unix systems, this is usually determined by
  looking at the symlink information on `/etc/localtime`. But in general, it's
  very platform specific and heuristic oriented. On some platforms, this may
  require extra dependencies. (For example, `windows-sys` on Windows.)
* **tz-fat** (enabled by default) -
  When enabled, Jiff will "fatten" time zone data with extra transitions to
  make time zone lookups faster. This may result in increased heap memory
  (when loading time zones from `/usr/share/zoneinfo`) or increased binary
  size (when using the `jiff-static` proc macros). Note that this doesn't add
  more transitions than are likely already in `/usr/share/zoneinfo`, depending
  on how it was generated.
* **tzdb-bundle-always** -
  When enabled, Jiff will forcefully depend on the `jiff-tzdb` crate, which
  embeds an entire copy of the Time Zone Database. You should avoid this unless
  you have a specific need for it, since it is better to rely on your system's
  copy of time zone information. (Which may be updated multiple times per
  year.)
* **tzdb-bundle-platform** (enabled by default) -
  When enabled, Jiff will depend on `jiff-tzdb` only for platforms where it is
  known that there is no canonical copy of the Time Zone Database. For example,
  Windows.
* **tzdb-zoneinfo** (enabled by default) -
  When enabled, Jiff will attempt to look for your system's copy of the Time
  Zone Database.
* **tzdb-concatenated** (enabled by default) -
  When enabled, Jiff will attempt to look for a system copy of the
  [Concatenated Time Zone Database]. This is primarily meant for reading time
  zone information on Android platforms. The `ANDROID_ROOT` and `ANDROID_DATA`
  environment variables (with sensible default fallbacks) are used to construct
  candidate paths to look for this database. For more on this, see the
  [Android section of the platform support documentation](crate::_documentation::platform#android).
* **static** -
  When enabled, new procedural macros will be added to the `tz` sub-module for
  creating static `TimeZone` values at compile-time. This adds a dependency on
  [`jiff-static`] and [`jiff-tzdb`]. `jiff-static` defines the macros, and Jiff
  re-exports them. This also enables `static-tz`.
* **static-tz** -
  When enabled, a `jiff::tz::include` procedural macro will become available.
  This takes a TZif file path, like `/usr/share/zoneinfo/Israel`, as input and
  returns a `TimeZone` value at compile time.

### Performance features

* **perf-inline** (enabled by default) -
  When enabled, a number of `inline(always)` annotations are used inside of
  Jiff to improve performance. This can especially impact formatting and
  parsing of datetimes. If the extra performance isn't needed or if you want
  to prioritize smaller binary sizes and shorter compilation times over
  runtime performance, then it can be useful to disable this feature.

[`jiff-static`]: https://docs.rs/jiff-static
[`jiff-tzdb`]: https://docs.rs/jiff-tzdb
[Concatenated Time Zone Database]: https://android.googlesource.com/platform/libcore/+/jb-mr2-release/luni/src/main/java/libcore/util/ZoneInfoDB.java
*/

#![no_std]
// Lots of rustdoc links break when disabling default features because docs
// aren't written conditionally.
#![cfg_attr(
    all(
        feature = "std",
        feature = "serde",
        feature = "static",
        feature = "tzdb-zoneinfo"
    ),
    deny(rustdoc::broken_intra_doc_links)
)]
#![cfg_attr(
    not(all(
        feature = "std",
        feature = "serde",
        feature = "static",
        feature = "tzdb-zoneinfo"
    )),
    allow(rustdoc::broken_intra_doc_links)
)]
// These are just too annoying to squash otherwise.
#![cfg_attr(
    not(all(
        feature = "std",
        feature = "tzdb-zoneinfo",
        feature = "tzdb-concatenated",
        feature = "tz-system",
    )),
    allow(dead_code, unused_imports)
)]
// No clue why this thing is still unstable because it's pretty amazing. This
// adds Cargo feature annotations to items in the rustdoc output. Which is
// sadly hugely beneficial for this crate due to the number of features.
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
// We generally want all types to impl Debug.
#![warn(missing_debug_implementations)]
// Document ALL THE THINGS!
#![deny(missing_docs)]
// See: https://github.com/rust-lang/rust/pull/121364
#![allow(unknown_lints, ambiguous_negative_literals)]
// See: https://github.com/rust-lang/rust/pull/121364
#![doc(test(attr(allow(unknown_lints, ambiguous_negative_literals))))]

// It should be possible to support other pointer widths, but this library
// hasn't been tested nor thought about much in contexts with pointers less
// than 32 bits.
//
// If you need support for 8-bit or 16-bit, please submit a bug report at
// https://github.com/BurntSushi/jiff
#[cfg(not(any(target_pointer_width = "32", target_pointer_width = "64")))]
compile_error!("jiff currently not supported on non-{32,64}");

#[cfg(any(test, feature = "std"))]
extern crate std;

#[cfg(any(test, feature = "alloc"))]
extern crate alloc;

pub use crate::{
    error::Error,
    signed_duration::{SignedDuration, SignedDurationRound},
    span::{
        Span, SpanArithmetic, SpanCompare, SpanFieldwise, SpanRelativeTo,
        SpanRound, SpanTotal, ToSpan, Unit,
    },
    timestamp::{
        Timestamp, TimestampArithmetic, TimestampDifference,
        TimestampDisplayWithOffset, TimestampRound, TimestampSeries,
    },
    util::round::mode::RoundMode,
    zoned::{Zoned, ZonedArithmetic, ZonedDifference, ZonedRound, ZonedWith},
};

#[macro_use]
mod logging;

pub mod civil;
mod duration;
mod error;
pub mod fmt;
#[cfg(feature = "std")]
mod now;
#[doc(hidden)]
pub mod shared;
mod signed_duration;
mod span;
mod timestamp;
pub mod tz;
mod util;
mod zoned;

/// Longer form documentation for Jiff.
pub mod _documentation {
    #[doc = include_str!("../COMPARE.md")]
    pub mod comparison {}
    #[doc = include_str!("../DESIGN.md")]
    pub mod design {}
    #[doc = include_str!("../PLATFORM.md")]
    pub mod platform {}
    #[doc = include_str!("../CHANGELOG.md")]
    pub mod changelog {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
    #[test]
    fn now_works() {
        let _ = crate::logging::Logger::init();

        let zdt = Zoned::now();
        std::println!("{zdt}");
    }

    #[cfg(feature = "std")]
    #[test]
    fn ranges() {
        use crate::util::t;

        dbg!((t::SpanYears::MIN, t::SpanYears::MAX));
        dbg!((t::SpanMonths::MIN, t::SpanMonths::MAX));
        dbg!((t::SpanWeeks::MIN, t::SpanWeeks::MAX));
        dbg!((t::SpanDays::MIN, t::SpanDays::MAX));
        dbg!((t::SpanHours::MIN, t::SpanHours::MAX));
        dbg!((t::SpanMinutes::MIN, t::SpanMinutes::MAX));
        dbg!((t::SpanSeconds::MIN, t::SpanSeconds::MAX));
        dbg!((t::SpanMilliseconds::MIN, t::SpanMilliseconds::MAX));
        dbg!((t::SpanMicroseconds::MIN, t::SpanMicroseconds::MAX));
        dbg!((t::SpanNanoseconds::MIN, t::SpanNanoseconds::MAX));
        dbg!((t::UnixSeconds::MIN, t::UnixSeconds::MAX));
        dbg!((t::UnixEpochDay::MIN, t::UnixEpochDay::MAX));
    }

    #[cfg(feature = "std")]
    #[test]
    fn maximally_long_span() {
        use crate::{fmt::friendly, util::t};

        let span = Span::new()
            .years(t::SpanYears::MAX_REPR)
            .months(t::SpanMonths::MAX_REPR)
            .weeks(t::SpanWeeks::MAX_REPR)
            .days(t::SpanDays::MAX_REPR)
            .hours(t::SpanHours::MAX_REPR)
            .minutes(t::SpanMinutes::MAX_REPR)
            .seconds(t::SpanSeconds::MAX_REPR)
            .milliseconds(t::SpanMilliseconds::MAX_REPR)
            .microseconds(t::SpanMicroseconds::MAX_REPR)
            .nanoseconds(t::SpanNanoseconds::MAX_REPR)
            .negate();
        std::println!("{span}");
        std::println!("{span:#}");

        let long_printer = friendly::SpanPrinter::new()
            .designator(friendly::Designator::Verbose)
            .spacing(friendly::Spacing::BetweenUnitsAndDesignators)
            .comma_after_designator(true);
        std::println!("{}", long_printer.span_to_string(&span));
    }

    #[cfg(feature = "std")]
    #[test]
    fn maximally_long_duration() {
        let sdur = SignedDuration::MIN;
        std::println!("{sdur}");
        std::println!("{sdur:#}");
    }
}
