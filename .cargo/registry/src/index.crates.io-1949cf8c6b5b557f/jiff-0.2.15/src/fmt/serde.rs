/*!
This module provides helpers to use with [Serde].

Some helpers, like those for `Timestamp`, are exposed as modules meant
to be used with Serde's [`with` attribute]. Others, like for `Span` and
`SignedDuration`, only provide serialization helpers to be used with Serde's
[`serialize_with` attribute].

# Module hierarchy

The available helpers can be more quickly understood by looking at a fully
rendered tree of this module's hierarchy. Only the leaves of the tree are
usable with Serde's attributes. For each leaf, the full path is spelled out for
easy copy & paste.

* [`duration`]
    * [`friendly`](self::duration::friendly)
        * [`compact`](self::duration::friendly::compact)
            * [`jiff::fmt::serde::duration::friendly::compact::required`](self::duration::friendly::compact::required)
            * [`jiff::fmt::serde::duration::friendly::compact::optional`](self::duration::friendly::compact::optional)
* [`span`]
    * [`friendly`](self::span::friendly)
        * [`compact`](self::span::friendly::compact)
            * [`jiff::fmt::serde::span::friendly::compact::required`](self::span::friendly::compact::required)
            * [`jiff::fmt::serde::span::friendly::compact::optional`](self::span::friendly::compact::optional)
* [`timestamp`]
    * [`second`](self::timestamp::second)
        * [`jiff::fmt::serde::timestamp::second::required`](self::timestamp::second::required)
        * [`jiff::fmt::serde::timestamp::second::optional`](self::timestamp::second::optional)
    * [`millisecond`](self::timestamp::millisecond)
        * [`jiff::fmt::serde::timestamp::millisecond::required`](self::timestamp::millisecond::required)
        * [`jiff::fmt::serde::timestamp::millisecond::optional`](self::timestamp::millisecond::optional)
    * [`microsecond`](self::timestamp::millisecond)
        * [`jiff::fmt::serde::timestamp::microsecond::required`](self::timestamp::microsecond::required)
        * [`jiff::fmt::serde::timestamp::microsecond::optional`](self::timestamp::microsecond::optional)
    * [`nanosecond`](self::timestamp::millisecond)
        * [`jiff::fmt::serde::timestamp::nanosecond::required`](self::timestamp::nanosecond::required)
        * [`jiff::fmt::serde::timestamp::nanosecond::optional`](self::timestamp::nanosecond::optional)
* [`tz`]
    * [`jiff::fmt::serde::tz::required`](self::tz::required)
    * [`jiff::fmt::serde::tz::optional`](self::tz::optional)

# Example: timestamps as an integer

This example shows how to deserialize an integer number of seconds since the
Unix epoch into a [`Timestamp`](crate::Timestamp). And the reverse operation
for serialization:

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

# Example: optional timestamp support

And this example shows how to use an `Option<Timestamp>` instead of a
`Timestamp`. Note that in this case, we show how to roundtrip the number of
**milliseconds** since the Unix epoch:

```
use jiff::Timestamp;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Record {
    #[serde(with = "jiff::fmt::serde::timestamp::millisecond::optional")]
    timestamp: Option<Timestamp>,
}

let json = r#"{"timestamp":1517644800123}"#;
let got: Record = serde_json::from_str(&json)?;
assert_eq!(got.timestamp, Some(Timestamp::from_millisecond(1517644800_123)?));
assert_eq!(serde_json::to_string(&got)?, json);

# Ok::<(), Box<dyn std::error::Error>>(())
```

# Example: the "friendly" duration format

The [`Span`](crate::Span) and [`SignedDuration`](crate::SignedDuration) types
in this crate both implement Serde's `Serialize` and `Deserialize` traits. For
`Serialize`, they both use the [ISO 8601 Temporal duration format], but for
`Deserialize`, they both support the ISO 8601 Temporal duration format and
the ["friendly" duration format] simultaneously. In order to serialize either
type in the "friendly" format, you can either define your own serialization
functions or use one of the convenience routines provided by this module. For
example:

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
    got.span,
    1.year().months(2).hours(36).milliseconds(1100).fieldwise(),
);

let expected = r#"{"span":"1y 2mo 36h 1100ms"}"#;
assert_eq!(serde_json::to_string(&got).unwrap(), expected);

# Ok::<(), Box<dyn std::error::Error>>(())
```

[Serde]: https://serde.rs/
[`with` attribute]: https://serde.rs/field-attrs.html#with
[`serialize_with` attribute]: https://serde.rs/field-attrs.html#serialize_with
[ISO 8601 Temporal duration format]: crate::fmt::temporal
["friendly" duration format]: crate::fmt::friendly
*/

