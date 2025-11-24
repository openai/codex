# The API design rationale for Jiff

This document discusses some of the design decisions that led to Jiff's API.
The purpose of writing this document is to help folks understand _why_ Jiff's
API is the way it is, above and beyond "Jiff did it this way to match
[Temporal]."

This document is written as an FAQ, although it is restricted in scope to the
API design of Jiff. This isn't a FAQ for questions like, "How do I add 1 day
to a zoned datetime?"

Unlike "[Comparison with other Rust datetime crates][comparison]," this
document is _opinionated_. That is, some value judgments are expressed that are
opinion based, and on which reasonable people may disagree.


## Why name the library "jiff"?

I wanted something short and related to time for the library name. Most of the
"obvious" names are taken.

I thought a lot about phrases or words that had some connection to time. There
are more than you think. One phrase I heard a lot as a kid who grew up in New
England was, "I'll be back in a jiff" or "I'll be back in a jiffy." The meaning
of that phrase was, roughly, "I'll be back very soon." So "jiff" refers to some
"short span of time." Since "jiff" was shorter than "jiffy," that's what I went
with.

Jiff is pronounced like "gif" with a soft "g," as in "gem."


## Why build another datetime library?

At the time of writing, there are four existing prominent datetime libraries
for Rust: [`chrono`], [`time`], [`hifitime`] and [`icu`]. "[Comparison with
other Rust datetime crates][comparison]" goes over the "facts of comparison"
between the crates (mostly for just `chrono` and `time`), but it intentionally
leaves out value judgments. To answer this question, I have to introduce value
judgments and my opinions on existing libraries. **These are opinions on which
reasonable people can disagree.** Moreover, since these are opinions meant to
justify an alternative, these opinions tend to be oriented on the faults (as
this author sees them) in existing crates as opposed to their benefits.

Broadly speaking, my view on Rust datetime libraries was that they had reached
a local maximum, and there didn't seem to be much movement toward breaking out
of that local maximum and getting to something that was categorically better.
Much of my thoughts revolve around not just the functionality provided, but
also the API design of these crates. Thus, I perceived a gap in the ecosystem
that I felt like I could fill.

I'll share my brief thoughts on each crate. I'll cover "why not contribute to
an existing library and make it better" in the next question.

### `chrono`

In my view, Chrono is the closest to Jiff in terms of the functionality it
provides. It has some support for DST safe arithmetic for example, and even
some support for doing calendar math. But its support is incomplete. While one
can add units of days to Chrono datetimes, Chrono lacks the ability to do math
on multiple calendar units at the same time. Moreover, Chrono cannot _produce_
calendar durations between two time zone aware datetimes.

Moreover, Chrono's integration with the [IANA Time Zone Database] is somewhat
spotty. Support for it isn't included in the `chrono` crate itself, but
is instead something you need to opt into with additional crates, such
as [`chrono-tz`] or [`tzfile`]. And each crate comes with its own set of
trade-offs. `chrono-tz` embeds the entire database into your binary and makes
the time zones available at compile time, while `tzfile` reads the database
from your system's copy of the database (i.e., `/usr/share/zoneinfo` on Unix).
In my view, this creates a difficult situation for non-expert users of Chrono
where the "right" choice isn't obvious. In my opinion, the _default_ should be
to read time zone transition data from the system's copy of the database on
disk, and only bundle the data into the binary as a last resort when a copy of
the database isn't reliably available (like on Windows). The main reason for
this is that the database is frequently updated (a few times each year) since
the rules for time zone transitions can change. If that time zone data is
embedded into your binary, then you need to 1) wait for `chrono-tz` to update
their data and 2) re-compile your application and ship it out to users.

In contrast, Jiff _abstracts_ the method of time zone data discovery. That
is, there are no API differences between "read time zone data from the system
database" and "read embedded time zone data." Of course, Jiff provides options
to choose which method to use (like forcing bundling on Unix), but all of this
is transparent to users of Jiff itself. That is, _Jiff tries to do the right
thing by default_. There's no need to go off and find a different crate to
handle time zone data for you.

While on the topic of time zones, Chrono also has no support for serializing
IANA time zone identifiers. This implies that if you have a time zone aware
datetime in Chrono (whether by `chrono-tz` or `tzfile`), then you can't
losslessly serialize and deserialize it. Namely, serialization will lose
the time zone the datetime is associated with, and instead only include
the offset. Then when it's deserialized, you're left with an offset-only
datetime that won't, for example, provide DST safe arithmetic based on the
original time zone. In contrast, Jiff follows [RFC 9557] to support embedding
IANA time zone identifiers in the serialized representation. For example,
`2024-07-21T17:11-04[America/New_York]`.

