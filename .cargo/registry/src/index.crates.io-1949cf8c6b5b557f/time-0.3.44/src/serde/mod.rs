//! Differential formats for serde.
// This also includes the serde implementations for all types. This doesn't need to be externally
// documented, though.

// Types with guaranteed stable serde representations. Strings are avoided to allow for optimal
// representations in various binary forms.

/// Consume the next item in a sequence.
macro_rules! item {
    ($seq:expr, $name:literal) => {
        $seq.next_element()?
            .ok_or_else(|| <A::Error as serde::de::Error>::custom(concat!("expected ", $name)))
    };
}

#[cfg(any(feature = "formatting", feature = "parsing"))]
pub mod iso8601;
#[cfg(any(feature = "formatting", feature = "parsing"))]
pub mod rfc2822;
#[cfg(any(feature = "formatting", feature = "parsing"))]
pub mod rfc3339;
pub mod timestamp;
mod visitor;

#[cfg(feature = "serde-human-readable")]
use alloc::string::ToString;
use core::marker::PhantomData;

#[cfg(feature = "serde-human-readable")]
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
/// Generate a custom serializer and deserializer from a format string or an existing format.
///
/// The syntax accepted by this macro is the same as [`format_description::parse()`], which can
/// be found in [the book](https://time-rs.github.io/book/api/format-description.html).
///
/// # Usage
///
/// Invoked as `serde::format_description!(mod_name, Date, FORMAT)` where `FORMAT` is either a
/// `"<format string>"` or something that implements
#[cfg_attr(
    all(feature = "formatting", feature = "parsing"),
    doc = "[`Formattable`](crate::formatting::Formattable) and \
           [`Parsable`](crate::parsing::Parsable)."
)]
#[cfg_attr(
    all(feature = "formatting", not(feature = "parsing")),
    doc = "[`Formattable`](crate::formatting::Formattable)."
)]
#[cfg_attr(
    all(not(feature = "formatting"), feature = "parsing"),
    doc = "[`Parsable`](crate::parsing::Parsable)."
)]
/// This puts a module named `mod_name` in the current scope that can be used to format `Date`
/// structs. A submodule (`mod_name::option`) is also generated for `Option<Date>`. Both
/// modules are only visible in the current scope by default. To increase visibility, you can
/// specify `pub`, `pub(crate)`, or similar before the module name:
/// `serde::format_description!(pub mod_name, Date, FORMAT)`.
///
/// The returned `Option` will contain a deserialized value if present and `None` if the field
/// is present but the value is `null` (or the equivalent in other formats). To return `None`
/// when the field is not present, you should use `#[serde(default)]` on the field.
///
/// # Examples
///
/// Using a format string:
///
/// ```rust,no_run
/// # use time::OffsetDateTime;
#[cfg_attr(
    all(feature = "formatting", feature = "parsing"),
    doc = "use ::serde::{Serialize, Deserialize};"
)]
#[cfg_attr(
    all(feature = "formatting", not(feature = "parsing")),
    doc = "use ::serde::Serialize;"
)]
#[cfg_attr(
    all(not(feature = "formatting"), feature = "parsing"),
    doc = "use ::serde::Deserialize;"
)]
/// use time::serde;
///
/// // Makes a module `mod my_format { ... }`.
/// serde::format_description!(my_format, OffsetDateTime, "hour=[hour], minute=[minute]");
///
/// # #[allow(dead_code)]
#[cfg_attr(
    all(feature = "formatting", feature = "parsing"),
    doc = "#[derive(Serialize, Deserialize)]"
)]
#[cfg_attr(
    all(feature = "formatting", not(feature = "parsing")),
    doc = "#[derive(Serialize)]"
)]
#[cfg_attr(
    all(not(feature = "formatting"), feature = "parsing"),
    doc = "#[derive(Deserialize)]"
)]
/// struct SerializesWithCustom {
///     #[serde(with = "my_format")]
///     dt: OffsetDateTime,
///     #[serde(with = "my_format::option")]
///     maybe_dt: Option<OffsetDateTime>,
/// }
/// ```
/// 
/// Define the format separately to be used in multiple places:
/// ```rust,no_run
/// # use time::OffsetDateTime;
#[cfg_attr(
    all(feature = "formatting", feature = "parsing"),
    doc = "use ::serde::{Serialize, Deserialize};"
)]
#[cfg_attr(
    all(feature = "formatting", not(feature = "parsing")),
    doc = "use ::serde::Serialize;"
)]
#[cfg_attr(
    all(not(feature = "formatting"), feature = "parsing"),
    doc = "use ::serde::Deserialize;"
)]
/// use time::serde;
/// use time::format_description::BorrowedFormatItem;
///
/// const DATE_TIME_FORMAT: &[BorrowedFormatItem<'_>] = time::macros::format_description!(
///     "hour=[hour], minute=[minute]"
/// );
///
/// // Makes a module `mod my_format { ... }`.
/// serde::format_description!(my_format, OffsetDateTime, DATE_TIME_FORMAT);
///
/// # #[allow(dead_code)]
#[cfg_attr(
    all(feature = "formatting", feature = "parsing"),
    doc = "#[derive(Serialize, Deserialize)]"
)]
#[cfg_attr(
    all(feature = "formatting", not(feature = "parsing")),
    doc = "#[derive(Serialize)]"
)]
#[cfg_attr(
    all(not(feature = "formatting"), feature = "parsing"),
    doc = "#[derive(Deserialize)]"
)]
/// struct SerializesWithCustom {
///     #[serde(with = "my_format")]
///     dt: OffsetDateTime,
///     #[serde(with = "my_format::option")]
///     maybe_dt: Option<OffsetDateTime>,
/// }
///
/// fn main() {
///     # #[expect(unused_variables)]
///     let str_ts = OffsetDateTime::now_utc().format(DATE_TIME_FORMAT).unwrap();
/// }
/// ```
/// 
/// Customize the configuration of ISO 8601 formatting/parsing:
/// ```rust,no_run
/// # use time::OffsetDateTime;
#[cfg_attr(
    all(feature = "formatting", feature = "parsing"),
    doc = "use ::serde::{Serialize, Deserialize};"
)]
#[cfg_attr(
    all(feature = "formatting", not(feature = "parsing")),
    doc = "use ::serde::Serialize;"
)]
#[cfg_attr(
    all(not(feature = "formatting"), feature = "parsing"),
    doc = "use ::serde::Deserialize;"
)]
/// use time::serde;
/// use time::format_description::well_known::{iso8601, Iso8601};
///
/// # #[allow(dead_code)]
/// const CONFIG: iso8601::EncodedConfig = iso8601::Config::DEFAULT
///     .set_year_is_six_digits(false)
///     .encode();
/// # #[allow(dead_code)]
/// const FORMAT: Iso8601<CONFIG> = Iso8601::<CONFIG>;
///
/// // Makes a module `mod my_format { ... }`.
/// serde::format_description!(my_format, OffsetDateTime, FORMAT);
///
/// # #[allow(dead_code)]
#[cfg_attr(
    all(feature = "formatting", feature = "parsing"),
    doc = "#[derive(Serialize, Deserialize)]"
)]
#[cfg_attr(
    all(feature = "formatting", not(feature = "parsing")),
    doc = "#[derive(Serialize)]"
)]
#[cfg_attr(
    all(not(feature = "formatting"), feature = "parsing"),
    doc = "#[derive(Deserialize)]"
)]
/// struct SerializesWithCustom {
///     #[serde(with = "my_format")]
///     dt: OffsetDateTime,
///     #[serde(with = "my_format::option")]
///     maybe_dt: Option<OffsetDateTime>,
/// }
/// # fn main() {}
/// ```
/// 
/// [`format_description::parse()`]: crate::format_description::parse()
#[cfg(all(feature = "macros", any(feature = "formatting", feature = "parsing")))]
pub use time_macros::serde_format_description as format_description;

