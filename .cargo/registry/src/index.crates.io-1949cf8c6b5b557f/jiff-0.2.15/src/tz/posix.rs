/*!
Provides a parser for [POSIX's `TZ` environment variable][posix-env].

NOTE: Sadly, at time of writing, the actual parser is in `src/shared/posix.rs`.
This is so it can be shared (via simple code copying) with proc macros like
the one found in `jiff-tzdb-static`. The parser populates a "lowest common
denominator" data type. In normal use in Jiff, this type is converted into
the types defined below. This module still does provide the various time zone
operations. Only the parsing is written elsewhere.

The `TZ` environment variable is most commonly used to set a time zone. For
example, `TZ=America/New_York`. But it can also be used to tersely define DST
transitions. Moreover, the format is not just used as an environment variable,
but is also included at the end of TZif files (version 2 or greater). The IANA
Time Zone Database project also [documents the `TZ` variable][iana-env] with
a little more commentary.

Note that we (along with pretty much everyone else) don't strictly follow
POSIX here. Namely, `TZ=America/New_York` isn't a POSIX compatible usage,
and I believe it technically should be `TZ=:America/New_York`. Nevertheless,
apparently some group of people (IANA folks?) decided `TZ=America/New_York`
should be fine. From the [IANA `theory.html` documentation][iana-env]:

> It was recognized that allowing the TZ environment variable to take on values
> such as 'America/New_York' might cause "old" programs (that expect TZ to have
> a certain form) to operate incorrectly; consideration was given to using
> some other environment variable (for example, TIMEZONE) to hold the string
> used to generate the TZif file's name. In the end, however, it was decided
> to continue using TZ: it is widely used for time zone purposes; separately
> maintaining both TZ and TIMEZONE seemed a nuisance; and systems where "new"
> forms of TZ might cause problems can simply use legacy TZ values such as
> "EST5EDT" which can be used by "new" programs as well as by "old" programs
> that assume pre-POSIX TZ values.

Indeed, even [musl subscribes to this behavior][musl-env]. So that's what we do
here too.

Note that a POSIX time zone like `EST5` corresponds to the UTC offset `-05:00`,
and `GMT-4` corresponds to the UTC offset `+04:00`. Yes, it's backwards. How
fun.

# IANA v3+ Support

While this module and many of its types are directly associated with POSIX,
this module also plays a supporting role for `TZ` strings in the IANA TZif
binary format for versions 2 and greater. Specifically, for versions 3 and
greater, some minor extensions are supported here via `IanaTz::parse`. But
using `PosixTz::parse` is limited to parsing what is specified by POSIX.
Nevertheless, we generally use `IanaTz::parse` everywhere, even when parsing
the `TZ` environment variable. The reason for this is that it seems to be what
other programs do in practice (for example, GNU date).

# `no-std` and `no-alloc` support

A big part of this module works fine in core-only environments. But because
core-only environments provide means of indirection, and embedding a
`PosixTimeZone` into a `TimeZone` without indirection would use up a lot of
space (and thereby make `Zoned` quite chunky), we provide core-only support
principally through a proc macro. Namely, a `PosixTimeZone` can be parsed by
the proc macro and then turned into static data.

POSIX time zone support isn't explicitly provided directly as a public API
for core-only environments, but is implicitly supported via TZif. (Since TZif
data contains POSIX time zone strings.)

[posix-env]: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap08.html#tag_08_03
[iana-env]: https://data.iana.org/time-zones/tzdb-2024a/theory.html#functions
[musl-env]: https://wiki.musl-libc.org/environment-variables
*/

use crate::{
    civil::DateTime,
    error::{err, Error, ErrorContext},
    shared,
    timestamp::Timestamp,
    tz::{
        timezone::TimeZoneAbbreviation, AmbiguousOffset, Dst, Offset,
        TimeZoneOffsetInfo, TimeZoneTransition,
    },
    util::{array_str::Abbreviation, escape::Bytes, parse},
};

