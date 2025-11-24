/*!
Defines data types shared between `jiff` and `jiff-static`.

While this module exposes types that can be imported outside of `jiff` itself,
there are *no* semver guarantees provided. That is, this module is _not_ part
of Jiff's public API. The only guarantee of compatibility that is provided
is that `jiff-static x.y.z` works with one and only one version of Jiff,
corresponding to `jiff x.y.z` (i.e., the same version number).

# Design

This module is really accomplishing two different things at the same time.

Firstly, it is a way to provide types that can be used to construct a static
`TimeZone`. The proc macros in `jiff-static` generate code using these
types (and a few routines).

Secondly, it provides a way to parse TZif data without `jiff-static`
depending on `jiff` via a Cargo dependency. This actually requires copying
the code in this module (which is why it is kinda sectioned off from the rest
of jiff) into the `jiff-static` crate. This can be done automatically with
`jiff-cli`:

```text
jiff-cli generate shared
```

The copying of code is pretty unfortunate, because it means both crates have to
compile it. However, the alternatives aren't great either.

One alternative is to have `jiff-static` explicitly depend on `jiff` in its
`Cargo.toml`. Then Jiff could expose the parsing routines, as it does here,
and `jiff-static` could use them directly. Unfortunately, this means that
`jiff` cannot depend on `jiff-static`. And that in turn means that `jiff`
cannot re-export the macros. Users will need to explicitly depend on and use
`jiff-static`. Moreover, this could result in some potential surprises
since `jiff-static` will need to have an `=x.y.z` dependency on Jiff for
compatibility reasons. That in turn means that the version of Jiff actually
used is not determine by the user's `jiff = "x.y.z"` line, but rather by the
user's `jiff-static = "x'.y'.z'"` line. This is overall annoying and not a
good user experience. Plus, it inverts the typical relationship between crates
and their proc macros (e.g., `serde` and `serde_derive`) and thus could result
in other unanticipated surprises.

Another obvious alternative is to split this code out into a separate crate
that both `jiff` and `jiff-static` depend on. However, the API exposed in
this module does not provide a coherent user experience. It would either need a
ton of work to turn it into a coherent user experience or it would need to be
published as a `jiff-internal-use-only` crate that I find to be very annoying
and confusing. Moreover, a separate crate introduces a new semver boundary
beneath Jiff. I've found these sorts of things to overall increase maintenance
burden (see ripgrep and regex for cases where I did this).

I overall decided that the least bad choice was to copy a little code (under
2,000 source lines of code at present I believe). Since the copy is managed
automatically via `jiff-cli generate shared`, we remove the downside of the
code getting out of sync. The only downside is extra compile time. Since I
generally only expect `jiff-static` to be used in niche circumstances, I
prefer this trade-off over the other choices.

More context on how I arrived at this design can be found here:
<https://github.com/BurntSushi/jiff/issues/256>

# Particulars

When this code is copied to `jiff-static`, the following transformations are
done:

* A header is added to indicate that the copied file is auto-generated.
* All `#[cfg(feature = "alloc")]` annotations are removed. The `jiff-static`
  proc macro always runs in a context where the standard library is available.
* Any code between `// only-jiff-start` and `// only-jiff-end` comments is
  removed. Nesting isn't supported.

Otherwise, this module is specifically organized in a way that doesn't rely on
any other part of Jiff. The one exception are routines to convert from these
exposed types to other internal types inside of Jiff. This is necessary for
building a static `TimeZone`. But these conversion routines are removed when
this module is copied to `jiff-static`.
*/

/// An alias for TZif data whose backing storage has a `'static` lifetime.
// only-jiff-start
pub type TzifStatic = Tzif<
    &'static str,
    &'static str,
    &'static [TzifLocalTimeType],
    &'static [i64],
    &'static [TzifDateTime],
    &'static [TzifDateTime],
    &'static [TzifTransitionInfo],
>;
// only-jiff-end

/// An alias for TZif data whose backing storage is on the heap.
#[cfg(feature = "alloc")]
pub type TzifOwned = Tzif<
    alloc::string::String,
    self::util::array_str::Abbreviation,
    alloc::vec::Vec<TzifLocalTimeType>,
    alloc::vec::Vec<i64>,
    alloc::vec::Vec<TzifDateTime>,
    alloc::vec::Vec<TzifDateTime>,
    alloc::vec::Vec<TzifTransitionInfo>,
