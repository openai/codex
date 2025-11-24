use crate::{
    civil::{Date, DateTime, Time},
    error::Error,
    tz::{Offset, TimeZone, TimeZoneDatabase},
    util::borrow::StringCow,
    Timestamp, Zoned,
};

/// A low level representation of a parsed Temporal ISO 8601 datetime string.
///
/// Most users should not need to use or care about this type. Its purpose is
/// to represent the individual components of a datetime string for more
/// flexible parsing when use cases call for it.
///
/// One can parse into `Pieces` via [`Pieces::parse`]. Its date, time
/// (optional), offset (optional) and time zone annotation (optional) can be
/// queried independently. Each component corresponds to the following in a
/// datetime string:
///
/// ```text
/// {date}T{time}{offset}[{time-zone-annotation}]
/// ```
///
/// For example:
///
/// ```text
/// 2025-01-03T19:54-05[America/New_York]
/// ```
///
/// A date is the only required component.
///
/// A `Pieces` can also be constructed from structured values via its `From`
/// trait implementations. The `From` trait has the following implementations
/// available:
///
/// * `From<Date>` creates a `Pieces` with just a civil [`Date`]. All other
/// components are left empty.
/// * `From<DateTime>` creates a `Pieces` with a civil [`Date`] and [`Time`].
/// The offset and time zone annotation are left empty.
/// * `From<Timestamp>` creates a `Pieces` from a [`Timestamp`] using
/// a Zulu offset. This signifies that the precise instant is known, but the
/// local time's offset from UTC is unknown. The [`Date`] and [`Time`] are
/// determined via `Offset::UTC.to_datetime(timestamp)`. The time zone
/// annotation is left empty.
/// * `From<(Timestamp, Offset)>` creates a `Pieces` from a [`Timestamp`] and
/// an [`Offset`]. The [`Date`] and [`Time`] are determined via
/// `offset.to_datetime(timestamp)`. The time zone annotation is left empty.
/// * `From<&Zoned>` creates a `Pieces` from a [`Zoned`]. This populates all
/// fields of a `Pieces`.
///
/// A `Pieces` can be converted to a Temporal ISO 8601 string via its `Display`
/// trait implementation.
///
/// # Example: distinguishing between `Z`, `+00:00` and `-00:00`
///
/// With `Pieces`, it's possible to parse a datetime string and inspect the
/// "type" of its offset when it is zero. This makes use of the
/// [`PiecesOffset`] and [`PiecesNumericOffset`] auxiliary types.
///
/// ```
/// use jiff::{
///     fmt::temporal::{Pieces, PiecesNumericOffset, PiecesOffset},
///     tz::Offset,
/// };
///
/// let pieces = Pieces::parse("1970-01-01T00:00:00Z")?;
/// let off = pieces.offset().unwrap();
/// // Parsed as Zulu.
/// assert_eq!(off, PiecesOffset::Zulu);
/// // Gets converted from Zulu to UTC, i.e., just zero.
/// assert_eq!(off.to_numeric_offset(), Offset::UTC);
///
/// let pieces = Pieces::parse("1970-01-01T00:00:00-00:00")?;
/// let off = pieces.offset().unwrap();
/// // Parsed as a negative zero.
/// assert_eq!(off, PiecesOffset::from(
///     PiecesNumericOffset::from(Offset::UTC).with_negative_zero(),
/// ));
/// // Gets converted from -00:00 to UTC, i.e., just zero.
/// assert_eq!(off.to_numeric_offset(), Offset::UTC);
///
/// let pieces = Pieces::parse("1970-01-01T00:00:00+00:00")?;
/// let off = pieces.offset().unwrap();
/// // Parsed as a positive zero.
/// assert_eq!(off, PiecesOffset::from(
///     PiecesNumericOffset::from(Offset::UTC),
/// ));
/// // Gets converted from -00:00 to UTC, i.e., just zero.
/// assert_eq!(off.to_numeric_offset(), Offset::UTC);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// It's rare to need to care about these differences, but the above example
/// demonstrates that `Pieces` doesn't try to do any automatic translation for
/// you.
///
/// # Example: it is very easy to misuse `Pieces`
///
/// This example shows how easily you can shoot yourself in the foot with
/// `Pieces`:
///
/// ```
/// use jiff::{fmt::temporal::{Pieces, TimeZoneAnnotation}, tz};
///
/// let mut pieces = Pieces::parse("2025-01-03T07:55+02[Africa/Cairo]")?;
/// pieces = pieces.with_offset(tz::offset(-10));
/// // This is nonsense because the offset isn't compatible with the time zone!
/// // Moreover, the actual instant that this timestamp represents has changed.
/// assert_eq!(pieces.to_string(), "2025-01-03T07:55:00-10:00[Africa/Cairo]");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// In the above example, we take a parsed `Pieces`, change its offset and
/// then format it back into a string. There are no speed bumps or errors.
/// A `Pieces` will just blindly follow your instruction, even if it produces
/// a nonsense result. Nonsense results are still parsable back into `Pieces`:
///
/// ```
/// use jiff::{civil, fmt::temporal::Pieces, tz::{TimeZone, offset}};
///
/// let pieces = Pieces::parse("2025-01-03T07:55:00-10:00[Africa/Cairo]")?;
/// assert_eq!(pieces.date(), civil::date(2025, 1, 3));
/// assert_eq!(pieces.time(), Some(civil::time(7, 55, 0, 0)));
/// assert_eq!(pieces.to_numeric_offset(), Some(offset(-10)));
/// assert_eq!(pieces.to_time_zone()?, Some(TimeZone::get("Africa/Cairo")?));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// This exemplifies that `Pieces` is a mostly "dumb" type that passes
/// through the data it contains, even if it doesn't make sense.
///
/// # Case study: how to parse `2025-01-03T17:28-05` into `Zoned`
///
/// One thing in particular that `Pieces` enables callers to do is side-step
/// some of the stricter requirements placed on the higher level parsing
/// functions (such as `Zoned`'s `FromStr` trait implementation). For example,
/// parsing a datetime string into a `Zoned` _requires_ that the string contain
/// a time zone annotation. Namely, parsing `2025-01-03T17:28-05` into a
/// `Zoned` will fail:
///
/// ```
/// use jiff::Zoned;
///
/// assert_eq!(
///     "2025-01-03T17:28-05".parse::<Zoned>().unwrap_err().to_string(),
///     "failed to find time zone in square brackets in \
///      \"2025-01-03T17:28-05\", which is required for \
///      parsing a zoned instant",
/// );
/// ```
///
/// The above fails because an RFC 3339 timestamp only contains an offset,
/// not a time zone, and thus the resulting `Zoned` could never do time zone
/// aware arithmetic.
///
/// However, in some cases, you might want to bypass these protections and
/// creat a `Zoned` value with a fixed offset time zone anyway. For example,
/// perhaps your use cases don't need time zone aware arithmetic, but want to
/// preserve the offset anyway. This can be accomplished with `Pieces`:
///
/// ```
/// use jiff::{fmt::temporal::Pieces, tz::TimeZone};
///
/// let pieces = Pieces::parse("2025-01-03T17:28-05")?;
/// let time = pieces.time().unwrap_or_else(jiff::civil::Time::midnight);
/// let dt = pieces.date().to_datetime(time);
/// let Some(offset) = pieces.to_numeric_offset() else {
///     let msg = format!(
///         "datetime string has no offset, \
///          and thus cannot be parsed into an instant",
///     );
///     return Err(msg.into());
/// };
/// let zdt = TimeZone::fixed(offset).to_zoned(dt)?;
/// assert_eq!(zdt.to_string(), "2025-01-03T17:28:00-05:00[-05:00]");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// One problem with the above code snippet is that it completely ignores if
/// a time zone annotation is present. If it is, it probably makes sense to use
/// it, but "fall back" to a fixed offset time zone if it isn't (which the
/// higher level `Zoned` parsing function won't do for you):
///
/// ```
/// use jiff::{fmt::temporal::Pieces, tz::TimeZone};
///
/// let timestamp = "2025-01-02T15:13-05";
///
/// let pieces = Pieces::parse(timestamp)?;
/// let time = pieces.time().unwrap_or_else(jiff::civil::Time::midnight);
/// let dt = pieces.date().to_datetime(time);
/// let tz = match pieces.to_time_zone()? {
///     Some(tz) => tz,
///     None => {
///         let Some(offset) = pieces.to_numeric_offset() else {
///             let msg = format!(
///                 "timestamp `{timestamp}` has no time zone \
///                  or offset, and thus cannot be parsed into \
///                  an instant",
///             );
///             return Err(msg.into());
///         };
///         TimeZone::fixed(offset)
///     }
/// };
/// // We don't bother with offset conflict resolution. And note that
/// // this uses automatic "compatible" disambiguation in the case of
/// // discontinuities. Of course, this is all moot if `TimeZone` is
/// // fixed. The above code handles the case where it isn't!
/// let zdt = tz.to_zoned(dt)?;
/// assert_eq!(zdt.to_string(), "2025-01-02T15:13:00-05:00[-05:00]");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// This is mostly the same as above, but if an annotation is present, we use
/// a `TimeZone` derived from that over the offset present.
///
/// However, this still doesn't quite capture what happens when parsing into a
/// `Zoned` value. In particular, parsing into a `Zoned` is _also_ doing offset
/// conflict resolution for you. An offset conflict occurs when there is a
/// mismatch between the offset in an RFC 3339 timestamp and the time zone in
/// an RFC 9557 time zone annotation.
///
/// For example, `2024-06-14T17:30-05[America/New_York]` has a mismatch
/// since the date is in daylight saving time, but the offset, `-05`, is the
/// offset for standard time in `America/New_York`. If this datetime were
/// fed to the above code, then the `-05` offset would be completely ignored
/// and `America/New_York` would resolve the datetime based on its rules. In
/// this case, you'd get `2024-06-14T17:30-04`, which is a different instant
/// than the original datetime!
///
/// You can either implement your own conflict resolution or use
/// [`tz::OffsetConflict`](crate::tz::OffsetConflict) to do it for you.
///
/// ```
/// use jiff::{fmt::temporal::Pieces, tz::{OffsetConflict, TimeZone}};
///
/// let timestamp = "2024-06-14T17:30-05[America/New_York]";
/// // The default for conflict resolution when parsing into a `Zoned` is
/// // actually `Reject`, but we use `AlwaysOffset` here to show a different
/// // strategy. You'll want to pick the conflict resolution that suits your
/// // needs. The `Reject` strategy is what you should pick if you aren't
/// // sure.
/// let conflict_resolution = OffsetConflict::AlwaysOffset;
///
/// let pieces = Pieces::parse(timestamp)?;
/// let time = pieces.time().unwrap_or_else(jiff::civil::Time::midnight);
/// let dt = pieces.date().to_datetime(time);
/// let ambiguous_zdt = match pieces.to_time_zone()? {
///     Some(tz) => {
///         match pieces.to_numeric_offset() {
///             None => tz.into_ambiguous_zoned(dt),
///             Some(offset) => {
///                 conflict_resolution.resolve(dt, offset, tz)?
///             }
///         }
///     }
///     None => {
///         let Some(offset) = pieces.to_numeric_offset() else {
///             let msg = format!(
///                 "timestamp `{timestamp}` has no time zone \
///                  or offset, and thus cannot be parsed into \
///                  an instant",
///             );
///             return Err(msg.into());
///         };
///         // Won't even be ambiguous, but gets us the same
///         // type as the branch above.
///         TimeZone::fixed(offset).into_ambiguous_zoned(dt)
///     }
/// };
/// // We do compatible disambiguation here like we do in the previous
/// // examples, but you could choose any strategy. As with offset conflict
/// // resolution, if you aren't sure what to pick, a safe choice here would
/// // be `ambiguous_zdt.unambiguous()`, which will return an error if the
/// // datetime is ambiguous in any way. Then, if you ever hit an error, you
/// // can examine the case to see if it should be handled in a different way.
/// let zdt = ambiguous_zdt.compatible()?;
/// // Notice that we now have a different civil time and offset, but the
/// // instant it corresponds to is the same as the one we started with.
/// assert_eq!(zdt.to_string(), "2024-06-14T18:30:00-04:00[America/New_York]");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// The above has effectively completely rebuilt the higher level `Zoned`
/// parsing routine, but with a fallback to a fixed time zone when a time zone
/// annotation is not present.
///
/// # Case study: inferring the time zone of RFC 3339 timestamps
///
/// As [one real world use case details][infer-time-zone], it might be
/// desirable to try and infer the time zone of RFC 3339 timestamps with
/// varying offsets. This might be applicable when:
///
/// * You have out-of-band information, possibly contextual, that indicates
/// the timestamps have to come from a fixed set of time zones.
/// * The time zones have different standard offsets.
/// * You have a specific desire or need to use a [`Zoned`] value for its
/// ergonomics and time zone aware handling. After all, in this case, you
/// believe the timestamps to actually be generated from a specific time zone,
/// but the interchange format doesn't support carrying that information. Or
/// the source data simply omits it.
///
/// In other words, you might be trying to make the best of a bad situation.
///
/// A `Pieces` can help you accomplish this because it gives you access to each
/// component of a parsed datetime, and thus lets you implement arbitrary logic
/// for how to translate that into a `Zoned`. In this case, there is
/// contextual information that Jiff can't possibly know about.
///
/// The general approach we take here is to make use of
/// [`tz::OffsetConflict`](crate::tz::OffsetConflict) to query whether a
/// timestamp has a fixed offset compatible with a particular time zone. And if
/// so, we can _probably_ assume it comes from that time zone. One hitch is
/// that it's possible for the timestamp to be valid for multiple time zones,
/// so we check that as well.
///
/// In the use case linked above, we have fixed offset timestamps from
/// `America/Chicago` and `America/New_York`. So let's try implementing the
/// above strategy. Note that we assume our inputs are RFC 3339 fixed offset
/// timestamps and error otherwise. This is just to keep things simple. To
/// handle data that is more varied, see the previous case study where we
/// respect a time zone annotation if it's present, and fall back to a fixed
/// offset time zone if it isn't.
///
/// ```
/// use jiff::{fmt::temporal::Pieces, tz::{OffsetConflict, TimeZone}, Zoned};
///
/// // The time zones we're allowed to choose from.
/// let tzs = &[
///     TimeZone::get("America/New_York")?,
///     TimeZone::get("America/Chicago")?,
/// ];
///
/// // Here's our data that lacks time zones. The task is to assign a time zone
/// // from `tzs` to each below and convert it to a `Zoned`. If we fail on any
/// // one, then we substitute `None`.
/// let data = &[
///     "2024-01-13T10:33-05",
///     "2024-01-25T12:15-06",
///     "2024-03-10T02:30-05",
///     "2024-06-08T14:01-05",
///     "2024-06-12T11:46-04",
///     "2024-11-03T01:30-05",
/// ];
/// // Our answers.
/// let mut zdts: Vec<Option<Zoned>> = vec![];
/// for string in data {
///     // Parse and gather up the data that we can from the input.
///     // In this case, that's a civil datetime and an offset from UTC.
///     let pieces = Pieces::parse(string)?;
///     let time = pieces.time().unwrap_or_else(jiff::civil::Time::midnight);
///     let dt = pieces.date().to_datetime(time);
///     let Some(offset) = pieces.to_numeric_offset() else {
///         // A robust implementation should use a TZ annotation if present.
///         return Err("missing offset".into());
///     };
///     // Now collect all time zones that are valid for this timestamp.
///     let mut candidates = vec![];
///     for tz in tzs {
///         let result = OffsetConflict::Reject.resolve(dt, offset, tz.clone());
///         // The parsed offset isn't valid for this time zone, so reject it.
///         let Ok(ambiguous_zdt) = result else { continue };
///         // This can never fail because we used the "reject" conflict
///         // resolution strategy. It will never return an ambiguous
///         // `Zoned` since we always have a valid offset that does
///         // disambiguation for us.
///         let zdt = ambiguous_zdt.unambiguous().unwrap();
///         candidates.push(zdt);
///     }
///     if candidates.len() == 1 {
///         zdts.push(Some(candidates.pop().unwrap()));
///     } else {
///         zdts.push(None);
///     }
/// }
/// assert_eq!(zdts, vec![
///     Some("2024-01-13T10:33-05[America/New_York]".parse()?),
///     Some("2024-01-25T12:15-06[America/Chicago]".parse()?),
///     // Failed because the clock time falls in a gap in the
///     // transition to daylight saving time, and it could be
///     // valid for either America/New_York or America/Chicago.
///     None,
///     Some("2024-06-08T14:01-05[America/Chicago]".parse()?),
///     Some("2024-06-12T11:46-04[America/New_York]".parse()?),
///     // Failed because the clock time falls in a fold in the
///     // transition out of daylight saving time, and it could be
///     // valid for either America/New_York or America/Chicago.
///     None,
/// ]);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// The one hitch here is that if the time zones are close to each
/// geographically and both have daylight saving time, then there are some
/// RFC 3339 timestamps that are truly ambiguous. For example,
/// `2024-11-03T01:30-05` is perfectly valid for both `America/New_York` and
/// `America/Chicago`. In this case, there is no way to tell which time zone
/// the timestamp belongs to. It might be reasonable to return an error in
/// this case or omit the timestamp. It depends on what you need to do.
///
/// With more effort, it would also be possible to optimize the above routine
/// by utilizing [`TimeZone::preceding`] and [`TimeZone::following`] to get
/// the exact boundaries of each time zone transition. Then you could use an
/// offset lookup table for each range to determine the appropriate time zone.
///
/// [infer-time-zone]: https://github.com/BurntSushi/jiff/discussions/181#discussioncomment-11729435
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Pieces<'n> {
    date: Date,
    time: Option<Time>,
    offset: Option<PiecesOffset>,
    time_zone_annotation: Option<TimeZoneAnnotation<'n>>,
}

