# Platform support

This document describes Jiff's platform support. That is, it describes the
interaction points between this library and its environment. Most of the
details in this document are written down elsewhere on individual APIs, but
this document serves to centralize everything in one place.

As a general rule, interaction with the environment requires that Jiff's
`std` feature is enabled. The `std` feature is what allows Jiff to read
environment variables and files, for example.

Before starting, let's cover some vocabulary first.

## Vocabulary

This section defines the key terms used below when describing platform support.
We also try to contextualize the concepts to make their meaning concrete in a
way that hopefully relates to your lived experience.

* [Civil time]: The time you see on your clock. And in general, the time that
the humans in your approximate geographic vicinity also see. That is, civil
time is a human coordinated agreement for communicating time in a particular
geographic region. Civil time is also known as: local time, plain time, naive
time, clock time and others.
* [Time zone]: A set of rules for determining the civil (or "local") time,
via an offset from UTC, in a particular geographic region. In many cases, the
offset in a particular time zone can vary over the course of a year through
transitions into and out of [daylight saving time]. A time zone is necessary
to convert civil time into a precise unambiguous instant in time.
* [IANA Time Zone Database]: A directory on your system containing a store of
files, one per time zone, which encode the time at which transitions between
UTC offsets occur in a specific geographic region. In effect, each time zone
file provides a mapping between civil (or "local") time and UTC. The format
of each file is called TZif and is specified by [RFC 8536]. This database is
typically found at `/usr/share/zoneinfo` and only on Unix systems (including
macOS). Other environments, like Windows and WASM, do not have a standard copy
of the Time Zone Database. (Jiff will instead embed it into your program by
default on these platforms.)
* [IANA time zone identifier]: A short ASCII human readable string identifying
a time zone in the IANA Time Zone Database. The time zone for where I live,
`America/New_York`, has an entry at `/usr/share/zoneinfo/America/New_York`
on my system. IANA time zone identifiers are used by Jiff's `Zoned` type
to losslessly roundtrip datetimes via an interchange format specified by
[Temporal] that draws inspiration from [RFC 3339], [RFC 9557] and [ISO 8601].

## Environment variables

Jiff generally only reads two environment variables. These variables are
read on all platforms that support environment variables. So for example,
Jiff will respect `TZ` on Windows. Note though that some environments, like
`wasm32-wasip1` or `wasm32-unknown-emscripten`, are sandboxed by default. A
sandboxed environment typically makes reading environment variables set outside
the sandbox impossible (or require opt-in support, such as [wasmtime]'s
`-S inherit-env` or `--env` flags).

Jiff may read additional environment variables for platform specific
integration.

### `TZDIR`

The `TZDIR` environment variable tells Jiff where to look for the
[IANA Time Zone Database]. When it isn't set, Jiff will check a few standard
locations for the database. It's usually found at `/usr/share/zoneinfo`.

It can be useful to set this for non-standard environments or when you
specifically want Jiff to prefer using a non-system copy of the database.
(If you want Jiff to _only_ use a non-system copy of the database, then you'll
need to use `TimeZoneDatabase::from_dir` and use the resulting handle
explicitly.)

If a IANA Time Zone Database could not be found at `TZDIR`, then Jiff will
still attempt to look for a database at the standard locations (like
`/usr/share/zoneinfo`).

### `TZ`

The `TZ` environment variable overrides and sets the default system time zone.
It is [specified by POSIX][POSIX TZ]. Jiff implements the POSIX specification
(even on non-POSIX platforms like Windows) with some common extensions.

It is useful to set `TZ` when Jiff could not detect (or had a problem
detecting) the system time zone, or if the system time zone is wrong in a
specific circumstance.

Summarizing POSIX (and common extensions supported by GNU libc and musl), the
`TZ` environment variable accepts these kinds of values:

