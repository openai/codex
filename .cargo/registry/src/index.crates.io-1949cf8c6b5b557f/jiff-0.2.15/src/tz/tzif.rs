/*!
This module provides support for TZif binary files from the [Time Zone
Database].

These binary files are the ones commonly found in Unix distributions in the
`/usr/share/zoneinfo` directory.

[Time Zone Database]: https://www.iana.org/time-zones
*/

use core::ops::Range;

#[cfg(feature = "alloc")]
use alloc::{string::String, vec::Vec};

use crate::{
    civil::DateTime,
    error::Error,
    shared::{self, util::array_str::Abbreviation},
    timestamp::Timestamp,
    tz::{
        posix::PosixTimeZone, timezone::TimeZoneAbbreviation, AmbiguousOffset,
        Dst, Offset, TimeZoneOffsetInfo, TimeZoneTransition,
    },
};

/// The owned variant of `Tzif`.
#[cfg(feature = "alloc")]
pub(crate) type TzifOwned = Tzif<
    String,
    Abbreviation,
    Vec<shared::TzifLocalTimeType>,
    Vec<i64>,
    Vec<shared::TzifDateTime>,
    Vec<shared::TzifDateTime>,
    Vec<shared::TzifTransitionInfo>,
>;

/// The static variant of `Tzif`.
pub(crate) type TzifStatic = Tzif<
    &'static str,
    &'static str,
    &'static [shared::TzifLocalTimeType],
    &'static [i64],
    &'static [shared::TzifDateTime],
    &'static [shared::TzifDateTime],
    &'static [shared::TzifTransitionInfo],
>;

/// A time zone based on IANA TZif formatted data.
///
/// TZif is a binary format described by RFC 8536. Its typical structure is to
/// define a single time zone per file in the `/usr/share/zoneinfo` directory
/// on Unix systems. The name of a time zone is its file path with the
/// `/usr/share/zoneinfo/` prefix stripped from it.
///
/// This type doesn't provide any facilities for dealing with files on disk
/// or the `/usr/share/zoneinfo` directory. This type is just for parsing the
/// contents of TZif formatted data in memory, and turning it into a data type
/// that can be used as a time zone.
#[derive(Debug)]
// not part of Jiff's public API
#[doc(hidden)]
// This ensures the alignment of this type is always *at least* 8 bytes. This
// is required for the pointer tagging inside of `TimeZone` to be sound. At
// time of writing (2024-02-24), this explicit `repr` isn't required on 64-bit
// systems since the type definition is such that it will have an alignment of
// at least 8 bytes anyway. But this *is* required for 32-bit systems, where
// the type definition at present only has an alignment of 4 bytes.
#[repr(align(8))]
pub struct Tzif<STR, ABBREV, TYPES, TIMESTAMPS, STARTS, ENDS, INFOS> {
    inner: shared::Tzif<STR, ABBREV, TYPES, TIMESTAMPS, STARTS, ENDS, INFOS>,
    /// The POSIX time zone for this TZif data, if present.
    ///
    /// Note that this is also present on `shared::Tzif`, but uses the
    /// `shared::PosixTimeZone` type, which isn't quite what we want here.
    ///
    /// For now we just duplicate it, which is slightly unfortunate. But this
    /// is small and not a huge deal. Ideally we can clean this up later.
    posix_tz: Option<PosixTimeZone<ABBREV>>,
}