/// The result of parsing the POSIX `TZ` environment variable.
///
/// A `TZ` variable can either be a time zone string with an optional DST
/// transition rule, or it can begin with a `:` followed by an arbitrary set of
/// bytes that is implementation defined.
///
/// In practice, the content following a `:` is treated as an IANA time zone
/// name. Moreover, even if the `TZ` string doesn't start with a `:` but
/// corresponds to a IANA time zone name, then it is interpreted as such.
/// (See the module docs.) However, this type only encapsulates the choices
/// strictly provided by POSIX: either a time zone string with an optional DST
/// transition rule, or an implementation defined string with a `:` prefix. If,
/// for example, `TZ="America/New_York"`, then that case isn't encapsulated by
/// this type. Callers needing that functionality will need to handle the error
/// returned by parsing this type and layer their own semantics on top.
#[cfg(feature = "tz-system")]
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PosixTzEnv {
    /// A valid POSIX time zone with an optional DST transition rule.
    Rule(PosixTimeZoneOwned),
    /// An implementation defined string. This occurs when the `TZ` value
    /// starts with a `:`. The string returned here does not include the `:`.
    Implementation(alloc::boxed::Box<str>),
}

#[cfg(feature = "tz-system")]
impl PosixTzEnv {
    /// Parse a POSIX `TZ` environment variable string from the given bytes.
    fn parse(bytes: impl AsRef<[u8]>) -> Result<PosixTzEnv, Error> {
        let bytes = bytes.as_ref();
        if bytes.get(0) == Some(&b':') {
            let Ok(string) = core::str::from_utf8(&bytes[1..]) else {
                return Err(err!(
                    "POSIX time zone string with a ':' prefix contains \
                     invalid UTF-8: {:?}",
                    Bytes(&bytes[1..]),
                ));
            };
            Ok(PosixTzEnv::Implementation(string.into()))
        } else {
            PosixTimeZone::parse(bytes).map(PosixTzEnv::Rule)
        }
    }

    /// Parse a POSIX `TZ` environment variable string from the given `OsStr`.
    pub(crate) fn parse_os_str(
        osstr: impl AsRef<std::ffi::OsStr>,
    ) -> Result<PosixTzEnv, Error> {
        PosixTzEnv::parse(parse::os_str_bytes(osstr.as_ref())?)
    }
}

#[cfg(feature = "tz-system")]
impl core::fmt::Display for PosixTzEnv {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match *self {
            PosixTzEnv::Rule(ref tz) => write!(f, "{tz}"),
            PosixTzEnv::Implementation(ref imp) => write!(f, ":{imp}"),
        }
    }
}

/// An owned POSIX time zone.
///
/// That is, a POSIX time zone whose abbreviations are inlined into the
/// representation. As opposed to a static POSIX time zone whose abbreviations
/// are `&'static str`.
pub(crate) type PosixTimeZoneOwned = PosixTimeZone<Abbreviation>;

/// An owned POSIX time zone whose abbreviations are `&'static str`.
pub(crate) type PosixTimeZoneStatic = PosixTimeZone<&'static str>;