* `America/New_York` sets the time zone via a IANA time zone identifier.
* `/usr/share/zoneinfo/America/New_York` sets the time zone by providing a path
to a TZif formatted file.
* `EST5EDT,M3.2.0,M11.1.0` sets the time zone using a POSIX daylight saving
time rule. The rule shown here is for `US/Eastern` at time of writing (2024).
This is useful for specifying a custom time zone without generating TZif data,
but is rarely used in practice.

When `TZ` isn't set, then Jiff uses heuristics to detect the system's
configured time zone. If this automatic detection fails, please first check
for an [existing issue for your platform][issue-platform], and if one doesn't
exist, please [file a new issue][issue-new]. Otherwise, setting `TZ` should be
considered as a work-around.

### `ANDROID_ROOT` and `ANDROID_DATA`

These environment variables are read to help determine the location of
Android's [Concatenated Time Zone Database]. If `ANDROID_ROOT` is not defined,
then Jiff uses `/system` as its default value. If `ANDROID_DATA` is not
defined, then Jiff uses `/data/misc` as its default value.

Note that these environment variables are not necessarily only read on
Android, although they likely only make sense in the context of an Android
environment. This is because Jiff's support for the Concatenated Time
Zone Database is platform independent. For example, Jiff will let users
create a database from a Concatenated Time Zone Database file via the
`TimeZoneDatabase::from_concatenated_path` API on _any_ platform. This is
intended to enable maximum flexibility, and because there is no specific reason
to make the Concatenated Time Zone Database format Android-specific.

## Platforms

This section lists the platforms that Jiff has explicit support for. Support
may not be perfect, so if something isn't working as it should, check the
list of [existing platform related issues][issue-platform]. If you can't find
one that matches your specific problem, [create a new issue][issue-new].

For each platform, there are generally three things to consider:

1. Whether getting the current time is supported.
2. How Jiff finds the IANA Time Zone Database.
3. How Jiff finds the system configured time zone.

We answer these questions for each platform.

### Unix

#### Current time

All Unix platforms should be supported in terms of getting the current time.
This support comes from Rust's standard library.

#### IANA Time Zone Database

The vast majority of Unix systems, including macOS, store a copy of the IANA
time zone database at `/usr/share/zoneinfo`, which Jiff will automatically
detect. If your Unix system uses a different directory, you may try to submit
a PR adding support for it in Jiff proper, or just set the `TZDIR` environment
variable.

The existence of `/usr/share/zoneinfo` is not guaranteed in all Unix
environments. For example, stripped down Docker containers might omit a full
copy of the time zone database. Jiff will still work in such environments, but
all IANA time zone identifier lookups will fail. To fix this, you can either
install the IANA Time Zone Database into your environment, or you can enable
the Jiff crate feature `tzdb-bundle-always`. This compile time setting will
cause Jiff to depend on `jiff-tzdb`, which includes a complete copy of the IANA
Time Zone Database embedded into the compiled artifact.

Bundling the IANA Time Zone Database should only be done as a last resort.
Especially on Unix systems, it is greatly preferred to use the system copy of
the database, as the database is typically updated a few times each year. By
using the system copy, Jiff will automatically pick up updates without needing
to be recompiled.

But if bundling is needed, it is a fine solution. It just means that Jiff will
need to be re-compiled after `jiff-tzdb` is updated when a new IANA Time Zone
Database release is published.

#### System time zone

On most Unix systems, the system configured time zone manifests as a symbolic
link at `/etc/localtime`. The symbolic link usually points to a file in
your system copy of the IANA Time Zone Database. For example, on my Linux
system:

```text
$ ls -l /etc/localtime
lrwxrwxrwx 1 root root 36 Jul 15 20:26 /etc/localtime -> /usr/share/zoneinfo/America/New_York
```

And my macOS system:

```text
$ ls -l /etc/localtime
lrwxr-xr-x  1 root  wheel  42 Jun 20 07:13 /etc/localtime -> /var/db/timezone/zoneinfo/America/New_York
```