There are a variety of other things that Jiff supports which Chrono does
not, but that's covered in [the comparison section between Jiff and
Chrono][comparison-chrono].

As for API design, I found Chrono's API to be overengineered and difficult to
deal with. This is a frustratingly vague complaint, but here are some things
that I believe contribute to that opinion:

* I've used Chrono for various things, and I found it very difficult to figure
out from its API what the right operations to use were.
* Chrono has been steadily deprecating huge portions of its API in favor of
fallible routines. I find the resulting documentation difficult to read and the
naming to be very clunky.
* The fallible routines return a mixture of `Option<T>` and `Result<T, E>`.
This is frustrating in my experience because the `Option<T>` _usually_ needs to
be converted to a human readable error message, and I think the library should
make use of its contextual information to do this for you. Indeed, Jiff rarely
returns `Option<T>` and instead returns `Result<T, E>` with a contextualized
human readable error message. (Although I'm sure the error messages could use a
lot of improvement.)
* I find the use of generics in Chrono to be overengineered. It has some
key benefits, for example, making the notion of "time zone" an open concept
that can be defined by users of the crate. But in my opinion, this is rarely
needed. The Chrono crate ecosystem makes use of this via the `chrono-tz` and
`tzfile` crates, but Jiff covers both of those use cases (broadly speaking)
automatically.
* Chrono overall puts a large emphasis on "fixed offset" datetimes, but these
are rarely the right abstraction to use. It's possible this is due to the fact
that IANA time zone support is external to Chrono itself. Instead, Jiff just
tries to do the right thing by default, and gives users IANA time zone support
out of the box. Jiff does support fixed offset datetimes as well, but they are
de-emphasized.
* I find generic traits like `Datelike` to also be very confusing because they
split the APIs of types like `NaiveDate` into concrete methods and generic
methods, and there's no obvious rhyme or reason as to how those methods are
split up.
* Chrono's API doesn't offer clean on-ramps from civil ("naive") datetimes to
time zone aware datetimes. For example, Chrono provides `NaiveDate::succ_opt`
to get the next day, but this method isn't available on `NaiveDateTime` or
`DateTime`. Instead, to implement it correctly on `DateTime`, you have to get
the naive date, get the date for tomorrow via `succ_opt`, and then convert
it back to the same `NaiveDateTime` and finally make it time zone aware by
applying the original time zone to it. In contrast, in Jiff, it's just a matter
of calling `Zoned::tomorrow`. Indeed, (almost) any method you can call on
`civil::Date` in Jiff is also available on `civil::DateTime` and likewise for
`Zoned`. This makes transitioning between datetime types very easy. Chrono
almost appears to provide this same experience via traits like `Datelike`,
but doesn't fully commit and, in my opinion, the end result has a feeling of
arbitrariness to it.
* Chrono lacks a standard timestamp type. You can approximate this with a
`DateTime<Utc>`, but it's a more complicated type that includes a full datetime
representation. In contrast, Jiff provides a `Timestamp` type for when you just
want the number of seconds from the Unix epoch. And then integrates it with the
rest of the datetime types in a consistent way.

### `time`

My main gripe with the `time` crate is that it has no [IANA Time Zone Database]
support at all. This means it cannot do DST safe arithmetic. Consequently, it
emphasizes the use of "fixed offset" datetimes in a similar fashion as Chrono,
except `time` does not provide any extension mechanism like a `TimeZone` trait.
In my view, fixed offset datetimes are rarely the right thing to use. In my
opinion, this makes writing correct datetime code with the `time` crate rather
difficult. In contrast, Jiff provides full [IANA Time Zone Database] support,
and it should be very rare to need fixed offset datetimes (although Jiff does
support them via `TimeZone::fixed`).

The `time` crate also, at present, relies on unsound `libc` APIs for
determining the current time zone offset, but makes them sound by requiring (by
default) that `UtcOffset::current_local_offset` is only called when there is
only 1 thread. This is a result of the `libc` APIs accessing the environment
in a way that is unsynchronized with Rust's standard library access to the
environment. This is likely a temporary limitation, but at time of writing,
this state has persisted for quite some time already. In contrast, Jiff detects
the current time zone of the platform on its own without use of `libc`, and
thus sidesteps this issue. (This is also what Chrono does.)