use self::visitor::Visitor;
#[cfg(feature = "parsing")]
use crate::format_description::{modifier, BorrowedFormatItem, Component};
use crate::{
    Date, Duration, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcDateTime, UtcOffset, Weekday,
};

/// The format used when serializing and deserializing a human-readable `Date`.
#[cfg(feature = "parsing")]
const DATE_FORMAT: &[BorrowedFormatItem<'_>] = &[
    BorrowedFormatItem::Component(Component::Year(modifier::Year::default())),
    BorrowedFormatItem::Literal(b"-"),
    BorrowedFormatItem::Component(Component::Month(modifier::Month::default())),
    BorrowedFormatItem::Literal(b"-"),
    BorrowedFormatItem::Component(Component::Day(modifier::Day::default())),
];

impl Serialize for Date {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[cfg(feature = "serde-human-readable")]
        if serializer.is_human_readable() {
            let Ok(s) = self.format(&DATE_FORMAT) else {
                return Err(S::Error::custom("failed formatting `Date`"));
            };
            return serializer.serialize_str(&s);
        }

        (self.year(), self.ordinal()).serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for Date {
    #[inline]
    fn deserialize<D: Deserializer<'a>>(deserializer: D) -> Result<Self, D::Error> {
        if cfg!(feature = "serde-human-readable") && deserializer.is_human_readable() {
            deserializer.deserialize_any(Visitor::<Self>(PhantomData))
        } else {
            deserializer.deserialize_tuple(2, Visitor::<Self>(PhantomData))
        }
    }
}