>;

/// An alias for TZif transition data whose backing storage is on the heap.
#[cfg(feature = "alloc")]
pub type TzifTransitionsOwned = TzifTransitions<
    alloc::vec::Vec<i64>,
    alloc::vec::Vec<TzifDateTime>,
    alloc::vec::Vec<TzifDateTime>,
    alloc::vec::Vec<TzifTransitionInfo>,
>;

#[derive(Clone, Debug)]
pub struct Tzif<STR, ABBREV, TYPES, TIMESTAMPS, STARTS, ENDS, INFOS> {
    pub fixed: TzifFixed<STR, ABBREV>,
    pub types: TYPES,
    pub transitions: TzifTransitions<TIMESTAMPS, STARTS, ENDS, INFOS>,
}

#[derive(Clone, Debug)]
pub struct TzifFixed<STR, ABBREV> {
    pub name: Option<STR>,
    /// An ASCII byte corresponding to the version number. So, 0x50 is '2'.
    ///
    /// This is unused. It's only used in `test` compilation for emitting
    /// diagnostic data about TZif files. If we really need to use this, we
    /// should probably just convert it to an actual integer.
    pub version: u8,
    pub checksum: u32,
    pub designations: STR,
    pub posix_tz: Option<PosixTimeZone<ABBREV>>,
}

#[derive(Clone, Copy, Debug)]
pub struct TzifLocalTimeType {
    pub offset: i32,
    pub is_dst: bool,
    pub designation: (u8, u8), // inclusive..exclusive
    pub indicator: TzifIndicator,
}

/// This enum corresponds to the possible indicator values for standard/wall
/// and UT/local.
///
/// Note that UT+Wall is not allowed.
///
/// I honestly have no earthly clue what they mean. I've read the section about
/// them in RFC 8536 several times and I can't make sense of it. I've even
/// looked at data files that have these set and still can't make sense of
/// them. I've even looked at what other datetime libraries do with these, and
/// they all seem to just ignore them. Like, WTF. I've spent the last couple
/// months of my life steeped in time, and I just cannot figure this out. Am I
/// just dumb?
///
/// Anyway, we parse them, but otherwise ignore them because that's what all
/// the cool kids do.
///
/// The default is `LocalWall`, which also occurs when no indicators are
/// present.
///
/// I tried again and still don't get it. Here's a dump for `Pacific/Honolulu`:
///
/// ```text
/// $ ./scripts/jiff-debug tzif /usr/share/zoneinfo/Pacific/Honolulu
/// TIME ZONE NAME
///   /usr/share/zoneinfo/Pacific/Honolulu
/// LOCAL TIME TYPES
///   000: offset=-10:31:26, is_dst=false, designation=LMT, indicator=local/wall
///   001: offset=-10:30, is_dst=false, designation=HST, indicator=local/wall
///   002: offset=-09:30, is_dst=true, designation=HDT, indicator=local/wall
///   003: offset=-09:30, is_dst=true, designation=HWT, indicator=local/wall
///   004: offset=-09:30, is_dst=true, designation=HPT, indicator=ut/std
///   005: offset=-10, is_dst=false, designation=HST, indicator=local/wall
/// TRANSITIONS
///   0000: -9999-01-02T01:59:59 :: -377705023201 :: type=0, -10:31:26, is_dst=false, LMT, local/wall
///   0001: 1896-01-13T22:31:26 :: -2334101314 :: type=1, -10:30, is_dst=false, HST, local/wall
///   0002: 1933-04-30T12:30:00 :: -1157283000 :: type=2, -09:30, is_dst=true, HDT, local/wall
///   0003: 1933-05-21T21:30:00 :: -1155436200 :: type=1, -10:30, is_dst=false, HST, local/wall
///   0004: 1942-02-09T12:30:00 :: -880198200 :: type=3, -09:30, is_dst=true, HWT, local/wall
///   0005: 1945-08-14T23:00:00 :: -769395600 :: type=4, -09:30, is_dst=true, HPT, ut/std
///   0006: 1945-09-30T11:30:00 :: -765376200 :: type=1, -10:30, is_dst=false, HST, local/wall
///   0007: 1947-06-08T12:30:00 :: -712150200 :: type=5, -10, is_dst=false, HST, local/wall
/// POSIX TIME ZONE STRING
///   HST10
/// ```
///
/// See how type 004 has a ut/std indicator? What the fuck does that mean?
/// All transitions are defined in terms of UTC. I confirmed this with `zdump`:
///
/// ```text
/// $ zdump -v Pacific/Honolulu | rg 1945
/// Pacific/Honolulu  Tue Aug 14 22:59:59 1945 UT = Tue Aug 14 13:29:59 1945 HWT isdst=1 gmtoff=-34200
/// Pacific/Honolulu  Tue Aug 14 23:00:00 1945 UT = Tue Aug 14 13:30:00 1945 HPT isdst=1 gmtoff=-34200
/// Pacific/Honolulu  Sun Sep 30 11:29:59 1945 UT = Sun Sep 30 01:59:59 1945 HPT isdst=1 gmtoff=-34200
/// Pacific/Honolulu  Sun Sep 30 11:30:00 1945 UT = Sun Sep 30 01:00:00 1945 HST isdst=0 gmtoff=-37800
/// ```
///
/// The times match up. All of them. The indicators don't seem to make a
/// difference. I'm clearly missing something.
#[derive(Clone, Copy, Debug)]
pub enum TzifIndicator {
    LocalWall,
    LocalStandard,
    UTStandard,
}