I overall find the API of `time` to be easier to understand than Chrono, likely
because there are fewer generics. But `time` is also supporting a lot less than
Chrono and Jiff (because of the missing time zone support).

As with Chrono, I've done a
[more detailed comparison between Jiff and `time`][comparison-time].

### `hifitime`

`hifitime` is more of a specialized datetime library suited to scientific
applications, and so while Jiff and `hifitime` have overlapping use cases,
`hifitime` fundamentally has a different target demographic than Jiff. As noted
in a [comparison between Jiff and `hifitime`][comparison-hifitime], `hifitime`
doesn't have any time zone support, but it does support conversions between
many different time scales and leap second support. Leap second support, in
this context, means that the durations between two points in time take leap
seconds into account in `hifitime`, but Jiff pretends as if they don't exist.

In terms of building a new datetime library, I felt like `hifitime` wasn't
really targeting the "general purpose datetime library" use case that I felt
`chrono` and `time` were. And so, whether it existed or not didn't really
impact whether another _general purpose_ datetime library should be built.

### `icu`

`icu` is, as I understand it, still under construction with respect to datetime
support. For example, it doesn't have [IANA Time Zone Database] support.
But, it does support locale aware formatting of datetimes and non-Gregorian
calendars.

When I started working on Jiff, I didn't have a good understanding of what the
`icu` crate offers. I still don't really. In part because the API is difficult
for me to understand and in part because I haven't dedicated a ton of time to
studying its API. But either way, I don't think it is currently in a position
to be a general purpose datetime library and I wasn't clear on what its goals
were.

Since I haven't spent a lot of time with `icu`, I didn't have much to say about
it in my [comparison with it and Jiff][comparison-icu].


## Why not contribute to an existing library instead of building a new one?

Given that Rust already has at least two prominent datetime libraries, doesn't
adding another one just make things worse? And why not contribute to an
existing library to make it better instead of starting over from scratch?

I first want to say that I acknowledge that throwing a new crate into the
ecosystem, and adding yet another choice, does actually come with downsides.
There is a cost to having too many choices, and when possible, I do believe it
is better to improve an existing project rather than start a new one.

Improving existing projects can be difficult. Jiff has a different design than
both `chrono` and `time`. Evolving either one of those crates into what Jiff is
would, in my view, require a huge amount of resources. Not just in time, but in
social capital as well. Because it wouldn't be greenfield development done by
one person making all of the design choices, but instead someone from outside
the project trying to convince the maintainers of established projects to move
in a radically different direction. I know what it's like to be on the side of
maintaining an established API for a library with a lot of users. There is a
huge inertial cost to making sweeping API changes.

Moreover, when I started Jiff, I was not a domain expert in datetime libraries
or datetime handling in general. Therefore, my opinion that `chrono` and `time`
_could_ be better would arguably not carry a lot of weight. It was only through
the process of actually building a datetime library did I learn enough to form
nuanced opinions about the status quo. When I started, my opinions were much
more vague (but still strong enough to start this project).

On top of all of this, I was _intrinsically_ motivated to work on this problem.
I found it very interesting, and especially because I perceived there to be a
gap in the ecosystem that I thought I could fill in. I had a vision for what
a datetime library _should_ look like. And it took a lot of iteration to get
from my initial vision to something that works in practice. Doing this on an
existing datetime library with real users would be extremely difficult.

And speaking as someone who has had folks publish _better_ versions of some of
my own crates, I know what it's like to be on the other end of this. Sometimes
you just have to start fresh.


## Are there any published alternative perspectives on Rust datetime libraries?

Here's a list. More may be added in the future:

* [Commentary from the original author of the `chrono` crate.][alt1]

[alt1]: https://github.com/BurntSushi/jiff/issues/63


## Why are there two duration types?

The two duration types provided by Jiff are `Span` and `SignedDuration`. A
`SignedDuration` is effectively identical to a `std::time::Duration`, but it's
signed instead of unsigned. A `Span` is also a duration type, but is likely
different than most other duration types you've used before.

While a `SignedDuration` can be thought of as a single integer corresponding
to the number of nanoseconds between two points in time, a `Span` is a
collection of individual unit values that combine to represent the difference
between two point in time. Stated more concretely, while the spans `2
hours` and `120 minutes` both correspond to the same duration of time, when
represented as a Jiff `Span`, they correspond to two distinct values in
memory. This is something that is fundamentally not expressible by a type
like `SignedDuration`, where `2 hours` and `120 minutes` are completely
indistinguishable.

