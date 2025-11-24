use crate::{
    civil::DateTime,
    error::{err, Error, ErrorContext},
    shared::util::itime::IAmbiguousOffset,
    tz::{Offset, TimeZone},
    Timestamp, Zoned,
};

/// Configuration for resolving ambiguous datetimes in a particular time zone.
///
/// This is useful for specifying how to disambiguate ambiguous datetimes at
/// runtime. For example, as configuration for parsing [`Zoned`] values via
/// [`fmt::temporal::DateTimeParser::disambiguation`](crate::fmt::temporal::DateTimeParser::disambiguation).
///
/// Note that there is no difference in using
/// `Disambiguation::Compatible.disambiguate(ambiguous_timestamp)` and
/// `ambiguous_timestamp.compatible()`. They are equivalent. The purpose of
/// this enum is to expose the disambiguation strategy as a runtime value for
/// configuration purposes.
///
/// The default value is `Disambiguation::Compatible`, which matches the
/// behavior specified in [RFC 5545 (iCalendar)]. Namely, when an ambiguous
/// datetime is found in a fold (the clocks are rolled back), then the earlier
/// time is selected. And when an ambiguous datetime is found in a gap (the
/// clocks are skipped forward), then the later time is selected.
///
/// This enum is non-exhaustive so that other forms of disambiguation may be
/// added in semver compatible releases.
///
/// [RFC 5545 (iCalendar)]: https://datatracker.ietf.org/doc/html/rfc5545
///
/// # Example
///
/// This example shows the default disambiguation mode ("compatible") when
/// given a datetime that falls in a "gap" (i.e., a forwards DST transition).
///
/// ```
/// use jiff::{civil::date, tz};
///
/// let newyork = tz::db().get("America/New_York")?;
/// let ambiguous = newyork.to_ambiguous_zoned(date(2024, 3, 10).at(2, 30, 0, 0));
///
/// // NOTE: This is identical to `ambiguous.compatible()`.
/// let zdt = ambiguous.disambiguate(tz::Disambiguation::Compatible)?;
/// assert_eq!(zdt.datetime(), date(2024, 3, 10).at(3, 30, 0, 0));
/// // In compatible mode, forward transitions select the later
/// // time. In the EST->EDT transition, that's the -04 (EDT) offset.
/// assert_eq!(zdt.offset(), tz::offset(-4));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example: parsing
///
/// This example shows how to set the disambiguation configuration while
/// parsing a [`Zoned`] datetime. In this example, we always prefer the earlier
/// time.
///
/// ```
/// use jiff::{civil::date, fmt::temporal::DateTimeParser, tz};
///
/// static PARSER: DateTimeParser = DateTimeParser::new()
///     .disambiguation(tz::Disambiguation::Earlier);
///
/// let zdt = PARSER.parse_zoned("2024-03-10T02:30[America/New_York]")?;
/// // In earlier mode, forward transitions select the earlier time, unlike
/// // in compatible mode. In this case, that's the pre-DST offset of -05.
/// assert_eq!(zdt.datetime(), date(2024, 3, 10).at(1, 30, 0, 0));
/// assert_eq!(zdt.offset(), tz::offset(-5));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub enum Disambiguation {
    /// In a backward transition, the earlier time is selected. In forward
    /// transition, the later time is selected.
    ///
    /// This is equivalent to [`AmbiguousTimestamp::compatible`] and
    /// [`AmbiguousZoned::compatible`].
    #[default]
    Compatible,
    /// The earlier time is always selected.
    ///
    /// This is equivalent to [`AmbiguousTimestamp::earlier`] and
    /// [`AmbiguousZoned::earlier`].
    Earlier,
    /// The later time is always selected.
    ///
    /// This is equivalent to [`AmbiguousTimestamp::later`] and
    /// [`AmbiguousZoned::later`].
    Later,
    /// When an ambiguous datetime is encountered, this strategy will always
    /// result in an error. This is useful if you need to require datetimes
    /// from users to unambiguously refer to a specific instant.
    ///
    /// This is equivalent to [`AmbiguousTimestamp::unambiguous`] and
    /// [`AmbiguousZoned::unambiguous`].
    Reject,
}