/// Convenience routines for serializing [`SignedDuration`](crate::SignedDuration)
/// values.
///
/// These convenience routines exist because the `Serialize` implementation for
/// `SignedDuration` always uses the ISO 8601 duration format. These routines
/// provide a way to use the "[friendly](crate::fmt::friendly)" format.
///
/// Only serialization routines are provided because a `SignedDuration`'s
/// `Deserialize` implementation automatically handles both the ISO 8601
/// duration format and the "friendly" format.
///
/// # Advice
///
/// The `Serialize` implementation uses ISO 8601 because it is a widely
/// accepted interchange format for communicating durations. If you need to
/// inter-operate with other systems, it is almost certainly the correct
/// choice.
///
/// The "friendly" format does not adhere to any universal specified format.
/// However, it is perhaps easier to read. Beyond that, its utility for
/// `SignedDuration` is somewhat less compared to [`Span`](crate::Span), since
/// for `Span`, the friendly format preserves all components of the `Span`
/// faithfully. But a `SignedDuration` is just a 96-bit integer of nanoseconds,
/// so there are no individual components to preserve. Still, even with a
/// `SignedDuration`, you might prefer the friendly format.
///
/// # Available routines
///
/// A [`SpanPrinter`](crate::fmt::friendly::SpanPrinter) has a lot of different
/// configuration options. The convenience routines provided by this module
/// only cover a small space of those options since it isn't feasible to
/// provide a convenience routine for every possible set of configuration
/// options.
///
/// While more convenience routines could be added (please file an issue), only
/// the most common or popular such routines can be feasibly added. So in the
/// case where a convenience routine isn't available for the configuration you
/// want, you can very easily define your own `serialize_with` routine.
///
/// The recommended approach is to define a function and a type that
/// implements the `std::fmt::Display` trait. This way, if a serializer can
/// efficiently support `Display` implementations, then an allocation can be
/// avoided.
///
/// ```
/// use jiff::SignedDuration;
///
/// #[derive(Debug, serde::Deserialize, serde::Serialize)]
/// struct Data {
///     #[serde(serialize_with = "custom_friendly")]
///     duration: SignedDuration,
/// }
///
/// let json = r#"{"duration": "36 hours 1100ms"}"#;
/// let got: Data = serde_json::from_str(&json).unwrap();
/// assert_eq!(got.duration, SignedDuration::new(36 * 60 * 60 + 1, 100_000_000));
///
/// let expected = r#"{"duration":"36:00:01.100"}"#;
/// assert_eq!(serde_json::to_string(&got).unwrap(), expected);
///
/// fn custom_friendly<S: serde::Serializer>(
///     duration: &SignedDuration,
///     se: S,
/// ) -> Result<S::Ok, S::Error> {
///     struct Custom<'a>(&'a SignedDuration);
///
///     impl<'a> std::fmt::Display for Custom<'a> {
///         fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
///             use jiff::fmt::{friendly::SpanPrinter, StdFmtWrite};
///
///             static PRINTER: SpanPrinter = SpanPrinter::new()
///                 .hours_minutes_seconds(true)
///                 .precision(Some(3));
///
///             PRINTER
///                 .print_duration(self.0, StdFmtWrite(f))
///                 .map_err(|_| core::fmt::Error)
///         }
///     }
///
///     se.collect_str(&Custom(duration))
/// }
/// ```
///
/// Recall from above that you only need a custom serialization routine
/// for this. Namely, deserialization automatically supports parsing all
/// configuration options for serialization unconditionally.
pub mod duration {
    /// Serialize a `Span` in the [`friendly`](crate::fmt::friendly) duration
    /// format.
    pub mod friendly {
        /// Serialize a `SignedDuration` in the
        /// [`friendly`](crate::fmt::friendly) duration format using compact
        /// designators.
        pub mod compact {
            use crate::fmt::{friendly, StdFmtWrite};

            struct CompactDuration<'a>(&'a crate::SignedDuration);

            impl<'a> core::fmt::Display for CompactDuration<'a> {
                fn fmt(
                    &self,
                    f: &mut core::fmt::Formatter,
                ) -> core::fmt::Result {
                    static PRINTER: friendly::SpanPrinter =
                        friendly::SpanPrinter::new()
                            .designator(friendly::Designator::Compact);
                    PRINTER
                        .print_duration(self.0, StdFmtWrite(f))
                        .map_err(|_| core::fmt::Error)
                }
            }

            /// Serialize a required `SignedDuration` in the [`friendly`]
            /// duration format using compact designators.
            #[inline]
            pub fn required<S: serde::Serializer>(
                duration: &crate::SignedDuration,
                se: S,
            ) -> Result<S::Ok, S::Error> {
                se.collect_str(&CompactDuration(duration))
            }

            /// Serialize an optional `SignedDuration` in the [`friendly`]
            /// duration format using compact designators.
            #[inline]
            pub fn optional<S: serde::Serializer>(
                duration: &Option<crate::SignedDuration>,
                se: S,
            ) -> Result<S::Ok, S::Error> {
                match *duration {
                    None => se.serialize_none(),
                    Some(ref duration) => required(duration, se),
                }
            }
        }
    }
}

/// Convenience routines for serializing [`Span`](crate::Span) values.
///
/// These convenience routines exist because the `Serialize` implementation for
/// `Span` always uses the ISO 8601 duration format. These routines provide a
/// way to use the "[friendly](crate::fmt::friendly)" format.
///
/// Only serialization routines are provided because a `Span`'s `Deserialize`
/// implementation automatically handles both the ISO 8601 duration format and
/// the "friendly" format.
///
/// # Advice
///
/// The `Serialize` implementation uses ISO 8601 because it is a widely
/// accepted interchange format for communicating durations. If you need to
/// inter-operate with other systems, it is almost certainly the correct choice.
///
/// The "friendly" format does not adhere to any universal specified format.
/// However, it is perhaps easier to read, and crucially, unambiguously
/// represents all components of a `Span` faithfully. (In contrast, the ISO
/// 8601 format always normalizes sub-second durations into fractional seconds,
/// which means durations like `1100ms` and `1s100ms` are alwasys considered
/// equivalent.)
///
/// # Available routines
///
/// A [`SpanPrinter`](crate::fmt::friendly::SpanPrinter) has a lot of different
/// configuration options. The convenience routines provided by this module
/// only cover a small space of those options since it isn't feasible to
/// provide a convenience routine for every possible set of configuration
/// options.
///
/// While more convenience routines could be added (please file an issue), only
/// the most common or popular such routines can be feasibly added. So in the
/// case where a convenience routine isn't available for the configuration you
/// want, you can very easily define your own `serialize_with` routine.
///
/// The recommended approach is to define a function and a type that
/// implements the `std::fmt::Display` trait. This way, if a serializer can
/// efficiently support `Display` implementations, then an allocation can be
/// avoided.
///
/// ```
/// use jiff::{Span, ToSpan};
///
/// #[derive(Debug, serde::Deserialize, serde::Serialize)]
/// struct Data {
///     #[serde(serialize_with = "custom_friendly")]
///     duration: Span,
/// }
///
/// let json = r#"{"duration": "1 year 2 months 36 hours 1100ms"}"#;
/// let got: Data = serde_json::from_str(&json).unwrap();
/// assert_eq!(
///     got.duration,
///     1.year().months(2).hours(36).milliseconds(1100).fieldwise(),
/// );
///
/// let expected = r#"{"duration":"1 year, 2 months, 36:00:01.100"}"#;
/// assert_eq!(serde_json::to_string(&got).unwrap(), expected);
///
/// fn custom_friendly<S: serde::Serializer>(
///     span: &Span,
///     se: S,
/// ) -> Result<S::Ok, S::Error> {
///     struct Custom<'a>(&'a Span);
///
///     impl<'a> std::fmt::Display for Custom<'a> {
///         fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
///             use jiff::fmt::{
///                 friendly::{Designator, Spacing, SpanPrinter},
///                 StdFmtWrite,
///             };
///
///             static PRINTER: SpanPrinter = SpanPrinter::new()
///                 .designator(Designator::Verbose)
///                 .comma_after_designator(true)
///                 .spacing(Spacing::BetweenUnitsAndDesignators)
///                 .hours_minutes_seconds(true)
///                 .precision(Some(3));
///
///             PRINTER
///                 .print_span(self.0, StdFmtWrite(f))
///                 .map_err(|_| core::fmt::Error)
///         }
///     }
///
///     se.collect_str(&Custom(span))
/// }
/// ```
///
/// Recall from above that you only need a custom serialization routine
/// for this. Namely, deserialization automatically supports parsing all
/// configuration options for serialization unconditionally.
pub mod span {
    /// Serialize a `Span` in the [`friendly`](crate::fmt::friendly) duration
    /// format.
    pub mod friendly {
        /// Serialize a `Span` in the [`friendly`](crate::fmt::friendly)
        /// duration format using compact designators.
        pub mod compact {
            use crate::fmt::{friendly, StdFmtWrite};