/// The set of transitions in TZif data, laid out in column orientation.
///
/// The column orientation is used to make TZ lookups faster. Specifically,
/// for finding an offset for a timestamp, we do a binary search on
/// `timestamps`. For finding an offset for a local datetime, we do a binary
/// search on `civil_starts`. By making these two distinct sequences with
/// nothing else in them, we make them as small as possible and thus improve
/// cache locality.
///
/// All sequences in this type are in correspondence with one another. They
/// are all guaranteed to have the same length.
#[derive(Clone, Debug)]
pub struct TzifTransitions<TIMESTAMPS, STARTS, ENDS, INFOS> {
    /// The timestamp at which this transition begins.
    pub timestamps: TIMESTAMPS,
    /// The wall clock time for when a transition begins.
    pub civil_starts: STARTS,
    /// The wall clock time for when a transition ends.
    ///
    /// This is only non-zero when the transition kind is a gap or a fold.
    pub civil_ends: ENDS,
    /// Any other relevant data about a transition, such as its local type
    /// index and the transition kind.
    pub infos: INFOS,
}

/// TZif transition info beyond the timestamp and civil datetime.
///
/// For example, this contains a transition's "local type index," which in
/// turn gives access to the offset (among other metadata) for that transition.
#[derive(Clone, Copy, Debug)]
pub struct TzifTransitionInfo {
    /// The index into the sequence of local time type records. This is what
    /// provides the correct offset (from UTC) that is active beginning at
    /// this transition.
    pub type_index: u8,
    /// The boundary condition for quickly determining if a given wall clock
    /// time is ambiguous (i.e., falls in a gap or a fold).
    pub kind: TzifTransitionKind,
}