/// A possibly ambiguous [`Offset`].
///
/// An `AmbiguousOffset` is part of both [`AmbiguousTimestamp`] and
/// [`AmbiguousZoned`], which are created by
/// [`TimeZone::to_ambiguous_timestamp`] and
/// [`TimeZone::to_ambiguous_zoned`], respectively.
///
/// When converting a civil datetime in a particular time zone to a precise
/// instant in time (that is, either `Timestamp` or `Zoned`), then the primary
/// thing needed to form a precise instant in time is an [`Offset`]. The
/// problem is that some civil datetimes are ambiguous. That is, some do not
/// exist (because they fall into a gap, where some civil time is skipped),
/// or some are repeated (because they fall into a fold, where some civil time
/// is repeated).
///
/// The purpose of this type is to represent that ambiguity when it occurs.
/// The ambiguity is manifest through the offset choice: it is either the
/// offset _before_ the transition or the offset _after_ the transition. This
/// is true regardless of whether the ambiguity occurs as a result of a gap
/// or a fold.
///
/// It is generally considered very rare to need to inspect values of this
/// type directly. Instead, higher level routines like
/// [`AmbiguousZoned::compatible`] or [`AmbiguousZoned::unambiguous`] will
/// implement a strategy for you.
///
/// # Example
///
/// This example shows how the "compatible" disambiguation strategy is
/// implemented. Recall that the "compatible" strategy chooses the offset
/// corresponding to the civil datetime after a gap, and the offset
/// corresponding to the civil datetime before a gap.
///
/// ```
/// use jiff::{civil::date, tz::{self, AmbiguousOffset}};
///
/// let tz = tz::db().get("America/New_York")?;
/// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
/// let offset = match tz.to_ambiguous_timestamp(dt).offset() {
///     AmbiguousOffset::Unambiguous { offset } => offset,
///     // This is counter-intuitive, but in order to get the civil datetime
///     // *after* the gap, we need to select the offset from *before* the
///     // gap.
///     AmbiguousOffset::Gap { before, .. } => before,
///     AmbiguousOffset::Fold { before, .. } => before,
/// };
/// assert_eq!(offset.to_timestamp(dt)?.to_string(), "2024-03-10T07:30:00Z");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmbiguousOffset {
    /// The offset for a particular civil datetime and time zone is
    /// unambiguous.
    ///
    /// This is the overwhelmingly common case. In general, the only time this
    /// case does not occur is when there is a transition to a different time
    /// zone (rare) or to/from daylight saving time (occurs for 1 hour twice
    /// in year in many geographic locations).
    Unambiguous {
        /// The offset from UTC for the corresponding civil datetime given. The
        /// offset is determined via the relevant time zone data, and in this
        /// case, there is only one possible offset that could be applied to
        /// the given civil datetime.
        offset: Offset,
    },
    /// The offset for a particular civil datetime and time zone is ambiguous
    /// because there is a gap.
    ///
    /// This most commonly occurs when a civil datetime corresponds to an hour
    /// that was "skipped" in a jump to DST (daylight saving time).
    Gap {
        /// The offset corresponding to the time before a gap.
        ///
        /// For example, given a time zone of `America/Los_Angeles`, the offset
        /// for time immediately preceding `2020-03-08T02:00:00` is `-08`.
        before: Offset,
        /// The offset corresponding to the later time in a gap.
        ///
        /// For example, given a time zone of `America/Los_Angeles`, the offset
        /// for time immediately following `2020-03-08T02:59:59` is `-07`.
        after: Offset,
    },
    /// The offset for a particular civil datetime and time zone is ambiguous
    /// because there is a fold.
    ///
    /// This most commonly occurs when a civil datetime corresponds to an hour
    /// that was "repeated" in a jump to standard time from DST (daylight
    /// saving time).
    Fold {
        /// The offset corresponding to the earlier time in a fold.
        ///
        /// For example, given a time zone of `America/Los_Angeles`, the offset
        /// for time on the first `2020-11-01T01:00:00` is `-07`.
        before: Offset,
        /// The offset corresponding to the earlier time in a fold.
        ///
        /// For example, given a time zone of `America/Los_Angeles`, the offset
        /// for time on the second `2020-11-01T01:00:00` is `-08`.
        after: Offset,
    },
}

impl AmbiguousOffset {
    #[inline]
    pub(crate) const fn from_iambiguous_offset_const(
        iaoff: IAmbiguousOffset,
    ) -> AmbiguousOffset {
        match iaoff {
            IAmbiguousOffset::Unambiguous { offset } => {
                let offset = Offset::from_ioffset_const(offset);
                AmbiguousOffset::Unambiguous { offset }
            }
            IAmbiguousOffset::Gap { before, after } => {
                let before = Offset::from_ioffset_const(before);
                let after = Offset::from_ioffset_const(after);
                AmbiguousOffset::Gap { before, after }
            }
            IAmbiguousOffset::Fold { before, after } => {
                let before = Offset::from_ioffset_const(before);
                let after = Offset::from_ioffset_const(after);
                AmbiguousOffset::Fold { before, after }
            }
        }
    }
}