/// A POSIX time zone.
///
/// # On "reasonable" POSIX time zones
///
/// Jiff only supports "reasonable" POSIX time zones. A "reasonable" POSIX time
/// zone is a POSIX time zone that has a DST transition rule _when_ it has a
/// DST time zone abbreviation. Without the transition rule, it isn't possible
/// to know when DST starts and stops.
///
/// POSIX technically allows a DST time zone abbreviation *without* a
/// transition rule, but the behavior is literally unspecified. So Jiff just
/// rejects them.
///
/// Note that if you're confused as to why Jiff accepts `TZ=EST5EDT` (where
/// `EST5EDT` is an example of an _unreasonable_ POSIX time zone), that's
/// because Jiff rejects `EST5EDT` and instead attempts to use it as an IANA
/// time zone identifier. And indeed, the IANA Time Zone Database contains an
/// entry for `EST5EDT` (presumably for legacy reasons).
///
/// Also, we expect `TZ` strings parsed from IANA v2+ formatted `tzfile`s to
/// also be reasonable or parsing fails. This also seems to be consistent with
/// the [GNU C Library]'s treatment of the `TZ` variable: it only documents
/// support for reasonable POSIX time zone strings.
///
/// Note that a V2 `TZ` string is precisely identical to a POSIX `TZ`
/// environment variable string. A V3 `TZ` string however supports signed DST
/// transition times, and hours in the range `0..=167`. The V2 and V3 here
/// reference how `TZ` strings are defined in the TZif format specified by
/// [RFC 9636]. V2 is the original version of it straight from POSIX, where as
/// V3+ corresponds to an extension added to V3 (and newer versions) of the
/// TZif format. V3 is a superset of V2, so in practice, Jiff just permits
/// V3 everywhere.
///
/// [GNU C Library]: https://www.gnu.org/software/libc/manual/2.25/html_node/TZ-Variable.html
/// [RFC 9636]: https://datatracker.ietf.org/doc/rfc9636/
#[derive(Clone, Debug, Eq, PartialEq)]
// NOT part of Jiff's public API
#[doc(hidden)]
// This ensures the alignment of this type is always *at least* 8 bytes. This
// is required for the pointer tagging inside of `TimeZone` to be sound. At
// time of writing (2024-02-24), this explicit `repr` isn't required on 64-bit
// systems since the type definition is such that it will have an alignment of
// at least 8 bytes anyway. But this *is* required for 32-bit systems, where
// the type definition at present only has an alignment of 4 bytes.
#[repr(align(8))]
pub struct PosixTimeZone<ABBREV> {
    inner: shared::PosixTimeZone<ABBREV>,
}

impl PosixTimeZone<Abbreviation> {
    /// Parse a IANA tzfile v3+ `TZ` string from the given bytes.
    #[cfg(feature = "alloc")]
    pub(crate) fn parse(
        bytes: impl AsRef<[u8]>,
    ) -> Result<PosixTimeZoneOwned, Error> {
        let bytes = bytes.as_ref();
        let inner = shared::PosixTimeZone::parse(bytes.as_ref())
            .map_err(Error::shared)
            .map_err(|e| {
                e.context(err!("invalid POSIX TZ string {:?}", Bytes(bytes)))
            })?;
        Ok(PosixTimeZone { inner })
    }

    /// Like `parse`, but parses a POSIX TZ string from a prefix of the
    /// given input. And remaining input is returned.
    #[cfg(feature = "alloc")]
    pub(crate) fn parse_prefix<'b, B: AsRef<[u8]> + ?Sized + 'b>(
        bytes: &'b B,
    ) -> Result<(PosixTimeZoneOwned, &'b [u8]), Error> {
        let bytes = bytes.as_ref();
        let (inner, remaining) =
            shared::PosixTimeZone::parse_prefix(bytes.as_ref())
                .map_err(Error::shared)
                .map_err(|e| {
                    e.context(err!(
                        "invalid POSIX TZ string {:?}",
                        Bytes(bytes)
                    ))
                })?;
        Ok((PosixTimeZone { inner }, remaining))
    }

    /// Converts from the shared-but-internal API for use in proc macros.
    #[cfg(feature = "alloc")]
    pub(crate) fn from_shared_owned(
        sh: shared::PosixTimeZone<Abbreviation>,
    ) -> PosixTimeZoneOwned {
        PosixTimeZone { inner: sh }
    }
}

impl PosixTimeZone<&'static str> {
    /// Converts from the shared-but-internal API for use in proc macros.
    ///
    /// This works in a `const` context by requiring that the time zone
    /// abbreviations are `static` strings. This is used when converting
    /// code generated by a proc macro to this Jiff internal type.
    pub(crate) const fn from_shared_const(
        sh: shared::PosixTimeZone<&'static str>,
    ) -> PosixTimeZoneStatic {
        PosixTimeZone { inner: sh }
    }
}

impl<ABBREV: AsRef<str>> PosixTimeZone<ABBREV> {
    /// Returns the appropriate time zone offset to use for the given
    /// timestamp.
    ///
    /// If you need information like whether the offset is in DST or not, or
    /// the time zone abbreviation, then use `PosixTimeZone::to_offset_info`.
    /// But that API may be more expensive to use, so only use it if you need
    /// the additional data.
    pub(crate) fn to_offset(&self, timestamp: Timestamp) -> Offset {
        Offset::from_ioffset_const(
            self.inner.to_offset(timestamp.to_itimestamp_const()),
        )
    }