impl TzifStatic {
    /// Converts from the shared-but-internal API for use in proc macros.
    ///
    /// This specifically works in a `const` context. And it requires that
    /// caller to pass in the parsed `Tzif` in its fixed form along with the
    /// variable length local time types and transitions. (Technically, the
    /// TZ identifier and the designations are also variable length despite
    /// being parsed of `TzifFixed`, but in practice they can be handled just
    /// fine via `&'static str`.)
    ///
    /// Notice that the `types` and `transitions` are *not* from the `shared`
    /// API, but rather, from the types defined in this module. They have to
    /// be this way because there's a conversion step that occurs. In practice,
    /// this sort of thing is embedded as a literal in source code via a proc
    /// macro. Like this:
    ///
    /// ```text
    /// static TZIF: Tzif<&str, &str, &[LocalTimeType], &[Transition]> =
    ///     Tzif::from_shared_const(
    ///         shared::TzifFixed {
    ///             name: Some("America/New_York"),
    ///             version: b'3',
    ///             checksum: 0xDEADBEEF,
    ///             designations: "ESTEDT",
    ///             posix_tz: None,
    ///         },
    ///         &[
    ///             shared::TzifLocalTimeType {
    ///                 offset: -5 * 60 * 60,
    ///                 is_dst: false,
    ///                 designation: 0..3,
    ///                 indicator: shared::TzifIndicator::LocalWall,
    ///             }.to_jiff(),
    ///         ],
    ///         &[
    ///             shared::TzifTransition {
    ///                 timestamp: 123456789,
    ///                 type_index: 0,
    ///             }.to_jiff(-5, -5),
    ///         ],
    ///     );
    /// ```
    ///
    /// Or something like that anyway. The point is, our `static` slices are
    /// variable length and they need to be the right types. At least, I
    /// couldn't see a simpler way to arrange this.
    pub(crate) const fn from_shared_const(
        sh: shared::TzifStatic,
    ) -> TzifStatic {
        let posix_tz = match sh.fixed.posix_tz {
            None => None,
            Some(posix_tz) => Some(PosixTimeZone::from_shared_const(posix_tz)),
        };
        Tzif { inner: sh, posix_tz }
    }
}

#[cfg(feature = "alloc")]
impl TzifOwned {
    /// Parses the given data as a TZif formatted file.
    ///
    /// The name given is attached to the `Tzif` value returned, but is
    /// otherwise not significant.
    ///
    /// If the given data is not recognized to be valid TZif, then an error is
    /// returned.
    ///
    /// In general, callers may assume that it is safe to pass arbitrary or
    /// even untrusted data to this function and count on it not panicking
    /// or using resources that aren't limited to a small constant factor of
    /// the size of the data itself. That is, callers can reliably limit the
    /// resources used by limiting the size of the data given to this parse
    /// function.
    pub(crate) fn parse(
        name: Option<String>,
        bytes: &[u8],
    ) -> Result<Self, Error> {
        let sh =
            shared::TzifOwned::parse(name, bytes).map_err(Error::shared)?;
        Ok(TzifOwned::from_shared_owned(sh))
    }

    /// Converts from the shared-but-internal API for use in proc macros.
    ///
    /// This is not `const` since it accepts owned values on the heap for
    /// variable length data inside `Tzif`.
    pub(crate) fn from_shared_owned(sh: shared::TzifOwned) -> TzifOwned {
        let posix_tz = match sh.fixed.posix_tz {
            None => None,
            Some(posix_tz) => Some(PosixTimeZone::from_shared_owned(posix_tz)),
        };
        Tzif { inner: sh, posix_tz }
    }
}