/// The kind of a transition.
///
/// This is used when trying to determine the offset for a local datetime. It
/// indicates how the corresponding civil datetimes in `civil_starts` and
/// `civil_ends` should be interpreted. That is, there are three possible
/// cases:
///
/// 1. The offset of this transition is equivalent to the offset of the
/// previous transition. That means there are no ambiguous civil datetimes
/// between the transitions. This can occur, e.g., when the time zone
/// abbreviation changes.
/// 2. The offset of the transition is greater than the offset of the previous
/// transition. That means there is a "gap" in local time between the
/// transitions. This typically corresponds to entering daylight saving time.
/// It is usually, but not always, 1 hour.
/// 3. The offset of the transition is less than the offset of the previous
/// transition. That means there is a "fold" in local time where time is
/// repeated. This typically corresponds to leaving daylight saving time. It
/// is usually, but not always, 1 hour.
///
/// # More explanation
///
/// This, when combined with `civil_starts` and `civil_ends` in
/// `TzifTransitions`, explicitly represents ambiguous wall clock times that
/// occur at the boundaries of transitions.
///
/// The start of the wall clock time is always the earlier possible wall clock
/// time that could occur with this transition's corresponding offset. For a
/// gap, it's the previous transition's offset. For a fold, it's the current
/// transition's offset.
///
/// For example, DST for `America/New_York` began on `2024-03-10T07:00:00+00`.
/// The offset prior to this instant in time is `-05`, corresponding
/// to standard time (EST). Thus, in wall clock time, DST began at
/// `2024-03-10T02:00:00`. And since this is a DST transition that jumps ahead
/// an hour, the start of DST also corresponds to the start of a gap. That is,
/// the times `02:00:00` through `02:59:59` never appear on a clock for this
/// hour. The question is thus: which offset should we apply to `02:00:00`?
/// We could apply the offset from the earlier transition `-05` and get
/// `2024-03-10T01:00:00-05` (that's `2024-03-10T06:00:00+00`), or we could
/// apply the offset from the later transition `-04` and get
/// `2024-03-10T03:00:00-04` (that's `2024-03-10T07:00:00+00`).
///
/// So in the above, we would have a `Gap` variant where `start` (inclusive) is
/// `2024-03-10T02:00:00` and `end` (exclusive) is `2024-03-10T03:00:00`.
///
/// The fold case is the same idea, but where the same time is repeated.
/// For example, in `America/New_York`, standard time began on
/// `2024-11-03T06:00:00+00`. The offset prior to this instant in time
/// is `-04`, corresponding to DST (EDT). Thus, in wall clock time, DST
/// ended at `2024-11-03T02:00:00`. However, since this is a fold, the
/// actual set of ambiguous times begins at `2024-11-03T01:00:00` and
/// ends at `2024-11-03T01:59:59.999999999`. That is, the wall clock time
/// `2024-11-03T02:00:00` is unambiguous.
///
/// So in the fold case above, we would have a `Fold` variant where
/// `start` (inclusive) is `2024-11-03T01:00:00` and `end` (exclusive) is
/// `2024-11-03T02:00:00`.
///
/// Since this gets bundled in with the sorted sequence of transitions, we'll
/// use the "start" time in all three cases as our target of binary search.
/// Once we land on a transition, we'll know our given wall clock time is
/// greater than or equal to its start wall clock time. At that point, to
/// determine if there is ambiguity, we merely need to determine if the given
/// wall clock time is less than the corresponding `end` time. If it is, then
/// it falls in a gap or fold. Otherwise, it's unambiguous.
///
/// Note that we could compute these datetime values while searching for the
/// correct transition, but there's a fair bit of math involved in going
/// between timestamps (which is what TZif gives us) and calendar datetimes
/// (which is what we're given as input). It is also necessary that we offset
/// the timestamp given in TZif at some point, since it is in UTC and the
/// datetime given is in wall clock time. So I decided it would be worth
/// pre-computing what we need in terms of what the input is. This way, we
/// don't need to do any conversions, or indeed, any arithmetic at all, for
/// time zone lookups. We *could* store these as transitions, but then the
/// input datetime would need to be converted to a timestamp before searching
/// the transitions.
#[derive(Clone, Copy, Debug)]
pub enum TzifTransitionKind {
    /// This transition cannot possibly lead to an unambiguous offset because
    /// its offset is equivalent to the offset of the previous transition.
    ///
    /// Has an entry in `civil_starts`, but corresponding entry in `civil_ends`
    /// is always zeroes (i.e., meaningless).
    Unambiguous,
    /// This occurs when this transition's offset is strictly greater than the
    /// previous transition's offset. This effectively results in a "gap" of
    /// time equal to the difference in the offsets between the two
    /// transitions.
    ///
    /// Has an entry in `civil_starts` for when the gap starts (inclusive) in
    /// local time. Also has an entry in `civil_ends` for when the fold ends
    /// (exclusive) in local time.
    Gap,
    /// This occurs when this transition's offset is strictly less than the
    /// previous transition's offset. This results in a "fold" of time where
    /// the two transitions have an overlap where it is ambiguous which one
    /// applies given a wall clock time. In effect, a span of time equal to the
    /// difference in the offsets is repeated.
    ///
    /// Has an entry in `civil_starts` for when the fold starts (inclusive) in
    /// local time. Also has an entry in `civil_ends` for when the fold ends
    /// (exclusive) in local time.
    Fold,
}