Jiff examines the symbolic link metadata to extract the IANA time zone
identifier from the file path. In the above two examples, that would be
`America/New_York`. The identifier is then used to do a lookup in the system
copy of the IANA Time Zone Database.

If `/etc/localtime` is not a symbolic link, then Jiff reads it directly as a
TZif file. When this happens, Jiff cannot feasibly know the IANA time zone
identifier. While arithmetic on the resulting `Zoned` value will still be DST
safe, one cannot losslessly serialize and deserialize it since Jiff won't be
able to include the IANA time zone identifier in the serialized format. When
such a `Zoned` value is serialized, the offset of the datetime will be used
in lieu of the IANA time zone identifier.

(NOTE: Not all Unix systems follow this pattern. If your system uses a
different way to configure the system time zone, please check [available
platform issues][issue-platform] for a related issue. If one doesn't exist,
please [create a new issue][issue-new].)

### Android

#### Current time

All Android platforms should be supported in terms of getting the current time.
This support comes from Rust's standard library.

#### IANA Time Zone Database

Unlike effectively every other Unix system, Android has its own special time
zone database format. While it still makes use of TZif formatted data for
defining time zone transitions themselves, it does not use the `zoneinfo`
directory format (where there is one file per time zone). Instead, it
_concatenates_ all time zone files into one single file. This is combined with
some meta data that makes it quick to search for time zones by their IANA time
zone identifier.