/// A possibly ambiguous [`Timestamp`], created by
/// [`TimeZone::to_ambiguous_timestamp`].
///
/// While this is called an ambiguous _timestamp_, the thing that is
/// actually ambiguous is the offset. That is, an ambiguous timestamp is
/// actually a pair of a [`civil::DateTime`](crate::civil::DateTime) and an
/// [`AmbiguousOffset`].
///
/// When the offset is ambiguous, it either represents a gap (civil time is
/// skipped) or a fold (civil time is repeated). In both cases, there are, by
/// construction, two different offsets to choose from: the offset from before
/// the transition and the offset from after the transition.
///
/// The purpose of this type is to represent that ambiguity (when it occurs)
/// and enable callers to make a choice about how to resolve that ambiguity.
/// In some cases, you might want to reject ambiguity altogether, which is
/// supported by the [`AmbiguousTimestamp::unambiguous`] routine.
///
/// This type provides four different out-of-the-box disambiguation strategies:
///
/// * [`AmbiguousTimestamp::compatible`] implements the
/// [`Disambiguation::Compatible`] strategy. In the case of a gap, the offset
/// after the gap is selected. In the case of a fold, the offset before the
/// fold occurs is selected.
/// * [`AmbiguousTimestamp::earlier`] implements the
/// [`Disambiguation::Earlier`] strategy. This always selects the "earlier"
/// offset.
/// * [`AmbiguousTimestamp::later`] implements the
/// [`Disambiguation::Later`] strategy. This always selects the "later"
/// offset.
/// * [`AmbiguousTimestamp::unambiguous`] implements the
/// [`Disambiguation::Reject`] strategy. It acts as an assertion that the
/// offset is unambiguous. If it is ambiguous, then an appropriate error is
/// returned.
///
/// The [`AmbiguousTimestamp::disambiguate`] method can be used with the
/// [`Disambiguation`] enum when the disambiguation strategy isn't known until
/// runtime.
///
/// Note also that these aren't the only disambiguation strategies. The
/// [`AmbiguousOffset`] type, accessible via [`AmbiguousTimestamp::offset`],
/// exposes the full details of the ambiguity. So any strategy can be
/// implemented.
///
/// # Example
///
/// This example shows how the "compatible" disambiguation strategy is
/// implemented. Recall that the "compatible" strategy chooses the offset
/// corresponding to the civil datetime after a gap, and the offset
/// corresponding to the civil datetime before a gap.
///
/// ```
/// use jiff::{civil::date, tz::{self, AmbiguousOffset}};
///
/// let tz = tz::db().get("America/New_York")?;
/// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
/// let offset = match tz.to_ambiguous_timestamp(dt).offset() {
///     AmbiguousOffset::Unambiguous { offset } => offset,
///     // This is counter-intuitive, but in order to get the civil datetime
///     // *after* the gap, we need to select the offset from *before* the
///     // gap.
///     AmbiguousOffset::Gap { before, .. } => before,
///     AmbiguousOffset::Fold { before, .. } => before,
/// };
/// assert_eq!(offset.to_timestamp(dt)?.to_string(), "2024-03-10T07:30:00Z");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmbiguousTimestamp {
    dt: DateTime,
    offset: AmbiguousOffset,
}

impl AmbiguousTimestamp {
    #[inline]
    pub(crate) fn new(
        dt: DateTime,
        kind: AmbiguousOffset,
    ) -> AmbiguousTimestamp {
        AmbiguousTimestamp { dt, offset: kind }
    }