impl<
        STR: AsRef<str>,
        ABBREV: AsRef<str>,
        TYPES: AsRef<[shared::TzifLocalTimeType]>,
        TIMESTAMPS: AsRef<[i64]>,
        STARTS: AsRef<[shared::TzifDateTime]>,
        ENDS: AsRef<[shared::TzifDateTime]>,
        INFOS: AsRef<[shared::TzifTransitionInfo]>,
    > Tzif<STR, ABBREV, TYPES, TIMESTAMPS, STARTS, ENDS, INFOS>
{
    /// Returns the name given to this TZif data in its constructor.
    pub(crate) fn name(&self) -> Option<&str> {
        self.inner.fixed.name.as_ref().map(|n| n.as_ref())
    }

    /// Returns the appropriate time zone offset to use for the given
    /// timestamp.
    pub(crate) fn to_offset(&self, timestamp: Timestamp) -> Offset {
        match self.to_local_time_type(timestamp) {
            Ok(typ) => Offset::from_seconds_unchecked(typ.offset),
            Err(tz) => tz.to_offset(timestamp),
        }
    }

    /// Returns the appropriate time zone offset to use for the given
    /// timestamp.
    ///
    /// This also includes whether the offset returned should be considered to
    /// be DST or not, along with the time zone abbreviation (e.g., EST for
    /// standard time in New York, and EDT for DST in New York).
    pub(crate) fn to_offset_info(
        &self,
        timestamp: Timestamp,
    ) -> TimeZoneOffsetInfo<'_> {
        let typ = match self.to_local_time_type(timestamp) {
            Ok(typ) => typ,
            Err(tz) => return tz.to_offset_info(timestamp),
        };
        let abbreviation =
            TimeZoneAbbreviation::Borrowed(self.designation(typ));
        TimeZoneOffsetInfo {
            offset: Offset::from_seconds_unchecked(typ.offset),
            dst: Dst::from(typ.is_dst),
            abbreviation,
        }
    }

    /// Returns the local time type for the timestamp given.
    ///
    /// If one could not be found, then this implies that the caller should
    /// use the POSIX time zone returned in the error variant.
    fn to_local_time_type(
        &self,
        timestamp: Timestamp,
    ) -> Result<&shared::TzifLocalTimeType, &PosixTimeZone<ABBREV>> {
        let timestamp = timestamp.as_second();
        // This is guaranteed because we always push at least one transition.
        // This isn't guaranteed by TZif since it might have 0 transitions,
        // but we always add a "dummy" first transition with our minimum
        // `Timestamp` value. TZif doesn't do this because there is no
        // universal minimum timestamp. (`i64::MIN` is a candidate, but that's
        // likely to cause overflow in readers that don't do error checking.)
        //
        // The result of the dummy transition is that the code below is simpler
        // with fewer special cases.
        let timestamps = self.timestamps();
        assert!(!timestamps.is_empty(), "transitions is non-empty");
        let index = if timestamp > *timestamps.last().unwrap() {
            timestamps.len() - 1
        } else {
            let search = self.timestamps().binary_search(&timestamp);
            match search {
                // Since the first transition is always Timestamp::MIN, it's
                // impossible for any timestamp to sort before it.
                Err(0) => {
                    unreachable!("impossible to come before Timestamp::MIN")
                }
                Ok(i) => i,
                // i points to the position immediately after the matching
                // timestamp. And since we know that i>0 because of the i==0
                // check above, we can safely subtract 1.
                Err(i) => i.checked_sub(1).expect("i is non-zero"),
            }
        };
        // Our index is always in bounds. The only way it couldn't be is if
        // binary search returns an Err(len) for a time greater than the
        // maximum transition. But we account for that above by converting
        // Err(len) to Err(len-1).
        debug_assert!(index < timestamps.len());
        // RFC 8536 says: "Local time for timestamps on or after the last
        // transition is specified by the TZ string in the footer (Section 3.3)
        // if present and nonempty; otherwise, it is unspecified."
        //
        // Subtracting 1 is OK because we know self.transitions is not empty.
        let index = if index < timestamps.len() - 1 {
            // This is the typical case in "fat" TZif files: we found a
            // matching transition.
            index
        } else {
            match self.posix_tz() {
                // This is the typical case in "slim" TZif files, where the
                // last transition is, as I understand it, the transition at
                // which a consistent rule started that a POSIX TZ string can
                // fully describe. For example, (as of 2024-03-27) the last
                // transition in the "fat" America/New_York TZif file is
                // in 2037, where as in the "slim" version it is 2007.
                //
                // This is likely why some things break with the "slim"
                // version: they don't support POSIX TZ strings (or don't
                // support them correctly).
                Some(tz) => return Err(tz),
                // This case is technically unspecified, but I think the
                // typical thing to do is to just use the last transition.
                // I'm not 100% sure on this one.
                None => index,
            }
        };
        Ok(self.local_time_type(index))
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
        // This implementation very nearly mirrors `to_local_time_type`
        // above in the beginning: we do a binary search to find transition
        // applicable for the given datetime. Except, we do it on wall clock
        // times instead of timestamps. And in particular, each transition
        // begins with a possibly ambiguous range of wall clock times
        // corresponding to either a "gap" or "fold" in time.
        let dtt = shared::TzifDateTime::new(
            dt.year(),
            dt.month(),
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second(),
        );
        let (starts, ends) = (self.civil_starts(), self.civil_ends());
        assert!(!starts.is_empty(), "transitions is non-empty");
        let this_index = match starts.binary_search(&dtt) {
            Err(0) => unreachable!("impossible to come before DateTime::MIN"),
            Ok(i) => i,
            Err(i) => i.checked_sub(1).expect("i is non-zero"),
        };
        debug_assert!(this_index < starts.len());

        let this_offset = self.local_time_type(this_index).offset;
        // This is a little tricky, but we need to check for ambiguous civil
        // datetimes before possibly using the POSIX TZ string. Namely, a
        // datetime could be ambiguous with respect to the last transition,
        // and we should handle that according to the gap/fold determined for
        // that transition. We cover this case in tests in tz/mod.rs for the
        // Pacific/Honolulu time zone, whose last transition begins with a gap.
        match self.transition_kind(this_index) {
            shared::TzifTransitionKind::Gap if dtt < ends[this_index] => {
                // A gap/fold can only appear when there exists a previous
                // transition.
                let prev_index = this_index.checked_sub(1).unwrap();
                let prev_offset = self.local_time_type(prev_index).offset;
                return AmbiguousOffset::Gap {
                    before: Offset::from_seconds_unchecked(prev_offset),
                    after: Offset::from_seconds_unchecked(this_offset),
                };
            }
            shared::TzifTransitionKind::Fold if dtt < ends[this_index] => {
                // A gap/fold can only appear when there exists a previous
                // transition.
                let prev_index = this_index.checked_sub(1).unwrap();
                let prev_offset = self.local_time_type(prev_index).offset;
                return AmbiguousOffset::Fold {
                    before: Offset::from_seconds_unchecked(prev_offset),
                    after: Offset::from_seconds_unchecked(this_offset),
                };
            }
            _ => {}
        }
        // The datetime given is not ambiguous with respect to any of the
        // transitions in the TZif data. But, if we matched at or after the
        // last transition, then we need to use the POSIX TZ string (which
        // could still return an ambiguous offset).
        if this_index == starts.len() - 1 {
            if let Some(tz) = self.posix_tz() {
                return tz.to_ambiguous_kind(dt);
            }
            // This case is unspecified according to RFC 8536. It means that
            // the given datetime exceeds all transitions *and* there is no
            // POSIX TZ string. So this can happen in V1 files for example.
            // But those should hopefully be essentially non-existent nowadays
            // (2024-03). In any case, we just fall through to using the last
            // transition, which does seem likely to be wrong ~half the time
            // in time zones with DST. But there really isn't much else we can
            // do I think.
        }
        AmbiguousOffset::Unambiguous {
            offset: Offset::from_seconds_unchecked(this_offset),
        }
    }

    /// Returns the timestamp of the most recent time zone transition prior
    /// to the timestamp given. If one doesn't exist, `None` is returned.
    pub(crate) fn previous_transition(
        &self,
        ts: Timestamp,
    ) -> Option<TimeZoneTransition> {
        assert!(!self.timestamps().is_empty(), "transitions is non-empty");
        let mut timestamp = ts.as_second();
        if ts.subsec_nanosecond() != 0 {
            timestamp = timestamp.saturating_add(1);
        }
        let search = self.timestamps().binary_search(&timestamp);
        let index = match search {
            Ok(i) | Err(i) => i.checked_sub(1)?,
        };
        let index = if index == 0 {
            // The first transition is a dummy that we insert, so if we land on
            // it here, treat it as if it doesn't exist.
            return None;
        } else if index == self.timestamps().len() - 1 {
            if let Some(ref posix_tz) = self.posix_tz() {
                // Since the POSIX TZ must be consistent with the last
                // transition, it must be the case that tzif_last <=
                // posix_prev_trans in all cases. So the transition according
                // to the POSIX TZ is always correct here.
                //
                // What if this returns `None` though? I'm not sure in which
                // cases that could matter, and I think it might be a violation
                // of the TZif format if it does.
                //
                // It can return `None`! In the case of a time zone that
                // has eliminated DST, it might have historical time zone
                // transitions but a POSIX time zone without DST. (For example,
                // `America/Sao_Paulo`.) And thus, this would return `None`.
                // So if it does, we pretend as if the POSIX time zone doesn't
                // exist.
                if let Some(trans) = posix_tz.previous_transition(ts) {
                    return Some(trans);
                }
            }
            index
        } else {
            index
        };
        let timestamp = self.timestamps()[index];
        let typ = self.local_time_type(index);
        Some(TimeZoneTransition {
            timestamp: Timestamp::constant(timestamp, 0),
            offset: Offset::from_seconds_unchecked(typ.offset),
            abbrev: self.designation(typ),
            dst: Dst::from(typ.is_dst),
        })
    }

    /// Returns the timestamp of the soonest time zone transition after the
    /// timestamp given. If one doesn't exist, `None` is returned.
    pub(crate) fn next_transition(
        &self,
        ts: Timestamp,
    ) -> Option<TimeZoneTransition> {
        assert!(!self.timestamps().is_empty(), "transitions is non-empty");
        let timestamp = ts.as_second();
        let search = self.timestamps().binary_search(&timestamp);
        let index = match search {
            Ok(i) => i.checked_add(1)?,
            Err(i) => i,
        };
        let index = if index == 0 {
            // The first transition is a dummy that we insert, so if we land on
            // it here, treat it as if it doesn't exist.
            return None;
        } else if index >= self.timestamps().len() {
            if let Some(posix_tz) = self.posix_tz() {
                // Since the POSIX TZ must be consistent with the last
                // transition, it must be the case that next.timestamp <=
                // posix_next_tans in all cases. So the transition according to
                // the POSIX TZ is always correct here.
                //
                // What if this returns `None` though? I'm not sure in which
                // cases that could matter, and I think it might be a violation
                // of the TZif format if it does.
                //
                // In the "previous" case above, this could return `None` even
                // when there are historical time zone transitions in the case
                // of a time zone eliminating DST (e.g., `America/Sao_Paulo`).
                // But unlike the previous case, if we get `None` here, then
                // that is the real answer because there are no other known
                // future time zone transitions.
                //
                // 2025-05-05: OK, this could return `None` and this is fine.
                // It happens for time zones that had DST but then stopped
                // it at some point in the past. The POSIX time zone has no
                // DST and thus returns `None`. That's fine. But there was a
                // problem: we were using the POSIX time zone even when there
                // was a historical time zone transition after the timestamp
                // given. That was fixed by changing the condition when we get
                // here: it can only happen when the timestamp given comes at
                // or after all historical time zone transitions.
                return posix_tz.next_transition(ts);
            }
            self.timestamps().len() - 1
        } else {
            index
        };
        let timestamp = self.timestamps()[index];
        let typ = self.local_time_type(index);
        Some(TimeZoneTransition {
            timestamp: Timestamp::constant(timestamp, 0),
            offset: Offset::from_seconds_unchecked(typ.offset),
            abbrev: self.designation(typ),
            dst: Dst::from(typ.is_dst),
        })
    }

    fn designation(&self, typ: &shared::TzifLocalTimeType) -> &str {
        // OK because we verify that the designation range on every local
        // time type is a valid range into `self.designations`.
        &self.designations()[typ.designation()]
    }

    fn local_time_type(
        &self,
        transition_index: usize,
    ) -> &shared::TzifLocalTimeType {
        // OK because we require that `type_index` always points to a valid
        // local time type.
        &self.types()[usize::from(self.infos()[transition_index].type_index)]
    }

    fn transition_kind(
        &self,
        transition_index: usize,
    ) -> shared::TzifTransitionKind {
        self.infos()[transition_index].kind
    }

    fn posix_tz(&self) -> Option<&PosixTimeZone<ABBREV>> {
        self.posix_tz.as_ref()
    }

    fn designations(&self) -> &str {
        self.inner.fixed.designations.as_ref()
    }

    fn types(&self) -> &[shared::TzifLocalTimeType] {
        self.inner.types.as_ref()
    }

    fn timestamps(&self) -> &[i64] {
        self.inner.transitions.timestamps.as_ref()
    }

    fn civil_starts(&self) -> &[shared::TzifDateTime] {
        self.inner.transitions.civil_starts.as_ref()
    }

    fn civil_ends(&self) -> &[shared::TzifDateTime] {
        self.inner.transitions.civil_ends.as_ref()
    }

    fn infos(&self) -> &[shared::TzifTransitionInfo] {
        self.inner.transitions.infos.as_ref()
    }
}