This format is technically unnamed, but Jiff refers to it as the [Concatenated
Time Zone Database] format. It has no formal specification. Jiff's
implementation was done by inferring the format implemented by the Android
Platform and also the implementation in [Go's standard library]. In practice
this tends to work well, although there are obviously no guarantees. This is
a practical trade-off given that there doesn't appear to be any obvious
alternative. Moreover, others (such as Go, a project maintained by the same
company that maintains Android) are already doing it, so it seems likely that
if Android decides to make breaking changes to the format, they'll need to
version it in some way to avoid breaking the ecosystem.

Note that Jiff supports reading this format on all platforms, not just Android.
For example, Jiff users can use the `TimeZoneDatabase::from_concatenated_path`
API to create a `TimeZoneDatabase` from a concatenated `tzdata` file on any
platform.

If users of Jiff are uncomfortable relying on Android's "unstable" time zone
database format, then there are a few options available to them after disabling
the `tzdb-concatenated` crate feature:

* They can own the responsibility of putting a standard `zoneinfo` database
installation into their environment. Then set the `TZDIR` environment variable
to point at it, and Jiff will automatically use it.
* Enable the `tzdb-bundle-always` crate feature. This will cause the entire
time zone database to be compiled into your binary. Nothing else needs to be
done. Jiff will automatically use the bundled copy.
* Manually create `TimeZone` values via `TimeZone::tzif` from TZif formatted
data. With this approach, you may need to change how you use Jiff in some
cases. For example, any `in_tz` method will need to be changed to use the
`to_zoned` equivalent.
* Embed specific time zones into your binary with `jiff::tz::get` or
`jiff::tz::include`. This requires enabling Jiff's `static` feature.

#### System time zone

The system time zone on Android is discovered by reading the
`persist.sys.timezone` property.

Note that in addition to Android developers citing the [Concatenated Time Zone
Database] format as unstable, they also discourage the discovery of the system
time zone through properties as well. (See [chrono#1018] and [chrono#1148]
for some discussion on this topic.) For Jiff at least, there is no feasible
alternative. Apparently, the blessed API is to use their Java libraries, but
that doesn't seem feasible to me inside of Jiff since I (Jiff's author) is
unaware of a mechanism for easily calling Java code from Rust. The only option
left is to use their `libc` APIs, which they did at least improve to make them
thread safe, but this isn't enough for Jiff. For Jiff, we really want the
actual IANA time zone identifier, and it isn't clear how to discover this from
their `libc` APIs. Moreover, Jiff supports far more sophisticated operations on
a time zone (like dealing with discontinuities in civil time) that cannot be
implemented on top of `libc`-style APIs. Using Android's `libc` APIs for time
handling would be a huge regression compared to all other platforms.

It's worth noting that all other popular Unix systems provide at least some
reliable means of both querying the time zone database _and_ discovering the
system-wide IANA time zone identifier. Why Android is incapable of following
the existing conventions for Unix systems is unclear.

If users of Jiff are uncomfortable relying on Android's `persist.sys.timezone`
property, then they should avoid APIs like `Zoned::now` and `TimeZone::system`.
Instead, they can use `TimeZone::unknown()`, which is what the fallback time
zone would be when the system time zone cannot be discovered.

### Windows

#### Current time

All Windows platforms should be supported in terms of getting the current time.
This support comes from Rust's standard library.

#### IANA Time Zone Database

Windows does not have a canonical installation of the IANA Time Zone Database
like Unix. Because of this, and because of the importance of time zone support
to Jiff's design, Jiff will automatically embed an entire copy of the IANA Time
Zone Database into your binary on Windows.

The automatic bundling is done via the Jiff crate feature
`tzdb-bundle-platform`. This is a _target activated feature_. Namely, it is
enabled by default, but only results in a bundled database on an enumerated set
of platforms (where Windows is one of them). If you want to opt out of bundling
the database on Windows, you'll need to disable this feature.

Bundling the IANA Time Zone Database is not ideal, since after a new release of
the database, you'll need to wait for the `jiff-tzdb` crate to be updated. Then
you'll need to update your dependency version and re-compile your software to
get the database updates.

One alternative is to point Jiff to a copy of the IANA Time Zone Database via
the `TZDIR` environment variable. Even on Windows, Jiff will attempt to read
the directory specified as a time zone database. But you'll likely need to
manage the database yourself.

#### System time zone

Jiff currently uses [`GetDynamicTimeZoneInformation`] from the Windows C API
to query the current time zone information. This provides a value of type
[`DYNAMIC_TIME_ZONE_INFORMATION`]. Jiff uses the `TimeZoneKeyName` member
of that type to do a lookup in Unicode's [CLDR XML data] that maps Windows
time zone names to IANA time zone identifiers. The resulting IANA time zone
identifier is then used as a key to find a time zone in the configured IANA
Time Zone Database.

### WASM

There are a variety of WASM targets available for Rust that service different
use cases. Here is a possibly incomplete list of those targets and a short
imprecise blurb about them:

* `wasm32-unknown-emscripten`: Sandboxed and emulates Unix as much as possible.
* `wasm32-wasi` and `wasm32-wasip1`: Provides a sandbox with capability-based
security. This is not typically used in web browsers. [wasmtime] is an example
of a runtime that can run programs compiled for these targets.
* `wasm{32,64}-unknown-unknown`: Typically used for web deployments to run in
a browser via `wasm-pack`. But, crucially, not exclusively so.

Jiff supports all of these targets, but the nature of that support varies. Each
target is discussed in the sections below.

#### The `js` crate feature

Jiff comes with a `js` crate feature that is disabled by default. It is a
_target activated feature_ that enables dependencies on the `js-sys` and
`wasm-bindgen` crates. This feature is intended to be enabled only in binaries,
tests and benchmarks when it is known that the code will be running in a
web context. Consequently, this feature only activates this support for the
`wasm{32,64}-unknown-unknown` targets. It has no effect on any other target,
including other WASM targets.

Library crates should generally never enable Jiff's `js` feature or even
forward it. Applications using your library can depend on Jiff directly and
enable the feature.

#### Current time

* `wasm32-unknown-emscripten`: Supported via Rust's standard library.
* `wasm32-wasi*`: Supported via Rust's standard library.
* `wasm{32,64}-unknown-unknown`: `std::time::SystemTime::now()`, and thus
`Zoned::now()`, panics in Jiff's default configuration. Enabling Jiff's `js`
feature will cause Jiff to assume a web context and use JavaScript's
[`Date.now`] API to determine the current time.

#### IANA Time Zone Database

None of the WASM targets have a canonical installation of the IANA Time Zone
Database. Because of this, and because of the importance of time zone support
to Jiff's design, Jiff will automatically embed an entire copy of the IANA
Time Zone Database into your binary on all WASM targets.

The automatic bundling is done via the Jiff crate feature
`tzdb-bundle-platform`. This is a _target activated feature_. Namely, it is
enabled by default, but only results in a bundled database on an enumerated set
of platforms (where WASM is one of them). If you want to opt out of bundling
the database on WASM targets, you'll need to disable this feature.

Bundling the IANA Time Zone Database is not ideal, since after a new release of
the database, you'll need to wait for the `jiff-tzdb` crate to be updated. Then
you'll need to update your dependency version and re-compile your software to
get the database updates.

Some WASM targets, like `wasm32-wasip1`, can actually read the host's
IANA Time Zone Database (e.g., on Unix), but this requires relaxing its
sandbox restrictions so that the code can read system directories like
`/usr/share/zoneinfo`. That is, it won't work out of the box. The same applies
to the `wasm32-unknown-emscripten` target. (Although this author could not
figure out how to relax emscripten's sandbox.)

#### System time zone

* `wasm32-unknown-emscripten`: Unsupported.
* `wasm32-wasi*`: Unsupported. But you may set the `TZ` environment variable
via your WASM runtime, and Jiff will respect it. For example, with [wasmtime],
that's `--env TZ=America/New_York`.
* `wasm{32,64}-unknown-unknown`: Unsupported in Jiff's default configuration.
Enabling Jiff's `js` feature will cause Jiff to assume a web context and use
JavaScript's [`Intl.DateTimeFormat`] API to determine the system configured
IANA time zone identifier. This time zone identifier is then used to look up
the time zone in Jiff's configured IANA Time Zone Database.

[Civil time]: https://en.wikipedia.org/wiki/Civil_time
[Time zone]: https://en.wikipedia.org/wiki/Time_zone
[daylight saving time]: https://en.wikipedia.org/wiki/Daylight_saving_time
[IANA Time Zone Database]: https://en.wikipedia.org/wiki/Tz_database
[IANA time zone identifier]: https://data.iana.org/time-zones/theory.html#naming
[RFC 8536]: https://datatracker.ietf.org/doc/html/rfc8536
[wasmtime]: https://wasmtime.dev/
[POSIX TZ]: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap08.html
[RFC 3339]: https://www.rfc-editor.org/rfc/rfc3339
[RFC 9557]: https://www.rfc-editor.org/rfc/rfc9557.html
[ISO 8601]: https://www.iso.org/iso-8601-date-and-time-format.html
[Temporal]: https://tc39.es/proposal-temporal
[issue-platform]: https://github.com/BurntSushi/jiff/issues?q=label%3Aplatform
[issue-new]: https://github.com/BurntSushi/jiff/issues/new
[`GetDynamicTimeZoneInformation`]: https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/nf-timezoneapi-getdynamictimezoneinformation
[`DYNAMIC_TIME_ZONE_INFORMATION`]: https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/ns-timezoneapi-dynamic_time_zone_information
[CLDR XML data]: https://github.com/unicode-org/cldr/raw/main/common/supplemental/windowsZones.xml
[`Intl.DateTimeFormat`]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl/DateTimeFormat/resolvedOptions#timezone
[`Date.now`]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/now
[Concatenated Time Zone Database]: https://android.googlesource.com/platform/libcore/+/jb-mr2-release/luni/src/main/java/libcore/util/ZoneInfoDB.java
[Go's standard library]: https://github.com/golang/go/blob/19e923182e590ae6568c2c714f20f32512aeb3e3/src/time/zoneinfo_android.go
[chrono#1018]: https://github.com/chronotope/chrono/pull/1018
[chrono#1148]: https://github.com/chronotope/chrono/pull/1148