impl<'n> Pieces<'n> {
    /// Parses a Temporal ISO 8601 datetime string into a `Pieces`.
    ///
    /// This is a convenience routine for
    /// [`DateTimeParser::parses_pieces`](crate::fmt::temporal::DateTimeParser::parse_pieces).
    ///
    /// Note that the `Pieces` returned is parameterized by the lifetime of
    /// `input`. This is because it might borrow a sub-slice of `input` for
    /// a time zone annotation name. For example,
    /// `Canada/Yukon` in `2025-01-03T16:42-07[Canada/Yukon]`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil, fmt::temporal::Pieces, tz::TimeZone};
    ///
    /// let pieces = Pieces::parse("2025-01-03T16:42[Canada/Yukon]")?;
    /// assert_eq!(pieces.date(), civil::date(2025, 1, 3));
    /// assert_eq!(pieces.time(), Some(civil::time(16, 42, 0, 0)));
    /// assert_eq!(pieces.to_numeric_offset(), None);
    /// assert_eq!(pieces.to_time_zone()?, Some(TimeZone::get("Canada/Yukon")?));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn parse<I: ?Sized + AsRef<[u8]> + 'n>(
        input: &'n I,
    ) -> Result<Pieces<'n>, Error> {
        let input = input.as_ref();
        super::DEFAULT_DATETIME_PARSER.parse_pieces(input)
    }

    /// Returns the civil date in this `Pieces`.
    ///
    /// Note that every `Pieces` value is guaranteed to have a `Date`.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil, fmt::temporal::Pieces};
    ///
    /// let pieces = Pieces::parse("2025-01-03")?;
    /// assert_eq!(pieces.date(), civil::date(2025, 1, 3));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn date(&self) -> Date {
        self.date
    }

    /// Returns the civil time in this `Pieces`.
    ///
    /// The time component is optional. In
    /// [`DateTimeParser`](crate::fmt::temporal::DateTimeParser), parsing
    /// into types that require a time (like [`DateTime`]) when a time is
    /// missing automatically set the time to midnight. (Or, more precisely,
    /// the first instant of the day.)
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil, fmt::temporal::Pieces, Zoned};
    ///
    /// let pieces = Pieces::parse("2025-01-03T14:49:01")?;
    /// assert_eq!(pieces.date(), civil::date(2025, 1, 3));
    /// assert_eq!(pieces.time(), Some(civil::time(14, 49, 1, 0)));
    ///
    /// // tricksy tricksy, the first instant of 2015-10-18 in Sao Paulo is
    /// // not midnight!
    /// let pieces = Pieces::parse("2015-10-18[America/Sao_Paulo]")?;
    /// // Parsing into pieces just gives us the component parts, so no time:
    /// assert_eq!(pieces.time(), None);
    ///
    /// // But if this uses higher level routines to parse into a `Zoned`,
    /// // then we can see that the missing time implies the first instant
    /// // of the day:
    /// let zdt: Zoned = "2015-10-18[America/Sao_Paulo]".parse()?;
    /// assert_eq!(zdt.time(), jiff::civil::time(1, 0, 0, 0));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn time(&self) -> Option<Time> {
        self.time
    }

    /// Returns the offset in this `Pieces`.
    ///
    /// The offset returned can be infallibly converted to a numeric offset,
    /// i.e., [`Offset`]. But it also includes extra data to indicate whether
    /// a `Z` or a `-00:00` was parsed. (Neither of which are representable by
    /// an `Offset`, which doesn't distinguish between Zulu and UTC and doesn't
    /// represent negative and positive zero differently.)
    ///
    /// # Example
    ///
    /// This example shows how different flavors of `Offset::UTC` can be parsed
    /// and inspected.
    ///
    /// ```
    /// use jiff::{
    ///     fmt::temporal::{Pieces, PiecesNumericOffset, PiecesOffset},
    ///     tz::Offset,
    /// };
    ///
    /// let pieces = Pieces::parse("1970-01-01T00:00:00Z")?;
    /// let off = pieces.offset().unwrap();
    /// // Parsed as Zulu.
    /// assert_eq!(off, PiecesOffset::Zulu);
    /// // Gets converted from Zulu to UTC, i.e., just zero.
    /// assert_eq!(off.to_numeric_offset(), Offset::UTC);
    ///
    /// let pieces = Pieces::parse("1970-01-01T00:00:00-00:00")?;
    /// let off = pieces.offset().unwrap();
    /// // Parsed as a negative zero.
    /// assert_eq!(off, PiecesOffset::from(
    ///     PiecesNumericOffset::from(Offset::UTC).with_negative_zero(),
    /// ));
    /// // Gets converted from -00:00 to UTC, i.e., just zero.
    /// assert_eq!(off.to_numeric_offset(), Offset::UTC);
    ///
    /// let pieces = Pieces::parse("1970-01-01T00:00:00+00:00")?;
    /// let off = pieces.offset().unwrap();
    /// // Parsed as a positive zero.
    /// assert_eq!(off, PiecesOffset::from(
    ///     PiecesNumericOffset::from(Offset::UTC),
    /// ));
    /// // Gets converted from -00:00 to UTC, i.e., just zero.
    /// assert_eq!(off.to_numeric_offset(), Offset::UTC);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn offset(&self) -> Option<PiecesOffset> {
        self.offset
    }

    /// Returns the time zone annotation in this `Pieces`.
    ///
    /// A time zone annotation is optional. The higher level
    /// [`DateTimeParser`](crate::fmt::temporal::DateTimeParser)
    /// requires a time zone annotation when parsing into a [`Zoned`].
    ///
    /// A time zone annotation is either an offset, or more commonly, an IANA
    /// time zone identifier.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{fmt::temporal::{Pieces, TimeZoneAnnotation}, tz::offset};
    ///
    /// // A time zone annotation from a name:
    /// let pieces = Pieces::parse("2025-01-02T16:47-05[America/New_York]")?;
    /// assert_eq!(
    ///     pieces.time_zone_annotation().unwrap(),
    ///     &TimeZoneAnnotation::from("America/New_York"),
    /// );
    ///
    /// // A time zone annotation from an offset:
    /// let pieces = Pieces::parse("2025-01-02T16:47-05[-05:00]")?;
    /// assert_eq!(
    ///     pieces.time_zone_annotation().unwrap(),
    ///     &TimeZoneAnnotation::from(offset(-5)),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn time_zone_annotation(&self) -> Option<&TimeZoneAnnotation<'n>> {
        self.time_zone_annotation.as_ref()
    }

    /// A convenience routine for converting an offset on this `Pieces`,
    /// if present, to a numeric [`Offset`].
    ///
    /// This collapses the offsets `Z`, `-00:00` and `+00:00` all to
    /// [`Offset::UTC`]. If you need to distinguish between them, then use
    /// [`Pieces::offset`].
    ///
    /// # Example
    ///
    /// This example shows how `Z`, `-00:00` and `+00:00` all map to the same
    /// [`Offset`] value:
    ///
    /// ```
    /// use jiff::{fmt::temporal::Pieces, tz::Offset};
    ///
    /// let pieces = Pieces::parse("1970-01-01T00:00:00Z")?;
    /// assert_eq!(pieces.to_numeric_offset(), Some(Offset::UTC));
    ///
    /// let pieces = Pieces::parse("1970-01-01T00:00:00-00:00")?;
    /// assert_eq!(pieces.to_numeric_offset(), Some(Offset::UTC));
    ///
    /// let pieces = Pieces::parse("1970-01-01T00:00:00+00:00")?;
    /// assert_eq!(pieces.to_numeric_offset(), Some(Offset::UTC));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn to_numeric_offset(&self) -> Option<Offset> {
        self.offset().map(|poffset| poffset.to_numeric_offset())
    }

    /// A convenience routine for converting a time zone annotation, if
    /// present, into a [`TimeZone`].
    ///
    /// If no annotation is on this `Pieces`, then this returns `Ok(None)`.
    ///
    /// This may return an error if the time zone annotation is a name and it
    /// couldn't be found in Jiff's global time zone database.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{fmt::temporal::Pieces, tz::{TimeZone, offset}};
    ///
    /// // No time zone annotations means you get `Ok(None)`:
    /// let pieces = Pieces::parse("2025-01-03T17:13-05")?;
    /// assert_eq!(pieces.to_time_zone()?, None);
    ///
    /// // An offset time zone annotation gets you a fixed offset `TimeZone`:
    /// let pieces = Pieces::parse("2025-01-03T17:13-05[-05]")?;
    /// assert_eq!(pieces.to_time_zone()?, Some(TimeZone::fixed(offset(-5))));
    ///
    /// // A time zone annotation name gets you a IANA time zone:
    /// let pieces = Pieces::parse("2025-01-03T17:13-05[America/New_York]")?;
    /// assert_eq!(pieces.to_time_zone()?, Some(TimeZone::get("America/New_York")?));
    ///
    /// // A time zone annotation name that doesn't exist gives you an error:
    /// let pieces = Pieces::parse("2025-01-03T17:13-05[Australia/Bluey]")?;
    /// assert_eq!(
    ///     pieces.to_time_zone().unwrap_err().to_string(),
    ///     "failed to find time zone `Australia/Bluey` in time zone database",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn to_time_zone(&self) -> Result<Option<TimeZone>, Error> {
        self.to_time_zone_with(crate::tz::db())
    }

    /// A convenience routine for converting a time zone annotation, if
    /// present, into a [`TimeZone`] using the given [`TimeZoneDatabase`].
    ///
    /// If no annotation is on this `Pieces`, then this returns `Ok(None)`.
    ///
    /// This may return an error if the time zone annotation is a name and it
    /// couldn't be found in Jiff's global time zone database.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{fmt::temporal::Pieces, tz::TimeZone};
    ///
    /// // A time zone annotation name gets you a IANA time zone:
    /// let pieces = Pieces::parse("2025-01-03T17:13-05[America/New_York]")?;
    /// assert_eq!(
    ///     pieces.to_time_zone_with(jiff::tz::db())?,
    ///     Some(TimeZone::get("America/New_York")?),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn to_time_zone_with(
        &self,
        db: &TimeZoneDatabase,
    ) -> Result<Option<TimeZone>, Error> {
        let Some(ann) = self.time_zone_annotation() else { return Ok(None) };
        ann.to_time_zone_with(db).map(Some)
    }

    /// Set the date on this `Pieces` to the one given.
    ///
    /// A `Date` is the minimal piece of information necessary to create a
    /// `Pieces`. This method will override any previous setting.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil, fmt::temporal::Pieces, Timestamp};
    ///
    /// let pieces = Pieces::from(civil::date(2025, 1, 3));
    /// assert_eq!(pieces.to_string(), "2025-01-03");
    ///
    /// // Alternatively, build a `Pieces` from another data type, and the
    /// // date field will be automatically populated.
    /// let pieces = Pieces::from(Timestamp::from_second(1735930208)?);
    /// assert_eq!(pieces.date(), civil::date(2025, 1, 3));
    /// assert_eq!(pieces.to_string(), "2025-01-03T18:50:08Z");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn with_date(self, date: Date) -> Pieces<'n> {
        Pieces { date, ..self }
    }

    /// Set the time on this `Pieces` to the one given.
    ///
    /// Setting a [`Time`] on `Pieces` is optional. When formatting a
    /// `Pieces` to a string, a missing `Time` may be omitted from the datetime
    /// string in some cases. See [`Pieces::with_offset`] for more details.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{civil, fmt::temporal::Pieces};
    ///
    /// let pieces = Pieces::from(civil::date(2025, 1, 3))
    ///     .with_time(civil::time(13, 48, 0, 0));
    /// assert_eq!(pieces.to_string(), "2025-01-03T13:48:00");
    /// // Alternatively, build a `Pieces` from a `DateTime` directly:
    /// let pieces = Pieces::from(civil::date(2025, 1, 3).at(13, 48, 0, 0));
    /// assert_eq!(pieces.to_string(), "2025-01-03T13:48:00");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn with_time(self, time: Time) -> Pieces<'n> {
        Pieces { time: Some(time), ..self }
    }

    /// Set the offset on this `Pieces` to the one given.
    ///
    /// Setting the offset on `Pieces` is optional.
    ///
    /// The type of offset is polymorphic, and includes anything that can be
    /// infallibly converted into a [`PiecesOffset`]. This includes an
    /// [`Offset`].
    ///
    /// This refers to the offset in the [RFC 3339] component of a Temporal
    /// ISO 8601 datetime string.
    ///
    /// Since a string like `2025-01-03+11` is not valid, if a `Pieces` has
    /// an offset set but no [`Time`] set, then formatting the `Pieces` will
    /// write an explicit `Time` set to midnight.
    ///
    /// Note that this is distinct from [`Pieces::with_time_zone_offset`].
    /// This routine sets the offset on the datetime, while
    /// `Pieces::with_time_zone_offset` sets the offset inside the time zone
    /// annotation. When the timestamp offset and the time zone annotation
    /// offset are both present, then they must be equivalent or else the
    /// datetime string is not a valid Temporal ISO 8601 string. However, a
    /// `Pieces` will let you format a string with mismatching offsets.
    ///
    /// # Example
    ///
    /// This example shows how easily you can shoot yourself in the foot with
    /// this routine:
    ///
    /// ```
    /// use jiff::{fmt::temporal::{Pieces, TimeZoneAnnotation}, tz};
    ///
    /// let mut pieces = Pieces::parse("2025-01-03T07:55+02[+02]")?;
    /// pieces = pieces.with_offset(tz::offset(-10));
    /// // This is nonsense because the offsets don't match!
    /// // And notice also that the instant that this timestamp refers to has
    /// // changed.
    /// assert_eq!(pieces.to_string(), "2025-01-03T07:55:00-10:00[+02:00]");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This exemplifies that `Pieces` is a mostly "dumb" type that passes
    /// through the data it contains, even if it doesn't make sense.
    ///
    /// # Example: changing the offset can change the instant
    ///
    /// Consider this case where a `Pieces` is created directly from a
    /// `Timestamp`, and then the offset is changed.
    ///
    /// ```
    /// use jiff::{fmt::temporal::Pieces, tz, Timestamp};
    ///
    /// let pieces = Pieces::from(Timestamp::UNIX_EPOCH)
    ///     .with_offset(tz::offset(-5));
    /// assert_eq!(pieces.to_string(), "1970-01-01T00:00:00-05:00");
    /// ```
    ///
    /// You might do this naively as a way of printing the timestamp of the
    /// Unix epoch with an offset of `-05` from UTC. But the above does not
    /// correspond to the Unix epoch:
    ///
    /// ```
    /// use jiff::{Timestamp, ToSpan, Unit};
    ///
    /// let ts: Timestamp = "1970-01-01T00:00:00-05:00".parse()?;
    /// assert_eq!(
    ///     ts.since((Unit::Hour, Timestamp::UNIX_EPOCH))?,
    ///     5.hours().fieldwise(),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This further exemplifies how `Pieces` is just a "dumb" type that
    /// passes through the data it contains.
    ///
    /// This specific example is also why `Pieces` has a `From` trait
    /// implementation for `(Timestamp, Offset)`, which correspond more to
    /// what you want:
    ///
    /// ```
    /// use jiff::{fmt::temporal::Pieces, tz, Timestamp};
    ///
    /// let pieces = Pieces::from((Timestamp::UNIX_EPOCH, tz::offset(-5)));
    /// assert_eq!(pieces.to_string(), "1969-12-31T19:00:00-05:00");
    /// ```
    ///
    /// A decent mental model of `Pieces` is that setting fields on `Pieces`
    /// can't change the values in memory of other fields.
    ///
    /// # Example: setting an offset forces a time to be written
    ///
    /// Consider these cases where formatting a `Pieces` won't write a
    /// [`Time`]:
    ///
    /// ```
    /// use jiff::fmt::temporal::Pieces;
    ///
    /// let pieces = Pieces::from(jiff::civil::date(2025, 1, 3));
    /// assert_eq!(pieces.to_string(), "2025-01-03");
    ///
    /// let pieces = Pieces::from(jiff::civil::date(2025, 1, 3))
    ///     .with_time_zone_name("Africa/Cairo");
    /// assert_eq!(pieces.to_string(), "2025-01-03[Africa/Cairo]");
    /// ```
    ///
    /// This works because the resulting strings are valid. In particular, when
    /// one parses a `2025-01-03[Africa/Cairo]` into a `Zoned`, it results in a
    /// time component of midnight automatically (or more precisely, the first
    /// instead of the corresponding day):
    ///
    /// ```
    /// use jiff::{civil::Time, Zoned};
    ///
    /// let zdt: Zoned = "2025-01-03[Africa/Cairo]".parse()?;
    /// assert_eq!(zdt.time(), Time::midnight());
    ///
    /// // tricksy tricksy, the first instant of 2015-10-18 in Sao Paulo is
    /// // not midnight!
    /// let zdt: Zoned = "2015-10-18[America/Sao_Paulo]".parse()?;
    /// assert_eq!(zdt.time(), jiff::civil::time(1, 0, 0, 0));
    /// // This happens because midnight didn't appear on the clocks in
    /// // Sao Paulo on 2015-10-18. So if you try to parse a datetime with
    /// // midnight, automatic disambiguation kicks in and chooses the time
    /// // after the gap automatically:
    /// let zdt: Zoned = "2015-10-18T00:00:00[America/Sao_Paulo]".parse()?;
    /// assert_eq!(zdt.time(), jiff::civil::time(1, 0, 0, 0));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// However, if you have a date and an offset, then since things like
    /// `2025-01-03+10` aren't valid Temporal ISO 8601 datetime strings, the
    /// default midnight time is automatically written:
    ///
    /// ```
    /// use jiff::{fmt::temporal::Pieces, tz};
    ///
    /// let pieces = Pieces::from(jiff::civil::date(2025, 1, 3))
    ///     .with_offset(tz::offset(-5));
    /// assert_eq!(pieces.to_string(), "2025-01-03T00:00:00-05:00");
    ///
    /// let pieces = Pieces::from(jiff::civil::date(2025, 1, 3))
    ///     .with_offset(tz::offset(2))
    ///     .with_time_zone_name("Africa/Cairo");
    /// assert_eq!(pieces.to_string(), "2025-01-03T00:00:00+02:00[Africa/Cairo]");
    /// ```
    ///
    /// # Example: formatting a Zulu or `-00:00` offset
    ///
    /// A [`PiecesOffset`] encapsulates not just a numeric offset, but also
    /// whether a `Z` or a signed zero are used. While it's uncommon to need
    /// this, this permits one to format a `Pieces` using either of these
    /// constructs:
    ///
    /// ```
    /// use jiff::{
    ///     civil,
    ///     fmt::temporal::{Pieces, PiecesNumericOffset, PiecesOffset},
    ///     tz::Offset,
    /// };
    ///
    /// let pieces = Pieces::from(civil::date(1970, 1, 1).at(0, 0, 0, 0))
    ///     .with_offset(Offset::UTC);
    /// assert_eq!(pieces.to_string(), "1970-01-01T00:00:00+00:00");
    ///
    /// let pieces = Pieces::from(civil::date(1970, 1, 1).at(0, 0, 0, 0))
    ///     .with_offset(PiecesOffset::Zulu);
    /// assert_eq!(pieces.to_string(), "1970-01-01T00:00:00Z");
    ///
    /// let pieces = Pieces::from(civil::date(1970, 1, 1).at(0, 0, 0, 0))
    ///     .with_offset(PiecesNumericOffset::from(Offset::UTC).with_negative_zero());
    /// assert_eq!(pieces.to_string(), "1970-01-01T00:00:00-00:00");
    /// ```
    ///
    /// [RFC 3339]: https://www.rfc-editor.org/rfc/rfc3339
    #[inline]
    pub fn with_offset<T: Into<PiecesOffset>>(self, offset: T) -> Pieces<'n> {
        Pieces { offset: Some(offset.into()), ..self }
    }

    /// Sets the time zone annotation on this `Pieces` to the given time zone
    /// name.
    ///
    /// Setting a time zone annotation on `Pieces` is optional.
    ///
    /// This is a convenience routine for using
    /// [`Pieces::with_time_zone_annotation`] with an explicitly constructed
    /// [`TimeZoneAnnotation`] for a time zone name.
    ///
    /// # Example
    ///
    /// This example shows how easily you can shoot yourself in the foot with
    /// this routine:
    ///
    /// ```
    /// use jiff::fmt::temporal::{Pieces, TimeZoneAnnotation};
    ///
    /// let mut pieces = Pieces::parse("2025-01-03T07:55+02[Africa/Cairo]")?;
    /// pieces = pieces.with_time_zone_name("Australia/Bluey");
    /// // This is nonsense because `Australia/Bluey` isn't a valid time zone!
    /// assert_eq!(pieces.to_string(), "2025-01-03T07:55:00+02:00[Australia/Bluey]");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This exemplifies that `Pieces` is a mostly "dumb" type that passes
    /// through the data it contains, even if it doesn't make sense.
    #[inline]
    pub fn with_time_zone_name<'a>(self, name: &'a str) -> Pieces<'a> {
        self.with_time_zone_annotation(TimeZoneAnnotation::from(name))
    }

    /// Sets the time zone annotation on this `Pieces` to the given offset.
    ///
    /// Setting a time zone annotation on `Pieces` is optional.
    ///
    /// This is a convenience routine for using
    /// [`Pieces::with_time_zone_annotation`] with an explicitly constructed
    /// [`TimeZoneAnnotation`] for a time zone offset.
    ///
    /// Note that this is distinct from [`Pieces::with_offset`]. This
    /// routine sets the offset inside the time zone annotation, while
    /// `Pieces::with_offset` sets the offset on the timestamp itself. When the
    /// timestamp offset and the time zone annotation offset are both present,
    /// then they must be equivalent or else the datetime string is not a valid
    /// Temporal ISO 8601 string. However, a `Pieces` will let you format a
    /// string with mismatching offsets.
    ///
    /// # Example
    ///
    /// This example shows how easily you can shoot yourself in the foot with
    /// this routine:
    ///
    /// ```
    /// use jiff::{fmt::temporal::{Pieces, TimeZoneAnnotation}, tz};
    ///
    /// let mut pieces = Pieces::parse("2025-01-03T07:55+02[Africa/Cairo]")?;
    /// pieces = pieces.with_time_zone_offset(tz::offset(-7));
    /// // This is nonsense because the offset `+02` does not match `-07`.
    /// assert_eq!(pieces.to_string(), "2025-01-03T07:55:00+02:00[-07:00]");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This exemplifies that `Pieces` is a mostly "dumb" type that passes
    /// through the data it contains, even if it doesn't make sense.
    #[inline]
    pub fn with_time_zone_offset(self, offset: Offset) -> Pieces<'static> {
        self.with_time_zone_annotation(TimeZoneAnnotation::from(offset))
    }

    /// Returns a new `Pieces` with the given time zone annotation.
    ///
    /// Setting a time zone annotation on `Pieces` is optional.
    ///
    /// You may find it more convenient to use
    /// [`Pieces::with_time_zone_name`] or [`Pieces::with_time_zone_offset`].
    ///
    /// # Example
    ///
    /// This example shows how easily you can shoot yourself in the foot with
    /// this routine:
    ///
    /// ```
    /// use jiff::fmt::temporal::{Pieces, TimeZoneAnnotation};
    ///
    /// let mut pieces = Pieces::parse("2025-01-03T07:55+02[Africa/Cairo]")?;
    /// pieces = pieces.with_time_zone_annotation(
    ///     TimeZoneAnnotation::from("Canada/Yukon"),
    /// );
    /// // This is nonsense because the offset `+02` is never valid for the
    /// // `Canada/Yukon` time zone.
    /// assert_eq!(pieces.to_string(), "2025-01-03T07:55:00+02:00[Canada/Yukon]");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This exemplifies that `Pieces` is a mostly "dumb" type that passes
    /// through the data it contains, even if it doesn't make sense.
    #[inline]
    pub fn with_time_zone_annotation<'a>(
        self,
        ann: TimeZoneAnnotation<'a>,
    ) -> Pieces<'a> {
        Pieces { time_zone_annotation: Some(ann), ..self }
    }

    /// Converts this `Pieces` into an "owned" value whose lifetime is
    /// `'static`.
    ///
    /// Ths "owned" value in this context refers to the time zone annotation
    /// name, if present. For example, `Canada/Yukon` in
    /// `2025-01-03T07:55-07[Canada/Yukon]`. When parsing into a `Pieces`,
    /// the time zone annotation name is borrowed. But callers may find it more
    /// convenient to work with an owned value. By calling this method, the
    /// borrowed string internally will be copied into a new string heap
    /// allocation.
    ///
    /// If `Pieces` doesn't have a time zone annotation, is already owned or
    /// the time zone annotation is an offset, then this is a no-op.
    #[cfg(feature = "alloc")]
    #[inline]
    pub fn into_owned(self) -> Pieces<'static> {
        Pieces {
            date: self.date,
            time: self.time,
            offset: self.offset,
            time_zone_annotation: self
                .time_zone_annotation
                .map(|ann| ann.into_owned()),
        }
    }
}

