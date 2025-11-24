// Copyright 2016 The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! This Rust crate implements IDNA
//! [per the WHATWG URL Standard](https://url.spec.whatwg.org/#idna).
//!
//! It also exposes the underlying algorithms from [*Unicode IDNA Compatibility Processing*
//! (Unicode Technical Standard #46)](http://www.unicode.org/reports/tr46/)
//! and [Punycode (RFC 3492)](https://tools.ietf.org/html/rfc3492).
//!
//! Quoting from [UTS #46’s introduction](http://www.unicode.org/reports/tr46/#Introduction):
//!
//! > Initially, domain names were restricted to ASCII characters.
//! > A system was introduced in 2003 for internationalized domain names (IDN).
//! > This system is called Internationalizing Domain Names for Applications,
//! > or IDNA2003 for short.
//! > This mechanism supports IDNs by means of a client software transformation
//! > into a format known as Punycode.
//! > A revision of IDNA was approved in 2010 (IDNA2008).
//! > This revision has a number of incompatibilities with IDNA2003.
//! >
//! > The incompatibilities force implementers of client software,
//! > such as browsers and emailers,
//! > to face difficult choices during the transition period
//! > as registries shift from IDNA2003 to IDNA2008.
//! > This document specifies a mechanism
//! > that minimizes the impact of this transition for client software,
//! > allowing client software to access domains that are valid under either system.
#![no_std]

// For forwards compatibility
#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

#[cfg(not(feature = "alloc"))]
compile_error!("the `alloc` feature must be enabled");

// Avoid a breaking change if in the future there's a use case for
// having a Bring-Your-Own-ICU4X-Data constructor for `Uts46` and
// not also having compiled data in the binary.
#[cfg(not(feature = "compiled_data"))]
compile_error!("the `compiled_data` feature must be enabled");

use alloc::borrow::Cow;
use alloc::string::String;
pub use uts46::AsciiDenyList;
use uts46::Uts46;

mod deprecated;
pub mod punycode;
pub mod uts46;

#[allow(deprecated)]
pub use crate::deprecated::{Config, Idna};

/// Type indicating that there were errors during UTS #46 processing.
#[derive(Default, Debug)]
#[non_exhaustive]
pub struct Errors {}

impl From<Errors> for Result<(), Errors> {
    fn from(e: Errors) -> Result<(), Errors> {
        Err(e)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Errors {}

#[cfg(not(feature = "std"))]
impl core::error::Error for Errors {}

impl core::fmt::Display for Errors {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self, f)
    }
}

/// The [domain to ASCII](https://url.spec.whatwg.org/#concept-domain-to-ascii) algorithm;
/// version returning a `Cow`.
///
/// Most applications should be using this function rather than the sibling functions,
/// and most applications should pass [`AsciiDenyList::URL`] as the second argument.
/// Passing [`AsciiDenyList::URL`] as the second argument makes this function also
/// perform the [forbidden domain code point](https://url.spec.whatwg.org/#forbidden-domain-code-point)
/// check in addition to the [domain to ASCII](https://url.spec.whatwg.org/#concept-domain-to-ascii)
/// algorithm.
///
/// Returns the ASCII representation a domain name,
/// normalizing characters (upper-case to lower-case and other kinds of equivalence)
/// and using Punycode as necessary.
///
/// This process may fail.
///
/// If you have a `&str` instead of `&[u8]`, just call `.to_bytes()` on it before
/// passing it to this function. It's still preferable to use this function over
/// the sibling functions that take `&str`.
pub fn domain_to_ascii_cow(
    domain: &[u8],
    ascii_deny_list: AsciiDenyList,
) -> Result<Cow<'_, str>, Errors> {
    Uts46::new().to_ascii(
        domain,
        ascii_deny_list,
        uts46::Hyphens::Allow,
        uts46::DnsLength::Ignore,
    )
}

/// The [domain to ASCII](https://url.spec.whatwg.org/#concept-domain-to-ascii) algorithm;
/// version returning `String` and no ASCII deny list (i.e. _UseSTD3ASCIIRules=false_).
///
/// This function exists for backward-compatibility. Consider using [`domain_to_ascii_cow`]
/// instead.
///
/// Return the ASCII representation a domain name,
/// normalizing characters (upper-case to lower-case and other kinds of equivalence)
/// and using Punycode as necessary.
///
/// This process may fail.
pub fn domain_to_ascii(domain: &str) -> Result<String, Errors> {
    domain_to_ascii_cow(domain.as_bytes(), AsciiDenyList::EMPTY).map(|cow| cow.into_owned())
}

/// The [domain to ASCII](https://url.spec.whatwg.org/#concept-domain-to-ascii) algorithm,
/// with the `beStrict` flag set.
///
/// Note that this rejects various real-world names including:
/// * YouTube CDN nodes
/// * Some GitHub user pages
/// * Pseudo-hosts used by various TXT record-based protocols.
pub fn domain_to_ascii_strict(domain: &str) -> Result<String, Errors> {
    Uts46::new()
        .to_ascii(
            domain.as_bytes(),
            uts46::AsciiDenyList::STD3,
            uts46::Hyphens::Check,
            uts46::DnsLength::Verify,
        )
        .map(|cow| cow.into_owned())
}

/// The [domain to Unicode](https://url.spec.whatwg.org/#concept-domain-to-unicode) algorithm;
/// version returning `String` and no ASCII deny list (i.e. _UseSTD3ASCIIRules=false_).
///
/// This function exists for backward-compatibility. Consider using [`Uts46::to_user_interface`]
/// or [`Uts46::to_unicode`].
///
/// Return the Unicode representation of a domain name,
/// normalizing characters (upper-case to lower-case and other kinds of equivalence)
/// and decoding Punycode as necessary.
///
/// If the second item of the tuple indicates an error, the first item of the tuple
/// denotes errors using the REPLACEMENT CHARACTERs in order to be able to illustrate
/// errors to the user. When the second item of the return tuple signals an error,
/// the first item of the tuple must not be used in a network protocol.
pub fn domain_to_unicode(domain: &str) -> (String, Result<(), Errors>) {
    let (cow, result) = Uts46::new().to_unicode(
        domain.as_bytes(),
        uts46::AsciiDenyList::EMPTY,
        uts46::Hyphens::Allow,
    );
    (cow.into_owned(), result)
}