    /// Returns the appropriate time zone offset to use for the given
    /// timestamp.
    ///
    /// This also includes whether the offset returned should be considered
    /// to be "DST" or not, along with the time zone abbreviation (e.g., EST
    /// for standard time in New York, and EDT for DST in New York).
    pub(crate) fn to_offset_info(
        &self,
        timestamp: Timestamp,
    ) -> TimeZoneOffsetInfo<'_> {
        let (ioff, abbrev, is_dst) =
            self.inner.to_offset_info(timestamp.to_itimestamp_const());
        let offset = Offset::from_ioffset_const(ioff);
        let abbreviation = TimeZoneAbbreviation::Borrowed(abbrev);
        TimeZoneOffsetInfo { offset, dst: Dst::from(is_dst), abbreviation }
    }

    /// Returns a possibly ambiguous timestamp for the given civil datetime.
    ///
    /// The given datetime should correspond to the "wall" clock time of what
    /// humans use to tell time for this time zone.
    ///
    /// Note that "ambiguous timestamp" is represented by the possible
    /// selection of offsets that could be applied to the given datetime. In
    /// general, it is only ambiguous around transitions to-and-from DST. The
    /// ambiguity can arise as a "fold" (when a particular wall clock time is
    /// repeated) or as a "gap" (when a particular wall clock time is skipped
    /// entirely).
    pub(crate) fn to_ambiguous_kind(&self, dt: DateTime) -> AmbiguousOffset {
        let iamoff = self.inner.to_ambiguous_kind(dt.to_idatetime_const());
        AmbiguousOffset::from_iambiguous_offset_const(iamoff)
    }

    /// Returns the timestamp of the most recent time zone transition prior
    /// to the timestamp given. If one doesn't exist, `None` is returned.
    pub(crate) fn previous_transition(
        &self,
        timestamp: Timestamp,
    ) -> Option<TimeZoneTransition> {
        let (its, ioff, abbrev, is_dst) =
            self.inner.previous_transition(timestamp.to_itimestamp_const())?;
        let timestamp = Timestamp::from_itimestamp_const(its);
        let offset = Offset::from_ioffset_const(ioff);
        let dst = Dst::from(is_dst);
        Some(TimeZoneTransition { timestamp, offset, abbrev, dst })
    }

    /// Returns the timestamp of the soonest time zone transition after the
    /// timestamp given. If one doesn't exist, `None` is returned.
    pub(crate) fn next_transition(
        &self,
        timestamp: Timestamp,
    ) -> Option<TimeZoneTransition> {
        let (its, ioff, abbrev, is_dst) =
            self.inner.next_transition(timestamp.to_itimestamp_const())?;
        let timestamp = Timestamp::from_itimestamp_const(its);
        let offset = Offset::from_ioffset_const(ioff);
        let dst = Dst::from(is_dst);
        Some(TimeZoneTransition { timestamp, offset, abbrev, dst })
    }
}

impl<ABBREV: AsRef<str>> core::fmt::Display for PosixTimeZone<ABBREV> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.inner, f)
    }
}

// The tests below require parsing which requires alloc.
#[cfg(feature = "alloc")]
#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "tz-system")]
    #[test]
    fn parse_posix_tz() {
        // We used to parse this and then error when we tried to
        // convert to a "reasonable" POSIX time zone with a DST
        // transition rule. We never actually used unreasonable POSIX
        // time zones and it was complicating the type definitions, so
        // now we just reject it outright.
        assert!(PosixTzEnv::parse("EST5EDT").is_err());

        let tz = PosixTzEnv::parse(":EST5EDT").unwrap();
        assert_eq!(tz, PosixTzEnv::Implementation("EST5EDT".into()));

        // We require implementation strings to be UTF-8, because we're
        // sensible.
        assert!(PosixTzEnv::parse(b":EST5\xFFEDT").is_err());
    }
}