impl From<Date> for Pieces<'static> {
    #[inline]
    fn from(date: Date) -> Pieces<'static> {
        Pieces { date, time: None, offset: None, time_zone_annotation: None }
    }
}

impl From<DateTime> for Pieces<'static> {
    #[inline]
    fn from(dt: DateTime) -> Pieces<'static> {
        Pieces::from(dt.date()).with_time(dt.time())
    }
}

impl From<Timestamp> for Pieces<'static> {
    #[inline]
    fn from(ts: Timestamp) -> Pieces<'static> {
        let dt = Offset::UTC.to_datetime(ts);
        Pieces::from(dt).with_offset(PiecesOffset::Zulu)
    }
}

impl From<(Timestamp, Offset)> for Pieces<'static> {
    #[inline]
    fn from((ts, offset): (Timestamp, Offset)) -> Pieces<'static> {
        Pieces::from(offset.to_datetime(ts)).with_offset(offset)
    }
}

impl<'a> From<&'a Zoned> for Pieces<'a> {
    #[inline]
    fn from(zdt: &'a Zoned) -> Pieces<'a> {
        let mut pieces =
            Pieces::from(zdt.datetime()).with_offset(zdt.offset());
        if let Some(name) = zdt.time_zone().iana_name() {
            pieces = pieces.with_time_zone_name(name);
        } else {
            pieces = pieces.with_time_zone_offset(zdt.offset());
        }
        pieces
    }
}