One of the key advantages of a `Span` is that it can represent units of
non-uniform length. For example, not every month has the same number of days,
but a `Span` can still represent units of months because it tracks the values
of each unit independently. For example, Jiff is smart enough to know that the
difference between `2024-03-01` and `2024-04-01` is the same number of months
as `2024-04-01` and `2024-05-01`, even though the number of days is different:

```rust
use jiff::{civil::date, ToSpan, Unit};

fn main() -> anyhow::Result<()> {
    let date1 = date(2024, 3, 1);
    let date2 = date(2024, 4, 1);
    let date3 = date(2024, 5, 1);

    // When computing durations between `Date` values,
    // the spans default to days.
    assert_eq!(date1.until(date2)?, 31.days().fieldwise());
    assert_eq!(date2.until(date3)?, 30.days().fieldwise());

    // But we can request bigger units!
    assert_eq!(date1.until((Unit::Month, date2))?, 1.month().fieldwise());
    assert_eq!(date2.until((Unit::Month, date3))?, 1.month().fieldwise());

    Ok(())
}
```

While most folks are very in tune with the fact that years and months have
non-uniform length, a less obvious truth is that days themselves also have
non-uniform length in the presence of time zones. For example, `2024-03-10` in
`America/New_York` was only 23 hours long (the region entered daylight saving
time, creating a gap in time), while `2024-11-03` was 25 hours long (the region
left daylight saving time, creating a fold in time). Being unaware of this
corner case leads to folks assuming that "1 day" and "24 hours" are _always_
exactly equivalent. But they aren't. The design of Jiff leans into this and
ensures that so long as you're using `Span` to encode a concept of days and are
doing arithmetic with it on `Zoned` values, then you can never get it wrong.
Jiff will always take time zones into account when dealing with units of days
or bigger.

The design of `Span` comes from [Temporal], which [uses only one duration
type][temporal-one-duration]. From that issue, there are some significant
advantages to using a `Span`. In my own words:

