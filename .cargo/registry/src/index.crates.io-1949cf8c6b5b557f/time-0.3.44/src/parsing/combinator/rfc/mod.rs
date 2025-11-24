//! Combinators for rules as defined in a standard.
//!
//! When applicable, these rules have been converted strictly following the ABNF syntax as specified
//! in [RFC 2234].
//!
//! [RFC 2234]: https://datatracker.ietf.org/doc/html/rfc2234

pub(crate) mod iso8601;
pub(crate) mod rfc2234;
pub(crate) mod rfc2822;