impl<'n> core::fmt::Display for Pieces<'n> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::fmt::StdFmtWrite;

        let precision =
            f.precision().map(|p| u8::try_from(p).unwrap_or(u8::MAX));
        super::DateTimePrinter::new()
            .precision(precision)
            .print_pieces(self, StdFmtWrite(f))
            .map_err(|_| core::fmt::Error)
    }
}

/// An offset parsed from a Temporal ISO 8601 datetime string, for use with
/// [`Pieces`].
///
/// One can almost think of this as effectively equivalent to an `Offset`. And
/// indeed, all `PiecesOffset` values can be convert to an `Offset`. However,
/// some offsets in a datetime string have a different connotation that can't
/// be captured by an `Offset`.
///
/// For example, the offsets `Z`, `-00:00` and `+00:00` all map to
/// [`Offset::UTC`] after parsing. However, `Z` and `-00:00` generally
/// indicate that the offset from local time is unknown, where as `+00:00`
/// indicates that the offset from local is _known_ and is zero. This type
/// permits callers to inspect what offset was actually written.
///
/// # Example
///
/// This example shows how one can create Temporal ISO 8601 datetime strings
/// with `+00:00`, `-00:00` or `Z` offsets.
///
/// ```
/// use jiff::{
///     fmt::temporal::{Pieces, PiecesNumericOffset},
///     tz::Offset,
///     Timestamp,
/// };
///
/// // If you create a `Pieces` from a `Timestamp` with a UTC offset,
/// // then this is interpreted as "the offset from UTC is known and is
/// // zero."
/// let pieces = Pieces::from((Timestamp::UNIX_EPOCH, Offset::UTC));
/// assert_eq!(pieces.to_string(), "1970-01-01T00:00:00+00:00");
///
/// // Otherwise, if you create a `Pieces` from just a `Timestamp` with
/// // no offset, then it is interpreted as "the offset from UTC is not
/// // known." Typically, this is rendered with `Z` for "Zulu":
/// let pieces = Pieces::from(Timestamp::UNIX_EPOCH);
/// assert_eq!(pieces.to_string(), "1970-01-01T00:00:00Z");
///
/// // But it might be the case that you want to use `-00:00` instead,
/// // perhaps to conform to some existing convention or legacy
/// // applications that require it:
/// let pieces = Pieces::from(Timestamp::UNIX_EPOCH)
///     .with_offset(
///         PiecesNumericOffset::from(Offset::UTC).with_negative_zero(),
///     );
/// assert_eq!(pieces.to_string(), "1970-01-01T00:00:00-00:00");
/// ```
///
/// Without `Pieces`, it's not otherwise possible to emit a `-00:00` offset.
/// For example,
/// [`DateTimePrinter::print_timestamp`](crate::fmt::temporal::DateTimePrinter::print_timestamp)
/// will always emit `Z`, which is consider semantically identical to `-00:00`
/// by [RFC 9557]. There's no specific use case where it's expected that you
/// should need to write `-00:00` instead of `Z`, but it's conceivable legacy
/// or otherwise inflexible applications might want it. Or perhaps, in some
/// systems, there is a distinction to draw between `Z` and `-00:00`.
///
/// [RFC 9557]: https://www.rfc-editor.org/rfc/rfc9557.html
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum PiecesOffset {
    /// The "Zulu" offset, corresponding to UTC in a context where the offset
    /// for civil time is unknown or unavailable.
    ///
    /// [RFC 9557] defines this as equivalent in semantic meaning to `-00:00`:
    ///
    /// > If the time in UTC is known, but the offset to local time is unknown,
    /// > this can be represented with an offset of `Z`. (The original version
    /// > of this specification provided `-00:00` for this purpose, which is
    /// > not allowed by ISO-8601:2000 and therefore is less interoperable;
    /// > Section 3.3 of RFC 5322 describes a related convention for email,
    /// > which does not have this problem). This differs semantically from an
    /// > offset of `+00:00`, which implies that UTC is the preferred reference
    /// > point for the specified time.
    ///
    /// [RFC 9557]: https://www.rfc-editor.org/rfc/rfc9557
    Zulu,
    /// A specific numeric offset, including whether the parsed sign is
    /// negative.
    ///
    /// The sign is usually redundant, since an `Offset` is itself signed. But
    /// it can be used to distinguish between `+00:00` (`+00` is the preferred
    /// offset) and `-00:00` (`+00` is what should be used, but only because
    /// the offset to local time is not known). Generally speaking, one should
    /// regard `-00:00` as equivalent to `Z`, per RFC 9557.
    Numeric(PiecesNumericOffset),
}