/// The representation we use to represent a civil datetime.
///
/// We don't use `shared::util::itime::IDateTime` here because we specifically
/// do not need to represent fractional seconds. This lets us easily represent
/// what we need in 8 bytes instead of the 12 bytes used by `IDateTime`.
///
/// Moreover, we pack the fields into a single `i64` to make comparisons
/// extremely cheap. This is especially useful since we do a binary search on
/// `&[TzifDateTime]` when doing a TZ lookup for a civil datetime.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct TzifDateTime {
    bits: i64,
}

impl TzifDateTime {
    pub const ZERO: TzifDateTime = TzifDateTime::new(0, 0, 0, 0, 0, 0);

    pub const fn new(
        year: i16,
        month: i8,
        day: i8,
        hour: i8,
        minute: i8,
        second: i8,
    ) -> TzifDateTime {
        // TzifDateTime { year, month, day, hour, minute, second }
        let mut bits = (year as u64) << 48;
        bits |= (month as u64) << 40;
        bits |= (day as u64) << 32;
        bits |= (hour as u64) << 24;
        bits |= (minute as u64) << 16;
        bits |= (second as u64) << 8;
        // The least significant 8 bits remain 0.
        TzifDateTime { bits: bits as i64 }
    }

    pub const fn year(self) -> i16 {
        (self.bits as u64 >> 48) as u16 as i16
    }

    pub const fn month(self) -> i8 {
        (self.bits as u64 >> 40) as u8 as i8
    }

    pub const fn day(self) -> i8 {
        (self.bits as u64 >> 32) as u8 as i8
    }

    pub const fn hour(self) -> i8 {
        (self.bits as u64 >> 24) as u8 as i8
    }

    pub const fn minute(self) -> i8 {
        (self.bits as u64 >> 16) as u8 as i8
    }

    pub const fn second(self) -> i8 {
        (self.bits as u64 >> 8) as u8 as i8
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PosixTimeZone<ABBREV> {
    pub std_abbrev: ABBREV,
    pub std_offset: PosixOffset,
    pub dst: Option<PosixDst<ABBREV>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PosixDst<ABBREV> {
    pub abbrev: ABBREV,
    pub offset: PosixOffset,
    pub rule: PosixRule,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PosixRule {
    pub start: PosixDayTime,
    pub end: PosixDayTime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PosixDayTime {
    pub date: PosixDay,
    pub time: PosixTime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PosixDay {
    /// Julian day in a year, no counting for leap days.
    ///
    /// Valid range is `1..=365`.
    JulianOne(i16),
    /// Julian day in a year, counting for leap days.
    ///
    /// Valid range is `0..=365`.
    JulianZero(i16),
    /// The nth weekday of a month.
    WeekdayOfMonth {
        /// The month.
        ///
        /// Valid range is: `1..=12`.
        month: i8,
        /// The week.
        ///
        /// Valid range is `1..=5`.
        ///
        /// One interesting thing to note here (or my interpretation anyway),
        /// is that a week of `4` means the "4th weekday in a month" where as
        /// a week of `5` means the "last weekday in a month, even if it's the
        /// 4th weekday."
        week: i8,
        /// The weekday.
        ///
        /// Valid range is `0..=6`, with `0` corresponding to Sunday.
        weekday: i8,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PosixTime {
    pub second: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PosixOffset {
    pub second: i32,
}

// only-jiff-start
impl TzifStatic {
    pub const fn into_jiff(self) -> crate::tz::tzif::TzifStatic {
        crate::tz::tzif::TzifStatic::from_shared_const(self)
    }
}
// only-jiff-end

// only-jiff-start
impl PosixTimeZone<&'static str> {
    pub const fn into_jiff(self) -> crate::tz::posix::PosixTimeZoneStatic {
        crate::tz::posix::PosixTimeZone::from_shared_const(self)
    }
}
// only-jiff-end

// Does not require `alloc`, but is only used when `alloc` is enabled.
#[cfg(feature = "alloc")]
pub(crate) mod crc32;
pub(crate) mod posix;
#[cfg(feature = "alloc")]
pub(crate) mod tzif;
pub(crate) mod util;