* It more closely lines up with ISO 8601 durations, which themselves combine
calendar and clock units.
* With a `Span`, it is very easy to move between `5 years 2 months` and
the number of hours in that same span.
* Jiff's `Span` type specifically represents each unit as distinct from the
others. In contrast, most absolute duration types (like `std::time::Duration`
and Jiff's own `SignedDuration`), are "just" a 96-bit integer number of
nanoseconds. This means that, for example, `1 hour 30 minutes` is impossible to
differentiate from `90 minutes`. But depending on the use case, you might want
one or the other. Jiff's `Span` design (copied from Temporal) enables users
to express durations in whatever units they want. And this expression can be
manipulated via APIs like `Span::round` in intuitive ways.

A `SignedDuration` is still useful in some respects. For example, when you
need tighter integration with the standard library's `std::time::Duration`
(since a `SignedDuration` is the same, but just signed), or when you need
better performance than what `Span` gives you. In particular, since a `Span`
keeps track of the values for each individual unit, it is a much heavier type
than a `SignedDuration`. It uses up more stack space and also required more
computation to do arithmetic with it.


## Why isn't there a `TimeZone` trait?

First, let's start by explaining what a `TimeZone` is. In Jiff, a `TimeZone`
is a concrete type that cannot be extended by users of Jiff. Instead, users of
Jiff are forced to use one of three different kinds of time zones:

* A "fixed offset" time zone where the civil time for any particular instant
is computed by simply adding or subtracting a fixed number of seconds from UTC.
The `TimeZone::fixed` constructor enables callers to build time zones with any
offset within the limits imposed by Jiff.
* A [POSIX time zone][POSIX TZ], typically set via the `TZ` environment
variable. These are rarely used by end users, but do provide a way to specify
a rule for when daylight saving time transitions occur thoughtout the year.
(But it does not support historical transitions that might not conform to the
current rule.) The `TimeZone::posix` constructor enables callers to build a
`TimeZone` with a POSIX time zone string.
* [TZif formatted data][RFC 8536], usually from the
[IANA Time Zone Database]. This data contains historical time zone transitions
in addition to rules governing the future in the form of POSIX TZ strings.The
`TimeZone::tzif` constructor enables callers to build a `TimeZone` with any
TZif formatted data.

The `jiff::tz::TimeZoneDatabase` automatically looks for TZif formatted
files in your system's copy of the IANA Time Zone Database, usually at
`/usr/share/zoneinfo`. (On Windows, Jiff embeds a copy of the IANA Time Zone
Database into the compiled artifact itself.)

So why isn't `TimeZone` a trait? Well, the above formulation should cover the
_vast majority_ of use cases. And even if that doesn't cover everything, it
is possible for callers to use `TimeZone::tzif` to construct arbitrary time
zones by building their own TZif data. This is a somewhat large hurdle though,
so if this is something that is commonly needed, I'm open to exploring other
options for building custom time zones. For example, perhaps we introduce a way
to describe time zone transitions in Rust code that can then be used to build a
`TimeZone` directly. But, the benefit of TZif is that it is inter-operable and
a standard. There are tools that can build them.

An important thing to note here is that I actually approach questions like
"Why isn't `TimeZone` a trait?" as instead "Why _should_ `TimeZone` be a trait?"
In particular, I personally perceive costs to introducing generics, especially
on a fundamental type in the crate. For example, if `TimeZone` were a trait,
then `Zoned` would not be a concrete type. It would be generic over a type
parameter that implements the `TimeZone` trait. This in turn implies that
anyone _using_ a `Zoned` in their own types or APIs needs to think about the
`TimeZone` trait and likely incorporate it into their own type signatures. This
is because a `TimeZone` trait implies an open system that infects everything
it touches. The complexity isn't contained. But a concrete `TimeZone` type,
like what Jiff has, encapsulates everything there is about time zones.

(Making `Zoned` generic over a type parameter with a _default_ type does
contain the complexity in some cases, but not all. I explored using default
type parameters in Jiff for supporting [leap seconds][github-issue-leap], and
it was overall quite awkward in my opinion.)

The trade off is that we do give up some flexibility. For example, Chrono uses
a `TimeZone` trait design. This enables external crates to provide their own
implementations of the `TimeZone` trait. But the two principle instances of
this occurring, `chrono-tz` and `tzfile`, are both supported by Jiff itself.
With that said, one key advantage of Chrono's design is that it permits its
zone aware datetime type (`DateTime<T>`) to be `Copy` if `T` is `Copy`. It is
somewhat difficult (although not literally impossible) to make a TZif-backed
time zone implement `Copy`, but a _reference_ to it is `Copy` and a reference
to it can still implement Chrono's `TimeZone` trait. (And indeed, this is what
the `tzfile` crate does.) This ultimately leads to more flexibility compared
to Jiff, where its `Zoned` type embeds a `TimeZone` and a `TimeZone` cannot
easily be made `Copy` without giving up something else.

My opinions on the costs of generics tend to overestimate them compared to many
others in my experience, so your mileage may vary on where you land on this
issue. Buy in my opinion, being able to just write `Zoned` as a concrete type
without any generics is a huge win for comprehensibility.


## Why doesn't `TimeZone` implement `Copy`?

When initially setting out to build Jiff, I _really_ wanted the `TimeZone` type
to implement `Copy`. The reason why I wanted it to implement `Copy` is because
I wanted all datetime types to have "plain old data" semantics. That is, I want
callers to think of them as small immutable bits of data that can be freely
copied without worry. This makes APIs a little nicer because you can ask for a
`Zoned` instead of a `&Zoned`, assuming the `Zoned` type is small enough.

But, in order for `Zoned` to be `Copy`, it must be the case that `TimeZone` is
`Copy`. This is because `Zoned` embeds a `TimeZone`. Indeed, this is the reason
for its existence: it couples an instant in time with a particular geographic
region. This makes it possible to expose very ergonomic high level APIs that
don't require the caller to keep passing in a `TimeZone` value repeatedly.

So, how can a `TimeZone` be `Copy`? Well, both fixed offset and POSIX time
zones could be `Copy`. There's no huge challenge in that. (Internally, a POSIX
time zone is not currently `Copy` because there's no reason for it given what
we're about to discuss.) The main challenge is the TZif-backed time zone. TZif
formatted data can contain an arbitrary number of time zone transitions. There
is just no way to avoid some kind of dynamic memory allocation.