impl PiecesOffset {
    /// Converts this offset to a concrete numeric offset in all cases.
    ///
    /// If this was a `Z` or a `-00:00` offset, then `Offset::UTC` is returned.
    /// In all other cases, the underlying numeric offset is returned as-is.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{
    ///     fmt::temporal::{Pieces, PiecesNumericOffset, PiecesOffset},
    ///     tz::Offset,
    /// };
    ///
    /// let pieces = Pieces::parse("1970-01-01T00:00:00Z")?;
    /// let off = pieces.offset().unwrap();
    /// // Parsed as Zulu.
    /// assert_eq!(off, PiecesOffset::Zulu);
    /// // Gets converted from Zulu to UTC, i.e., just zero.
    /// assert_eq!(off.to_numeric_offset(), Offset::UTC);
    ///
    /// let pieces = Pieces::parse("1970-01-01T00:00:00-00:00")?;
    /// let off = pieces.offset().unwrap();
    /// // Parsed as a negative zero.
    /// assert_eq!(off, PiecesOffset::from(
    ///     PiecesNumericOffset::from(Offset::UTC).with_negative_zero(),
    /// ));
    /// // Gets converted from -00:00 to UTC, i.e., just zero.
    /// assert_eq!(off.to_numeric_offset(), Offset::UTC);
    ///
    /// let pieces = Pieces::parse("1970-01-01T00:00:00+00:00")?;
    /// let off = pieces.offset().unwrap();
    /// // Parsed as a positive zero.
    /// assert_eq!(off, PiecesOffset::from(
    ///     PiecesNumericOffset::from(Offset::UTC),
    /// ));
    /// // Gets converted from -00:00 to UTC, i.e., just zero.
    /// assert_eq!(off.to_numeric_offset(), Offset::UTC);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn to_numeric_offset(&self) -> Offset {
        match *self {
            PiecesOffset::Zulu => Offset::UTC,
            // -00:00 and +00:00 both collapse to zero here.
            PiecesOffset::Numeric(ref noffset) => noffset.offset(),
        }
    }
}