            struct CompactSpan<'a>(&'a crate::Span);

            impl<'a> core::fmt::Display for CompactSpan<'a> {
                fn fmt(
                    &self,
                    f: &mut core::fmt::Formatter,
                ) -> core::fmt::Result {
                    static PRINTER: friendly::SpanPrinter =
                        friendly::SpanPrinter::new()
                            .designator(friendly::Designator::Compact);
                    PRINTER
                        .print_span(self.0, StdFmtWrite(f))
                        .map_err(|_| core::fmt::Error)
                }
            }

            /// Serialize a required `Span` in the [`friendly`] duration format
            /// using compact designators.
            #[inline]
            pub fn required<S: serde::Serializer>(
                span: &crate::Span,
                se: S,
            ) -> Result<S::Ok, S::Error> {
                se.collect_str(&CompactSpan(span))
            }

            /// Serialize an optional `Span` in the [`friendly`] duration
            /// format using compact designators.
            #[inline]
            pub fn optional<S: serde::Serializer>(
                span: &Option<crate::Span>,
                se: S,
            ) -> Result<S::Ok, S::Error> {
                match *span {
                    None => se.serialize_none(),
                    Some(ref span) => required(span, se),
                }
            }
        }
    }
}

/// Convenience routines for (de)serializing [`Timestamp`](crate::Timestamp) as
/// raw integer values.
///
/// At present, the helpers are limited to serializing and deserializing
/// [`Timestamp`](crate::Timestamp) values as an integer number of seconds,
/// milliseconds, microseconds or nanoseconds.
///
/// # Advice
///
/// In general, these helpers should only be used to interface with "legacy"
/// APIs that transmit times as integer number of seconds (or milliseconds or
/// whatever). If you're designing a new API and need to transmit instants in
/// time that don't care about time zones, then you should use `Timestamp`
/// directly. It will automatically use RFC 3339. (And if you do want to
/// include the time zone, then using [`Zoned`](crate::Zoned) directly will
/// work as well by utilizing the RFC 9557 extension to RFC 3339.)
pub mod timestamp {
    use serde::de;

    /// A generic visitor for `Option<Timestamp>`.
    struct OptionalVisitor<V>(V);

    impl<'de, V: de::Visitor<'de, Value = crate::Timestamp>> de::Visitor<'de>
        for OptionalVisitor<V>
    {
        type Value = Option<crate::Timestamp>;

        fn expecting(
            &self,
            f: &mut core::fmt::Formatter,
        ) -> core::fmt::Result {
            f.write_str(
                "an integer number of seconds from the Unix epoch or `None`",
            )
        }

        #[inline]
        fn visit_some<D: de::Deserializer<'de>>(
            self,
            de: D,
        ) -> Result<Option<crate::Timestamp>, D::Error> {
            de.deserialize_i64(self.0).map(Some)
        }