impl Serialize for Duration {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[cfg(feature = "serde-human-readable")]
        if serializer.is_human_readable() {
            return serializer.collect_str(&format_args!(
                "{}{}.{:>09}",
                if self.is_negative() { "-" } else { "" },
                self.whole_seconds().unsigned_abs(),
                self.subsec_nanoseconds().abs(),
            ));
        }

        (self.whole_seconds(), self.subsec_nanoseconds()).serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for Duration {
    #[inline]
    fn deserialize<D: Deserializer<'a>>(deserializer: D) -> Result<Self, D::Error> {
        if cfg!(feature = "serde-human-readable") && deserializer.is_human_readable() {
            deserializer.deserialize_any(Visitor::<Self>(PhantomData))
        } else {
            deserializer.deserialize_tuple(2, Visitor::<Self>(PhantomData))
        }
    }
}

/// The format used when serializing and deserializing a human-readable `OffsetDateTime`.
#[cfg(feature = "parsing")]
const OFFSET_DATE_TIME_FORMAT: &[BorrowedFormatItem<'_>] = &[
    BorrowedFormatItem::Compound(DATE_FORMAT),
    BorrowedFormatItem::Literal(b" "),
    BorrowedFormatItem::Compound(TIME_FORMAT),
    BorrowedFormatItem::Literal(b" "),
    BorrowedFormatItem::Compound(UTC_OFFSET_FORMAT),
];

impl Serialize for OffsetDateTime {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[cfg(feature = "serde-human-readable")]
        if serializer.is_human_readable() {
            let Ok(s) = self.format(&OFFSET_DATE_TIME_FORMAT) else {
                return Err(S::Error::custom("failed formatting `OffsetDateTime`"));
            };
            return serializer.serialize_str(&s);
        }

        (
            self.year(),
            self.ordinal(),
            self.hour(),
            self.minute(),
            self.second(),
            self.nanosecond(),
            self.offset().whole_hours(),
            self.offset().minutes_past_hour(),
            self.offset().seconds_past_minute(),
        )
            .serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for OffsetDateTime {
    #[inline]
    fn deserialize<D: Deserializer<'a>>(deserializer: D) -> Result<Self, D::Error> {
        if cfg!(feature = "serde-human-readable") && deserializer.is_human_readable() {
            deserializer.deserialize_any(Visitor::<Self>(PhantomData))
        } else {
            deserializer.deserialize_tuple(9, Visitor::<Self>(PhantomData))
        }
    }
}

/// The format used when serializing and deserializing a human-readable `PrimitiveDateTime`.
#[cfg(feature = "parsing")]
const PRIMITIVE_DATE_TIME_FORMAT: &[BorrowedFormatItem<'_>] = &[
    BorrowedFormatItem::Compound(DATE_FORMAT),
    BorrowedFormatItem::Literal(b" "),
    BorrowedFormatItem::Compound(TIME_FORMAT),
];

impl Serialize for PrimitiveDateTime {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[cfg(feature = "serde-human-readable")]
        if serializer.is_human_readable() {
            let Ok(s) = self.format(&PRIMITIVE_DATE_TIME_FORMAT) else {
                return Err(S::Error::custom("failed formatting `PrimitiveDateTime`"));
            };
            return serializer.serialize_str(&s);
        }

        (
            self.year(),
            self.ordinal(),
            self.hour(),
            self.minute(),
            self.second(),
            self.nanosecond(),
        )
            .serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for PrimitiveDateTime {
    #[inline]
    fn deserialize<D: Deserializer<'a>>(deserializer: D) -> Result<Self, D::Error> {
        if cfg!(feature = "serde-human-readable") && deserializer.is_human_readable() {
            deserializer.deserialize_any(Visitor::<Self>(PhantomData))
        } else {
            deserializer.deserialize_tuple(6, Visitor::<Self>(PhantomData))
        }
    }
}