impl From<Offset> for PiecesOffset {
    #[inline]
    fn from(offset: Offset) -> PiecesOffset {
        PiecesOffset::from(PiecesNumericOffset::from(offset))
    }
}

impl From<PiecesNumericOffset> for PiecesOffset {
    #[inline]
    fn from(offset: PiecesNumericOffset) -> PiecesOffset {
        PiecesOffset::Numeric(offset)
    }
}

/// A specific numeric offset, including the sign of the offset, for use with
/// [`Pieces`].
///
/// # Signedness
///
/// The sign attached to this type is usually redundant, since the underlying
/// [`Offset`] is itself signed. But it can be used to distinguish between
/// `+00:00` (`+00` is the preferred offset) and `-00:00` (`+00` is what should
/// be used, but only because the offset to local time is not known). Generally
/// speaking, one should regard `-00:00` as equivalent to `Z`, per [RFC 9557].
///
/// [RFC 9557]: https://www.rfc-editor.org/rfc/rfc9557
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PiecesNumericOffset {
    offset: Offset,
    is_negative: bool,
}

impl PiecesNumericOffset {
    /// Returns the numeric offset.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{
    ///     fmt::temporal::{Pieces, PiecesOffset},
    ///     tz::Offset,
    /// };
    ///
    /// let pieces = Pieces::parse("1970-01-01T00:00:00-05:30")?;
    /// let off = match pieces.offset().unwrap() {
    ///     PiecesOffset::Numeric(off) => off,
    ///     _ => unreachable!(),
    /// };
    /// // This is really only useful if you care that an actual
    /// // numeric offset was written and not, e.g., `Z`. Otherwise,
    /// // you could just use `PiecesOffset::to_numeric_offset`.
    /// assert_eq!(
    ///     off.offset(),
    ///     Offset::from_seconds(-5 * 60 * 60 - 30 * 60).unwrap(),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn offset(&self) -> Offset {
        self.offset
    }

    /// Returns whether the sign of the offset is negative or not.
    ///
    /// When formatting a [`Pieces`] to a string, this is _only_ used to
    /// determine the rendered sign when the [`Offset`] is itself zero. In
    /// all other cases, the sign rendered matches the sign of the `Offset`.
    ///
    /// Since `Offset` does not keep track of a sign when its value is zero,
    /// when using the `From<Offset>` trait implementation for this type,
    /// `is_negative` is always set to `false` when the offset is zero.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{
    ///     fmt::temporal::{Pieces, PiecesOffset},
    ///     tz::Offset,
    /// };
    ///
    /// let pieces = Pieces::parse("1970-01-01T00:00:00-00:00")?;
    /// let off = match pieces.offset().unwrap() {
    ///     PiecesOffset::Numeric(off) => off,
    ///     _ => unreachable!(),
    /// };
    /// // The numeric offset component in this case is
    /// // indistiguisable from `Offset::UTC`. This is
    /// // because an `Offset` does not use different
    /// // representations for negative and positive zero.
    /// assert_eq!(off.offset(), Offset::UTC);
    /// // This is where `is_negative` comes in handy:
    /// assert_eq!(off.is_negative(), true);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn is_negative(&self) -> bool {
        self.is_negative
    }

    /// Sets this numeric offset to use `-00:00` if and only if the offset
    /// is zero.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{
    ///     fmt::temporal::{Pieces, PiecesNumericOffset},
    ///     tz::Offset,
    ///     Timestamp,
    /// };
    ///
    /// // If you create a `Pieces` from a `Timestamp` with a UTC offset,
    /// // then this is interpreted as "the offset from UTC is known and is
    /// // zero."
    /// let pieces = Pieces::from((Timestamp::UNIX_EPOCH, Offset::UTC));
    /// assert_eq!(pieces.to_string(), "1970-01-01T00:00:00+00:00");
    ///
    /// // Otherwise, if you create a `Pieces` from just a `Timestamp` with
    /// // no offset, then it is interpreted as "the offset from UTC is not
    /// // known." Typically, this is rendered with `Z` for "Zulu":
    /// let pieces = Pieces::from(Timestamp::UNIX_EPOCH);
    /// assert_eq!(pieces.to_string(), "1970-01-01T00:00:00Z");
    ///
    /// // But it might be the case that you want to use `-00:00` instead,
    /// // perhaps to conform to some existing convention or legacy
    /// // applications that require it:
    /// let pieces = Pieces::from(Timestamp::UNIX_EPOCH)
    ///     .with_offset(
    ///         PiecesNumericOffset::from(Offset::UTC).with_negative_zero(),
    ///     );
    /// assert_eq!(pieces.to_string(), "1970-01-01T00:00:00-00:00");
    /// ```
    #[inline]
    pub fn with_negative_zero(self) -> PiecesNumericOffset {
        PiecesNumericOffset { is_negative: true, ..self }
    }
}