    /// Returns the civil datetime that was used to create this ambiguous
    /// timestamp.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    /// let dt = date(2024, 7, 10).at(17, 15, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(ts.datetime(), dt);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn datetime(&self) -> DateTime {
        self.dt
    }

    /// Returns the possibly ambiguous offset that is the ultimate source of
    /// ambiguity.
    ///
    /// Most civil datetimes are not ambiguous, and thus, the offset will not
    /// be ambiguous either. In this case, the offset returned will be the
    /// [`AmbiguousOffset::Unambiguous`] variant.
    ///
    /// But, not all civil datetimes are unambiguous. There are exactly two
    /// cases where a civil datetime can be ambiguous: when a civil datetime
    /// does not exist (a gap) or when a civil datetime is repeated (a fold).
    /// In both such cases, the _offset_ is the thing that is ambiguous as
    /// there are two possible choices for the offset in both cases: the offset
    /// before the transition (whether it's a gap or a fold) or the offset
    /// after the transition.
    ///
    /// This type captures the fact that computing an offset from a civil
    /// datetime in a particular time zone is in one of three possible states:
    ///
    /// 1. It is unambiguous.
    /// 2. It is ambiguous because there is a gap in time.
    /// 3. It is ambiguous because there is a fold in time.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz::{self, AmbiguousOffset}};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    ///
    /// // Not ambiguous.
    /// let dt = date(2024, 7, 15).at(17, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(ts.offset(), AmbiguousOffset::Unambiguous {
    ///     offset: tz::offset(-4),
    /// });
    ///
    /// // Ambiguous because of a gap.
    /// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(ts.offset(), AmbiguousOffset::Gap {
    ///     before: tz::offset(-5),
    ///     after: tz::offset(-4),
    /// });
    ///
    /// // Ambiguous because of a fold.
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(ts.offset(), AmbiguousOffset::Fold {
    ///     before: tz::offset(-4),
    ///     after: tz::offset(-5),
    /// });
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn offset(&self) -> AmbiguousOffset {
        self.offset
    }

    /// Returns true if and only if this possibly ambiguous timestamp is
    /// actually ambiguous.
    ///
    /// This occurs precisely in cases when the offset is _not_
    /// [`AmbiguousOffset::Unambiguous`].
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz::{self, AmbiguousOffset}};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    ///
    /// // Not ambiguous.
    /// let dt = date(2024, 7, 15).at(17, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert!(!ts.is_ambiguous());
    ///
    /// // Ambiguous because of a gap.
    /// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert!(ts.is_ambiguous());
    ///
    /// // Ambiguous because of a fold.
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert!(ts.is_ambiguous());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn is_ambiguous(&self) -> bool {
        !matches!(self.offset(), AmbiguousOffset::Unambiguous { .. })
    }

    /// Disambiguates this timestamp according to the
    /// [`Disambiguation::Compatible`] strategy.
    ///
    /// If this timestamp is unambiguous, then this is a no-op.
    ///
    /// The "compatible" strategy selects the offset corresponding to the civil
    /// time after a gap, and the offset corresponding to the civil time before
    /// a fold. This is what is specified in [RFC 5545].
    ///
    /// [RFC 5545]: https://datatracker.ietf.org/doc/html/rfc5545
    ///
    /// # Errors
    ///
    /// This returns an error when the combination of the civil datetime
    /// and offset would lead to a `Timestamp` outside of the
    /// [`Timestamp::MIN`] and [`Timestamp::MAX`] limits. This only occurs
    /// when the civil datetime is "close" to its own [`DateTime::MIN`]
    /// and [`DateTime::MAX`] limits.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    ///
    /// // Not ambiguous.
    /// let dt = date(2024, 7, 15).at(17, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(
    ///     ts.compatible()?.to_string(),
    ///     "2024-07-15T21:30:00Z",
    /// );
    ///
    /// // Ambiguous because of a gap.
    /// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(
    ///     ts.compatible()?.to_string(),
    ///     "2024-03-10T07:30:00Z",
    /// );
    ///
    /// // Ambiguous because of a fold.
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(
    ///     ts.compatible()?.to_string(),
    ///     "2024-11-03T05:30:00Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn compatible(self) -> Result<Timestamp, Error> {
        let offset = match self.offset() {
            AmbiguousOffset::Unambiguous { offset } => offset,
            AmbiguousOffset::Gap { before, .. } => before,
            AmbiguousOffset::Fold { before, .. } => before,
        };
        offset.to_timestamp(self.dt)
    }

    /// Disambiguates this timestamp according to the
    /// [`Disambiguation::Earlier`] strategy.
    ///
    /// If this timestamp is unambiguous, then this is a no-op.
    ///
    /// The "earlier" strategy selects the offset corresponding to the civil
    /// time before a gap, and the offset corresponding to the civil time
    /// before a fold.
    ///
    /// # Errors
    ///
    /// This returns an error when the combination of the civil datetime
    /// and offset would lead to a `Timestamp` outside of the
    /// [`Timestamp::MIN`] and [`Timestamp::MAX`] limits. This only occurs
    /// when the civil datetime is "close" to its own [`DateTime::MIN`]
    /// and [`DateTime::MAX`] limits.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    ///
    /// // Not ambiguous.
    /// let dt = date(2024, 7, 15).at(17, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(
    ///     ts.earlier()?.to_string(),
    ///     "2024-07-15T21:30:00Z",
    /// );
    ///
    /// // Ambiguous because of a gap.
    /// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(
    ///     ts.earlier()?.to_string(),
    ///     "2024-03-10T06:30:00Z",
    /// );
    ///
    /// // Ambiguous because of a fold.
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(
    ///     ts.earlier()?.to_string(),
    ///     "2024-11-03T05:30:00Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn earlier(self) -> Result<Timestamp, Error> {
        let offset = match self.offset() {
            AmbiguousOffset::Unambiguous { offset } => offset,
            AmbiguousOffset::Gap { after, .. } => after,
            AmbiguousOffset::Fold { before, .. } => before,
        };
        offset.to_timestamp(self.dt)
    }

    /// Disambiguates this timestamp according to the
    /// [`Disambiguation::Later`] strategy.
    ///
    /// If this timestamp is unambiguous, then this is a no-op.
    ///
    /// The "later" strategy selects the offset corresponding to the civil
    /// time after a gap, and the offset corresponding to the civil time
    /// after a fold.
    ///
    /// # Errors
    ///
    /// This returns an error when the combination of the civil datetime
    /// and offset would lead to a `Timestamp` outside of the
    /// [`Timestamp::MIN`] and [`Timestamp::MAX`] limits. This only occurs
    /// when the civil datetime is "close" to its own [`DateTime::MIN`]
    /// and [`DateTime::MAX`] limits.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    ///
    /// // Not ambiguous.
    /// let dt = date(2024, 7, 15).at(17, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(
    ///     ts.later()?.to_string(),
    ///     "2024-07-15T21:30:00Z",
    /// );
    ///
    /// // Ambiguous because of a gap.
    /// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(
    ///     ts.later()?.to_string(),
    ///     "2024-03-10T07:30:00Z",
    /// );
    ///
    /// // Ambiguous because of a fold.
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(
    ///     ts.later()?.to_string(),
    ///     "2024-11-03T06:30:00Z",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn later(self) -> Result<Timestamp, Error> {
        let offset = match self.offset() {
            AmbiguousOffset::Unambiguous { offset } => offset,
            AmbiguousOffset::Gap { before, .. } => before,
            AmbiguousOffset::Fold { after, .. } => after,
        };
        offset.to_timestamp(self.dt)
    }

    /// Disambiguates this timestamp according to the
    /// [`Disambiguation::Reject`] strategy.
    ///
    /// If this timestamp is unambiguous, then this is a no-op.
    ///
    /// The "reject" strategy always returns an error when the timestamp
    /// is ambiguous.
    ///
    /// # Errors
    ///
    /// This returns an error when the combination of the civil datetime
    /// and offset would lead to a `Timestamp` outside of the
    /// [`Timestamp::MIN`] and [`Timestamp::MAX`] limits. This only occurs
    /// when the civil datetime is "close" to its own [`DateTime::MIN`]
    /// and [`DateTime::MAX`] limits.
    ///
    /// This also returns an error when the timestamp is ambiguous.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    ///
    /// // Not ambiguous.
    /// let dt = date(2024, 7, 15).at(17, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert_eq!(
    ///     ts.later()?.to_string(),
    ///     "2024-07-15T21:30:00Z",
    /// );
    ///
    /// // Ambiguous because of a gap.
    /// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert!(ts.unambiguous().is_err());
    ///
    /// // Ambiguous because of a fold.
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let ts = tz.to_ambiguous_timestamp(dt);
    /// assert!(ts.unambiguous().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn unambiguous(self) -> Result<Timestamp, Error> {
        let offset = match self.offset() {
            AmbiguousOffset::Unambiguous { offset } => offset,
            AmbiguousOffset::Gap { before, after } => {
                return Err(err!(
                    "the datetime {dt} is ambiguous since it falls into \
                     a gap between offsets {before} and {after}",
                    dt = self.dt,
                ));
            }
            AmbiguousOffset::Fold { before, after } => {
                return Err(err!(
                    "the datetime {dt} is ambiguous since it falls into \
                     a fold between offsets {before} and {after}",
                    dt = self.dt,
                ));
            }
        };
        offset.to_timestamp(self.dt)
    }

    /// Disambiguates this (possibly ambiguous) timestamp into a specific
    /// timestamp.
    ///
    /// This is the same as calling one of the disambiguation methods, but
    /// the method chosen is indicated by the option given. This is useful
    /// when the disambiguation option needs to be chosen at runtime.
    ///
    /// # Errors
    ///
    /// This returns an error if this would have returned a timestamp
    /// outside of its minimum and maximum values.
    ///
    /// This can also return an error when using the [`Disambiguation::Reject`]
    /// strategy. Namely, when using the `Reject` strategy, any ambiguous
    /// timestamp always results in an error.
    ///
    /// # Example
    ///
    /// This example shows the various disambiguation modes when given a
    /// datetime that falls in a "fold" (i.e., a backwards DST transition).
    ///
    /// ```
    /// use jiff::{civil::date, tz::{self, Disambiguation}};
    ///
    /// let newyork = tz::db().get("America/New_York")?;
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let ambiguous = newyork.to_ambiguous_timestamp(dt);
    ///
    /// // In compatible mode, backward transitions select the earlier
    /// // time. In the EDT->EST transition, that's the -04 (EDT) offset.
    /// let ts = ambiguous.clone().disambiguate(Disambiguation::Compatible)?;
    /// assert_eq!(ts.to_string(), "2024-11-03T05:30:00Z");
    ///
    /// // The earlier time in the EDT->EST transition is the -04 (EDT) offset.
    /// let ts = ambiguous.clone().disambiguate(Disambiguation::Earlier)?;
    /// assert_eq!(ts.to_string(), "2024-11-03T05:30:00Z");
    ///
    /// // The later time in the EDT->EST transition is the -05 (EST) offset.
    /// let ts = ambiguous.clone().disambiguate(Disambiguation::Later)?;
    /// assert_eq!(ts.to_string(), "2024-11-03T06:30:00Z");
    ///
    /// // Since our datetime is ambiguous, the 'reject' strategy errors.
    /// assert!(ambiguous.disambiguate(Disambiguation::Reject).is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn disambiguate(
        self,
        option: Disambiguation,
    ) -> Result<Timestamp, Error> {
        match option {
            Disambiguation::Compatible => self.compatible(),
            Disambiguation::Earlier => self.earlier(),
            Disambiguation::Later => self.later(),
            Disambiguation::Reject => self.unambiguous(),
        }
    }

    /// Convert this ambiguous timestamp into an ambiguous zoned date time by
    /// attaching a time zone.
    ///
    /// This is useful when you have a [`civil::DateTime`], [`TimeZone`] and
    /// want to convert it to an instant while applying a particular
    /// disambiguation strategy without an extra clone of the `TimeZone`.
    ///
    /// This isn't currently exposed because I believe use cases for crate
    /// users can be satisfied via [`TimeZone::into_ambiguous_zoned`] (which
    /// is implemented via this routine).
    #[inline]
    pub(crate) fn into_ambiguous_zoned(self, tz: TimeZone) -> AmbiguousZoned {
        AmbiguousZoned::new(self, tz)
    }
}

