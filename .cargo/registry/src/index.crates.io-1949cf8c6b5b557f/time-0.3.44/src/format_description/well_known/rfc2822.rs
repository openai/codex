//! The format described in RFC 2822.

/// The format described in [RFC 2822](https://tools.ietf.org/html/rfc2822#section-3.3).
///
/// Example: Fri, 21 Nov 1997 09:55:06 -0600
///
/// # Examples
#[cfg_attr(feature = "parsing", doc = "```rust")]
#[cfg_attr(not(feature = "parsing"), doc = "```rust,ignore")]
/// # use time::{format_description::well_known::Rfc2822, OffsetDateTime};
/// use time_macros::datetime;
/// assert_eq!(
///     OffsetDateTime::parse("Sat, 12 Jun 1993 13:25:19 GMT", &Rfc2822)?,
///     datetime!(1993-06-12 13:25:19 +00:00)
/// );
/// # Ok::<_, time::Error>(())
/// ```
///
#[cfg_attr(feature = "formatting", doc = "```rust")]
#[cfg_attr(not(feature = "formatting"), doc = "```rust,ignore")]
/// # use time::format_description::well_known::Rfc2822;
/// # use time_macros::datetime;
/// assert_eq!(
///     datetime!(1997-11-21 09:55:06 -06:00).format(&Rfc2822)?,
///     "Fri, 21 Nov 1997 09:55:06 -0600"
/// );
/// # Ok::<_, time::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rfc2822;