impl<STR: AsRef<str>, ABBREV, TYPES, TIMESTAMPS, STARTS, ENDS, INFOS> Eq
    for Tzif<STR, ABBREV, TYPES, TIMESTAMPS, STARTS, ENDS, INFOS>
{
}

impl<STR: AsRef<str>, ABBREV, TYPES, TIMESTAMPS, STARTS, ENDS, INFOS> PartialEq
    for Tzif<STR, ABBREV, TYPES, TIMESTAMPS, STARTS, ENDS, INFOS>
{
    fn eq(&self, rhs: &Self) -> bool {
        self.inner.fixed.name.as_ref().map(|n| n.as_ref())
            == rhs.inner.fixed.name.as_ref().map(|n| n.as_ref())
            && self.inner.fixed.checksum == rhs.inner.fixed.checksum
    }
}

impl shared::TzifLocalTimeType {
    fn designation(&self) -> Range<usize> {
        usize::from(self.designation.0)..usize::from(self.designation.1)
    }
}

impl core::fmt::Display for shared::TzifIndicator {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match *self {
            shared::TzifIndicator::LocalWall => write!(f, "local/wall"),
            shared::TzifIndicator::LocalStandard => write!(f, "local/std"),
            shared::TzifIndicator::UTStandard => write!(f, "ut/std"),
        }
    }
}