/// The format used when serializing and deserializing a human-readable `UtcDateTime`.
#[cfg(feature = "parsing")]
const UTC_DATE_TIME_FORMAT: &[BorrowedFormatItem<'_>] = PRIMITIVE_DATE_TIME_FORMAT;

impl Serialize for UtcDateTime {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[cfg(feature = "serde-human-readable")]
        if serializer.is_human_readable() {
            let Ok(s) = self.format(&PRIMITIVE_DATE_TIME_FORMAT) else {
                return Err(S::Error::custom("failed formatting `UtcDateTime`"));
            };
            return serializer.serialize_str(&s);
        }

        (
            self.year(),
            self.ordinal(),
            self.hour(),
            self.minute(),
            self.second(),
            self.nanosecond(),
        )
            .serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for UtcDateTime {
    #[inline]
    fn deserialize<D: Deserializer<'a>>(deserializer: D) -> Result<Self, D::Error> {
        if cfg!(feature = "serde-human-readable") && deserializer.is_human_readable() {
            deserializer.deserialize_any(Visitor::<Self>(PhantomData))
        } else {
            deserializer.deserialize_tuple(6, Visitor::<Self>(PhantomData))
        }
    }
}

/// The format used when serializing and deserializing a human-readable `Time`.
#[cfg(feature = "parsing")]
const TIME_FORMAT: &[BorrowedFormatItem<'_>] = &[
    BorrowedFormatItem::Component(Component::Hour(modifier::Hour::default())),
    BorrowedFormatItem::Literal(b":"),
    BorrowedFormatItem::Component(Component::Minute(modifier::Minute::default())),
    BorrowedFormatItem::Literal(b":"),
    BorrowedFormatItem::Component(Component::Second(modifier::Second::default())),
    BorrowedFormatItem::Literal(b"."),
    BorrowedFormatItem::Component(Component::Subsecond(modifier::Subsecond::default())),
];

impl Serialize for Time {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[cfg(feature = "serde-human-readable")]
        if serializer.is_human_readable() {
            let Ok(s) = self.format(&TIME_FORMAT) else {
                return Err(S::Error::custom("failed formatting `Time`"));
            };
            return serializer.serialize_str(&s);
        }

        (self.hour(), self.minute(), self.second(), self.nanosecond()).serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for Time {
    #[inline]
    fn deserialize<D: Deserializer<'a>>(deserializer: D) -> Result<Self, D::Error> {
        if cfg!(feature = "serde-human-readable") && deserializer.is_human_readable() {
            deserializer.deserialize_any(Visitor::<Self>(PhantomData))
        } else {
            deserializer.deserialize_tuple(4, Visitor::<Self>(PhantomData))
        }
    }
}

/// The format used when serializing and deserializing a human-readable `UtcOffset`.
#[cfg(feature = "parsing")]
const UTC_OFFSET_FORMAT: &[BorrowedFormatItem<'_>] = &[
    BorrowedFormatItem::Component(Component::OffsetHour(
        const {
            let mut m = modifier::OffsetHour::default();
            m.sign_is_mandatory = true;
            m
        },
    )),
    BorrowedFormatItem::Optional(&BorrowedFormatItem::Compound(&[
        BorrowedFormatItem::Literal(b":"),
        BorrowedFormatItem::Component(Component::OffsetMinute(
            const { modifier::OffsetMinute::default() },
        )),
        BorrowedFormatItem::Optional(&BorrowedFormatItem::Compound(&[
            BorrowedFormatItem::Literal(b":"),
            BorrowedFormatItem::Component(Component::OffsetSecond(
                const { modifier::OffsetSecond::default() },
            )),
        ])),
    ])),
];

impl Serialize for UtcOffset {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[cfg(feature = "serde-human-readable")]
        if serializer.is_human_readable() {
            let Ok(s) = self.format(&UTC_OFFSET_FORMAT) else {
                return Err(S::Error::custom("failed formatting `UtcOffset`"));
            };
            return serializer.serialize_str(&s);
        }

        (
            self.whole_hours(),
            self.minutes_past_hour(),
            self.seconds_past_minute(),
        )
            .serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for UtcOffset {
    #[inline]
    fn deserialize<D: Deserializer<'a>>(deserializer: D) -> Result<Self, D::Error> {
        if cfg!(feature = "serde-human-readable") && deserializer.is_human_readable() {
            deserializer.deserialize_any(Visitor::<Self>(PhantomData))
        } else {
            deserializer.deserialize_tuple(3, Visitor::<Self>(PhantomData))
        }
    }
}

impl Serialize for Weekday {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[cfg(feature = "serde-human-readable")]
        if serializer.is_human_readable() {
            #[cfg(not(feature = "std"))]
            use alloc::string::ToString;
            return self.to_string().serialize(serializer);
        }

        self.number_from_monday().serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for Weekday {
    #[inline]
    fn deserialize<D: Deserializer<'a>>(deserializer: D) -> Result<Self, D::Error> {
        if cfg!(feature = "serde-human-readable") && deserializer.is_human_readable() {
            deserializer.deserialize_any(Visitor::<Self>(PhantomData))
        } else {
            deserializer.deserialize_u8(Visitor::<Self>(PhantomData))
        }
    }
}

impl Serialize for Month {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[cfg(feature = "serde-human-readable")]
        if serializer.is_human_readable() {
            #[cfg(not(feature = "std"))]
            use alloc::string::String;
            return self.to_string().serialize(serializer);
        }

        u8::from(*self).serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for Month {
    #[inline]
    fn deserialize<D: Deserializer<'a>>(deserializer: D) -> Result<Self, D::Error> {
        if cfg!(feature = "serde-human-readable") && deserializer.is_human_readable() {
            deserializer.deserialize_any(Visitor::<Self>(PhantomData))
        } else {
            deserializer.deserialize_u8(Visitor::<Self>(PhantomData))
        }
    }
}