impl From<Offset> for PiecesNumericOffset {
    #[inline]
    fn from(offset: Offset) -> PiecesNumericOffset {
        // This can of course never return a -00:00 offset, only +00:00.
        PiecesNumericOffset { offset, is_negative: offset.is_negative() }
    }
}

/// An [RFC 9557] time zone annotation, for use with [`Pieces`].
///
/// A time zone annotation is either a time zone name (typically an IANA time
/// zone identifier) like `America/New_York`, or an offset like `-05:00`. This
/// is normally an implementation detail of parsing into a [`Zoned`], but the
/// raw annotation can be accessed via [`Pieces::time_zone_annotation`] after
/// parsing into a [`Pieces`].
///
/// The lifetime parameter refers to the lifetime of the time zone
/// name. The lifetime is static when the time zone annotation is
/// offset or if the name is owned. An owned value can be produced via
/// [`TimeZoneAnnotation::into_owned`] when the `alloc` crate feature is
/// enabled.
///
/// # Construction
///
/// If you're using [`Pieces`], then its [`Pieces::with_time_zone_name`] and
/// [`Pieces::with_time_zone_offset`] methods should absolve you of needing to
/// build values of this type explicitly. But if the need arises, there are
/// `From` impls for `&str` (time zone annotation name) and [`Offset`] (time
/// zone annotation offset) for this type.
///
/// # Example
///
/// ```
/// use jiff::{fmt::temporal::{Pieces, TimeZoneAnnotation}, tz::offset};
///
/// // A time zone annotation from a name:
/// let pieces = Pieces::parse("2025-01-02T16:47-05[America/New_York]")?;
/// assert_eq!(
///     pieces.time_zone_annotation().unwrap(),
///     &TimeZoneAnnotation::from("America/New_York"),
/// );
///
/// // A time zone annotation from an offset:
/// let pieces = Pieces::parse("2025-01-02T16:47-05[-05:00]")?;
/// assert_eq!(
///     pieces.time_zone_annotation().unwrap(),
///     &TimeZoneAnnotation::from(offset(-5)),
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// [RFC 9557]: https://www.rfc-editor.org/rfc/rfc9557.html
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TimeZoneAnnotation<'n> {
    pub(crate) kind: TimeZoneAnnotationKind<'n>,
    /// Whether the annotation is marked as "critical," i.e., with a
    /// `!` prefix. When enabled, it's supposed to make the annotation
    /// un-ignorable.
    ///
    /// This is basically unused. And there's no way for callers to flip this
    /// switch currently. But it can be queried after parsing. Jiff also
    /// doesn't alter its behavior based on this flag. In particular, Jiff
    /// basically always behaves as if `critical` is true.
    pub(crate) critical: bool,
}