        #[inline]
        fn visit_none<E: de::Error>(
            self,
        ) -> Result<Option<crate::Timestamp>, E> {
            Ok(None)
        }
    }

    /// (De)serialize an integer number of seconds from the Unix epoch.
    pub mod second {
        use serde::de;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = crate::Timestamp;

            fn expecting(
                &self,
                f: &mut core::fmt::Formatter,
            ) -> core::fmt::Result {
                f.write_str("an integer number of seconds from the Unix epoch")
            }

            #[inline]
            fn visit_i8<E: de::Error>(
                self,
                v: i8,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_u8<E: de::Error>(
                self,
                v: u8,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_i16<E: de::Error>(
                self,
                v: i16,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_u16<E: de::Error>(
                self,
                v: u16,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_i32<E: de::Error>(
                self,
                v: i32,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_u32<E: de::Error>(
                self,
                v: u32,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_i64<E: de::Error>(
                self,
                v: i64,
            ) -> Result<crate::Timestamp, E> {
                crate::Timestamp::from_second(v).map_err(de::Error::custom)
            }

            #[inline]
            fn visit_u64<E: de::Error>(
                self,
                v: u64,
            ) -> Result<crate::Timestamp, E> {
                let v = i64::try_from(v).map_err(|_| {
                    de::Error::custom(format_args!(
                        "got unsigned integer {v} seconds, \
                         which is too big to fit in a Jiff `Timestamp`",
                    ))
                })?;
                self.visit_i64(v)
            }

            #[inline]
            fn visit_i128<E: de::Error>(
                self,
                v: i128,
            ) -> Result<crate::Timestamp, E> {
                let v = i64::try_from(v).map_err(|_| {
                    de::Error::custom(format_args!(
                        "got signed integer {v} seconds, \
                         which is too big to fit in a Jiff `Timestamp`",
                    ))
                })?;
                self.visit_i64(v)
            }

            #[inline]
            fn visit_u128<E: de::Error>(
                self,
                v: u128,
            ) -> Result<crate::Timestamp, E> {
                let v = i64::try_from(v).map_err(|_| {
                    de::Error::custom(format_args!(
                        "got unsigned integer {v} seconds, \
                         which is too big to fit in a Jiff `Timestamp`",
                    ))
                })?;
                self.visit_i64(v)
            }
        }

        /// (De)serialize a required integer number of seconds from the Unix
        /// epoch.
        pub mod required {
            /// Serialize a required integer number of seconds since the Unix
            /// epoch.
            #[inline]
            pub fn serialize<S: serde::Serializer>(
                timestamp: &crate::Timestamp,
                se: S,
            ) -> Result<S::Ok, S::Error> {
                se.serialize_i64(timestamp.as_second())
            }

            /// Deserialize a required integer number of seconds since the
            /// Unix epoch.
            #[inline]
            pub fn deserialize<'de, D: serde::Deserializer<'de>>(
                de: D,
            ) -> Result<crate::Timestamp, D::Error> {
                de.deserialize_i64(super::Visitor)
            }
        }

        /// (De)serialize an optional integer number of seconds from the Unix
        /// epoch.
        pub mod optional {
            /// Serialize an optional integer number of seconds since the Unix
            /// epoch.
            #[inline]
            pub fn serialize<S: serde::Serializer>(
                timestamp: &Option<crate::Timestamp>,
                se: S,
            ) -> Result<S::Ok, S::Error> {
                match *timestamp {
                    None => se.serialize_none(),
                    Some(ts) => se.serialize_i64(ts.as_second()),
                }
            }

            /// Deserialize an optional integer number of seconds since the
            /// Unix epoch.
            #[inline]
            pub fn deserialize<'de, D: serde::Deserializer<'de>>(
                de: D,
            ) -> Result<Option<crate::Timestamp>, D::Error> {
                de.deserialize_option(super::super::OptionalVisitor(
                    super::Visitor,
                ))
            }
        }
    }

    /// (De)serialize an integer number of milliseconds from the Unix epoch.
    pub mod millisecond {
        use serde::de;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = crate::Timestamp;

            fn expecting(
                &self,
                f: &mut core::fmt::Formatter,
            ) -> core::fmt::Result {
                f.write_str(
                    "an integer number of milliseconds from the Unix epoch",
                )
            }

            #[inline]
            fn visit_i8<E: de::Error>(
                self,
                v: i8,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_u8<E: de::Error>(
                self,
                v: u8,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_i16<E: de::Error>(
                self,
                v: i16,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_u16<E: de::Error>(
                self,
                v: u16,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_i32<E: de::Error>(
                self,
                v: i32,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_u32<E: de::Error>(
                self,
                v: u32,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_i64<E: de::Error>(
                self,
                v: i64,
            ) -> Result<crate::Timestamp, E> {
                crate::Timestamp::from_millisecond(v)
                    .map_err(de::Error::custom)
            }

            #[inline]
            fn visit_u64<E: de::Error>(
                self,
                v: u64,
            ) -> Result<crate::Timestamp, E> {
                let v = i64::try_from(v).map_err(|_| {
                    de::Error::custom(format_args!(
                        "got unsigned integer {v} milliseconds, \
                         which is too big to fit in a Jiff `Timestamp`",
                    ))
                })?;
                self.visit_i64(v)
            }

            #[inline]
            fn visit_i128<E: de::Error>(
                self,
                v: i128,
            ) -> Result<crate::Timestamp, E> {
                let v = i64::try_from(v).map_err(|_| {
                    de::Error::custom(format_args!(
                        "got signed integer {v} milliseconds, \
                         which is too big to fit in a Jiff `Timestamp`",
                    ))
                })?;
                self.visit_i64(v)
            }

            #[inline]
            fn visit_u128<E: de::Error>(
                self,
                v: u128,
            ) -> Result<crate::Timestamp, E> {
                let v = i64::try_from(v).map_err(|_| {
                    de::Error::custom(format_args!(
                        "got unsigned integer {v} milliseconds, \
                         which is too big to fit in a Jiff `Timestamp`",
                    ))
                })?;
                self.visit_i64(v)
            }
        }

        /// (De)serialize a required integer number of milliseconds from the
        /// Unix epoch.
        pub mod required {
            /// Serialize a required integer number of milliseconds since the
            /// Unix epoch.
            #[inline]
            pub fn serialize<S: serde::Serializer>(
                timestamp: &crate::Timestamp,
                se: S,
            ) -> Result<S::Ok, S::Error> {
                se.serialize_i64(timestamp.as_millisecond())
            }

            /// Deserialize a required integer number of milliseconds since the
            /// Unix epoch.
            #[inline]
            pub fn deserialize<'de, D: serde::Deserializer<'de>>(
                de: D,
            ) -> Result<crate::Timestamp, D::Error> {
                de.deserialize_i64(super::Visitor)
            }
        }

        /// (De)serialize an optional integer number of milliseconds from the
        /// Unix epoch.
        pub mod optional {
            /// Serialize an optional integer number of milliseconds since the
            /// Unix epoch.
            #[inline]
            pub fn serialize<S: serde::Serializer>(
                timestamp: &Option<crate::Timestamp>,
                se: S,
            ) -> Result<S::Ok, S::Error> {
                match *timestamp {
                    None => se.serialize_none(),
                    Some(ts) => se.serialize_i64(ts.as_millisecond()),
                }
            }

            /// Deserialize an optional integer number of milliseconds since
            /// the Unix epoch.
            #[inline]
            pub fn deserialize<'de, D: serde::Deserializer<'de>>(
                de: D,
            ) -> Result<Option<crate::Timestamp>, D::Error> {
                de.deserialize_option(super::super::OptionalVisitor(
                    super::Visitor,
                ))
            }
        }
    }

    /// (De)serialize an integer number of microseconds from the Unix epoch.
    pub mod microsecond {
        use serde::de;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = crate::Timestamp;

            fn expecting(
                &self,
                f: &mut core::fmt::Formatter,
            ) -> core::fmt::Result {
                f.write_str(
                    "an integer number of microseconds from the Unix epoch",
                )
            }

            #[inline]
            fn visit_i8<E: de::Error>(
                self,
                v: i8,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_u8<E: de::Error>(
                self,
                v: u8,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_i16<E: de::Error>(
                self,
                v: i16,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_u16<E: de::Error>(
                self,
                v: u16,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_i32<E: de::Error>(
                self,
                v: i32,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_u32<E: de::Error>(
                self,
                v: u32,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i64(i64::from(v))
            }

            #[inline]
            fn visit_i64<E: de::Error>(
                self,
                v: i64,
            ) -> Result<crate::Timestamp, E> {
                crate::Timestamp::from_microsecond(v)
                    .map_err(de::Error::custom)
            }

            #[inline]
            fn visit_u64<E: de::Error>(
                self,
                v: u64,
            ) -> Result<crate::Timestamp, E> {
                let v = i64::try_from(v).map_err(|_| {
                    de::Error::custom(format_args!(
                        "got unsigned integer {v} microseconds, \
                         which is too big to fit in a Jiff `Timestamp`",
                    ))
                })?;
                self.visit_i64(v)
            }

            #[inline]
            fn visit_i128<E: de::Error>(
                self,
                v: i128,
            ) -> Result<crate::Timestamp, E> {
                let v = i64::try_from(v).map_err(|_| {
                    de::Error::custom(format_args!(
                        "got signed integer {v} microseconds, \
                         which is too big to fit in a Jiff `Timestamp`",
                    ))
                })?;
                self.visit_i64(v)
            }

            #[inline]
            fn visit_u128<E: de::Error>(
                self,
                v: u128,
            ) -> Result<crate::Timestamp, E> {
                let v = i64::try_from(v).map_err(|_| {
                    de::Error::custom(format_args!(
                        "got unsigned integer {v} microseconds, \
                         which is too big to fit in a Jiff `Timestamp`",
                    ))
                })?;
                self.visit_i64(v)
            }
        }

        /// (De)serialize a required integer number of microseconds from the
        /// Unix epoch.
        pub mod required {
            /// Serialize a required integer number of microseconds since the
            /// Unix epoch.
            #[inline]
            pub fn serialize<S: serde::Serializer>(
                timestamp: &crate::Timestamp,
                se: S,
            ) -> Result<S::Ok, S::Error> {
                se.serialize_i64(timestamp.as_microsecond())
            }

            /// Deserialize a required integer number of microseconds since the
            /// Unix epoch.
            #[inline]
            pub fn deserialize<'de, D: serde::Deserializer<'de>>(
                de: D,
            ) -> Result<crate::Timestamp, D::Error> {
                de.deserialize_i64(super::Visitor)
            }
        }

        /// (De)serialize an optional integer number of microseconds from the
        /// Unix epoch.
        pub mod optional {
            /// Serialize an optional integer number of microseconds since the
            /// Unix epoch.
            #[inline]
            pub fn serialize<S: serde::Serializer>(
                timestamp: &Option<crate::Timestamp>,
                se: S,
            ) -> Result<S::Ok, S::Error> {
                match *timestamp {
                    None => se.serialize_none(),
                    Some(ts) => se.serialize_i64(ts.as_microsecond()),
                }
            }

            /// Deserialize an optional integer number of microseconds since
            /// the Unix epoch.
            #[inline]
            pub fn deserialize<'de, D: serde::Deserializer<'de>>(
                de: D,
            ) -> Result<Option<crate::Timestamp>, D::Error> {
                de.deserialize_option(super::super::OptionalVisitor(
                    super::Visitor,
                ))
            }
        }
    }

    /// (De)serialize an integer number of nanoseconds from the Unix epoch.
    pub mod nanosecond {
        use serde::de;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = crate::Timestamp;

            fn expecting(
                &self,
                f: &mut core::fmt::Formatter,
            ) -> core::fmt::Result {
                f.write_str(
                    "an integer number of nanoseconds from the Unix epoch",
                )
            }

            #[inline]
            fn visit_i64<E: de::Error>(
                self,
                v: i64,
            ) -> Result<crate::Timestamp, E> {
                self.visit_i128(i128::from(v))
            }

            #[inline]
            fn visit_u64<E: de::Error>(
                self,
                v: u64,
            ) -> Result<crate::Timestamp, E> {
                self.visit_u128(u128::from(v))
            }

            #[inline]
            fn visit_i128<E: de::Error>(
                self,
                v: i128,
            ) -> Result<crate::Timestamp, E> {
                crate::Timestamp::from_nanosecond(v).map_err(de::Error::custom)
            }

            #[inline]
            fn visit_u128<E: de::Error>(
                self,
                v: u128,
            ) -> Result<crate::Timestamp, E> {
                let v = i128::try_from(v).map_err(|_| {
                    de::Error::custom(format_args!(
                        "got unsigned integer {v} nanoseconds, \
                         which is too big to fit in a Jiff `Timestamp`",
                    ))
                })?;
                self.visit_i128(v)
            }
        }

        /// (De)serialize a required integer number of nanoseconds from the
        /// Unix epoch.
        pub mod required {
            /// Serialize a required integer number of nanoseconds since the
            /// Unix epoch.
            #[inline]
            pub fn serialize<S: serde::Serializer>(
                timestamp: &crate::Timestamp,
                se: S,
            ) -> Result<S::Ok, S::Error> {
                se.serialize_i128(timestamp.as_nanosecond())
            }

            /// Deserialize a required integer number of nanoseconds since the
            /// Unix epoch.
            #[inline]
            pub fn deserialize<'de, D: serde::Deserializer<'de>>(
                de: D,
            ) -> Result<crate::Timestamp, D::Error> {
                de.deserialize_i128(super::Visitor)
            }
        }

        /// (De)serialize an optional integer number of nanoseconds from the
        /// Unix epoch.
        pub mod optional {
            /// Serialize an optional integer number of nanoseconds since the
            /// Unix epoch.
            #[inline]
            pub fn serialize<S: serde::Serializer>(
                timestamp: &Option<crate::Timestamp>,
                se: S,
            ) -> Result<S::Ok, S::Error> {
                match *timestamp {
                    None => se.serialize_none(),
                    Some(ts) => se.serialize_i128(ts.as_nanosecond()),
                }
            }

            /// Deserialize an optional integer number of nanoseconds since the
            /// Unix epoch.
            #[inline]
            pub fn deserialize<'de, D: serde::Deserializer<'de>>(
                de: D,
            ) -> Result<Option<crate::Timestamp>, D::Error> {
                de.deserialize_option(super::super::OptionalVisitor(
                    super::Visitor,
                ))
            }
        }
    }
}

/// Convenience routines for (de)serializing [`TimeZone`](crate::tz::TimeZone)
/// values.
///
/// The `required` and `optional` sub-modules each provide serialization and
/// deserialization routines. They are meant to be used with Serde's
/// [`with` attribute].
///
/// # Advice
///
/// Serializing time zones is useful when you want to accept user configuration
/// selecting a time zone to use. This might be beneficial when one cannot rely
/// on a system's time zone.
///
/// Note that when deserializing time zones that are IANA time zone
/// identifiers, Jiff will automatically use the implicit global database to
/// resolve the identifier to an actual time zone. If you do not want to use
/// Jiff's global time zone database for this, you'll need to write your own
/// Serde integration.
///
/// [`with` attribute]: https://serde.rs/field-attrs.html#with
///
/// # Example
///
/// ```
/// use jiff::tz::TimeZone;
///
/// #[derive(Debug, serde::Deserialize, serde::Serialize)]
/// struct Record {
///     #[serde(with = "jiff::fmt::serde::tz::required")]
///     tz: TimeZone,
/// }
///
/// let json = r#"{"tz":"America/Nuuk"}"#;
/// let got: Record = serde_json::from_str(&json)?;
/// assert_eq!(got.tz, TimeZone::get("America/Nuuk")?);
/// assert_eq!(serde_json::to_string(&got)?, json);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example: serializing an unknown `TimeZone` works
///
/// For example, when a time zone was created from
/// [`TimeZone::system`](crate::tz::TimeZone::system) and a system configured
/// time zone could not be found. One can artifically create this situation
/// with [`TimeZone::unknown`](crate::tz::TimeZone::unknown):
///
/// ```
/// use jiff::tz::TimeZone;
///
/// #[derive(Debug, serde::Deserialize, serde::Serialize)]
/// struct Record {
///     #[serde(with = "jiff::fmt::serde::tz::required")]
///     tz: TimeZone,
/// }
///
/// let record = Record { tz: TimeZone::unknown() };
/// assert_eq!(
///     serde_json::to_string(&record)?,
///     r#"{"tz":"Etc/Unknown"}"#,
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// And it deserializes as well:
///
/// ```
/// use jiff::tz::TimeZone;
///
/// #[derive(Debug, serde::Deserialize, serde::Serialize)]
/// struct Record {
///     #[serde(with = "jiff::fmt::serde::tz::required")]
///     tz: TimeZone,
/// }
///
/// let json = r#"{"tz":"Etc/Unknown"}"#;
/// let got: Record = serde_json::from_str(&json)?;
/// assert!(got.tz.is_unknown());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// An unknown time zone is "allowed" to percolate through Jiff because it's
/// usually not desirable to return an error and completely fail if a system
/// time zone could not be detected. On the other hand, by using a special
/// `Etc/Unknown` identifier for this case, it still surfaces the fact that
/// something has gone wrong.
pub mod tz {
    use serde::de;

    use crate::fmt::{temporal, StdFmtWrite};

    struct TemporalTimeZone<'a>(&'a crate::tz::TimeZone);

    impl<'a> core::fmt::Display for TemporalTimeZone<'a> {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            static PRINTER: temporal::DateTimePrinter =
                temporal::DateTimePrinter::new();
            PRINTER
                .print_time_zone(self.0, StdFmtWrite(f))
                .map_err(|_| core::fmt::Error)
        }
    }

    /// A required visitor for `TimeZone`.
    struct Visitor;

    impl<'de> de::Visitor<'de> for Visitor {
        type Value = crate::tz::TimeZone;

        fn expecting(
            &self,
            f: &mut core::fmt::Formatter,
        ) -> core::fmt::Result {
            f.write_str(
                "a string representing a time zone via an \
                 IANA time zone identifier, fixed offset from UTC \
                 or a POSIX time zone string",
            )
        }

        #[inline]
        fn visit_bytes<E: de::Error>(
            self,
            value: &[u8],
        ) -> Result<crate::tz::TimeZone, E> {
            static PARSER: temporal::DateTimeParser =
                temporal::DateTimeParser::new();
            PARSER.parse_time_zone(value).map_err(de::Error::custom)
        }

        #[inline]
        fn visit_str<E: de::Error>(
            self,
            value: &str,
        ) -> Result<crate::tz::TimeZone, E> {
            self.visit_bytes(value.as_bytes())
        }
    }

    /// A generic optional visitor for `TimeZone`.
    struct OptionalVisitor<V>(V);

    impl<'de, V: de::Visitor<'de, Value = crate::tz::TimeZone>>
        de::Visitor<'de> for OptionalVisitor<V>
    {
        type Value = Option<crate::tz::TimeZone>;

        fn expecting(
            &self,
            f: &mut core::fmt::Formatter,
        ) -> core::fmt::Result {
            f.write_str(
                "a string representing a time zone via an \
                 IANA time zone identifier, fixed offset from UTC \
                 or a POSIX time zone string",
            )
        }

        #[inline]
        fn visit_some<D: de::Deserializer<'de>>(
            self,
            de: D,
        ) -> Result<Option<crate::tz::TimeZone>, D::Error> {
            de.deserialize_str(self.0).map(Some)
        }

        #[inline]
        fn visit_none<E: de::Error>(
            self,
        ) -> Result<Option<crate::tz::TimeZone>, E> {
            Ok(None)
        }
    }

    /// (De)serialize a required [`TimeZone`](crate::tz::TimeZone).
    pub mod required {
        /// Serialize a required [`TimeZone`](crate::tz::TimeZone).
        ///
        /// This will result in an IANA time zone identifier, fixed offset or a
        /// POSIX time zone string.
        ///
        /// This can return an error in some cases when the `TimeZone` has no
        /// succinct string representation. For example, when the `TimeZone` is
        /// derived from a system `/etc/localtime` for which no IANA time zone
        /// identifier could be found.
        #[inline]
        pub fn serialize<S: serde::Serializer>(
            tz: &crate::tz::TimeZone,
            se: S,
        ) -> Result<S::Ok, S::Error> {
            if !tz.has_succinct_serialization() {
                return Err(<S::Error as serde::ser::Error>::custom(
                    "time zones without IANA identifiers that aren't either \
                     fixed offsets or a POSIX time zone can't be serialized \
                     (this typically occurs when this is a system time zone \
                      derived from `/etc/localtime` on Unix systems that \
                      isn't symlinked to an entry in `/usr/share/zoneinfo)",
                ));
            }
            se.collect_str(&super::TemporalTimeZone(tz))
        }

        /// Deserialize a required [`TimeZone`](crate::tz::TimeZone).
        ///
        /// This will attempt to parse an IANA time zone identifier, a fixed
        /// offset or a POSIX time zone string.
        #[inline]
        pub fn deserialize<'de, D: serde::Deserializer<'de>>(
            de: D,
        ) -> Result<crate::tz::TimeZone, D::Error> {
            de.deserialize_str(super::Visitor)
        }
    }

    /// (De)serialize an optional [`TimeZone`](crate::tz::TimeZone).
    pub mod optional {
        /// Serialize an optional [`TimeZone`](crate::tz::TimeZone).
        ///
        /// This will result in an IANA time zone identifier, fixed offset or a
        /// POSIX time zone string.
        ///
        /// This can return an error in some cases when the `TimeZone` has no
        /// succinct string representation. For example, when the `TimeZone` is
        /// derived from a system `/etc/localtime` for which no IANA time zone
        /// identifier could be found.
        #[inline]
        pub fn serialize<S: serde::Serializer>(
            tz: &Option<crate::tz::TimeZone>,
            se: S,
        ) -> Result<S::Ok, S::Error> {
            match *tz {
                None => se.serialize_none(),
                Some(ref tz) => super::required::serialize(tz, se),
            }
        }

        /// Deserialize an optional [`TimeZone`](crate::tz::TimeZone).
        ///
        /// This will attempt to parse an IANA time zone identifier, a fixed
        /// offset or a POSIX time zone string.
        #[inline]
        pub fn deserialize<'de, D: serde::Deserializer<'de>>(
            de: D,
        ) -> Result<Option<crate::tz::TimeZone>, D::Error> {
            de.deserialize_option(super::OptionalVisitor(super::Visitor))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        span::span_eq, SignedDuration, Span, SpanFieldwise, Timestamp, ToSpan,
    };

    #[test]
    fn duration_friendly_compact_required() {
        #[derive(Debug, serde::Deserialize, serde::Serialize)]
        struct Data {
            #[serde(
                serialize_with = "crate::fmt::serde::duration::friendly::compact::required"
            )]
            duration: SignedDuration,
        }

        let json = r#"{"duration":"36 hours 1100ms"}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(
            got.duration,
            SignedDuration::new(36 * 60 * 60 + 1, 100_000_000)
        );

        let expected = r#"{"duration":"36h 1s 100ms"}"#;
        assert_eq!(serde_json::to_string(&got).unwrap(), expected);
    }

    #[test]
    fn duration_friendly_compact_optional() {
        #[derive(Debug, serde::Deserialize, serde::Serialize)]
        struct Data {
            #[serde(
                serialize_with = "crate::fmt::serde::duration::friendly::compact::optional"
            )]
            duration: Option<SignedDuration>,
        }

        let json = r#"{"duration":"36 hours 1100ms"}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(
            got.duration,
            Some(SignedDuration::new(36 * 60 * 60 + 1, 100_000_000))
        );

        let expected = r#"{"duration":"36h 1s 100ms"}"#;
        assert_eq!(serde_json::to_string(&got).unwrap(), expected);
    }

    #[test]
    fn span_friendly_compact_required() {
        #[derive(Debug, serde::Deserialize, serde::Serialize)]
        struct Data {
            #[serde(
                serialize_with = "crate::fmt::serde::span::friendly::compact::required"
            )]
            span: Span,
        }

        let json = r#"{"span":"1 year 2 months 36 hours 1100ms"}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        span_eq!(got.span, 1.year().months(2).hours(36).milliseconds(1100));

        let expected = r#"{"span":"1y 2mo 36h 1100ms"}"#;
        assert_eq!(serde_json::to_string(&got).unwrap(), expected);
    }

    #[test]
    fn span_friendly_compact_optional() {
        #[derive(Debug, serde::Deserialize, serde::Serialize)]
        struct Data {
            #[serde(
                serialize_with = "crate::fmt::serde::span::friendly::compact::optional"
            )]
            span: Option<Span>,
        }

        let json = r#"{"span":"1 year 2 months 36 hours 1100ms"}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(
            got.span.map(SpanFieldwise),
            Some(1.year().months(2).hours(36).milliseconds(1100).fieldwise())
        );

        let expected = r#"{"span":"1y 2mo 36h 1100ms"}"#;
        assert_eq!(serde_json::to_string(&got).unwrap(), expected);
    }

    #[test]
    fn timestamp_second_required() {
        #[derive(Debug, serde::Deserialize, serde::Serialize)]
        struct Data {
            #[serde(with = "crate::fmt::serde::timestamp::second::required")]
            ts: Timestamp,
        }

        let json = r#"{"ts":1517644800}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(got.ts, Timestamp::from_second(1517644800).unwrap());
        assert_eq!(serde_json::to_string(&got).unwrap(), json);
    }

    #[test]
    fn timestamp_second_optional() {
        #[derive(Debug, serde::Deserialize, serde::Serialize)]
        struct Data {
            #[serde(with = "crate::fmt::serde::timestamp::second::optional")]
            ts: Option<Timestamp>,
        }

        let json = r#"{"ts":1517644800}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(got.ts, Some(Timestamp::from_second(1517644800).unwrap()));
        assert_eq!(serde_json::to_string(&got).unwrap(), json);
    }

    #[test]
    fn timestamp_millisecond_required() {
        #[derive(Debug, serde::Deserialize, serde::Serialize)]
        struct Data {
            #[serde(
                with = "crate::fmt::serde::timestamp::millisecond::required"
            )]
            ts: Timestamp,
        }

        let json = r#"{"ts":1517644800000}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(
            got.ts,
            Timestamp::from_millisecond(1517644800_000).unwrap()
        );
        assert_eq!(serde_json::to_string(&got).unwrap(), json);

        let json = r#"{"ts":1517644800123}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(
            got.ts,
            Timestamp::from_millisecond(1517644800_123).unwrap()
        );
        assert_eq!(serde_json::to_string(&got).unwrap(), json);
    }

    #[test]
    fn timestamp_millisecond_optional() {
        #[derive(Debug, serde::Deserialize, serde::Serialize)]
        struct Data {
            #[serde(
                with = "crate::fmt::serde::timestamp::millisecond::optional"
            )]
            ts: Option<Timestamp>,
        }

        let json = r#"{"ts":1517644800000}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(
            got.ts,
            Some(Timestamp::from_millisecond(1517644800_000).unwrap())
        );
        assert_eq!(serde_json::to_string(&got).unwrap(), json);

        let json = r#"{"ts":1517644800123}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(
            got.ts,
            Some(Timestamp::from_millisecond(1517644800_123).unwrap())
        );
        assert_eq!(serde_json::to_string(&got).unwrap(), json);
    }

    #[test]
    fn timestamp_microsecond_required() {
        #[derive(Debug, serde::Deserialize, serde::Serialize)]
        struct Data {
            #[serde(
                with = "crate::fmt::serde::timestamp::microsecond::required"
            )]
            ts: Timestamp,
        }

        let json = r#"{"ts":1517644800000000}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(
            got.ts,
            Timestamp::from_microsecond(1517644800_000000).unwrap()
        );
        assert_eq!(serde_json::to_string(&got).unwrap(), json);

        let json = r#"{"ts":1517644800123456}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(
            got.ts,
            Timestamp::from_microsecond(1517644800_123456).unwrap()
        );
        assert_eq!(serde_json::to_string(&got).unwrap(), json);
    }

    #[test]
    fn timestamp_microsecond_optional() {
        #[derive(Debug, serde::Deserialize, serde::Serialize)]
        struct Data {
            #[serde(
                with = "crate::fmt::serde::timestamp::microsecond::optional"
            )]
            ts: Option<Timestamp>,
        }

        let json = r#"{"ts":1517644800000000}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(
            got.ts,
            Some(Timestamp::from_microsecond(1517644800_000000).unwrap())
        );
        assert_eq!(serde_json::to_string(&got).unwrap(), json);

        let json = r#"{"ts":1517644800123456}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(
            got.ts,
            Some(Timestamp::from_microsecond(1517644800_123456).unwrap())
        );
        assert_eq!(serde_json::to_string(&got).unwrap(), json);
    }

    #[test]
    fn timestamp_nanosecond_required() {
        #[derive(Debug, serde::Deserialize, serde::Serialize)]
        struct Data {
            #[serde(
                with = "crate::fmt::serde::timestamp::nanosecond::required"
            )]
            ts: Timestamp,
        }

        let json = r#"{"ts":1517644800000000000}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(
            got.ts,
            Timestamp::from_nanosecond(1517644800_000000000).unwrap()
        );
        assert_eq!(serde_json::to_string(&got).unwrap(), json);

        let json = r#"{"ts":1517644800123456789}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(
            got.ts,
            Timestamp::from_nanosecond(1517644800_123456789).unwrap()
        );
        assert_eq!(serde_json::to_string(&got).unwrap(), json);
    }

    #[test]
    fn timestamp_nanosecond_optional() {
        #[derive(Debug, serde::Deserialize, serde::Serialize)]
        struct Data {
            #[serde(
                with = "crate::fmt::serde::timestamp::nanosecond::optional"
            )]
            ts: Option<Timestamp>,
        }

        let json = r#"{"ts":1517644800000000000}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(
            got.ts,
            Some(Timestamp::from_nanosecond(1517644800_000000000).unwrap())
        );
        assert_eq!(serde_json::to_string(&got).unwrap(), json);

        let json = r#"{"ts":1517644800123456789}"#;
        let got: Data = serde_json::from_str(&json).unwrap();
        assert_eq!(
            got.ts,
            Some(Timestamp::from_nanosecond(1517644800_123456789).unwrap())
        );
        assert_eq!(serde_json::to_string(&got).unwrap(), json);
    }
}
