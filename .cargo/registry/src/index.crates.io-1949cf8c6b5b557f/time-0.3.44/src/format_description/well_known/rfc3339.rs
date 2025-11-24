//! The format described in RFC 3339.

/// The format described in [RFC 3339](https://tools.ietf.org/html/rfc3339#section-5.6).
///
/// Format example: 1985-04-12T23:20:50.52Z
///
/// # Examples
#[cfg_attr(feature = "parsing", doc = "```rust")]
#[cfg_attr(not(feature = "parsing"), doc = "```rust,ignore")]
/// # use time::{format_description::well_known::Rfc3339, OffsetDateTime};
/// # use time_macros::datetime;
/// assert_eq!(
///     OffsetDateTime::parse("1985-04-12T23:20:50.52Z", &Rfc3339)?,
///     datetime!(1985-04-12 23:20:50.52 +00:00)
/// );
/// # Ok::<_, time::Error>(())
/// ```
///
#[cfg_attr(feature = "formatting", doc = "```rust")]
#[cfg_attr(not(feature = "formatting"), doc = "```rust,ignore")]
/// # use time::format_description::well_known::Rfc3339;
/// # use time_macros::datetime;
/// assert_eq!(
///     datetime!(1985-04-12 23:20:50.52 +00:00).format(&Rfc3339)?,
///     "1985-04-12T23:20:50.52Z"
/// );
/// # Ok::<_, time::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rfc3339;