impl<'n> TimeZoneAnnotation<'n> {
    /// Returns the "kind" of this annotation. The kind is either a name or an
    /// offset.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::fmt::temporal::{Pieces, TimeZoneAnnotation};
    ///
    /// // A time zone annotation from a name, which doesn't necessarily have
    /// // to point to a valid IANA time zone.
    /// let pieces = Pieces::parse("2025-01-02T16:47-05[Australia/Bluey]")?;
    /// assert_eq!(
    ///     pieces.time_zone_annotation().unwrap(),
    ///     &TimeZoneAnnotation::from("Australia/Bluey"),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn kind(&self) -> &TimeZoneAnnotationKind<'n> {
        &self.kind
    }

    /// Returns true when this time zone is marked as "critical." This occurs
    /// when the time zone annotation is preceded by a `!`. It is meant to
    /// signify that, basically, implementations should error if the annotation
    /// is invalid in some way. And when it's absent, it's left up to the
    /// implementation's discretion about what to do (including silently
    /// ignoring the invalid annotation).
    ///
    /// Generally speaking, Jiff ignores this altogether for time zone
    /// annotations and behaves as if it's always true. But it's exposed here
    /// for callers to query in case it's useful.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::fmt::temporal::{Pieces, TimeZoneAnnotation};
    ///
    /// // not critical
    /// let pieces = Pieces::parse("2025-01-02T16:47-05[Australia/Bluey]")?;
    /// assert_eq!(
    ///     Some(false),
    ///     pieces.time_zone_annotation().map(|a| a.is_critical()),
    /// );
    ///
    /// // critical
    /// let pieces = Pieces::parse("2025-01-02T16:47-05[!Australia/Bluey]")?;
    /// assert_eq!(
    ///     Some(true),
    ///     pieces.time_zone_annotation().map(|a| a.is_critical()),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn is_critical(&self) -> bool {
        self.critical
    }

    /// A convenience routine for converting this annotation into a time zone.
    ///
    /// This can fail if the annotation contains a name that couldn't be found
    /// in the global time zone database. If you need to use something other
    /// than the global time zone database, then use
    /// [`TimeZoneAnnotation::to_time_zone_with`].
    ///
    /// Note that it may be more convenient to use
    /// [`Pieces::to_time_zone`].
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{fmt::temporal::Pieces, tz::TimeZone};
    ///
    /// let pieces = Pieces::parse("2025-01-02T16:47-05[Australia/Tasmania]")?;
    /// let ann = pieces.time_zone_annotation().unwrap();
    /// assert_eq!(
    ///     ann.to_time_zone().unwrap(),
    ///     TimeZone::get("Australia/Tasmania").unwrap(),
    /// );
    ///
    /// let pieces = Pieces::parse("2025-01-02T16:47-05[Australia/Bluey]")?;
    /// let ann = pieces.time_zone_annotation().unwrap();
    /// assert_eq!(
    ///     ann.to_time_zone().unwrap_err().to_string(),
    ///     "failed to find time zone `Australia/Bluey` in time zone database",
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn to_time_zone(&self) -> Result<TimeZone, Error> {
        self.to_time_zone_with(crate::tz::db())
    }

    /// This is like [`TimeZoneAnnotation::to_time_zone`], but permits the
    /// caller to pass in their own time zone database.
    ///
    /// This can fail if the annotation contains a name that couldn't be found
    /// in the global time zone database. If you need to use something other
    /// than the global time zone database, then use
    /// [`TimeZoneAnnotation::to_time_zone_with`].
    ///
    /// Note that it may be more convenient to use
    /// [`Pieces::to_time_zone_with`].
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::{fmt::temporal::Pieces, tz::TimeZone};
    ///
    /// let pieces = Pieces::parse("2025-01-02T16:47-05[Australia/Tasmania]")?;
    /// let ann = pieces.time_zone_annotation().unwrap();
    /// assert_eq!(
    ///     ann.to_time_zone_with(jiff::tz::db()).unwrap(),
    ///     TimeZone::get("Australia/Tasmania").unwrap(),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn to_time_zone_with(
        &self,
        db: &TimeZoneDatabase,
    ) -> Result<TimeZone, Error> {
        // NOTE: We don't currently utilize the critical flag here. Temporal
        // seems to ignore it. It's not quite clear what else we'd do with it,
        // particularly given that we provide a way to do conflict resolution
        // between offsets and time zones.
        let tz = match *self.kind() {
            TimeZoneAnnotationKind::Named(ref name) => {
                db.get(name.as_str())?
            }
            TimeZoneAnnotationKind::Offset(offset) => TimeZone::fixed(offset),
        };
        Ok(tz)
    }

    /// Converts this time zone annotation into an "owned" value whose lifetime
    /// is `'static`.
    ///
    /// If this was already an "owned" value or a time zone annotation offset,
    /// then this is a no-op.
    #[cfg(feature = "alloc")]
    #[inline]
    pub fn into_owned(self) -> TimeZoneAnnotation<'static> {
        TimeZoneAnnotation {
            kind: self.kind.into_owned(),
            critical: self.critical,
        }
    }
}

impl<'n> From<&'n str> for TimeZoneAnnotation<'n> {
    fn from(string: &'n str) -> TimeZoneAnnotation<'n> {
        let kind = TimeZoneAnnotationKind::from(string);
        TimeZoneAnnotation { kind, critical: false }
    }
}

impl From<Offset> for TimeZoneAnnotation<'static> {
    fn from(offset: Offset) -> TimeZoneAnnotation<'static> {
        let kind = TimeZoneAnnotationKind::from(offset);
        TimeZoneAnnotation { kind, critical: false }
    }
}

/// The kind of time zone found in an [RFC 9557] timestamp, for use with
/// [`Pieces`].
///
/// The lifetime parameter refers to the lifetime of the time zone
/// name. The lifetime is static when the time zone annotation is
/// offset or if the name is owned. An owned value can be produced via
/// [`TimeZoneAnnotation::into_owned`] when the `alloc` crate feature is
/// enabled.
///
/// [RFC 9557]: https://www.rfc-editor.org/rfc/rfc9557.html
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum TimeZoneAnnotationKind<'n> {
    /// The time zone annotation is a name, usually an IANA time zone
    /// identifier. For example, `America/New_York`.
    Named(TimeZoneAnnotationName<'n>),
    /// The time zone annotation is an offset. For example, `-05:00`.
    Offset(Offset),
}

impl<'n> TimeZoneAnnotationKind<'n> {
    /// Converts this time zone annotation kind into an "owned" value whose
    /// lifetime is `'static`.
    ///
    /// If this was already an "owned" value or a time zone annotation offset,
    /// then this is a no-op.
    #[cfg(feature = "alloc")]
    #[inline]
    pub fn into_owned(self) -> TimeZoneAnnotationKind<'static> {
        match self {
            TimeZoneAnnotationKind::Named(named) => {
                TimeZoneAnnotationKind::Named(named.into_owned())
            }
            TimeZoneAnnotationKind::Offset(offset) => {
                TimeZoneAnnotationKind::Offset(offset)
            }
        }
    }
}

impl<'n> From<&'n str> for TimeZoneAnnotationKind<'n> {
    fn from(string: &'n str) -> TimeZoneAnnotationKind<'n> {
        let name = TimeZoneAnnotationName::from(string);
        TimeZoneAnnotationKind::Named(name)
    }
}

impl From<Offset> for TimeZoneAnnotationKind<'static> {
    fn from(offset: Offset) -> TimeZoneAnnotationKind<'static> {
        TimeZoneAnnotationKind::Offset(offset)
    }
}

/// A time zone annotation parsed from a datetime string.
///
/// By default, a time zone annotation name borrows its name from the
/// input it was parsed from. When the `alloc` feature is enabled,
/// callers can de-couple the annotation from the parsed input with
/// [`TimeZoneAnnotationName::into_owned`].
///
/// A value of this type is usually found via [`Pieces::time_zone_annotation`],
/// but callers can also construct one via this type's `From<&str>` trait
/// implementation if necessary.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TimeZoneAnnotationName<'n> {
    name: StringCow<'n>,
}

impl<'n> TimeZoneAnnotationName<'n> {
    /// Returns the name of this time zone annotation as a string slice.
    ///
    /// Note that the lifetime of the string slice returned is tied to the
    /// lifetime of this time zone annotation. This may be shorter than the
    /// lifetime of the string, `'n`, in this annotation.
    #[inline]
    pub fn as_str<'a>(&'a self) -> &'a str {
        self.name.as_str()
    }

    /// Converts this time zone annotation name into an "owned" value whose
    /// lifetime is `'static`.
    ///
    /// If this was already an "owned" value, then this is a no-op.
    #[cfg(feature = "alloc")]
    #[inline]
    pub fn into_owned(self) -> TimeZoneAnnotationName<'static> {
        TimeZoneAnnotationName { name: self.name.into_owned() }
    }
}

impl<'n> From<&'n str> for TimeZoneAnnotationName<'n> {
    fn from(string: &'n str) -> TimeZoneAnnotationName<'n> {
        TimeZoneAnnotationName { name: StringCow::from(string) }
    }
}