Since we need dynamic memory allocation, there is really only one way to
make it `Copy` at this point: introduce some kind of caching mechanism with
a small `Copy` identifier that lets us "look up" the time zone in some kind
of global or thread-local cache. I thought a lot about how I might wire this
together, and I could not come up with a satisfactory design. I believe
garbage collection is the main challenge, but also synchronization overhead
for accessing a time zone is likely also a problem. The nice benefit of the
`TimeZone` type as it exists now is that it's just data. While _getting_ a
`TimeZone` from a `TimeZoneDatabase` might require synchronization and possibly
even I/O if the cache for it was invalidated, a `TimeZone` itself requires no
synchronization whatsoever to use it. It is just an `Arc` internally to make it
pointer-sized.


## Why isn't there a `SystemZoned` type? Or a `OffsetZoned` type?

Some datetime libraries have multiple different zone aware datetime types. Jiff
opts to have just one, and embeds support for the different types of time zones
that most people will ever need into that one type. Jiff does this via the
`TimeZone` type, which can be a fixed offset, a POSIX time zone or TZif-backed
(usually from the [IANA Time Zone Database]).


## Why doesn't Jiff support leap seconds?

The short summary is that the use cases for leap second support are rather
limited, and the effect they have on overall API complexity is quite large.
That is, I believe they would make the API of Jiff more complicated than the
value they bring to the domain.

A standard work-around for _part_ of the leap second problem---and usually the
one people care about---is to use custom TZif data that describes when each of
the leap seconds occurs. In effect, you can treat [TAI] as its own time zone.
This enables callers to compute accurate durations of time that span a leap
second (positive or negative). And indeed, so long as you build that TZif
data, Jiff supports this via `TimeZone::tzif`.

I wrote a lot more [about leap seconds on the issue
tracker][github-issue-leap].


## Why isn't there any integration with `std::time::Duration`?

The initial release of `jiff 0.1` originally left out any integration
points with `std::time::Duration`. Since then, `SignedDuration` has
been added to Jiff. And `TryFrom` trait implementations have been added
to both `SignedDuration` and `Span` to make conversions between it and
`std::time::Duration` easier.


## What are the `ZoneDifference` and `ZonedRound` types for?

The `ZonedDifference` and `ZonedRound` are builders for expressing the
parameters to the functions `Zoned::{since, until}` and `Zoned::round`,
respectively. For example, this:

```rust
use jiff::{civil::date, ToSpan};

let zdt1 = date(2024, 7, 16).at(22, 3, 23, 0).in_tz("America/New_York")?;
let zdt2 = date(2024, 8, 16).at(22, 3, 59, 0).in_tz("America/New_York")?;
assert_eq!(zdt1.until(&zdt2)?, 744.hours().seconds(36).fieldwise());

# Ok::<(), Box<dyn std::error::Error>>(())
```

Is equivalent to:

```rust
use jiff::{civil::date, ToSpan, ZonedDifference};

let zdt1 = date(2024, 7, 16).at(22, 3, 23, 0).in_tz("America/New_York")?;
let zdt2 = date(2024, 8, 16).at(22, 3, 59, 0).in_tz("America/New_York")?;
assert_eq!(
  zdt1.until(ZonedDifference::new(&zdt2))?,
  744.hours().seconds(36).fieldwise(),
);

# Ok::<(), Box<dyn std::error::Error>>(())
```

The point of this is that `ZonedDifference` permits specifying additional
configuration. For example, rounding the span returned:

```rust
use jiff::{civil::date, ToSpan, Unit, ZonedDifference};

let zdt1 = date(2024, 7, 16).at(22, 3, 23, 0).in_tz("America/New_York")?;
let zdt2 = date(2024, 8, 16).at(22, 3, 59, 0).in_tz("America/New_York")?;
assert_eq!(
    zdt1.until(ZonedDifference::new(&zdt2).smallest(Unit::Minute))?,
    744.hours().fieldwise(),
);

# Ok::<(), Box<dyn std::error::Error>>(())
```

By using a dedicated type to represent the parameters, we can enable ergonomic
uses of the API for common cases (by using `From<&Zoned> for ZonedDifference`
trait implementations) while still permitting callers to provide additional
configuration.

An alternative API would be to remove the additional parameters to
`Zoned::until`, and instead require callers to do span rounding themselves
explicitly. But this is more verbose and requires repeating the correct
zoned datetime to indicate how to interpret non-uniform units. For example,
instead of this:

```rust
use jiff::{civil::date, ToSpan, Unit, ZonedDifference};

let zdt1 = date(2024, 7, 16).at(22, 3, 23, 0).in_tz("America/New_York")?;
let zdt2 = date(2024, 8, 16).at(22, 3, 59, 0).in_tz("America/New_York")?;
let diff = ZonedDifference::new(&zdt2)
    .largest(Unit::Month)
    .smallest(Unit::Minute);
assert_eq!(zdt1.until(diff)?, 1.month().fieldwise());

# Ok::<(), Box<dyn std::error::Error>>(())
```

One would need to do this:

```rust
use jiff::{civil::date, RoundMode, SpanRound, ToSpan, Unit};

let zdt1 = date(2024, 7, 16).at(22, 3, 23, 0).in_tz("America/New_York")?;
let zdt2 = date(2024, 8, 16).at(22, 3, 59, 0).in_tz("America/New_York")?;
let span = zdt1.until(&zdt2)?;
let rounded = span.round(
    SpanRound::new()
        .largest(Unit::Month)
        .smallest(Unit::Minute)
        .relative(&zdt1)
        .mode(RoundMode::Trunc),
)?;
assert_eq!(rounded, 1.month().fieldwise());

# Ok::<(), Box<dyn std::error::Error>>(())
```

This is somewhat fiddly and easy to get wrong. Moreover, the `Zoned::until`
API, when rounding is enabled, will automatically use `RoundMode::Trunc`, since
this is what usually expects when computing the span between two datetimes. But
span rounding uses `RoundMode::HalfExpand` by default, corresponding to how you
were likely taught to round in school. (Rounds to the nearest unit, with ties
breaking away from zero.)

Similar reasoning applies to other "parameter builder" types like
`civil::DateTimeDifference` as well.


## Why isn't `Timestamp` called `Instant`?

The main reason why is because of the existence of `std::time::Instant`. While
that doesn't in and of itself prevent Jiff from using the name `Instant`, it
creates a naming clash with something that is _similar_ but different. Namely,
a Jiff `Timestamp` corresponds to a time from your system clock, where as an
`Instant` represents _monotonic_ time. The system clock might change or even
go backwards, where as a monotonic instant will always produce time that is
greater than or equal to a previous time.

An `Instant` is, for example, something you might use to measure the time
something takes in a program. Like in capturing a measurement for a benchmark.
Conversely, a `Timestamp` is something you use to represent time as represented
by the system. In particular, a `Timestamp` is like a `std::time::SystemTime`
and _not_ a `std::time::Instant`.

While [Temporal] uses the name `Instant` for their equivalent of Jiff's
`Timestamp` type, using the name `Instant` in Jiff would likely result in
serious confusion and conflicts in names when someone wants to use both an
`Instant` and a `Timestamp` in the same namespace.

[Temporal]: https://tc39.es/proposal-temporal/docs/
[temporal-one-duration]: https://github.com/tc39/proposal-temporal/issues/2915
[comparison]: http://docs.rs/jiff/*/jiff/_documentation/comparison/index.html
[comparison-chrono]: https://docs.rs/jiff/*/jiff/_documentation/comparison/index.html#chrono-v0438
[comparison-time]: https://docs.rs/jiff/*/jiff/_documentation/comparison/index.html#time-v0336
[comparison-hifitime]: https://docs.rs/jiff/*/jiff/_documentation/comparison/index.html#hifitime-v0390
[comparison-icu]: https://docs.rs/jiff/*/jiff/_documentation/comparison/index.html#icu-v150
[RFC 8536]: https://datatracker.ietf.org/doc/draft-murchison-rfc8536bis/
[RFC 9557]: https://datatracker.ietf.org/doc/rfc9557/
[POSIX TZ]: https://pubs.opengroup.org/onlinepubs/009695399/basedefs/xbd_chap08.html
[IANA Time Zone Database]: https://en.wikipedia.org/wiki/Tz_database
[TAI]: https://en.wikipedia.org/wiki/International_Atomic_Time
[github-issue-leap]: https://github.com/BurntSushi/jiff/issues/7
[`java.time`]: https://docs.oracle.com/javase/8/docs/api/java/time/package-summary.html
[NodaTime]: https://nodatime.org/
[`chrono-tz`]: https://docs.rs/chrono-tz
[`tzfile`]: https://docs.rs/tzfile
[`chrono`]: https://docs.rs/chrono
[`time`]: https://docs.rs/time
[`hifitime`]: https://docs.rs/hifitime
[`icu`]: https://docs.rs/icu