/// Does a quick check that returns true if the data might be in TZif format.
///
/// It is possible that this returns true even if the given data is not in TZif
/// format. However, it is impossible for this to return false when the given
/// data is TZif. That is, a false positive is allowed but a false negative is
/// not.
#[cfg(feature = "tzdb-zoneinfo")]
pub(crate) fn is_possibly_tzif(data: &[u8]) -> bool {
    data.starts_with(b"TZif")
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use alloc::{string::ToString, vec};

    #[cfg(not(miri))]
    use crate::tz::testdata::TZIF_TEST_FILES;

    use super::*;

    /// This converts TZif data into a human readable format.
    ///
    /// This is useful for debugging (via `./scripts/jiff-debug tzif`), but we
    /// also use it for snapshot testing to make reading the test output at
    /// least *somewhat* comprehensible for humans. Otherwise, one needs to
    /// read and understand Unix timestamps. That ain't going to fly.
    ///
    /// For this to work, we make sure everything in a `Tzif` value is
    /// represented in some way in this output.
    fn tzif_to_human_readable(tzif: &TzifOwned) -> String {
        use std::io::Write;

        fn datetime(dt: shared::TzifDateTime) -> DateTime {
            DateTime::constant(
                dt.year(),
                dt.month(),
                dt.day(),
                dt.hour(),
                dt.minute(),
                dt.second(),
                0,
            )
        }

        let mut out = tabwriter::TabWriter::new(vec![])
            .alignment(tabwriter::Alignment::Left);

        writeln!(out, "TIME ZONE NAME").unwrap();
        writeln!(out, "  {}", tzif.name().unwrap_or("UNNAMED")).unwrap();

        writeln!(out, "TIME ZONE VERSION").unwrap();
        writeln!(
            out,
            "  {}",
            char::try_from(tzif.inner.fixed.version).unwrap()
        )
        .unwrap();

        writeln!(out, "LOCAL TIME TYPES").unwrap();
        for (i, typ) in tzif.inner.types.iter().enumerate() {
            writeln!(
                out,
                "  {i:03}:\toffset={off}\t\
                   designation={desig}\t{dst}\tindicator={ind}",
                off = Offset::from_seconds_unchecked(typ.offset),
                desig = tzif.designation(&typ),
                dst = if typ.is_dst { "dst" } else { "" },
                ind = typ.indicator,
            )
            .unwrap();
        }
        if !tzif.timestamps().is_empty() {
            writeln!(out, "TRANSITIONS").unwrap();
            for i in 0..tzif.timestamps().len() {
                let timestamp = Timestamp::constant(tzif.timestamps()[i], 0);
                let dt = Offset::UTC.to_datetime(timestamp);
                let typ = tzif.local_time_type(i);
                let wall =
                    alloc::format!("{}", datetime(tzif.civil_starts()[i]));
                let ambiguous = match tzif.transition_kind(i) {
                    shared::TzifTransitionKind::Unambiguous => {
                        "unambiguous".to_string()
                    }
                    shared::TzifTransitionKind::Gap => {
                        let end = datetime(tzif.civil_ends()[i]);
                        alloc::format!(" gap-until({end})")
                    }
                    shared::TzifTransitionKind::Fold => {
                        let end = datetime(tzif.civil_ends()[i]);
                        alloc::format!("fold-until({end})")
                    }
                };

                writeln!(
                    out,
                    "  {i:04}:\t{dt:?}Z\tunix={ts}\twall={wall}\t\
                       {ambiguous}\t\
                       type={type_index}\t{off}\t\
                       {desig}\t{dst}",
                    ts = timestamp.as_second(),
                    type_index = tzif.infos()[i].type_index,
                    off = Offset::from_seconds_unchecked(typ.offset),
                    desig = tzif.designation(typ),
                    dst = if typ.is_dst { "dst" } else { "" },
                )
                .unwrap();
            }
        }
        if let Some(ref posix_tz) = tzif.posix_tz {
            writeln!(out, "POSIX TIME ZONE STRING").unwrap();
            writeln!(out, "  {}", posix_tz).unwrap();
        }
        String::from_utf8(out.into_inner().unwrap()).unwrap()
    }

    /// DEBUG COMMAND
    ///
    /// Takes environment variable `JIFF_DEBUG_TZIF_PATH` as input, and treats
    /// the value as a TZif file path. This test will open the file, parse it
    /// as a TZif and then dump debug data about the file in a human readable
    /// plain text format.
    #[cfg(feature = "std")]
    #[test]
    fn debug_tzif() -> anyhow::Result<()> {
        use anyhow::Context;

        let _ = crate::logging::Logger::init();

        const ENV: &str = "JIFF_DEBUG_TZIF_PATH";
        let Some(val) = std::env::var_os(ENV) else { return Ok(()) };
        let Ok(val) = val.into_string() else {
            anyhow::bail!("{ENV} has invalid UTF-8")
        };
        let bytes =
            std::fs::read(&val).with_context(|| alloc::format!("{val:?}"))?;
        let tzif = Tzif::parse(Some(val.to_string()), &bytes)?;
        std::eprint!("{}", tzif_to_human_readable(&tzif));
        Ok(())
    }

    #[cfg(not(miri))]
    #[test]
    fn tzif_parse_v2plus() {
        for tzif_test in TZIF_TEST_FILES {
            insta::assert_snapshot!(
                alloc::format!("{}_v2+", tzif_test.name),
                tzif_to_human_readable(&tzif_test.parse())
            );
        }
    }

    #[cfg(not(miri))]
    #[test]
    fn tzif_parse_v1() {
        for tzif_test in TZIF_TEST_FILES {
            insta::assert_snapshot!(
                alloc::format!("{}_v1", tzif_test.name),
                tzif_to_human_readable(&tzif_test.parse_v1())
            );
        }
    }

    /// This tests walks the /usr/share/zoneinfo directory (if it exists) and
    /// tries to parse every TZif formatted file it can find. We don't really
    /// do much with it other than to ensure we don't panic or return an error.
    /// That is, we check that we can parse each file, but not that we do so
    /// correctly.
    #[cfg(not(miri))]
    #[cfg(feature = "tzdb-zoneinfo")]
    #[cfg(target_os = "linux")]
    #[test]
    fn zoneinfo() {
        const TZDIR: &str = "/usr/share/zoneinfo";

        for result in walkdir::WalkDir::new(TZDIR) {
            // Just skip if we got an error traversing the directory tree.
            // These aren't related to our parsing, so it's some other problem
            // (like the directory not existing).
            let Ok(dent) = result else { continue };
            // This test can take some time in debug mode, so skip parsing
            // some of the less frequently used TZif files.
            let Some(name) = dent.path().to_str() else { continue };
            if name.contains("right/") || name.contains("posix/") {
                continue;
            }
            // Again, skip if we can't read. Not my monkeys, not my circus.
            let Ok(bytes) = std::fs::read(dent.path()) else { continue };
            if !is_possibly_tzif(&bytes) {
                continue;
            }
            let tzname = dent
                .path()
                .strip_prefix(TZDIR)
                .unwrap_or_else(|_| {
                    panic!("all paths in TZDIR have {TZDIR:?} prefix")
                })
                .to_str()
                .expect("all paths to be valid UTF-8")
                .to_string();
            // OK at this point, we're pretty sure `bytes` should be a TZif
            // binary file. So try to parse it and fail the test if it fails.
            if let Err(err) = Tzif::parse(Some(tzname), &bytes) {
                panic!("failed to parse TZif file {:?}: {err}", dent.path());
            }
        }
    }
}
