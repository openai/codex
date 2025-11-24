# Comparison with other Rust datetime crates

This document is meant to be a comparison between Jiff and each of the other
prominent open source datetime libraries for Rust. If you feel like there is a
library missing from this list, please file an issue about it. I would prefer
to only add libraries to this list that are being used in production or have a
substantial number of users.

The goal of this document is to be as _descriptive_ and _substantively
complete_ as possible. For example, "Chrono has a better API design than Jiff"
would be a pretty vague value judgment that someone could easily disagree with.
But, "Chrono allows using a zone-aware datetime type that is `Copy` while Jiff
does not" would be a factual comparison that someone might use to _support_ an
opinion that Chrono's API design is better than Jiff's. In other words, this
document should provide the "facts of comparison" but refrain from assigning
value judgments.

In terms of completeness, it is probably not realistic to expect 100%
completion here. We aren't hunting for Korok Seeds. Instead, this document
aims for _substantive_ completion. That is, if there's a point of difference
between Jiff and another library that would likely influence someone's decision
of which library to use, and can be articulated descriptively, then it should
probably be in this document.

The current status of this document is that it is both _incomplete_ and
_biased_. That is, this first draft was written by the author of Jiff without
any input from other crate maintainers. (To other crate maintainers: I welcome
feedback. Even if it's just filing an issue.)

Note that this document contains many code snippets. They can be tested with
`cargo test --doc _documentation::comparison` from the root of this repository.

## [`chrono`](https://docs.rs/chrono) (v0.4.38)

Chrono is a Rust datetime library that provides a time zone aware datetime
type.

For the following comparisons, a `Cargo.toml` with the following dependencies
should be able to run any of the programs in this section:

```toml
anyhow = "1.0.81"
chrono = "0.4.38"
chrono-tz = { version = "0.9.0", features = ["serde"] }
jiff = { version = "0.2.0", features = ["serde"] }
serde = "1.0.203"
serde_json = "1.0.117"
tzfile = "0.1.3"
```

### Time zone database integration

Jiff gives you automatic integration with your copy of the Time Zone Database.
On Unix, it's usually found at `/usr/share/zoneinfo`. On Windows, since there
is no canonical location, Jiff will depend on `jiff-tzdb` by default, which
will embed the entire database into your binary. Jiff hides these details from
you. For example, to convert a civil time into an absolute time in a particular
time zone:

```rust
use jiff::civil::date;

fn main() -> anyhow::Result<()> {
    let zdt = date(2024, 6, 30).at(9, 46, 0, 0).in_tz("America/New_York")?;
    assert_eq!(zdt.to_string(), "2024-06-30T09:46:00-04:00[America/New_York]");
    Ok(())
}
```

For Chrono, one recommended option is to use the
[`chrono-tz`](https://docs.rs/chrono-tz) crate:

```rust
use anyhow::Context;
use chrono::TimeZone;
use chrono_tz::America::New_York;

fn main() -> anyhow::Result<()> {
    let zdt = New_York.with_ymd_and_hms(2024, 6, 30, 9, 46, 0)
        .single()
        .context("invalid naive time")?;
    assert_eq!(zdt.to_string(), "2024-06-30 09:46:00 EDT");
    Ok(())
}
```

`chrono-tz` works by embedding an entire copy of the Time Zone Database into
your binary, where each time zone is represented as a Rust value that can be
imported via `use`. A disadvantage of this approach is that you're reliant on
`chrono-tz` updates to get the most recent time zone information. An advantage
of this approach is that you never need to worry about an end user's system
state. Another advantage is that this allows a `TimeZone` trait implementation
to be `Copy` via a `&Tz`, and that in turn allows a `chrono::DateTime` to be
`Copy`. In contrast, in Jiff, a `TimeZone` is never `Copy`. Since a `Zoned`
embeds a `TimeZone`, a `Zoned` is never `Copy` either.

Another recommended option is the [`tzfile`](https://docs.rs/tzfile) crate.
Unlike `chrono-tz`, the `tzfile` crate will try to read time zone data from
your system's copy of the Time Zone Database.

```rust
use anyhow::Context;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use tzfile::Tz;

#[cfg(unix)]
fn main() -> anyhow::Result<()> {
    let tz = Tz::named("America/New_York")?;
    let zdt = (&tz).with_ymd_and_hms(2024, 6, 30, 9, 46, 0)
        .single()
        .context("invalid naive time")?;
    assert_eq!(zdt.to_string(), "2024-06-30 09:46:00 EDT");
    Ok(())
}

// `tzfile` exposes a platform specific API, which means
// users of the crate have to deal with platform differences
// themselves.
#[cfg(not(unix))]
fn main() -> anyhow::Result<()> {
    Ok(())
}
```

Note though that at time of writing (2024-07-11), `tzfile::Tz::named` will
read and parse the corresponding time zone rules from disk on every call.
Conversely, in Jiff, all time zone lookups by name are cached. This may or may
not matter for your use case.

### Jiff losslessly roundtrips time zone aware datetimes

In Jiff, with `serde` support enabled, one can serialize and deserialize a
`Zoned` value losslessly. This means that, after deserialization, you can
expect it to still perform DST arithmetic:

```rust
use jiff::{civil::date, ToSpan, Zoned};

fn main() -> anyhow::Result<()> {
    let zdt = date(2024, 3, 10).at(1, 59, 59, 0).in_tz("America/New_York")?;

    let json = serde_json::to_string_pretty(&zdt)?;
    assert_eq!(json, "\"2024-03-10T01:59:59-05:00[America/New_York]\"");

    let got: Zoned = serde_json::from_str(&json)?;
    assert_eq!(got.to_string(), "2024-03-10T01:59:59-05:00[America/New_York]");
    let next = got.checked_add(1.minute())?;
    assert_eq!(next.to_string(), "2024-03-10T03:00:59-04:00[America/New_York]");

    Ok(())
}
```

Notice that when we add a minute, it jumps to `03:00` civil time because of the
transition into daylight saving time in my selected time zone. Notice also the
offset change from `-05` to `-04`.

Compare this with Chrono which also supports `serde`, but not with `chrono-tz`
or `tzfile`. One option is to use its `Local` implementation of its `TimeZone`
trait:

```rust,no_run
use anyhow::Context;
use chrono::{DateTime, FixedOffset, Local, TimeDelta, TimeZone};

fn main() -> anyhow::Result<()> {
    let zdt = Local.with_ymd_and_hms(2024, 3, 10, 1, 59, 59)
        .single()
        .context("invalid naive time")?;

    let json = serde_json::to_string_pretty(&zdt)?;
    // Chrono only serializes the offset, which makes lossless
    // deserialization impossible. Chrono loses the time zone
    // information.
    assert_eq!(json, "\"2024-03-10T01:59:59-05:00\"");

    // The serialized datetime has no time zone information,
    // so unless there is some out-of-band information saying
    // what its time zone is, we're forced to use a fixed offset:
    let got: DateTime<FixedOffset> = serde_json::from_str(&json)?;
    assert_eq!(got.to_string(), "2024-03-10 01:59:59 -05:00");
    let next = got.checked_add_signed(TimeDelta::minutes(1))
        .context("arithmetic failed")?;
    // This is correct for fixed offset, but it's no longer
    // DST aware.
    assert_eq!(next.to_string(), "2024-03-10 02:00:59 -05:00");

    // We could deserialize into a `DateTime<Local>`, but this
    // requires knowing that the time zone of the datetime matches
    // local time zone. Which you might know. But you might not.
    let got: DateTime<Local> = serde_json::from_str(&json)?;
    assert_eq!(got.to_string(), "2024-03-10 01:59:59 -05:00");
    let next = got.checked_add_signed(TimeDelta::minutes(1))
        .context("arithmetic failed")?;
    assert_eq!(next.to_string(), "2024-03-10 03:00:59 -04:00");

    Ok(())
}
```

Or, if you have a `Tz` from `chrono-tz`. But in this case, since `chrono-tz`
doesn't support Serde, you have to convert to a `DateTime<FixedOffset>`. Like
above, you'll lose DST safe arithmetic after deserialization:

```rust
use anyhow::Context;
use chrono::{DateTime, FixedOffset, TimeDelta, TimeZone};
use chrono_tz::America::New_York;

fn main() -> anyhow::Result<()> {
    let zdt = New_York.with_ymd_and_hms(2024, 3, 10, 1, 59, 59)
        .single()
        .context("invalid naive time")?;

    let json = serde_json::to_string_pretty(&zdt.fixed_offset())?;
    // Chrono only serializes the offset, which makes lossless
    // deserialization impossible. Chrono loses the time zone
    // information.
    assert_eq!(json, "\"2024-03-10T01:59:59-05:00\"");

    // The serialized datetime has no time zone information,
    // so unless there is some out-of-band information saying
    // what its time zone is, we're forced to use a fixed offset:
    let got: DateTime<FixedOffset> = serde_json::from_str(&json)?;
    assert_eq!(got.to_string(), "2024-03-10 01:59:59 -05:00");
    let next = got.checked_add_signed(TimeDelta::minutes(1))
        .context("arithmetic failed")?;
    // This is correct for fixed offset, but it's no longer
    // DST aware.
    assert_eq!(next.to_string(), "2024-03-10 02:00:59 -05:00");

    Ok(())
}
```

The main way to solve this problem (and is how `java.time`, Temporal and Jiff
solve it), is by supporting [RFC 9557]. Otherwise, the only way to fully
capture Jiff's functionality in Chrono is to define a custom serialization
format that includes the instant, the time zone identifier *and* the offset.
(The offset is used for conflict resolution when deserializing datetimes made
in the future for which their offset has changed due to changes in the time
zone database.)

[RFC 9557]: https://datatracker.ietf.org/doc/rfc9557/

### Jiff provides support for zone aware calendar arithmetic

With Jiff, you can add non-uniform units like days to time zone aware datetimes,
and get non-uniform units like days as a representation of a span between
datetimes. And they agree on the results.

```rust
use jiff::{civil::date, ToSpan, Unit};

fn main() -> anyhow::Result<()> {
    let zdt1 = date(2024, 3, 9).at(21, 0, 0, 0).in_tz("America/New_York")?;
    let zdt2 = zdt1.checked_add(1.day())?;

    // Even though 2 o'clock didn't occur on 2024-03-10, adding 1 day
    // returns the same civil time the next day.
    assert_eq!(zdt2.to_string(), "2024-03-10T21:00:00-04:00[America/New_York]");
    // The span of time is 23 hours:
    assert_eq!(&zdt2 - &zdt1, 23.hours().fieldwise());
    // But if you ask for the span in units of days, you get exactly 1:
    assert_eq!(zdt1.until((Unit::Day, &zdt2))?, 1.day().fieldwise());

    Ok(())
}
```

This is important and difficult to get right because some days are only 23
hours long (typically the day of the year where DST starts) and some days are
25 hours long (typically the day of the year where DST ends). With Jiff, you
can seamlessly go back-and-forth between calendar units and clock units without
worrying about whether "day" will be interpreted differently.

Chrono has some support for this. Namely, it can add units of days in a time
zone aware fashion, but it cannot produce spans of time involving days between
two zone aware datetimes that is consistent with adding days.

```rust
use anyhow::Context;
use chrono::{Days, TimeDelta, TimeZone};
use chrono_tz::America::New_York;

fn main() -> anyhow::Result<()> {
    let zdt1 = New_York.with_ymd_and_hms(2024, 3, 9, 21, 0, 0)
        .single()
        .context("invalid naive time")?;

    // Adding 1 day via TimeDelta leads to a result that is
    // 24 hours later, including the gap at 2am on 2024-03-10.
    // As a result, you get a different civil time, which is
    // usually not what is intended.
    let zdt2 = zdt1.checked_add_signed(TimeDelta::days(1))
        .context("adding a time delta failed")?;
    assert_eq!(zdt2.to_string(), "2024-03-10 22:00:00 EDT");

    // However, Chrono does expose a separate API for adding
    // units of days specifically. This does get you the
    // correct result.
    let zdt2 = zdt1.checked_add_days(Days::new(1))
        .context("adding days failed")?;
    assert_eq!(zdt2.to_string(), "2024-03-10 21:00:00 EDT");

    // The only way to compute a duration between two datetimes
    // in Chrono is with a `TimeDelta`:
    let delta = zdt2.signed_duration_since(&zdt1);
    // And since `TimeDelta` assumes all days are exactly 24
    // hours long, you get a result of `0` days. If this were
    // a fold, the number of days would be `1`, but you'd also
    // have a number of hours equal to `1`.
    assert_eq!(delta.num_days(), 0);

    Ok(())
}
```

### Jiff losslessly roundtrips durations

Jiff implements something close to ISO 8601 to provide lossless serialization
and deserialization of its `Span` type. A `Span` covers both calendar and clock
units.

```rust
use jiff::{Span, ToSpan};

fn main() -> anyhow::Result<()> {
    let span = 5.years().months(2).days(1).hours(20);
    let json = serde_json::to_string_pretty(&span)?;
    assert_eq!(json, "\"P5Y2M1DT20H\"");

    let got: Span = serde_json::from_str(&json)?;
    assert_eq!(got, span.fieldwise());

    Ok(())
}
```

Chrono [does not currently have Serde support for its duration type][serde-duration].

[serde-duration]: https://github.com/chronotope/chrono/issues/117

### Jiff supports dealing with gaps in civil time

A gap in civil time most typically occurs when a particular region enters
daylight saving time. When this happens, some time on the clocks in that region
is skipped. It never appears. (A fold happens when the clocks are rolled back,
usually when leaving daylight saving time. In this case, some time on the clock
is repeated.)

Jiff supports automatically selecting a "reasonable" choice in either case
via its "compatible" strategy (as specified by [RFC 5545]).

```rust
use jiff::Zoned;

fn main() -> anyhow::Result<()> {
    // This is a gap. The default strategy takes the time after the gap.
    let zdt: Zoned = "2024-03-10 02:30[America/New_York]".parse()?;
    assert_eq!(zdt.to_string(), "2024-03-10T03:30:00-04:00[America/New_York]");

    // This is a fold. The default strategy takes the time before the fold.
    let zdt: Zoned = "2024-11-03 01:30[America/New_York]".parse()?;
    // The time after the fold would be identical,
    // except the offset would be -05.
    assert_eq!(zdt.to_string(), "2024-11-03T01:30:00-04:00[America/New_York]");

    Ok(())
}
```

Jiff also exposes all information available with respect to ambiguous civil
datetimes via `tz::AmbiguousZoned`, `tz::AmbiguousTimestamp` and
`tz::AmbiguousOffset`. This enables callers to implement whatever strategy
they want.

While Chrono will let you deal with folds, it returns `MappedLocalTime::None`
in the case of a gap with no additional information. So there's really nothing
else you can conveniently do in this case except return an error:

```rust
use anyhow::Context;
use chrono::{offset::MappedLocalTime, TimeZone};
use chrono_tz::America::New_York;

fn main() -> anyhow::Result<()> {
    // For gaps, Chrono exposes no additional information.
    let mapped = New_York.with_ymd_and_hms(2024, 3, 10, 2, 30, 0);
    assert_eq!(mapped, MappedLocalTime::None);

    // For folds, Chrono gives you the two choices.
    // This is approximately equivalent to what Jiff exposes
    // in the case of a fold.
    let zdt = New_York.with_ymd_and_hms(2024, 11, 3, 1, 30, 0)
        .earliest()
        .context("invalid datetime")?;
    assert_eq!(zdt.to_string(), "2024-11-03 01:30:00 EDT");

    Ok(())
}
```

[RFC 5545]: https://datatracker.ietf.org/doc/html/rfc5545

### Jiff supports rounding durations

In Jiff, one can round the duration computed between two datetimes:

```rust
use jiff::{civil::date, RoundMode, ToSpan, Unit, ZonedDifference};

fn main() -> anyhow::Result<()> {
    let zdt1 = date(2001, 11, 18).at(8, 30, 0, 0).in_tz("America/New_York")?;
    let zdt2 = date(2024, 7, 11).at(22, 38, 0, 0).in_tz("America/New_York")?;

    let round_options = ZonedDifference::new(&zdt2)
        .largest(Unit::Year)
        .smallest(Unit::Day)
        .mode(RoundMode::HalfExpand);
    let span = zdt1.until(round_options)?;
    assert_eq!(span.fieldwise(), 22.years().months(7).days(24));

    Ok(())
}
```

While Chrono supports rounding datetimes themselves via its
`chrono::duration::DurationRound` trait, it does not support rounding durations
themselves. Indeed, its principle duration type, `TimeDelta`, is an "absolute"
duration like `std::time::Duration` (except that it is signed). It doesn't keep
track of individual units like Jiff does. Instead, everything gets normalized
into a 96-bit integer number of nanoseconds. With this representation, it is
impossible to do DST safe rounding to non-uniform units like days.

### Jiff supports zone-aware rounding of durations

Jiff's duration rounding is time zone aware. For example, if you're rounding to
a number of days, it knows to round 11.5 hours up to 1 day on days with gaps,
and round 12 hours down to 0 days on days with folds. The only requirement
is that we provide a reference datetime with which to interpret the span.

```rust
use jiff::{civil::date, SpanRound, ToSpan, Unit};

fn main() -> anyhow::Result<()> {
    let gapday = date(2024, 3, 10).in_tz("America/New_York")?;
    let foldday = date(2024, 11, 3).in_tz("America/New_York")?;

    let span1 = 11.hours().minutes(30);
    let span2 = span1.round(
        SpanRound::new().smallest(Unit::Day).relative(&gapday),
    )?;
    // rounds up, even though on a normal day 11.5 hours would round down.
    assert_eq!(span2, 1.day().fieldwise());

    let span1 = 12.hours();
    let span2 = span1.round(
        SpanRound::new().smallest(Unit::Day).relative(&foldday),
    )?;
    // rounds down, even though on a normal day 12 hours would round up.
    assert_eq!(span2, 0.days().fieldwise());

    Ok(())
}
```

As with the previous section, Chrono does not support rounding durations or
rounding units like `Days` with respect to a reference datetime.

### Jiff supports re-balancing durations

This example is like the one above, except we choose a smaller "largest" unit:

```rust
use jiff::{civil::date, RoundMode, ToSpan, Unit, ZonedDifference};

fn main() -> anyhow::Result<()> {
    let zdt1 = date(2001, 11, 18).at(8, 30, 0, 0).in_tz("America/New_York")?;
    let zdt2 = date(2024, 7, 11).at(22, 38, 0, 0).in_tz("America/New_York")?;

    let round_options = ZonedDifference::new(&zdt2)
        .largest(Unit::Month)
        .smallest(Unit::Day)
        .mode(RoundMode::HalfExpand);
    let span = zdt1.until(round_options)?;
    assert_eq!(span, 271.months().days(24).fieldwise());

    Ok(())
}
```

### Jiff supports getting the `nth` weekday from the current date

```rust
use jiff::civil::{date, Weekday};

fn main() -> anyhow::Result<()> {
    let zdt = date(2024, 7, 11).at(22, 59, 0, 0).in_tz("America/New_York")?;
    assert_eq!(zdt.weekday(), Weekday::Thursday);

    let next_tuesday = zdt.nth_weekday(1, Weekday::Tuesday)?;
    assert_eq!(
        next_tuesday.to_string(),
        "2024-07-16T22:59:00-04:00[America/New_York]",
    );

    Ok(())
}
```

Chrono does have `NaiveDate::from_weekday_of_month_opt`, but it only counts
the number of weekdays for a particular month. (The Jiff equivalent is
`nth_weekday_of_month`.) Moreover, Chrono's method is only available on naive
dates and not zone aware datetimes.

### Jiff supports detecting time zone offset conflicts

One of the problems with storing datetimes in the future is that time
zone rules can change. For example, if you stored the zone aware datetime
`2020-01-15T12:00-02[America/Sao_Paulo]` in 2018, then it would be considered
to be in daylight saving time with an offset of `-2`. However, in 2019,
daylight saving time was abolished in this time zone, which renders the
datetime invalid because its offset *should* be `-3`.

Jiff can detect these sorts of conflicts and will actually return a parse error
by default. We exemplify this by creating and serializing a zoned datetime from
an old copy of the Time Zone Database, and then try to parse it back using our
system's current copy of the Time Zone Database. (This also demonstrate's Jiff
support for using multiple copies of the Time Zone Database simultaneously.
But the main point here is to simulate the process of "serialize datetime,
time zone rules change, deserialize datetime.")

```rust,no_run
use jiff::{fmt::temporal::DateTimeParser, tz::{self, TimeZoneDatabase}};

// We use a custom parser with a default configuration because we need
// to ask the parser to use a different time zone database than the
// default. This can't be done via the nice `"...".parse()` API one
// would typically use.
static PARSER: DateTimeParser = DateTimeParser::new();

fn main() -> anyhow::Result<()> {
    // Open a version of tzdb from before Brazil announced its abolition
    // of daylight saving time.
    let tzdb2018 = TimeZoneDatabase::from_dir("path/to/tzdb-2018b")?;
    // Open the system tzdb.
    let tzdb = tz::db();

    // Parse the same datetime string with the same parser, but using two
    // different versions of tzdb.
    let dt = "2020-01-15T12:00[America/Sao_Paulo]";
    let zdt2018 = PARSER.parse_zoned_with(&tzdb2018, dt)?;
    let zdt = PARSER.parse_zoned_with(tzdb, dt)?;

    // Before DST was abolished, 2020-01-15 was in DST, which corresponded
    // to UTC offset -02. Since DST rules applied to datetimes in the
    // future, the 2018 version of tzdb would lead one to interpret
    // 2020-01-15 as being in DST.
    assert_eq!(zdt2018.offset(), tz::offset(-2));
    // But DST was abolished in 2019, which means that 2020-01-15 was no
    // no longer in DST. So after a tzdb update, the same datetime as above
    // now has a different offset.
    assert_eq!(zdt.offset(), tz::offset(-3));

    // So if you try to parse a datetime serialized from an older copy of
    // tzdb with a new copy of tzdb, you'll get an error under the default
    // configuration because of `OffsetConflict::Reject`. This would succeed if
    // you parsed it using tzdb2018!
    assert!(PARSER.parse_zoned_with(tzdb, zdt2018.to_string()).is_err());

    Ok(())
}
```

With Chrono, this sort of checking isn't possible in the first place because
it doesn't support an interchange format that includes the IANA time zone
identifier.

### Jiff supports adding durations with calendar units

Since `Span` is Jiff's single duration type that combines calendar and clock
units, one can freely add them together. The only requirement is that if a span
has calendar units, you need to provide a reference date. (Because 1 month from
April 1 is shorter than 1 month from May 1.)

```rust
use jiff::{civil::date, ToSpan};

fn main() -> anyhow::Result<()> {
    let span1 = 2.years().months(4).days(25).hours(23);
    let span2 = 3.hours();
    let span3 = span1.checked_add((span2, date(2024, 1, 1)))?;
    assert_eq!(span3.fieldwise(), 2.years().months(4).days(26).hours(2));

    Ok(())
}
```

While Chrono has types like `Months` and `Days`, there's no way to combine
them into one, and Chrono does not provide operations on both at the same time.

### Jiff supports zone-aware re-balancing of durations

If you have a span of `1.day()` and want to convert it to hours, then that
calculation depends on how long the day is. If you provide a civil date as
a relative reference point, then Jiff assumes the day is always 24 hours long:

```rust
use jiff::{civil, SpanRound, ToSpan, Unit};

fn main() -> anyhow::Result<()> {
    let relative = civil::date(2024, 4, 1);
    let span1 = 1.day();
    let span2 = span1.round(
        SpanRound::new().largest(Unit::Hour).relative(relative),
    )?;
    assert_eq!(span2, 24.hours().fieldwise());

    Ok(())
}
```

But if a reference date is provided with a time zone, then the re-balancing is
DST safe:

```rust
use jiff::{civil::date, SpanRound, ToSpan, Unit};

fn main() -> anyhow::Result<()> {
    // In the case of a gap (typically transitioning in DST):
    let zdt = date(2024, 3, 9).at(21, 0, 0, 0).in_tz("America/New_York")?;
    let span1 = 1.day();
    let span2 = span1.round(
        SpanRound::new().largest(Unit::Hour).relative(&zdt)
    )?;
    assert_eq!(span2, 23.hours().fieldwise());

    // In the case of a fold (typically transitioning out of DST):
    let zdt = date(2024, 11, 2).at(21, 0, 0, 0).in_tz("America/New_York")?;
    let span1 = 1.day();
    let span2 = span1.round(
        SpanRound::new().largest(Unit::Hour).relative(&zdt)
    )?;
    assert_eq!(span2, 25.hours().fieldwise());

    Ok(())
}
```

### Jiff is generally faster than Chrono

There are some cases where Chrono is faster than Jiff, but Jiff should
generally be competitive for equivalent operations. It's generally not
possible for Chrono or Jiff to always be faster than the other, since
they each use different representations for fundamental types. This in turn
makes some operations faster and others slower, depending on what you're
trying to do.

```text
$ cd bench
$ cargo bench -- --save-baseline base
[.. snip ..]
$ critcmp base -g '(.*)/(?:jiff|chrono)$'
group                                                           base//chrono                         base//jiff
-----                                                           ------------                         ----------
civil_datetime/add_days/diffyear/duration                       1.00      8.3±0.07ns        ? ?/sec    1.77     14.6±0.29ns        ? ?/sec
civil_datetime/add_days/sameyear/duration                       1.00      6.4±0.07ns        ? ?/sec    2.29     14.6±0.23ns        ? ?/sec
civil_datetime/to_datetime_static/bundled                       1.00     22.3±0.31ns        ? ?/sec
civil_datetime/to_datetime_static/zoneinfo                      1.12     18.2±0.33ns        ? ?/sec    1.00     16.2±0.17ns        ? ?/sec
civil_datetime/to_timestamp_tzdb_lookup/bundled                 1.00     32.0±0.28ns        ? ?/sec
civil_datetime/to_timestamp_tzdb_lookup/zoneinfo                52.41     2.1±0.01µs        ? ?/sec    1.00     40.7±0.17ns        ? ?/sec
date/add_days/diffyear/duration                                 1.00      5.9±0.04ns        ? ?/sec    1.11      6.5±0.04ns        ? ?/sec
date/add_days/sameyear/duration                                 1.00      2.1±0.03ns        ? ?/sec    3.08      6.5±0.05ns        ? ?/sec
date/difference_days/duration                                   1.23      3.4±0.03ns        ? ?/sec    1.00      2.8±0.02ns        ? ?/sec
date/tomorrow/diff-month                                        1.00      0.4±0.00ns        ? ?/sec    3.24      1.3±0.01ns        ? ?/sec
date/tomorrow/diff-year                                         1.24      1.8±0.02ns        ? ?/sec    1.00      1.4±0.01ns        ? ?/sec
date/tomorrow/same-month                                        1.00      0.4±0.02ns        ? ?/sec    1.93      0.8±0.01ns        ? ?/sec
date/yesterday/diff-month                                       1.00      0.4±0.00ns        ? ?/sec    3.31      1.3±0.01ns        ? ?/sec
date/yesterday/diff-year                                        1.96      2.1±0.02ns        ? ?/sec    1.00      1.1±0.01ns        ? ?/sec
date/yesterday/same-month                                       1.00      0.4±0.00ns        ? ?/sec    1.75      0.7±0.01ns        ? ?/sec
parse/civil_datetime                                            3.13     73.6±0.69ns        ? ?/sec    1.00     23.5±0.17ns        ? ?/sec
parse/rfc2822                                                   2.41     62.9±0.35ns        ? ?/sec    1.00     26.1±0.30ns        ? ?/sec
parse/strptime/oneshot                                          2.93    172.8±3.46ns        ? ?/sec    1.00     59.0±0.94ns        ? ?/sec
parse/strptime/prebuilt                                         1.00     91.0±1.25ns        ? ?/sec
print/civil_datetime                                            3.08    155.6±3.81ns        ? ?/sec    1.00     50.5±0.18ns        ? ?/sec
timestamp/add_time_secs/duration                                2.14      5.8±0.04ns        ? ?/sec    1.00      2.7±0.02ns        ? ?/sec
timestamp/add_time_subsec/duration                              1.84      5.8±0.04ns        ? ?/sec    1.00      3.1±0.05ns        ? ?/sec
timestamp/every_hour_in_week/byhand                             15.83  1654.8±2.72ns        ? ?/sec    1.00    104.5±1.31ns        ? ?/sec
timestamp/to_civil_datetime_offset_conversion                   1.61      7.1±0.04ns        ? ?/sec    1.00      4.4±0.04ns        ? ?/sec
timestamp/to_civil_datetime_static/America-New-York/bundled     1.00     21.3±0.12ns        ? ?/sec
timestamp/to_civil_datetime_static/America-New-York/zoneinfo    1.15     20.5±0.18ns        ? ?/sec    1.00     17.7±0.15ns        ? ?/sec
timestamp/to_civil_datetime_static/Asia-Shanghai/bundled        1.00     20.8±0.21ns        ? ?/sec
timestamp/to_civil_datetime_static/Asia-Shanghai/zoneinfo       2.60     18.7±0.09ns        ? ?/sec    1.00      7.2±0.05ns        ? ?/sec
zoned/fixed_offset_add_time/duration                            1.00      6.0±0.03ns        ? ?/sec    3.50     20.9±0.11ns        ? ?/sec
zoned/fixed_offset_to_civil_datetime                            5.92      5.4±0.01ns        ? ?/sec    1.00      0.9±0.02ns        ? ?/sec
zoned/fixed_offset_to_timestamp                                 3.17      1.2±0.01ns        ? ?/sec    1.00      0.4±0.01ns        ? ?/sec
```

Questions about benchmarks are
welcome in
[Discussions on GitHub](https://github.com/BurntSushi/jiff/discussions).

## [`time`](https://docs.rs/time) (v0.3.36)

`time` is a Rust datetime library that provides a time zone offset aware
datetime type.

For the following comparisons, a `Cargo.toml` with the following dependencies
should be able to run any of the programs in this section:

```toml
anyhow = "1.0.81"
jiff = { version = "0.2.0", features = ["serde"] }
time = { version = "0.3.36", features = ["local-offset", "macros", "parsing"] }
```

### Time zone database integration

Like `chrono`, the `time` crate does not come with any out of the box
functionality for reading your system's copy of the Time Zone Database. Unlike
Chrono, however, `time` does not have any way to use the Time Zone Database at
all. That is, there is nothing like `chrono-tz` or `tzfile` for `time`, and
`time` does not provide the extension points necessary in its API for such
a thing to exist. (The `chrono-tz` and `tzfile` crates work by implementing
Chrono's `TimeZone` trait.)

The main thing `time` supports is a concept of "local" time. In particular, it
is limited to determining your system's default time zone offset, but nothing
more. That is, it doesn't support DST safe arithmetic:

```rust
use anyhow::Context;
use time::{ext::NumericalDuration, macros::datetime, Duration};

fn main() -> anyhow::Result<()> {
    // We create a fixed datetime for testing purposes,
    // but it's the same sort of value we would get back
    // from `OffsetDateTime::now_local()`.
    let dt1 = datetime!(2024-03-10 01:30:00 -05:00);
    let dt2 = dt1.checked_add(1.hours())
      .context("datetime arithmetic failed")?;
    // The 2 o'clock hour didn't exist on 2024-03-10
    // in New York.
    assert_eq!(dt2.to_string(), "2024-03-10 2:30:00.0 -05:00:00");

    Ok(())
}
```

`time`, in its present design, is fundamentally incapable of doing daylight
saving time safe arithmetic because its `OffsetDateTime` type doesn't know
anything about the time zone rules. Compare this with Jiff, which lets you not
only create a datetime with an offset, but with a _time zone_:

```rust
use jiff::{civil::date, ToSpan};

fn main() -> anyhow::Result<()> {
    let zdt1 = date(2024, 3, 10).at(1, 30, 0, 0).in_tz("America/New_York")?;
    let zdt2 = zdt1.checked_add(1.hour())?;
    assert_eq!(zdt2.to_string(), "2024-03-10T03:30:00-04:00[America/New_York]");

    Ok(())
}
```

In my comparison with Chrono I went through a lot of examples involving
time zones. I did this because Chrono supports DST safe arithmetic generally,
but with a lot of nuanced differences from what Jiff supports. Conversely,
`time` doesn't really support time zones at all. (The main exception is that
`time` can return the system configured offset by virtue of platform APIs like
`libc`. But time zone support stops there.) So at this time, in this document,
we won't belabor the point.

### Jiff allows getting the current time safely from multiple threads

```rust
use jiff::Zoned;

fn main() -> anyhow::Result<()> {
    let handle = std::thread::spawn(|| {
        println!("{}", Zoned::now());
    });
    handle.join().unwrap();

    Ok(())
}
```

The output on my system of the above program is:

```text
2024-07-12T15:02:15.92054241-04:00[America/New_York]
```

Conversely, this program using the `time` crate:

```rust,no_run
use time::OffsetDateTime;

fn main() -> anyhow::Result<()> {
    let handle = std::thread::spawn(|| {
        println!("{}", OffsetDateTime::now_local().unwrap());
    });
    handle.join().unwrap();

    Ok(())
}
```

Has this output:

```text
thread '<unnamed>' panicked at main.rs:7:52:
called `Result::unwrap()` on an `Err` value: IndeterminateOffset
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
thread 'main' panicked at main.rs:9:19:
called `Result::unwrap()` on an `Err` value: Any { .. }
```

The reason for this is that `time` uses `libc` APIs for querying the local
time. These `libc` APIs may access the environment in a way that is not
synchronized with Rust's standard library, which leads to a path where safe
Rust code can be written to cause undefined behavior. `time` mitigates this
by checking how many threads are active. If it's a value other than `1`, then
`now_local()` fails.

Jiff avoids this by avoiding `libc`. Jiff does still read environment
variables, but only does so through Rust's standard library `std::env` module.
This makes Jiff's access to the environment sound.

The `time` crate does provide a way to change this behavior by
explicitly opting into the possibility of undefined behavior via
`time::util::local_offset::set_soundness`. Aside from that, it is likely that
this is a temporary state for `time` until it either implements the `libc`
functionality it needs by itself, or until [`std::env::set_var`] is marked
`unsafe`. (Which will likely happen in Rust 2024.)

[`std::env::set_var`]: https://doc.rust-lang.org/std/env/fn.set_var.html

### `time` supports its own custom format description

```rust
use time::{macros::format_description, OffsetDateTime};

fn main() -> anyhow::Result<()> {
    let format = format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second] \
         [offset_hour sign:mandatory]:[offset_minute]:[offset_second]"
    );
    let odt = OffsetDateTime::parse("2024-07-11 22:49:00 -04:00:00", &format)?;
    assert_eq!(odt.to_string(), "2024-07-11 22:49:00.0 -04:00:00");

    Ok(())
}
```

Jiff does support a `strptime`/`strftime` style API via the
`jiff::fmt::strtime` module.

### Jiff supports rounding datetimes

We use a `Zoned` with a `TimeZone` that has a fixed offset. This is same as
`time`'s `OffsetDateTime` type:

```rust
use jiff::{civil::date, tz::{self, TimeZone}, Unit, Zoned};

fn main() -> anyhow::Result<()> {
    let tz = TimeZone::fixed(tz::offset(-4));
    let zdt1 = date(2024, 7, 11).at(16, 46, 0, 0).to_zoned(tz)?;
    let zdt2 = zdt1.round(Unit::Hour)?;
    assert_eq!(zdt2.to_string(), "2024-07-11T17:00:00-04:00[-04:00]");

    Ok(())
}
```

Note though that because Jiff has support for time zones, you generally
shouldn't need to (and shouldn't _want_ to) use fixed offset datetimes. It's
because they don't take time zone rules into account and thus do not provide
DST safe arithmetic. Instead, the code above should be written like this
(unless you have a very specific reason to do otherwise):

```rust
use jiff::{civil::date, Unit, Zoned};

fn main() -> anyhow::Result<()> {
    // Can also use `.to_zoned(TimeZone::system())` to use your system's
    // default time zone.
    let zdt1 = date(2024, 7, 11).at(16, 46, 0, 0).in_tz("America/New_York")?;
    let zdt2 = zdt1.round(Unit::Hour)?;
    assert_eq!(zdt2.to_string(), "2024-07-11T17:00:00-04:00[America/New_York]");

    Ok(())
}
```

From here on, we won't use fixed offset datetimes in order to avoid encouraging
their use.

The `time` crate has no rounding APIs.

### Jiff supports rounding durations

In Jiff, one can round the duration computed between two datetimes

```rust
use jiff::{civil::date, RoundMode, ToSpan, Unit, ZonedDifference};

fn main() -> anyhow::Result<()> {
    let zdt1 = date(2001, 11, 18).at(8, 30, 0, 0).in_tz("America/New_York")?;
    let zdt2 = date(2024, 7, 11).at(22, 38, 0, 0).in_tz("America/New_York")?;

    let round_options = ZonedDifference::new(&zdt2)
        .largest(Unit::Year)
        .smallest(Unit::Day)
        .mode(RoundMode::HalfExpand);
    let span = zdt1.until(round_options)?;
    assert_eq!(span, 22.years().months(7).days(24).fieldwise());

    Ok(())
}
```

The `time` crate has no rounding APIs.

### Jiff provides support for calendar arithmetic

With Jiff, you can add durations with calendar units:

```rust
use jiff::{civil::date, ToSpan, Unit};

fn main() -> anyhow::Result<()> {
    let zdt1 = date(2024, 7, 11).at(21, 0, 0, 0).in_tz("America/New_York")?;
    let zdt2 = zdt1.checked_add(2.years().months(6).days(1))?;
    assert_eq!(zdt2.to_string(), "2027-01-12T21:00:00-05:00[America/New_York]");

    Ok(())
}
```

The `time` crate does provide a way to construct a `Duration` from units of
days via `Duration::days`, but this of course requires assuming that all days
are 24 hours long. And `time` does not support adding years or months.

### Jiff supports conveniently re-balancing durations

Aside from calendar arithmetic, Jiff also supports re-balancing durations
based on what you want the largest unit to be:

```rust
use jiff::{SpanRound, ToSpan, Unit};

fn main() -> anyhow::Result<()> {
    // Balance down to seconds.
    let span1 = 4.hours().minutes(36).seconds(59);
    let span2 = span1.round(SpanRound::new().largest(Unit::Second))?;
    assert_eq!(span2, 16_619.seconds().fieldwise());

    // Now go back by balancing up to hours.
    let span1 = 16_619.seconds();
    let span2 = span1.round(SpanRound::new().largest(Unit::Hour))?;
    assert_eq!(span2, 4.hours().minutes(36).seconds(59).fieldwise());

    Ok(())
}
```

The `time` crate's `Duration` type can go from bigger units down to smaller
units easily enough:

```rust
use time::{ext::NumericalDuration, Duration};

fn main() -> anyhow::Result<()> {
    let span = 4.hours() + 36.minutes() + 59.seconds();
    assert_eq!(span.whole_seconds(), 16_619);
    Ok(())
}
```

But going from smaller units back up to larger units is difficult:

```rust
use time::{ext::NumericalDuration, Duration};

fn main() -> anyhow::Result<()> {
    let span = 16_619.seconds();
    assert_eq!(span.whole_hours(), 4);
    assert_eq!(span.whole_minutes(), 276);
    assert_eq!(span.whole_seconds(), 16_619);

    Ok(())
}
```

Notice that the accessors just report how many whole units the span is. You
can't get the span broken down into smaller units. To achieve that, you need to
do the arithmetic yourself:

```rust
use time::{convert::{Hour, Minute, Second}, ext::NumericalDuration, Duration};

fn main() -> anyhow::Result<()> {
    let mut span = 16_619.seconds();
    assert_eq!(span.whole_hours(), 4);
    assert_eq!(span.whole_minutes() % Minute::per(Hour) as i64, 36);
    assert_eq!(span.whole_seconds() % Second::per(Minute) as i64, 59);

    Ok(())
}
```

### Jiff is generally faster than `time`

Unlike Chrono, at least for Jiff's benchmarks, there are few cases where `time`
is meaningfully faster than Jiff.

The following results were collected with `time 0.3.38`.

```text
$ cd bench
$ cargo bench -- --save-baseline base
[.. snip ..]
$ critcmp base -g '(.*)/(?:jiff|time)$'
group                                                           update//time                           update//jiff
-----                                                           ------------                           ------------
civil_datetime/add_days/diffyear/duration                       1.19     16.2±0.10ns        ? ?/sec    1.00     13.6±0.12ns        ? ?/sec
civil_datetime/add_days/sameyear/duration                       1.22     16.6±0.12ns        ? ?/sec    1.00     13.6±0.15ns        ? ?/sec
date/add_days/diffyear/duration                                 1.11      7.3±0.07ns        ? ?/sec    1.00      6.6±0.07ns        ? ?/sec
date/add_days/one/duration                                      1.34      7.3±0.05ns        ? ?/sec    1.00      5.4±0.04ns        ? ?/sec
date/add_days/sameyear/duration                                 1.16      7.6±0.08ns        ? ?/sec    1.00      6.6±0.07ns        ? ?/sec
date/days_in_month/leap/feb                                     13.11     5.1±0.16ns        ? ?/sec    1.00      0.4±0.01ns        ? ?/sec
date/days_in_month/leap/nofeb                                   9.11      3.5±0.16ns        ? ?/sec    1.00      0.4±0.00ns        ? ?/sec
date/days_in_month/noleap/feb                                   12.66     4.9±0.17ns        ? ?/sec    1.00      0.4±0.01ns        ? ?/sec
date/days_in_month/noleap/nofeb                                 8.40      3.3±0.10ns        ? ?/sec    1.00      0.4±0.00ns        ? ?/sec
date/difference_days/duration                                   1.42      4.1±0.05ns        ? ?/sec    1.00      2.9±0.03ns        ? ?/sec
date/tomorrow/diff-month                                        1.00      0.4±0.01ns        ? ?/sec    3.17      1.3±0.01ns        ? ?/sec
date/tomorrow/diff-year                                         1.00      0.5±0.01ns        ? ?/sec    2.99      1.4±0.02ns        ? ?/sec
date/tomorrow/same-month                                        1.00      0.4±0.01ns        ? ?/sec    2.00      0.8±0.01ns        ? ?/sec
date/yesterday/diff-month                                       1.00      0.3±0.01ns        ? ?/sec    4.08      1.3±0.01ns        ? ?/sec
date/yesterday/diff-year                                        1.00      0.7±0.01ns        ? ?/sec    1.57      1.1±0.01ns        ? ?/sec
date/yesterday/same-month                                       1.00      0.3±0.01ns        ? ?/sec    2.23      0.7±0.01ns        ? ?/sec
parse/civil_datetime                                            1.25     31.4±0.33ns        ? ?/sec    1.00     25.0±0.08ns        ? ?/sec
parse/rfc2822                                                   3.10     80.9±1.19ns        ? ?/sec    1.00     26.1±0.41ns        ? ?/sec
parse/strptime/oneshot                                                                                 1.00     59.9±1.03ns        ? ?/sec
parse/strptime/prebuilt                                         1.00    112.8±1.06ns        ? ?/sec
print/civil_datetime                                            1.00     37.6±0.49ns        ? ?/sec    1.48     55.7±0.31ns        ? ?/sec
timestamp/add_time_secs/duration                                7.05     19.1±0.20ns        ? ?/sec    1.00      2.7±0.03ns        ? ?/sec
timestamp/add_time_subsec/duration                              6.19     19.1±0.15ns        ? ?/sec    1.00      3.1±0.03ns        ? ?/sec
timestamp/every_hour_in_week/byhand                             32.25     3.4±0.03µs        ? ?/sec    1.00    105.0±0.83ns        ? ?/sec
timestamp/to_civil_datetime_offset_conversion                   3.14     14.6±0.16ns        ? ?/sec    1.00      4.7±0.05ns        ? ?/sec
timestamp/to_civil_datetime_offset_holistic                     4.02     18.7±0.07ns        ? ?/sec    1.00      4.7±0.04ns        ? ?/sec
zoned/fixed_offset_add_time/duration                            2.41     23.3±0.26ns        ? ?/sec    1.00      9.7±0.07ns        ? ?/sec
zoned/fixed_offset_to_civil_datetime                            1.00      0.8±0.00ns        ? ?/sec    1.26      1.0±0.03ns        ? ?/sec
zoned/fixed_offset_to_timestamp                                 6.92      2.7±0.02ns        ? ?/sec    1.00      0.4±0.00ns        ? ?/sec
```

Questions about benchmarks are
welcome in
[Discussions on GitHub](https://github.com/BurntSushi/jiff/discussions).

## [`hifitime`](https://docs.rs/hifitime) (v3.9.0)

`hifitime` is a datetime library with a focus on engineering and scientific
calculations where general relativity and time dilation matter. It supports
conversion between many different time scales: TAI, Terrestrial Time, UTC, GPST
and more. It also supports leap seconds.

For the following comparisons, a `Cargo.toml` with the following dependencies
should be able to run any of the programs in this section:

```toml
anyhow = "1.0.81"
hifitime = "3.9.0"
jiff = { version = "0.2.0", features = ["serde"] }
```

### Time zone database integration

Like the `time` crate, `hifitime` does not support time zones and does not have
any integration with the Time Zone Database. `hifitime` doesn't have any
equivalent to `OffsetDateTime` like in `time` either. The only datetime type
that `hifitime` has is `Epoch`, and it is an absolute time. While you can
convert between it and civil time (assuming civil time is in UTC), there is no
data type in `hifitime` for representing civil time.

### `hifitime` supports leap seconds

In particular, when computing a duration from two `Epoch` values that spans
a positive leap second (a second gets repeated), `hifitime` will correctly
report the accurate duration:

```rust
use hifitime::{Duration, Epoch};

fn main() -> anyhow::Result<()> {
    let e1: Epoch = "2015-06-30T23:00:00 UTC".parse()?;
    let e2: Epoch = "2015-07-01T00:00:00 UTC".parse()?;
    let duration = e2 - e1;
    assert_eq!(duration, Duration::from_seconds(3_601.0));

    Ok(())
}
```

Jiff, however, [does not support leap seconds][jiff-leap-seconds]:

```rust
use jiff::{Timestamp, ToSpan};

fn main() -> anyhow::Result<()> {
    let ts1: Timestamp = "2015-06-30T23:00:00Z".parse()?;
    let ts2: Timestamp = "2015-07-01T00:00:00Z".parse()?;
    let span = ts2 - ts1;
    assert_eq!(span, 3_600.seconds().fieldwise());

    Ok(())
}
```

So in this case, Jiff reports `3,600` seconds as the duration, but the _actual_
duration was `3,601` seconds, as reported by `hifitime`.

[jiff-leap-seconds]: https://github.com/BurntSushi/jiff/issues/7

### Jiff makes checked or saturating arithmetic explicit

For Jiff, whether you want to saturate or not is an explicit part of the API.
And implementations of the `Add` operator will panic on overflow:

```rust
use jiff::{Timestamp, ToSpan};

fn main() -> anyhow::Result<()> {
    let ts = Timestamp::MAX;
    assert!(ts.checked_add(1.day()).is_err());
    assert_eq!(ts.saturating_add(1.hour())?, ts);

    Ok(())
}
```

In contrast, `hifitime` appears to use saturating arithmetic everywhere (I've
not been able to find this behavior documented though, so I'm not clear on what
the intended semantics are):

```rust
use hifitime::{Duration, Epoch};

fn main() -> anyhow::Result<()> {
    let e1 = Epoch::from_unix_seconds(f64::MAX);
    let e2 = e1 + Duration::from_days(1.0);
    assert_eq!(e1, e2);

    Ok(())
}
```


## [`icu`](https://docs.rs/icu) (v1.5.0)

The ICU4X project fulfils a slightly different need than `jiff`. Its main
features are calendrical calculations (`icu::calendar`), supporting conversions
between different calendar systems such as Gregorian, Buddhist, Islamic,
Japanese, etc., as well as localized datetime formatting (`icu::datetime`).

It does not perform datetime or time-zone arithmetic, and does not have a
timestamp or duration type.

`icu` can be used to complement `jiff` when localized date formatting or
calendar conversions are required. To facilitate this, the
[`jiff-icu`](https://docs.rs/jiff-icu) crate makes conversions between Jiff
and ICU4X data types seamless. For example, to do localization starting from
a Jiff data type:

```text
use icu::{
    calendar::{japanese::Japanese, DateTime},
    datetime::TypedDateTimeFormatter,
    locid::locale,
};
use jiff::Timestamp;
use jiff_icu::ConvertFrom as _;

fn main() -> anyhow::Result<()> {
    let ts: Timestamp = "2024-09-10T23:37:20Z".parse()?;
    let zoned = ts.in_tz("Asia/Tokyo")?;

    // Create ICU datetime.
    let datetime = DateTime::convert_from(zoned.datetime());

    // Convert to Japanese calendar.
    let japanese_datetime = DateTime::new_from_iso(datetime, Japanese::new());

    // Format for the en-GB locale.
    let formatter = TypedDateTimeFormatter::try_new(
        &locale!("en-GB").into(),
        Default::default(),
    )?;

    // Assert that we get the expected result.
    assert_eq!(
        formatter.format(&japanese_datetime).to_string(),
        "Sept 11, 6 Reiwa, 08:37:20",
    );

    Ok(())
}
```

The above example requires the following dependency specifications:

```toml
anyhow = "1.0.81"
icu = { version = "1.5.0", features = ["std"] }
jiff = { version = "0.1.0", features = ["serde"] }
jiff-icu = { version = "0.1.0" }
```