/// A possibly ambiguous [`Zoned`], created by
/// [`TimeZone::to_ambiguous_zoned`].
///
/// While this is called an ambiguous zoned datetime, the thing that is
/// actually ambiguous is the offset. That is, an ambiguous zoned datetime
/// is actually a triple of a [`civil::DateTime`](crate::civil::DateTime), a
/// [`TimeZone`] and an [`AmbiguousOffset`].
///
/// When the offset is ambiguous, it either represents a gap (civil time is
/// skipped) or a fold (civil time is repeated). In both cases, there are, by
/// construction, two different offsets to choose from: the offset from before
/// the transition and the offset from after the transition.
///
/// The purpose of this type is to represent that ambiguity (when it occurs)
/// and enable callers to make a choice about how to resolve that ambiguity.
/// In some cases, you might want to reject ambiguity altogether, which is
/// supported by the [`AmbiguousZoned::unambiguous`] routine.
///
/// This type provides four different out-of-the-box disambiguation strategies:
///
/// * [`AmbiguousZoned::compatible`] implements the
/// [`Disambiguation::Compatible`] strategy. In the case of a gap, the offset
/// after the gap is selected. In the case of a fold, the offset before the
/// fold occurs is selected.
/// * [`AmbiguousZoned::earlier`] implements the
/// [`Disambiguation::Earlier`] strategy. This always selects the "earlier"
/// offset.
/// * [`AmbiguousZoned::later`] implements the
/// [`Disambiguation::Later`] strategy. This always selects the "later"
/// offset.
/// * [`AmbiguousZoned::unambiguous`] implements the
/// [`Disambiguation::Reject`] strategy. It acts as an assertion that the
/// offset is unambiguous. If it is ambiguous, then an appropriate error is
/// returned.
///
/// The [`AmbiguousZoned::disambiguate`] method can be used with the
/// [`Disambiguation`] enum when the disambiguation strategy isn't known until
/// runtime.
///
/// Note also that these aren't the only disambiguation strategies. The
/// [`AmbiguousOffset`] type, accessible via [`AmbiguousZoned::offset`],
/// exposes the full details of the ambiguity. So any strategy can be
/// implemented.
///
/// # Example
///
/// This example shows how the "compatible" disambiguation strategy is
/// implemented. Recall that the "compatible" strategy chooses the offset
/// corresponding to the civil datetime after a gap, and the offset
/// corresponding to the civil datetime before a gap.
///
/// ```
/// use jiff::{civil::date, tz::{self, AmbiguousOffset}};
///
/// let tz = tz::db().get("America/New_York")?;
/// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
/// let ambiguous = tz.to_ambiguous_zoned(dt);
/// let offset = match ambiguous.offset() {
///     AmbiguousOffset::Unambiguous { offset } => offset,
///     // This is counter-intuitive, but in order to get the civil datetime
///     // *after* the gap, we need to select the offset from *before* the
///     // gap.
///     AmbiguousOffset::Gap { before, .. } => before,
///     AmbiguousOffset::Fold { before, .. } => before,
/// };
/// let zdt = offset.to_timestamp(dt)?.to_zoned(ambiguous.into_time_zone());
/// assert_eq!(zdt.to_string(), "2024-03-10T03:30:00-04:00[America/New_York]");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmbiguousZoned {
    ts: AmbiguousTimestamp,
    tz: TimeZone,
}

impl AmbiguousZoned {
    #[inline]
    fn new(ts: AmbiguousTimestamp, tz: TimeZone) -> AmbiguousZoned {
        AmbiguousZoned { ts, tz }
    }

    /// Returns a reference to the time zone that was used to create this
    /// ambiguous zoned datetime.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    /// let dt = date(2024, 7, 10).at(17, 15, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(&tz, zdt.time_zone());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn time_zone(&self) -> &TimeZone {
        &self.tz
    }

    /// Consumes this ambiguous zoned datetime and returns the underlying
    /// `TimeZone`. This is useful if you no longer need the ambiguous zoned
    /// datetime and want its `TimeZone` without cloning it. (Cloning a
    /// `TimeZone` is cheap but not free.)
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    /// let dt = date(2024, 7, 10).at(17, 15, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(tz, zdt.into_time_zone());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn into_time_zone(self) -> TimeZone {
        self.tz
    }

    /// Returns the civil datetime that was used to create this ambiguous
    /// zoned datetime.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    /// let dt = date(2024, 7, 10).at(17, 15, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(zdt.datetime(), dt);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn datetime(&self) -> DateTime {
        self.ts.datetime()
    }

    /// Returns the possibly ambiguous offset that is the ultimate source of
    /// ambiguity.
    ///
    /// Most civil datetimes are not ambiguous, and thus, the offset will not
    /// be ambiguous either. In this case, the offset returned will be the
    /// [`AmbiguousOffset::Unambiguous`] variant.
    ///
    /// But, not all civil datetimes are unambiguous. There are exactly two
    /// cases where a civil datetime can be ambiguous: when a civil datetime
    /// does not exist (a gap) or when a civil datetime is repeated (a fold).
    /// In both such cases, the _offset_ is the thing that is ambiguous as
    /// there are two possible choices for the offset in both cases: the offset
    /// before the transition (whether it's a gap or a fold) or the offset
    /// after the transition.
    ///
    /// This type captures the fact that computing an offset from a civil
    /// datetime in a particular time zone is in one of three possible states:
    ///
    /// 1. It is unambiguous.
    /// 2. It is ambiguous because there is a gap in time.
    /// 3. It is ambiguous because there is a fold in time.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz::{self, AmbiguousOffset}};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    ///
    /// // Not ambiguous.
    /// let dt = date(2024, 7, 15).at(17, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(zdt.offset(), AmbiguousOffset::Unambiguous {
    ///     offset: tz::offset(-4),
    /// });
    ///
    /// // Ambiguous because of a gap.
    /// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(zdt.offset(), AmbiguousOffset::Gap {
    ///     before: tz::offset(-5),
    ///     after: tz::offset(-4),
    /// });
    ///
    /// // Ambiguous because of a fold.
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(zdt.offset(), AmbiguousOffset::Fold {
    ///     before: tz::offset(-4),
    ///     after: tz::offset(-5),
    /// });
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn offset(&self) -> AmbiguousOffset {
        self.ts.offset
    }

    /// Returns true if and only if this possibly ambiguous zoned datetime is
    /// actually ambiguous.
    ///
    /// This occurs precisely in cases when the offset is _not_
    /// [`AmbiguousOffset::Unambiguous`].
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz::{self, AmbiguousOffset}};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    ///
    /// // Not ambiguous.
    /// let dt = date(2024, 7, 15).at(17, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert!(!zdt.is_ambiguous());
    ///
    /// // Ambiguous because of a gap.
    /// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert!(zdt.is_ambiguous());
    ///
    /// // Ambiguous because of a fold.
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert!(zdt.is_ambiguous());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn is_ambiguous(&self) -> bool {
        !matches!(self.offset(), AmbiguousOffset::Unambiguous { .. })
    }

    /// Disambiguates this zoned datetime according to the
    /// [`Disambiguation::Compatible`] strategy.
    ///
    /// If this zoned datetime is unambiguous, then this is a no-op.
    ///
    /// The "compatible" strategy selects the offset corresponding to the civil
    /// time after a gap, and the offset corresponding to the civil time before
    /// a fold. This is what is specified in [RFC 5545].
    ///
    /// [RFC 5545]: https://datatracker.ietf.org/doc/html/rfc5545
    ///
    /// # Errors
    ///
    /// This returns an error when the combination of the civil datetime
    /// and offset would lead to a `Zoned` with a timestamp outside of the
    /// [`Timestamp::MIN`] and [`Timestamp::MAX`] limits. This only occurs
    /// when the civil datetime is "close" to its own [`DateTime::MIN`]
    /// and [`DateTime::MAX`] limits.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    ///
    /// // Not ambiguous.
    /// let dt = date(2024, 7, 15).at(17, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(
    ///     zdt.compatible()?.to_string(),
    ///     "2024-07-15T17:30:00-04:00[America/New_York]",
    /// );
    ///
    /// // Ambiguous because of a gap.
    /// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(
    ///     zdt.compatible()?.to_string(),
    ///     "2024-03-10T03:30:00-04:00[America/New_York]",
    /// );
    ///
    /// // Ambiguous because of a fold.
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(
    ///     zdt.compatible()?.to_string(),
    ///     "2024-11-03T01:30:00-04:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn compatible(self) -> Result<Zoned, Error> {
        let ts = self.ts.compatible().with_context(|| {
            err!(
                "error converting datetime {dt} to instant in time zone {tz}",
                dt = self.datetime(),
                tz = self.time_zone().diagnostic_name(),
            )
        })?;
        Ok(ts.to_zoned(self.tz))
    }

    /// Disambiguates this zoned datetime according to the
    /// [`Disambiguation::Earlier`] strategy.
    ///
    /// If this zoned datetime is unambiguous, then this is a no-op.
    ///
    /// The "earlier" strategy selects the offset corresponding to the civil
    /// time before a gap, and the offset corresponding to the civil time
    /// before a fold.
    ///
    /// # Errors
    ///
    /// This returns an error when the combination of the civil datetime
    /// and offset would lead to a `Zoned` with a timestamp outside of the
    /// [`Timestamp::MIN`] and [`Timestamp::MAX`] limits. This only occurs
    /// when the civil datetime is "close" to its own [`DateTime::MIN`]
    /// and [`DateTime::MAX`] limits.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    ///
    /// // Not ambiguous.
    /// let dt = date(2024, 7, 15).at(17, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(
    ///     zdt.earlier()?.to_string(),
    ///     "2024-07-15T17:30:00-04:00[America/New_York]",
    /// );
    ///
    /// // Ambiguous because of a gap.
    /// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(
    ///     zdt.earlier()?.to_string(),
    ///     "2024-03-10T01:30:00-05:00[America/New_York]",
    /// );
    ///
    /// // Ambiguous because of a fold.
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(
    ///     zdt.earlier()?.to_string(),
    ///     "2024-11-03T01:30:00-04:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn earlier(self) -> Result<Zoned, Error> {
        let ts = self.ts.earlier().with_context(|| {
            err!(
                "error converting datetime {dt} to instant in time zone {tz}",
                dt = self.datetime(),
                tz = self.time_zone().diagnostic_name(),
            )
        })?;
        Ok(ts.to_zoned(self.tz))
    }

    /// Disambiguates this zoned datetime according to the
    /// [`Disambiguation::Later`] strategy.
    ///
    /// If this zoned datetime is unambiguous, then this is a no-op.
    ///
    /// The "later" strategy selects the offset corresponding to the civil
    /// time after a gap, and the offset corresponding to the civil time
    /// after a fold.
    ///
    /// # Errors
    ///
    /// This returns an error when the combination of the civil datetime
    /// and offset would lead to a `Zoned` with a timestamp outside of the
    /// [`Timestamp::MIN`] and [`Timestamp::MAX`] limits. This only occurs
    /// when the civil datetime is "close" to its own [`DateTime::MIN`]
    /// and [`DateTime::MAX`] limits.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    ///
    /// // Not ambiguous.
    /// let dt = date(2024, 7, 15).at(17, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(
    ///     zdt.later()?.to_string(),
    ///     "2024-07-15T17:30:00-04:00[America/New_York]",
    /// );
    ///
    /// // Ambiguous because of a gap.
    /// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(
    ///     zdt.later()?.to_string(),
    ///     "2024-03-10T03:30:00-04:00[America/New_York]",
    /// );
    ///
    /// // Ambiguous because of a fold.
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(
    ///     zdt.later()?.to_string(),
    ///     "2024-11-03T01:30:00-05:00[America/New_York]",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn later(self) -> Result<Zoned, Error> {
        let ts = self.ts.later().with_context(|| {
            err!(
                "error converting datetime {dt} to instant in time zone {tz}",
                dt = self.datetime(),
                tz = self.time_zone().diagnostic_name(),
            )
        })?;
        Ok(ts.to_zoned(self.tz))
    }

    /// Disambiguates this zoned datetime according to the
    /// [`Disambiguation::Reject`] strategy.
    ///
    /// If this zoned datetime is unambiguous, then this is a no-op.
    ///
    /// The "reject" strategy always returns an error when the zoned datetime
    /// is ambiguous.
    ///
    /// # Errors
    ///
    /// This returns an error when the combination of the civil datetime
    /// and offset would lead to a `Zoned` with a timestamp outside of the
    /// [`Timestamp::MIN`] and [`Timestamp::MAX`] limits. This only occurs
    /// when the civil datetime is "close" to its own [`DateTime::MIN`]
    /// and [`DateTime::MAX`] limits.
    ///
    /// This also returns an error when the timestamp is ambiguous.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil::date, tz};
    ///
    /// let tz = tz::db().get("America/New_York")?;
    ///
    /// // Not ambiguous.
    /// let dt = date(2024, 7, 15).at(17, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert_eq!(
    ///     zdt.later()?.to_string(),
    ///     "2024-07-15T17:30:00-04:00[America/New_York]",
    /// );
    ///
    /// // Ambiguous because of a gap.
    /// let dt = date(2024, 3, 10).at(2, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert!(zdt.unambiguous().is_err());
    ///
    /// // Ambiguous because of a fold.
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let zdt = tz.to_ambiguous_zoned(dt);
    /// assert!(zdt.unambiguous().is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn unambiguous(self) -> Result<Zoned, Error> {
        let ts = self.ts.unambiguous().with_context(|| {
            err!(
                "error converting datetime {dt} to instant in time zone {tz}",
                dt = self.datetime(),
                tz = self.time_zone().diagnostic_name(),
            )
        })?;
        Ok(ts.to_zoned(self.tz))
    }

    /// Disambiguates this (possibly ambiguous) timestamp into a concrete
    /// time zone aware timestamp.
    ///
    /// This is the same as calling one of the disambiguation methods, but
    /// the method chosen is indicated by the option given. This is useful
    /// when the disambiguation option needs to be chosen at runtime.
    ///
    /// # Errors
    ///
    /// This returns an error if this would have returned a zoned datetime
    /// outside of its minimum and maximum values.
    ///
    /// This can also return an error when using the [`Disambiguation::Reject`]
    /// strategy. Namely, when using the `Reject` strategy, any ambiguous
    /// timestamp always results in an error.
    ///
    /// # Example
    ///
    /// This example shows the various disambiguation modes when given a
    /// datetime that falls in a "fold" (i.e., a backwards DST transition).
    ///
    /// ```
    /// use jiff::{civil::date, tz::{self, Disambiguation}};
    ///
    /// let newyork = tz::db().get("America/New_York")?;
    /// let dt = date(2024, 11, 3).at(1, 30, 0, 0);
    /// let ambiguous = newyork.to_ambiguous_zoned(dt);
    ///
    /// // In compatible mode, backward transitions select the earlier
    /// // time. In the EDT->EST transition, that's the -04 (EDT) offset.
    /// let zdt = ambiguous.clone().disambiguate(Disambiguation::Compatible)?;
    /// assert_eq!(
    ///     zdt.to_string(),
    ///     "2024-11-03T01:30:00-04:00[America/New_York]",
    /// );
    ///
    /// // The earlier time in the EDT->EST transition is the -04 (EDT) offset.
    /// let zdt = ambiguous.clone().disambiguate(Disambiguation::Earlier)?;
    /// assert_eq!(
    ///     zdt.to_string(),
    ///     "2024-11-03T01:30:00-04:00[America/New_York]",
    /// );
    ///
    /// // The later time in the EDT->EST transition is the -05 (EST) offset.
    /// let zdt = ambiguous.clone().disambiguate(Disambiguation::Later)?;
    /// assert_eq!(
    ///     zdt.to_string(),
    ///     "2024-11-03T01:30:00-05:00[America/New_York]",
    /// );
    ///
    /// // Since our datetime is ambiguous, the 'reject' strategy errors.
    /// assert!(ambiguous.disambiguate(Disambiguation::Reject).is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn disambiguate(self, option: Disambiguation) -> Result<Zoned, Error> {
        match option {
            Disambiguation::Compatible => self.compatible(),
            Disambiguation::Earlier => self.earlier(),
            Disambiguation::Later => self.later(),
            Disambiguation::Reject => self.unambiguous(),
        }
    }
}
