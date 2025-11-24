// Copyright 2013-2015 The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!

rust-url is an implementation of the [URL Standard](http://url.spec.whatwg.org/)
for the [Rust](http://rust-lang.org/) programming language.


# URL parsing and data structures

First, URL parsing may fail for various reasons and therefore returns a `Result`.

```
use url::{Url, ParseError};

assert!(Url::parse("http://[:::1]") == Err(ParseError::InvalidIpv6Address))
```

Let’s parse a valid URL and look at its components.

```
use url::{Url, Host, Position};
# use url::ParseError;
# fn run() -> Result<(), ParseError> {
let issue_list_url = Url::parse(
    "https://github.com/rust-lang/rust/issues?labels=E-easy&state=open"
)?;


assert!(issue_list_url.scheme() == "https");
assert!(issue_list_url.username() == "");
assert!(issue_list_url.password() == None);
assert!(issue_list_url.host_str() == Some("github.com"));
assert!(issue_list_url.host() == Some(Host::Domain("github.com")));
assert!(issue_list_url.port() == None);
assert!(issue_list_url.path() == "/rust-lang/rust/issues");
assert!(issue_list_url.path_segments().map(|c| c.collect::<Vec<_>>()) ==
        Some(vec!["rust-lang", "rust", "issues"]));
assert!(issue_list_url.query() == Some("labels=E-easy&state=open"));
assert!(&issue_list_url[Position::BeforePath..] == "/rust-lang/rust/issues?labels=E-easy&state=open");
assert!(issue_list_url.fragment() == None);
assert!(!issue_list_url.cannot_be_a_base());
# Ok(())
# }
# run().unwrap();
```

Some URLs are said to be *cannot-be-a-base*:
they don’t have a username, password, host, or port,
and their "path" is an arbitrary string rather than slash-separated segments:

```
use url::Url;
# use url::ParseError;

# fn run() -> Result<(), ParseError> {
let data_url = Url::parse("data:text/plain,Hello?World#")?;

assert!(data_url.cannot_be_a_base());
assert!(data_url.scheme() == "data");
assert!(data_url.path() == "text/plain,Hello");
assert!(data_url.path_segments().is_none());
assert!(data_url.query() == Some("World"));
assert!(data_url.fragment() == Some(""));
# Ok(())
# }
# run().unwrap();
```

## Default Features

Versions `<= 2.5.2` of the crate have no default features. Versions `> 2.5.2` have the default feature 'std'.
If you are upgrading across this boundary and you have specified `default-features = false`, then
you will need to add the 'std' feature or the 'alloc' feature to your dependency.
The 'std' feature has the same behavior as the previous versions. The 'alloc' feature
provides no_std support.

## Serde

Enable the `serde` feature to include `Deserialize` and `Serialize` implementations for `url::Url`.

# Base URL

Many contexts allow URL *references* that can be relative to a *base URL*:

```html
<link rel="stylesheet" href="../main.css">
```

Since parsed URLs are absolute, giving a base is required for parsing relative URLs:

```
use url::{Url, ParseError};

assert!(Url::parse("../main.css") == Err(ParseError::RelativeUrlWithoutBase))
```

Use the `join` method on an `Url` to use it as a base URL:

```
use url::Url;
# use url::ParseError;

# fn run() -> Result<(), ParseError> {
let this_document = Url::parse("http://servo.github.io/rust-url/url/index.html")?;
let css_url = this_document.join("../main.css")?;
assert_eq!(css_url.as_str(), "http://servo.github.io/rust-url/main.css");
# Ok(())
# }
# run().unwrap();
```

# Feature: `serde`

If you enable the `serde` feature, [`Url`](struct.Url.html) will implement
[`serde::Serialize`](https://docs.rs/serde/1/serde/trait.Serialize.html) and
[`serde::Deserialize`](https://docs.rs/serde/1/serde/trait.Deserialize.html).
See [serde documentation](https://serde.rs) for more information.

```toml
url = { version = "2", features = ["serde"] }
```

# Feature: `debugger_visualizer`

If you enable the `debugger_visualizer` feature, the `url` crate will include
a [natvis file](https://docs.microsoft.com/en-us/visualstudio/debugger/create-custom-views-of-native-objects)
for [Visual Studio](https://www.visualstudio.com/) that allows you to view
[`Url`](struct.Url.html) objects in the debugger.

This feature requires Rust 1.71 or later.

```toml
url = { version = "2", features = ["debugger_visualizer"] }
```

*/

#![no_std]
#![doc(html_root_url = "https://docs.rs/url/2.5.4")]
#![cfg_attr(
    feature = "debugger_visualizer",
    debugger_visualizer(natvis_file = "../../debug_metadata/url.natvis")
)]

pub use form_urlencoded;

// For forwards compatibility
#[cfg(feature = "std")]
extern crate std;

#[macro_use]
extern crate alloc;

#[cfg(feature = "serde")]
extern crate serde;

use crate::host::HostInternal;

use crate::net::IpAddr;
#[cfg(feature = "std")]
#[cfg(any(
    unix,
    windows,
    target_os = "redox",
    target_os = "wasi",
    target_os = "hermit"
))]
use crate::net::{SocketAddr, ToSocketAddrs};
use crate::parser::{to_u32, Context, Parser, SchemeType, USERINFO};
use alloc::borrow::ToOwned;
use alloc::str;
use alloc::string::{String, ToString};
use core::borrow::Borrow;
use core::convert::TryFrom;
use core::fmt::Write;
use core::ops::{Range, RangeFrom, RangeTo};
use core::{cmp, fmt, hash, mem};
use percent_encoding::utf8_percent_encode;
#[cfg(feature = "std")]
#[cfg(any(
    unix,
    windows,
    target_os = "redox",
    target_os = "wasi",
    target_os = "hermit"
))]
use std::io;
#[cfg(feature = "std")]
use std::path::{Path, PathBuf};

/// `std` version of `net`
#[cfg(feature = "std")]
pub(crate) mod net {
    pub use std::net::*;
}
/// `no_std` nightly version of `net`
#[cfg(not(feature = "std"))]
pub(crate) mod net {
    pub use core::net::*;
}

pub use crate::host::Host;
pub use crate::origin::{OpaqueOrigin, Origin};
pub use crate::parser::{ParseError, SyntaxViolation};
pub use crate::path_segments::PathSegmentsMut;
pub use crate::slicing::Position;
pub use form_urlencoded::EncodingOverride;

mod host;
mod origin;
mod parser;
mod path_segments;
mod slicing;

#[doc(hidden)]
pub mod quirks;

/// A parsed URL record.
#[derive(Clone)]
pub struct Url {
    /// Syntax in pseudo-BNF:
    ///
    ///   url = scheme ":" [ hierarchical | non-hierarchical ] [ "?" query ]? [ "#" fragment ]?
    ///   non-hierarchical = non-hierarchical-path
    ///   non-hierarchical-path = /* Does not start with "/" */
    ///   hierarchical = authority? hierarchical-path
    ///   authority = "//" userinfo? host [ ":" port ]?
    ///   userinfo = username [ ":" password ]? "@"
    ///   hierarchical-path = [ "/" path-segment ]+
    serialization: String,

    // Components
    scheme_end: u32,   // Before ':'
    username_end: u32, // Before ':' (if a password is given) or '@' (if not)
    host_start: u32,
    host_end: u32,
    host: HostInternal,
    port: Option<u16>,
    path_start: u32,             // Before initial '/', if any
    query_start: Option<u32>,    // Before '?', unlike Position::QueryStart
    fragment_start: Option<u32>, // Before '#', unlike Position::FragmentStart
}

/// Full configuration for the URL parser.
#[derive(Copy, Clone)]
#[must_use]
pub struct ParseOptions<'a> {
    base_url: Option<&'a Url>,
    encoding_override: EncodingOverride<'a>,
    violation_fn: Option<&'a dyn Fn(SyntaxViolation)>,
}

impl<'a> ParseOptions<'a> {
    /// Change the base URL
    ///
    /// See the notes of [`Url::join`] for more details about how this base is considered
    /// when parsing.
    pub fn base_url(mut self, new: Option<&'a Url>) -> Self {
        self.base_url = new;
        self
    }

    /// Override the character encoding of query strings.
    /// This is a legacy concept only relevant for HTML.
    pub fn encoding_override(mut self, new: EncodingOverride<'a>) -> Self {
        self.encoding_override = new;
        self
    }

    /// Call the provided function or closure for a non-fatal `SyntaxViolation`
    /// when it occurs during parsing. Note that since the provided function is
    /// `Fn`, the caller might need to utilize _interior mutability_, such as with
    /// a `RefCell`, to collect the violations.
    ///
    /// ## Example
    /// ```
    /// use std::cell::RefCell;
    /// use url::{Url, SyntaxViolation};
    /// # use url::ParseError;
    /// # fn run() -> Result<(), url::ParseError> {
    /// let violations = RefCell::new(Vec::new());
    /// let url = Url::options()
    ///     .syntax_violation_callback(Some(&|v| violations.borrow_mut().push(v)))
    ///     .parse("https:////example.com")?;
    /// assert_eq!(url.as_str(), "https://example.com/");
    /// assert_eq!(violations.into_inner(),
    ///            vec!(SyntaxViolation::ExpectedDoubleSlash));
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn syntax_violation_callback(mut self, new: Option<&'a dyn Fn(SyntaxViolation)>) -> Self {
        self.violation_fn = new;
        self
    }

    /// Parse an URL string with the configuration so far.
    pub fn parse(self, input: &str) -> Result<Url, crate::ParseError> {
        Parser {
            serialization: String::with_capacity(input.len()),
            base_url: self.base_url,
            query_encoding_override: self.encoding_override,
            violation_fn: self.violation_fn,
            context: Context::UrlParser,
        }
        .parse_url(input)
    }
}

impl Url {
    /// Parse an absolute URL from a string.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("https://example.net")?;
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// If the function can not parse an absolute URL from the given string,
    /// a [`ParseError`] variant will be returned.
    ///
    /// [`ParseError`]: enum.ParseError.html
    #[inline]
    pub fn parse(input: &str) -> Result<Url, crate::ParseError> {
        Url::options().parse(input)
    }

    /// Parse an absolute URL from a string and add params to its query string.
    ///
    /// Existing params are not removed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse_with_params("https://example.net?dont=clobberme",
    ///                                  &[("lang", "rust"), ("browser", "servo")])?;
    /// assert_eq!("https://example.net/?dont=clobberme&lang=rust&browser=servo", url.as_str());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// If the function can not parse an absolute URL from the given string,
    /// a [`ParseError`] variant will be returned.
    ///
    /// [`ParseError`]: enum.ParseError.html
    #[inline]
    pub fn parse_with_params<I, K, V>(input: &str, iter: I) -> Result<Url, crate::ParseError>
    where
        I: IntoIterator,
        I::Item: Borrow<(K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut url = Url::options().parse(input);

        if let Ok(ref mut url) = url {
            url.query_pairs_mut().extend_pairs(iter);
        }

        url
    }

    /// https://url.spec.whatwg.org/#potentially-strip-trailing-spaces-from-an-opaque-path
    fn strip_trailing_spaces_from_opaque_path(&mut self) {
        if !self.cannot_be_a_base() {
            return;
        }

        if self.fragment_start.is_some() {
            return;
        }

        if self.query_start.is_some() {
            return;
        }

        let trailing_space_count = self
            .serialization
            .chars()
            .rev()
            .take_while(|c| *c == ' ')
            .count();

        let start = self.serialization.len() - trailing_space_count;

        self.serialization.truncate(start);
    }

    /// Parse a string as an URL, with this URL as the base URL.
    ///
    /// The inverse of this is [`make_relative`].
    ///
    /// # Notes
    ///
    /// - A trailing slash is significant.
    ///   Without it, the last path component is considered to be a “file” name
    ///   to be removed to get at the “directory” that is used as the base.
    /// - A [scheme relative special URL](https://url.spec.whatwg.org/#scheme-relative-special-url-string)
    ///   as input replaces everything in the base URL after the scheme.
    /// - An absolute URL (with a scheme) as input replaces the whole base URL (even the scheme).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// // Base without a trailing slash
    /// # fn run() -> Result<(), ParseError> {
    /// let base = Url::parse("https://example.net/a/b.html")?;
    /// let url = base.join("c.png")?;
    /// assert_eq!(url.as_str(), "https://example.net/a/c.png");  // Not /a/b.html/c.png
    ///
    /// // Base with a trailing slash
    /// let base = Url::parse("https://example.net/a/b/")?;
    /// let url = base.join("c.png")?;
    /// assert_eq!(url.as_str(), "https://example.net/a/b/c.png");
    ///
    /// // Input as scheme relative special URL
    /// let base = Url::parse("https://alice.com/a")?;
    /// let url = base.join("//eve.com/b")?;
    /// assert_eq!(url.as_str(), "https://eve.com/b");
    ///
    /// // Input as absolute URL
    /// let base = Url::parse("https://alice.com/a")?;
    /// let url = base.join("http://eve.com/b")?;
    /// assert_eq!(url.as_str(), "http://eve.com/b");  // http instead of https

    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// If the function can not parse an URL from the given string
    /// with this URL as the base URL, a [`ParseError`] variant will be returned.
    ///
    /// [`ParseError`]: enum.ParseError.html
    /// [`make_relative`]: #method.make_relative
    #[inline]
    pub fn join(&self, input: &str) -> Result<Url, crate::ParseError> {
        Url::options().base_url(Some(self)).parse(input)
    }

    /// Creates a relative URL if possible, with this URL as the base URL.
    ///
    /// This is the inverse of [`join`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let base = Url::parse("https://example.net/a/b.html")?;
    /// let url = Url::parse("https://example.net/a/c.png")?;
    /// let relative = base.make_relative(&url);
    /// assert_eq!(relative.as_ref().map(|s| s.as_str()), Some("c.png"));
    ///
    /// let base = Url::parse("https://example.net/a/b/")?;
    /// let url = Url::parse("https://example.net/a/b/c.png")?;
    /// let relative = base.make_relative(&url);
    /// assert_eq!(relative.as_ref().map(|s| s.as_str()), Some("c.png"));
    ///
    /// let base = Url::parse("https://example.net/a/b/")?;
    /// let url = Url::parse("https://example.net/a/d/c.png")?;
    /// let relative = base.make_relative(&url);
    /// assert_eq!(relative.as_ref().map(|s| s.as_str()), Some("../d/c.png"));
    ///
    /// let base = Url::parse("https://example.net/a/b.html?c=d")?;
    /// let url = Url::parse("https://example.net/a/b.html?e=f")?;
    /// let relative = base.make_relative(&url);
    /// assert_eq!(relative.as_ref().map(|s| s.as_str()), Some("?e=f"));
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// If this URL can't be a base for the given URL, `None` is returned.
    /// This is for example the case if the scheme, host or port are not the same.
    ///
    /// [`join`]: #method.join
    pub fn make_relative(&self, url: &Url) -> Option<String> {
        if self.cannot_be_a_base() {
            return None;
        }

        // Scheme, host and port need to be the same
        if self.scheme() != url.scheme() || self.host() != url.host() || self.port() != url.port() {
            return None;
        }

        // We ignore username/password at this point

        // The path has to be transformed
        let mut relative = String::new();

        // Extract the filename of both URIs, these need to be handled separately
        fn extract_path_filename(s: &str) -> (&str, &str) {
            let last_slash_idx = s.rfind('/').unwrap_or(0);
            let (path, filename) = s.split_at(last_slash_idx);
            if filename.is_empty() {
                (path, "")
            } else {
                (path, &filename[1..])
            }
        }

        let (base_path, base_filename) = extract_path_filename(self.path());
        let (url_path, url_filename) = extract_path_filename(url.path());

        let mut base_path = base_path.split('/').peekable();
        let mut url_path = url_path.split('/').peekable();

        // Skip over the common prefix
        while base_path.peek().is_some() && base_path.peek() == url_path.peek() {
            base_path.next();
            url_path.next();
        }

        // Add `..` segments for the remainder of the base path
        for base_path_segment in base_path {
            // Skip empty last segments
            if base_path_segment.is_empty() {
                break;
            }

            if !relative.is_empty() {
                relative.push('/');
            }

            relative.push_str("..");
        }

        // Append the remainder of the other URI
        for url_path_segment in url_path {
            if !relative.is_empty() {
                relative.push('/');
            }

            relative.push_str(url_path_segment);
        }

        // Add the filename if they are not the same
        if !relative.is_empty() || base_filename != url_filename {
            // If the URIs filename is empty this means that it was a directory
            // so we'll have to append a '/'.
            //
            // Otherwise append it directly as the new filename.
            if url_filename.is_empty() {
                relative.push('/');
            } else {
                if !relative.is_empty() {
                    relative.push('/');
                }
                relative.push_str(url_filename);
            }
        }

        // Query and fragment are only taken from the other URI
        if let Some(query) = url.query() {
            relative.push('?');
            relative.push_str(query);
        }

        if let Some(fragment) = url.fragment() {
            relative.push('#');
            relative.push_str(fragment);
        }

        Some(relative)
    }

    /// Return a default `ParseOptions` that can fully configure the URL parser.
    ///
    /// # Examples
    ///
    /// Get default `ParseOptions`, then change base url
    ///
    /// ```rust
    /// use url::Url;
    /// # use url::ParseError;
    /// # fn run() -> Result<(), ParseError> {
    /// let options = Url::options();
    /// let api = Url::parse("https://api.example.com")?;
    /// let base_url = options.base_url(Some(&api));
    /// let version_url = base_url.parse("version.json")?;
    /// assert_eq!(version_url.as_str(), "https://api.example.com/version.json");
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn options<'a>() -> ParseOptions<'a> {
        ParseOptions {
            base_url: None,
            encoding_override: None,
            violation_fn: None,
        }
    }

    /// Return the serialization of this URL.
    ///
    /// This is fast since that serialization is already stored in the `Url` struct.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url_str = "https://example.net/";
    /// let url = Url::parse(url_str)?;
    /// assert_eq!(url.as_str(), url_str);
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.serialization
    }

    /// Return the serialization of this URL.
    ///
    /// This consumes the `Url` and takes ownership of the `String` stored in it.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url_str = "https://example.net/";
    /// let url = Url::parse(url_str)?;
    /// assert_eq!(String::from(url), url_str);
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    #[inline]
    #[deprecated(since = "2.3.0", note = "use Into<String>")]
    pub fn into_string(self) -> String {
        self.into()
    }

    /// For internal testing, not part of the public API.
    ///
    /// Methods of the `Url` struct assume a number of invariants.
    /// This checks each of these invariants and panic if one is not met.
    /// This is for testing rust-url itself.
    #[doc(hidden)]
    pub fn check_invariants(&self) -> Result<(), String> {
        macro_rules! assert {
            ($x: expr) => {
                if !$x {
                    return Err(format!(
                        "!( {} ) for URL {:?}",
                        stringify!($x),
                        self.serialization
                    ));
                }
            };
        }

        macro_rules! assert_eq {
            ($a: expr, $b: expr) => {
                {
                    let a = $a;
                    let b = $b;
                    if a != b {
                        return Err(format!("{:?} != {:?} ({} != {}) for URL {:?}",
                                           a, b, stringify!($a), stringify!($b),
                                           self.serialization))
                    }
                }
            }
        }

        assert!(self.scheme_end >= 1);
        assert!(self.byte_at(0).is_ascii_alphabetic());
        assert!(self
            .slice(1..self.scheme_end)
            .chars()
            .all(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '-' | '.')));
        assert_eq!(self.byte_at(self.scheme_end), b':');

        if self.slice(self.scheme_end + 1..).starts_with("//") {
            // URL with authority
            if self.username_end != self.serialization.len() as u32 {
                match self.byte_at(self.username_end) {
                    b':' => {
                        assert!(self.host_start >= self.username_end + 2);
                        assert_eq!(self.byte_at(self.host_start - 1), b'@');
                    }
                    b'@' => assert!(self.host_start == self.username_end + 1),
                    _ => assert_eq!(self.username_end, self.scheme_end + 3),
                }
            }
            assert!(self.host_start >= self.username_end);
            assert!(self.host_end >= self.host_start);
            let host_str = self.slice(self.host_start..self.host_end);
            match self.host {
                HostInternal::None => assert_eq!(host_str, ""),
                HostInternal::Ipv4(address) => assert_eq!(host_str, address.to_string()),
                HostInternal::Ipv6(address) => {
                    let h: Host<String> = Host::Ipv6(address);
                    assert_eq!(host_str, h.to_string())
                }
                HostInternal::Domain => {
                    if SchemeType::from(self.scheme()).is_special() {
                        assert!(!host_str.is_empty())
                    }
                }
            }
            if self.path_start == self.host_end {
                assert_eq!(self.port, None);
            } else {
                assert_eq!(self.byte_at(self.host_end), b':');
                let port_str = self.slice(self.host_end + 1..self.path_start);
                assert_eq!(
                    self.port,
                    Some(port_str.parse::<u16>().expect("Couldn't parse port?"))
                );
            }
            assert!(
                self.path_start as usize == self.serialization.len()
                    || matches!(self.byte_at(self.path_start), b'/' | b'#' | b'?')
            );
        } else {
            // Anarchist URL (no authority)
            assert_eq!(self.username_end, self.scheme_end + 1);
            assert_eq!(self.host_start, self.scheme_end + 1);
            assert_eq!(self.host_end, self.scheme_end + 1);
            assert_eq!(self.host, HostInternal::None);
            assert_eq!(self.port, None);
            if self.path().starts_with("//") {
                // special case when first path segment is empty
                assert_eq!(self.byte_at(self.scheme_end + 1), b'/');
                assert_eq!(self.byte_at(self.scheme_end + 2), b'.');
                assert_eq!(self.path_start, self.scheme_end + 3);
            } else {
                assert_eq!(self.path_start, self.scheme_end + 1);
            }
        }
        if let Some(start) = self.query_start {
            assert!(start >= self.path_start);
            assert_eq!(self.byte_at(start), b'?');
        }
        if let Some(start) = self.fragment_start {
            assert!(start >= self.path_start);
            assert_eq!(self.byte_at(start), b'#');
        }
        if let (Some(query_start), Some(fragment_start)) = (self.query_start, self.fragment_start) {
            assert!(fragment_start > query_start);
        }

        let other = Url::parse(self.as_str()).expect("Failed to parse myself?");
        assert_eq!(&self.serialization, &other.serialization);
        assert_eq!(self.scheme_end, other.scheme_end);
        assert_eq!(self.username_end, other.username_end);
        assert_eq!(self.host_start, other.host_start);
        assert_eq!(self.host_end, other.host_end);
        assert!(
            self.host == other.host ||
                // XXX No host round-trips to empty host.
                // See https://github.com/whatwg/url/issues/79
                (self.host_str(), other.host_str()) == (None, Some(""))
        );
        assert_eq!(self.port, other.port);
        assert_eq!(self.path_start, other.path_start);
        assert_eq!(self.query_start, other.query_start);
        assert_eq!(self.fragment_start, other.fragment_start);
        Ok(())
    }

    /// Return the origin of this URL (<https://url.spec.whatwg.org/#origin>)
    ///
    /// Note: this returns an opaque origin for `file:` URLs, which causes
    /// `url.origin() != url.origin()`.
    ///
    /// # Examples
    ///
    /// URL with `ftp` scheme:
    ///
    /// ```rust
    /// use url::{Host, Origin, Url};
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("ftp://example.com/foo")?;
    /// assert_eq!(url.origin(),
    ///            Origin::Tuple("ftp".into(),
    ///                          Host::Domain("example.com".into()),
    ///                          21));
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// URL with `blob` scheme:
    ///
    /// ```rust
    /// use url::{Host, Origin, Url};
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("blob:https://example.com/foo")?;
    /// assert_eq!(url.origin(),
    ///            Origin::Tuple("https".into(),
    ///                          Host::Domain("example.com".into()),
    ///                          443));
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// URL with `file` scheme:
    ///
    /// ```rust
    /// use url::{Host, Origin, Url};
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("file:///tmp/foo")?;
    /// assert!(!url.origin().is_tuple());
    ///
    /// let other_url = Url::parse("file:///tmp/foo")?;
    /// assert!(url.origin() != other_url.origin());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// URL with other scheme:
    ///
    /// ```rust
    /// use url::{Host, Origin, Url};
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("foo:bar")?;
    /// assert!(!url.origin().is_tuple());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    #[inline]
    pub fn origin(&self) -> Origin {
        origin::url_origin(self)
    }

    /// Return the scheme of this URL, lower-cased, as an ASCII string without the ':' delimiter.
    ///
    /// # Examples
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("file:///tmp/foo")?;
    /// assert_eq!(url.scheme(), "file");
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    #[inline]
    pub fn scheme(&self) -> &str {
        self.slice(..self.scheme_end)
    }

    /// Return whether the URL is special (has a special scheme)
    ///
    /// # Examples
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// assert!(Url::parse("http:///tmp/foo")?.is_special());
    /// assert!(Url::parse("file:///tmp/foo")?.is_special());
    /// assert!(!Url::parse("moz:///tmp/foo")?.is_special());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn is_special(&self) -> bool {
        let scheme_type = SchemeType::from(self.scheme());
        scheme_type.is_special()
    }

    /// Return whether the URL has an 'authority',
    /// which can contain a username, password, host, and port number.
    ///
    /// URLs that do *not* are either path-only like `unix:/run/foo.socket`
    /// or cannot-be-a-base like `data:text/plain,Stuff`.
    ///
    /// See also the `authority` method.
    ///
    /// # Examples
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("ftp://rms@example.com")?;
    /// assert!(url.has_authority());
    ///
    /// let url = Url::parse("unix:/run/foo.socket")?;
    /// assert!(!url.has_authority());
    ///
    /// let url = Url::parse("data:text/plain,Stuff")?;
    /// assert!(!url.has_authority());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    #[inline]
    pub fn has_authority(&self) -> bool {
        debug_assert!(self.byte_at(self.scheme_end) == b':');
        self.slice(self.scheme_end..).starts_with("://")
    }

    /// Return the authority of this URL as an ASCII string.
    ///
    /// Non-ASCII domains are punycode-encoded per IDNA if this is the host
    /// of a special URL, or percent encoded for non-special URLs.
    /// IPv6 addresses are given between `[` and `]` brackets.
    /// Ports are omitted if they match the well known port of a special URL.
    ///
    /// Username and password are percent-encoded.
    ///
    /// See also the `has_authority` method.
    ///
    /// # Examples
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("unix:/run/foo.socket")?;
    /// assert_eq!(url.authority(), "");
    /// let url = Url::parse("file:///tmp/foo")?;
    /// assert_eq!(url.authority(), "");
    /// let url = Url::parse("https://user:password@example.com/tmp/foo")?;
    /// assert_eq!(url.authority(), "user:password@example.com");
    /// let url = Url::parse("irc://àlex.рф.example.com:6667/foo")?;
    /// assert_eq!(url.authority(), "%C3%A0lex.%D1%80%D1%84.example.com:6667");
    /// let url = Url::parse("http://àlex.рф.example.com:80/foo")?;
    /// assert_eq!(url.authority(), "xn--lex-8ka.xn--p1ai.example.com");
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn authority(&self) -> &str {
        let scheme_separator_len = "://".len() as u32;
        if self.has_authority() && self.path_start > self.scheme_end + scheme_separator_len {
            self.slice(self.scheme_end + scheme_separator_len..self.path_start)
        } else {
            ""
        }
    }

    /// Return whether this URL is a cannot-be-a-base URL,
    /// meaning that parsing a relative URL string with this URL as the base will return an error.
    ///
    /// This is the case if the scheme and `:` delimiter are not followed by a `/` slash,
    /// as is typically the case of `data:` and `mailto:` URLs.
    ///
    /// # Examples
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("ftp://rms@example.com")?;
    /// assert!(!url.cannot_be_a_base());
    ///
    /// let url = Url::parse("unix:/run/foo.socket")?;
    /// assert!(!url.cannot_be_a_base());
    ///
    /// let url = Url::parse("data:text/plain,Stuff")?;
    /// assert!(url.cannot_be_a_base());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    #[inline]
    pub fn cannot_be_a_base(&self) -> bool {
        !self.slice(self.scheme_end + 1..).starts_with('/')
    }

    /// Return the username for this URL (typically the empty string)
    /// as a percent-encoded ASCII string.
    ///
    /// # Examples
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("ftp://rms@example.com")?;
    /// assert_eq!(url.username(), "rms");
    ///
    /// let url = Url::parse("ftp://:secret123@example.com")?;
    /// assert_eq!(url.username(), "");
    ///
    /// let url = Url::parse("https://example.com")?;
    /// assert_eq!(url.username(), "");
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn username(&self) -> &str {
        let scheme_separator_len = "://".len() as u32;
        if self.has_authority() && self.username_end > self.scheme_end + scheme_separator_len {
            self.slice(self.scheme_end + scheme_separator_len..self.username_end)
        } else {
            ""
        }
    }

    /// Return the password for this URL, if any, as a percent-encoded ASCII string.
    ///
    /// # Examples
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("ftp://rms:secret123@example.com")?;
    /// assert_eq!(url.password(), Some("secret123"));
    ///
    /// let url = Url::parse("ftp://:secret123@example.com")?;
    /// assert_eq!(url.password(), Some("secret123"));
    ///
    /// let url = Url::parse("ftp://rms@example.com")?;
    /// assert_eq!(url.password(), None);
    ///
    /// let url = Url::parse("https://example.com")?;
    /// assert_eq!(url.password(), None);
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn password(&self) -> Option<&str> {
        // This ':' is not the one marking a port number since a host can not be empty.
        // (Except for file: URLs, which do not have port numbers.)
        if self.has_authority()
            && self.username_end != self.serialization.len() as u32
            && self.byte_at(self.username_end) == b':'
        {
            debug_assert!(self.byte_at(self.host_start - 1) == b'@');
            Some(self.slice(self.username_end + 1..self.host_start - 1))
        } else {
            None
        }
    }

    /// Equivalent to `url.host().is_some()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("ftp://rms@example.com")?;
    /// assert!(url.has_host());
    ///
    /// let url = Url::parse("unix:/run/foo.socket")?;
    /// assert!(!url.has_host());
    ///
    /// let url = Url::parse("data:text/plain,Stuff")?;
    /// assert!(!url.has_host());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn has_host(&self) -> bool {
        !matches!(self.host, HostInternal::None)
    }

    /// Return the string representation of the host (domain or IP address) for this URL, if any.
    ///
    /// Non-ASCII domains are punycode-encoded per IDNA if this is the host
    /// of a special URL, or percent encoded for non-special URLs.
    /// IPv6 addresses are given between `[` and `]` brackets.
    ///
    /// Cannot-be-a-base URLs (typical of `data:` and `mailto:`) and some `file:` URLs
    /// don’t have a host.
    ///
    /// See also the `host` method.
    ///
    /// # Examples
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("https://127.0.0.1/index.html")?;
    /// assert_eq!(url.host_str(), Some("127.0.0.1"));
    ///
    /// let url = Url::parse("ftp://rms@example.com")?;
    /// assert_eq!(url.host_str(), Some("example.com"));
    ///
    /// let url = Url::parse("unix:/run/foo.socket")?;
    /// assert_eq!(url.host_str(), None);
    ///
    /// let url = Url::parse("data:text/plain,Stuff")?;
    /// assert_eq!(url.host_str(), None);
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn host_str(&self) -> Option<&str> {
        if self.has_host() {
            Some(self.slice(self.host_start..self.host_end))
        } else {
            None
        }
    }

    /// Return the parsed representation of the host for this URL.
    /// Non-ASCII domain labels are punycode-encoded per IDNA if this is the host
    /// of a special URL, or percent encoded for non-special URLs.
    ///
    /// Cannot-be-a-base URLs (typical of `data:` and `mailto:`) and some `file:` URLs
    /// don’t have a host.
    ///
    /// See also the `host_str` method.
    ///
    /// # Examples
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("https://127.0.0.1/index.html")?;
    /// assert!(url.host().is_some());
    ///
    /// let url = Url::parse("ftp://rms@example.com")?;
    /// assert!(url.host().is_some());
    ///
    /// let url = Url::parse("unix:/run/foo.socket")?;
    /// assert!(url.host().is_none());
    ///
    /// let url = Url::parse("data:text/plain,Stuff")?;
    /// assert!(url.host().is_none());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn host(&self) -> Option<Host<&str>> {
        match self.host {
            HostInternal::None => None,
            HostInternal::Domain => Some(Host::Domain(self.slice(self.host_start..self.host_end))),
            HostInternal::Ipv4(address) => Some(Host::Ipv4(address)),
            HostInternal::Ipv6(address) => Some(Host::Ipv6(address)),
        }
    }

    /// If this URL has a host and it is a domain name (not an IP address), return it.
    /// Non-ASCII domains are punycode-encoded per IDNA if this is the host
    /// of a special URL, or percent encoded for non-special URLs.
    ///
    /// # Examples
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("https://127.0.0.1/")?;
    /// assert_eq!(url.domain(), None);
    ///
    /// let url = Url::parse("mailto:rms@example.net")?;
    /// assert_eq!(url.domain(), None);
    ///
    /// let url = Url::parse("https://example.com/")?;
    /// assert_eq!(url.domain(), Some("example.com"));
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn domain(&self) -> Option<&str> {
        match self.host {
            HostInternal::Domain => Some(self.slice(self.host_start..self.host_end)),
            _ => None,
        }
    }

    /// Return the port number for this URL, if any.
    ///
    /// Note that default port numbers are never reflected by the serialization,
    /// use the `port_or_known_default()` method if you want a default port number returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("https://example.com")?;
    /// assert_eq!(url.port(), None);
    ///
    /// let url = Url::parse("https://example.com:443/")?;
    /// assert_eq!(url.port(), None);
    ///
    /// let url = Url::parse("ssh://example.com:22")?;
    /// assert_eq!(url.port(), Some(22));
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    #[inline]
    pub fn port(&self) -> Option<u16> {
        self.port
    }

    /// Return the port number for this URL, or the default port number if it is known.
    ///
    /// This method only knows the default port number
    /// of the `http`, `https`, `ws`, `wss` and `ftp` schemes.
    ///
    /// For URLs in these schemes, this method always returns `Some(_)`.
    /// For other schemes, it is the same as `Url::port()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("foo://example.com")?;
    /// assert_eq!(url.port_or_known_default(), None);
    ///
    /// let url = Url::parse("foo://example.com:1456")?;
    /// assert_eq!(url.port_or_known_default(), Some(1456));
    ///
    /// let url = Url::parse("https://example.com")?;
    /// assert_eq!(url.port_or_known_default(), Some(443));
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    #[inline]
    pub fn port_or_known_default(&self) -> Option<u16> {
        self.port.or_else(|| parser::default_port(self.scheme()))
    }

    /// Resolve a URL’s host and port number to `SocketAddr`.
    ///
    /// If the URL has the default port number of a scheme that is unknown to this library,
    /// `default_port_number` provides an opportunity to provide the actual port number.
    /// In non-example code this should be implemented either simply as `|| None`,
    /// or by matching on the URL’s `.scheme()`.
    ///
    /// If the host is a domain, it is resolved using the standard library’s DNS support.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let url = url::Url::parse("https://example.net/").unwrap();
    /// let addrs = url.socket_addrs(|| None).unwrap();
    /// std::net::TcpStream::connect(&*addrs)
    /// # ;
    /// ```
    ///
    /// ```
    /// /// With application-specific known default port numbers
    /// fn socket_addrs(url: url::Url) -> std::io::Result<Vec<std::net::SocketAddr>> {
    ///     url.socket_addrs(|| match url.scheme() {
    ///         "socks5" | "socks5h" => Some(1080),
    ///         _ => None,
    ///     })
    /// }
    /// ```
    #[cfg(feature = "std")]
    #[cfg(any(
        unix,
        windows,
        target_os = "redox",
        target_os = "wasi",
        target_os = "hermit"
    ))]
    pub fn socket_addrs(
        &self,
        default_port_number: impl Fn() -> Option<u16>,
    ) -> io::Result<alloc::vec::Vec<SocketAddr>> {
        // Note: trying to avoid the Vec allocation by returning `impl AsRef<[SocketAddr]>`
        // causes borrowck issues because the return value borrows `default_port_number`:
        //
        // https://github.com/rust-lang/rfcs/blob/master/text/1951-expand-impl-trait.md#scoping-for-type-and-lifetime-parameters
        //
        // > This RFC proposes that *all* type parameters are considered in scope
        // > for `impl Trait` in return position

        fn io_result<T>(opt: Option<T>, message: &str) -> io::Result<T> {
            opt.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, message))
        }

        let host = io_result(self.host(), "No host name in the URL")?;
        let port = io_result(
            self.port_or_known_default().or_else(default_port_number),
            "No port number in the URL",
        )?;
        Ok(match host {
            Host::Domain(domain) => (domain, port).to_socket_addrs()?.collect(),
            Host::Ipv4(ip) => vec![(ip, port).into()],
            Host::Ipv6(ip) => vec![(ip, port).into()],
        })
    }

    /// Return the path for this URL, as a percent-encoded ASCII string.
    /// For cannot-be-a-base URLs, this is an arbitrary string that doesn’t start with '/'.
    /// For other URLs, this starts with a '/' slash
    /// and continues with slash-separated path segments.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::{Url, ParseError};
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("https://example.com/api/versions?page=2")?;
    /// assert_eq!(url.path(), "/api/versions");
    ///
    /// let url = Url::parse("https://example.com")?;
    /// assert_eq!(url.path(), "/");
    ///
    /// let url = Url::parse("https://example.com/countries/việt nam")?;
    /// assert_eq!(url.path(), "/countries/vi%E1%BB%87t%20nam");
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn path(&self) -> &str {
        match (self.query_start, self.fragment_start) {
            (None, None) => self.slice(self.path_start..),
            (Some(next_component_start), _) | (None, Some(next_component_start)) => {
                self.slice(self.path_start..next_component_start)
            }
        }
    }

    /// Unless this URL is cannot-be-a-base,
    /// return an iterator of '/' slash-separated path segments,
    /// each as a percent-encoded ASCII string.
    ///
    /// Return `None` for cannot-be-a-base URLs.
    ///
    /// When `Some` is returned, the iterator always contains at least one string
    /// (which may be empty).
    ///
    /// # Examples
    ///
    /// ```
    /// use url::Url;
    ///
    /// # #[cfg(feature = "std")]
    /// # use std::error::Error;
    /// # #[cfg(not(feature = "std"))]
    /// # use core::error::Error;
    ///
    /// # fn run() -> Result<(), Box<dyn Error>> {
    /// let url = Url::parse("https://example.com/foo/bar")?;
    /// let mut path_segments = url.path_segments().ok_or_else(|| "cannot be base")?;
    /// assert_eq!(path_segments.next(), Some("foo"));
    /// assert_eq!(path_segments.next(), Some("bar"));
    /// assert_eq!(path_segments.next(), None);
    ///
    /// let url = Url::parse("https://example.com")?;
    /// let mut path_segments = url.path_segments().ok_or_else(|| "cannot be base")?;
    /// assert_eq!(path_segments.next(), Some(""));
    /// assert_eq!(path_segments.next(), None);
    ///
    /// let url = Url::parse("data:text/plain,HelloWorld")?;
    /// assert!(url.path_segments().is_none());
    ///
    /// let url = Url::parse("https://example.com/countries/việt nam")?;
    /// let mut path_segments = url.path_segments().ok_or_else(|| "cannot be base")?;
    /// assert_eq!(path_segments.next(), Some("countries"));
    /// assert_eq!(path_segments.next(), Some("vi%E1%BB%87t%20nam"));
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn path_segments(&self) -> Option<str::Split<'_, char>> {
        let path = self.path();
        path.strip_prefix('/').map(|remainder| remainder.split('/'))
    }

    /// Return this URL’s query string, if any, as a percent-encoded ASCII string.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("https://example.com/products?page=2")?;
    /// let query = url.query();
    /// assert_eq!(query, Some("page=2"));
    ///
    /// let url = Url::parse("https://example.com/products")?;
    /// let query = url.query();
    /// assert!(query.is_none());
    ///
    /// let url = Url::parse("https://example.com/?country=español")?;
    /// let query = url.query();
    /// assert_eq!(query, Some("country=espa%C3%B1ol"));
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn query(&self) -> Option<&str> {
        match (self.query_start, self.fragment_start) {
            (None, _) => None,
            (Some(query_start), None) => {
                debug_assert!(self.byte_at(query_start) == b'?');
                Some(self.slice(query_start + 1..))
            }
            (Some(query_start), Some(fragment_start)) => {
                debug_assert!(self.byte_at(query_start) == b'?');
                Some(self.slice(query_start + 1..fragment_start))
            }
        }
    }

    /// Parse the URL’s query string, if any, as `application/x-www-form-urlencoded`
    /// and return an iterator of (key, value) pairs.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::borrow::Cow;
    ///
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("https://example.com/products?page=2&sort=desc")?;
    /// let mut pairs = url.query_pairs();
    ///
    /// assert_eq!(pairs.count(), 2);
    ///
    /// assert_eq!(pairs.next(), Some((Cow::Borrowed("page"), Cow::Borrowed("2"))));
    /// assert_eq!(pairs.next(), Some((Cow::Borrowed("sort"), Cow::Borrowed("desc"))));
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```

    #[inline]
    pub fn query_pairs(&self) -> form_urlencoded::Parse<'_> {
        form_urlencoded::parse(self.query().unwrap_or("").as_bytes())
    }

    /// Return this URL’s fragment identifier, if any.
    ///
    /// A fragment is the part of the URL after the `#` symbol.
    /// The fragment is optional and, if present, contains a fragment identifier
    /// that identifies a secondary resource, such as a section heading
    /// of a document.
    ///
    /// In HTML, the fragment identifier is usually the id attribute of a an element
    /// that is scrolled to on load. Browsers typically will not send the fragment portion
    /// of a URL to the server.
    ///
    /// **Note:** the parser did *not* percent-encode this component,
    /// but the input may have been percent-encoded already.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = Url::parse("https://example.com/data.csv#row=4")?;
    ///
    /// assert_eq!(url.fragment(), Some("row=4"));
    ///
    /// let url = Url::parse("https://example.com/data.csv#cell=4,1-6,2")?;
    ///
    /// assert_eq!(url.fragment(), Some("cell=4,1-6,2"));
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn fragment(&self) -> Option<&str> {
        self.fragment_start.map(|start| {
            debug_assert!(self.byte_at(start) == b'#');
            self.slice(start + 1..)
        })
    }

    fn mutate<F: FnOnce(&mut Parser<'_>) -> R, R>(&mut self, f: F) -> R {
        let mut parser = Parser::for_setter(mem::take(&mut self.serialization));
        let result = f(&mut parser);
        self.serialization = parser.serialization;
        result
    }

    /// Change this URL’s fragment identifier.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("https://example.com/data.csv")?;
    /// assert_eq!(url.as_str(), "https://example.com/data.csv");

    /// url.set_fragment(Some("cell=4,1-6,2"));
    /// assert_eq!(url.as_str(), "https://example.com/data.csv#cell=4,1-6,2");
    /// assert_eq!(url.fragment(), Some("cell=4,1-6,2"));
    ///
    /// url.set_fragment(None);
    /// assert_eq!(url.as_str(), "https://example.com/data.csv");
    /// assert!(url.fragment().is_none());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn set_fragment(&mut self, fragment: Option<&str>) {
        // Remove any previous fragment
        if let Some(start) = self.fragment_start {
            debug_assert!(self.byte_at(start) == b'#');
            self.serialization.truncate(start as usize);
        }
        // Write the new one
        if let Some(input) = fragment {
            self.fragment_start = Some(to_u32(self.serialization.len()).unwrap());
            self.serialization.push('#');
            self.mutate(|parser| parser.parse_fragment(parser::Input::new_no_trim(input)))
        } else {
            self.fragment_start = None;
            self.strip_trailing_spaces_from_opaque_path();
        }
    }

    fn take_fragment(&mut self) -> Option<String> {
        self.fragment_start.take().map(|start| {
            debug_assert!(self.byte_at(start) == b'#');
            let fragment = self.slice(start + 1..).to_owned();
            self.serialization.truncate(start as usize);
            fragment
        })
    }

    fn restore_already_parsed_fragment(&mut self, fragment: Option<String>) {
        if let Some(ref fragment) = fragment {
            assert!(self.fragment_start.is_none());
            self.fragment_start = Some(to_u32(self.serialization.len()).unwrap());
            self.serialization.push('#');
            self.serialization.push_str(fragment);
        }
    }

    /// Change this URL’s query string. If `query` is `None`, this URL's
    /// query string will be cleared.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("https://example.com/products")?;
    /// assert_eq!(url.as_str(), "https://example.com/products");
    ///
    /// url.set_query(Some("page=2"));
    /// assert_eq!(url.as_str(), "https://example.com/products?page=2");
    /// assert_eq!(url.query(), Some("page=2"));
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn set_query(&mut self, query: Option<&str>) {
        let fragment = self.take_fragment();

        // Remove any previous query
        if let Some(start) = self.query_start.take() {
            debug_assert!(self.byte_at(start) == b'?');
            self.serialization.truncate(start as usize);
        }
        // Write the new query, if any
        if let Some(input) = query {
            self.query_start = Some(to_u32(self.serialization.len()).unwrap());
            self.serialization.push('?');
            let scheme_type = SchemeType::from(self.scheme());
            let scheme_end = self.scheme_end;
            self.mutate(|parser| {
                let vfn = parser.violation_fn;
                parser.parse_query(
                    scheme_type,
                    scheme_end,
                    parser::Input::new_trim_tab_and_newlines(input, vfn),
                )
            });
        } else {
            self.query_start = None;
            if fragment.is_none() {
                self.strip_trailing_spaces_from_opaque_path();
            }
        }

        self.restore_already_parsed_fragment(fragment);
    }

    /// Manipulate this URL’s query string, viewed as a sequence of name/value pairs
    /// in `application/x-www-form-urlencoded` syntax.
    ///
    /// The return value has a method-chaining API:
    ///
    /// ```rust
    /// # use url::{Url, ParseError};
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("https://example.net?lang=fr#nav")?;
    /// assert_eq!(url.query(), Some("lang=fr"));
    ///
    /// url.query_pairs_mut().append_pair("foo", "bar");
    /// assert_eq!(url.query(), Some("lang=fr&foo=bar"));
    /// assert_eq!(url.as_str(), "https://example.net/?lang=fr&foo=bar#nav");
    ///
    /// url.query_pairs_mut()
    ///     .clear()
    ///     .append_pair("foo", "bar & baz")
    ///     .append_pair("saisons", "\u{00C9}t\u{00E9}+hiver");
    /// assert_eq!(url.query(), Some("foo=bar+%26+baz&saisons=%C3%89t%C3%A9%2Bhiver"));
    /// assert_eq!(url.as_str(),
    ///            "https://example.net/?foo=bar+%26+baz&saisons=%C3%89t%C3%A9%2Bhiver#nav");
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// Note: `url.query_pairs_mut().clear();` is equivalent to `url.set_query(Some(""))`,
    /// not `url.set_query(None)`.
    ///
    /// The state of `Url` is unspecified if this return value is leaked without being dropped.
    pub fn query_pairs_mut(&mut self) -> form_urlencoded::Serializer<'_, UrlQuery<'_>> {
        let fragment = self.take_fragment();

        let query_start;
        if let Some(start) = self.query_start {
            debug_assert!(self.byte_at(start) == b'?');
            query_start = start as usize;
        } else {
            query_start = self.serialization.len();
            self.query_start = Some(to_u32(query_start).unwrap());
            self.serialization.push('?');
        }

        let query = UrlQuery {
            url: Some(self),
            fragment,
        };
        form_urlencoded::Serializer::for_suffix(query, query_start + "?".len())
    }

    fn take_after_path(&mut self) -> String {
        match (self.query_start, self.fragment_start) {
            (Some(i), _) | (None, Some(i)) => {
                let after_path = self.slice(i..).to_owned();
                self.serialization.truncate(i as usize);
                after_path
            }
            (None, None) => String::new(),
        }
    }

    /// Change this URL’s path.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("https://example.com")?;
    /// url.set_path("api/comments");
    /// assert_eq!(url.as_str(), "https://example.com/api/comments");
    /// assert_eq!(url.path(), "/api/comments");
    ///
    /// let mut url = Url::parse("https://example.com/api")?;
    /// url.set_path("data/report.csv");
    /// assert_eq!(url.as_str(), "https://example.com/data/report.csv");
    /// assert_eq!(url.path(), "/data/report.csv");
    ///
    /// // `set_path` percent-encodes the given string if it's not already percent-encoded.
    /// let mut url = Url::parse("https://example.com")?;
    /// url.set_path("api/some comments");
    /// assert_eq!(url.as_str(), "https://example.com/api/some%20comments");
    /// assert_eq!(url.path(), "/api/some%20comments");
    ///
    /// // `set_path` will not double percent-encode the string if it's already percent-encoded.
    /// let mut url = Url::parse("https://example.com")?;
    /// url.set_path("api/some%20comments");
    /// assert_eq!(url.as_str(), "https://example.com/api/some%20comments");
    /// assert_eq!(url.path(), "/api/some%20comments");
    ///
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn set_path(&mut self, mut path: &str) {
        let after_path = self.take_after_path();
        let old_after_path_pos = to_u32(self.serialization.len()).unwrap();
        let cannot_be_a_base = self.cannot_be_a_base();
        let scheme_type = SchemeType::from(self.scheme());
        self.serialization.truncate(self.path_start as usize);
        self.mutate(|parser| {
            if cannot_be_a_base {
                if path.starts_with('/') {
                    parser.serialization.push_str("%2F");
                    path = &path[1..];
                }
                parser.parse_cannot_be_a_base_path(parser::Input::new_no_trim(path));
            } else {
                let mut has_host = true; // FIXME
                parser.parse_path_start(
                    scheme_type,
                    &mut has_host,
                    parser::Input::new_no_trim(path),
                );
            }
        });
        self.restore_after_path(old_after_path_pos, &after_path);
    }

    /// Return an object with methods to manipulate this URL’s path segments.
    ///
    /// Return `Err(())` if this URL is cannot-be-a-base.
    #[allow(clippy::result_unit_err)]
    pub fn path_segments_mut(&mut self) -> Result<PathSegmentsMut<'_>, ()> {
        if self.cannot_be_a_base() {
            Err(())
        } else {
            Ok(path_segments::new(self))
        }
    }

    fn restore_after_path(&mut self, old_after_path_position: u32, after_path: &str) {
        let new_after_path_position = to_u32(self.serialization.len()).unwrap();
        let adjust = |index: &mut u32| {
            *index -= old_after_path_position;
            *index += new_after_path_position;
        };
        if let Some(ref mut index) = self.query_start {
            adjust(index)
        }
        if let Some(ref mut index) = self.fragment_start {
            adjust(index)
        }
        self.serialization.push_str(after_path)
    }

    /// Change this URL’s port number.
    ///
    /// Note that default port numbers are not reflected in the serialization.
    ///
    /// If this URL is cannot-be-a-base, does not have a host, or has the `file` scheme;
    /// do nothing and return `Err`.
    ///
    /// # Examples
    ///
    /// ```
    /// use url::Url;
    ///
    /// # #[cfg(feature = "std")]
    /// # use std::error::Error;
    /// # #[cfg(not(feature = "std"))]
    /// # use core::error::Error;
    ///
    /// # fn run() -> Result<(), Box<dyn Error>> {
    /// let mut url = Url::parse("ssh://example.net:2048/")?;
    ///
    /// url.set_port(Some(4096)).map_err(|_| "cannot be base")?;
    /// assert_eq!(url.as_str(), "ssh://example.net:4096/");
    ///
    /// url.set_port(None).map_err(|_| "cannot be base")?;
    /// assert_eq!(url.as_str(), "ssh://example.net/");
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// Known default port numbers are not reflected:
    ///
    /// ```rust
    /// use url::Url;
    ///
    /// # #[cfg(feature = "std")]
    /// # use std::error::Error;
    /// # #[cfg(not(feature = "std"))]
    /// # use core::error::Error;
    ///
    /// # fn run() -> Result<(), Box<dyn Error>> {
    /// let mut url = Url::parse("https://example.org/")?;
    ///
    /// url.set_port(Some(443)).map_err(|_| "cannot be base")?;
    /// assert!(url.port().is_none());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// Cannot set port for cannot-be-a-base URLs:
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("mailto:rms@example.net")?;
    ///
    /// let result = url.set_port(Some(80));
    /// assert!(result.is_err());
    ///
    /// let result = url.set_port(None);
    /// assert!(result.is_err());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    #[allow(clippy::result_unit_err)]
    pub fn set_port(&mut self, mut port: Option<u16>) -> Result<(), ()> {
        // has_host implies !cannot_be_a_base
        if !self.has_host() || self.host() == Some(Host::Domain("")) || self.scheme() == "file" {
            return Err(());
        }
        if port.is_some() && port == parser::default_port(self.scheme()) {
            port = None
        }
        self.set_port_internal(port);
        Ok(())
    }

    fn set_port_internal(&mut self, port: Option<u16>) {
        match (self.port, port) {
            (None, None) => {}
            (Some(_), None) => {
                self.serialization
                    .drain(self.host_end as usize..self.path_start as usize);
                let offset = self.path_start - self.host_end;
                self.path_start = self.host_end;
                if let Some(ref mut index) = self.query_start {
                    *index -= offset
                }
                if let Some(ref mut index) = self.fragment_start {
                    *index -= offset
                }
            }
            (Some(old), Some(new)) if old == new => {}
            (_, Some(new)) => {
                let path_and_after = self.slice(self.path_start..).to_owned();
                self.serialization.truncate(self.host_end as usize);
                write!(&mut self.serialization, ":{}", new).unwrap();
                let old_path_start = self.path_start;
                let new_path_start = to_u32(self.serialization.len()).unwrap();
                self.path_start = new_path_start;
                let adjust = |index: &mut u32| {
                    *index -= old_path_start;
                    *index += new_path_start;
                };
                if let Some(ref mut index) = self.query_start {
                    adjust(index)
                }
                if let Some(ref mut index) = self.fragment_start {
                    adjust(index)
                }
                self.serialization.push_str(&path_and_after);
            }
        }
        self.port = port;
    }

    /// Change this URL’s host.
    ///
    /// Removing the host (calling this with `None`)
    /// will also remove any username, password, and port number.
    ///
    /// # Examples
    ///
    /// Change host:
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("https://example.net")?;
    /// let result = url.set_host(Some("rust-lang.org"));
    /// assert!(result.is_ok());
    /// assert_eq!(url.as_str(), "https://rust-lang.org/");
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// Remove host:
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("foo://example.net")?;
    /// let result = url.set_host(None);
    /// assert!(result.is_ok());
    /// assert_eq!(url.as_str(), "foo:/");
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// Cannot remove host for 'special' schemes (e.g. `http`):
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("https://example.net")?;
    /// let result = url.set_host(None);
    /// assert!(result.is_err());
    /// assert_eq!(url.as_str(), "https://example.net/");
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// Cannot change or remove host for cannot-be-a-base URLs:
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("mailto:rms@example.net")?;
    ///
    /// let result = url.set_host(Some("rust-lang.org"));
    /// assert!(result.is_err());
    /// assert_eq!(url.as_str(), "mailto:rms@example.net");
    ///
    /// let result = url.set_host(None);
    /// assert!(result.is_err());
    /// assert_eq!(url.as_str(), "mailto:rms@example.net");
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// If this URL is cannot-be-a-base or there is an error parsing the given `host`,
    /// a [`ParseError`] variant will be returned.
    ///
    /// [`ParseError`]: enum.ParseError.html
    pub fn set_host(&mut self, host: Option<&str>) -> Result<(), ParseError> {
        if self.cannot_be_a_base() {
            return Err(ParseError::SetHostOnCannotBeABaseUrl);
        }

        let scheme_type = SchemeType::from(self.scheme());

        if let Some(host) = host {
            if host.is_empty() && scheme_type.is_special() && !scheme_type.is_file() {
                return Err(ParseError::EmptyHost);
            }
            let mut host_substr = host;
            // Otherwise, if c is U+003A (:) and the [] flag is unset, then
            if !host.starts_with('[') || !host.ends_with(']') {
                match host.find(':') {
                    Some(0) => {
                        // If buffer is the empty string, validation error, return failure.
                        return Err(ParseError::InvalidDomainCharacter);
                    }
                    // Let host be the result of host parsing buffer
                    Some(colon_index) => {
                        host_substr = &host[..colon_index];
                    }
                    None => {}
                }
            }
            if SchemeType::from(self.scheme()).is_special() {
                self.set_host_internal(Host::parse(host_substr)?, None);
            } else {
                self.set_host_internal(Host::parse_opaque(host_substr)?, None);
            }
        } else if self.has_host() {
            if scheme_type.is_special() && !scheme_type.is_file() {
                return Err(ParseError::EmptyHost);
            } else if self.serialization.len() == self.path_start as usize {
                self.serialization.push('/');
            }
            debug_assert!(self.byte_at(self.scheme_end) == b':');
            debug_assert!(self.byte_at(self.path_start) == b'/');

            let new_path_start = if scheme_type.is_file() {
                self.scheme_end + 3
            } else {
                self.scheme_end + 1
            };

            self.serialization
                .drain(new_path_start as usize..self.path_start as usize);
            let offset = self.path_start - new_path_start;
            self.path_start = new_path_start;
            self.username_end = new_path_start;
            self.host_start = new_path_start;
            self.host_end = new_path_start;
            self.port = None;
            if let Some(ref mut index) = self.query_start {
                *index -= offset
            }
            if let Some(ref mut index) = self.fragment_start {
                *index -= offset
            }
        }
        Ok(())
    }

    /// opt_new_port: None means leave unchanged, Some(None) means remove any port number.
    fn set_host_internal(&mut self, host: Host<String>, opt_new_port: Option<Option<u16>>) {
        let old_suffix_pos = if opt_new_port.is_some() {
            self.path_start
        } else {
            self.host_end
        };
        let suffix = self.slice(old_suffix_pos..).to_owned();
        self.serialization.truncate(self.host_start as usize);
        if !self.has_authority() {
            debug_assert!(self.slice(self.scheme_end..self.host_start) == ":");
            debug_assert!(self.username_end == self.host_start);
            self.serialization.push('/');
            self.serialization.push('/');
            self.username_end += 2;
            self.host_start += 2;
        }
        write!(&mut self.serialization, "{}", host).unwrap();
        self.host_end = to_u32(self.serialization.len()).unwrap();
        self.host = host.into();

        if let Some(new_port) = opt_new_port {
            self.port = new_port;
            if let Some(port) = new_port {
                write!(&mut self.serialization, ":{}", port).unwrap();
            }
        }
        let new_suffix_pos = to_u32(self.serialization.len()).unwrap();
        self.serialization.push_str(&suffix);

        let adjust = |index: &mut u32| {
            *index -= old_suffix_pos;
            *index += new_suffix_pos;
        };
        adjust(&mut self.path_start);
        if let Some(ref mut index) = self.query_start {
            adjust(index)
        }
        if let Some(ref mut index) = self.fragment_start {
            adjust(index)
        }
    }

    /// Change this URL’s host to the given IP address.
    ///
    /// If this URL is cannot-be-a-base, do nothing and return `Err`.
    ///
    /// Compared to `Url::set_host`, this skips the host parser.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::{Url, ParseError};
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("http://example.com")?;
    /// url.set_ip_host("127.0.0.1".parse().unwrap());
    /// assert_eq!(url.host_str(), Some("127.0.0.1"));
    /// assert_eq!(url.as_str(), "http://127.0.0.1/");
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// Cannot change URL's from mailto(cannot-be-base) to ip:
    ///
    /// ```rust
    /// use url::{Url, ParseError};
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("mailto:rms@example.com")?;
    /// let result = url.set_ip_host("127.0.0.1".parse().unwrap());
    ///
    /// assert_eq!(url.as_str(), "mailto:rms@example.com");
    /// assert!(result.is_err());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    #[allow(clippy::result_unit_err)]
    pub fn set_ip_host(&mut self, address: IpAddr) -> Result<(), ()> {
        if self.cannot_be_a_base() {
            return Err(());
        }

        let address = match address {
            IpAddr::V4(address) => Host::Ipv4(address),
            IpAddr::V6(address) => Host::Ipv6(address),
        };
        self.set_host_internal(address, None);
        Ok(())
    }

    /// Change this URL’s password.
    ///
    /// If this URL is cannot-be-a-base or does not have a host, do nothing and return `Err`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::{Url, ParseError};
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("mailto:rmz@example.com")?;
    /// let result = url.set_password(Some("secret_password"));
    /// assert!(result.is_err());
    ///
    /// let mut url = Url::parse("ftp://user1:secret1@example.com")?;
    /// let result = url.set_password(Some("secret_password"));
    /// assert_eq!(url.password(), Some("secret_password"));
    ///
    /// let mut url = Url::parse("ftp://user2:@example.com")?;
    /// let result = url.set_password(Some("secret2"));
    /// assert!(result.is_ok());
    /// assert_eq!(url.password(), Some("secret2"));
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    #[allow(clippy::result_unit_err)]
    pub fn set_password(&mut self, password: Option<&str>) -> Result<(), ()> {
        // has_host implies !cannot_be_a_base
        if !self.has_host() || self.host() == Some(Host::Domain("")) || self.scheme() == "file" {
            return Err(());
        }
        let password = password.unwrap_or_default();
        if !password.is_empty() {
            let host_and_after = self.slice(self.host_start..).to_owned();
            self.serialization.truncate(self.username_end as usize);
            self.serialization.push(':');
            self.serialization
                .extend(utf8_percent_encode(password, USERINFO));
            self.serialization.push('@');

            let old_host_start = self.host_start;
            let new_host_start = to_u32(self.serialization.len()).unwrap();
            let adjust = |index: &mut u32| {
                *index -= old_host_start;
                *index += new_host_start;
            };
            self.host_start = new_host_start;
            adjust(&mut self.host_end);
            adjust(&mut self.path_start);
            if let Some(ref mut index) = self.query_start {
                adjust(index)
            }
            if let Some(ref mut index) = self.fragment_start {
                adjust(index)
            }

            self.serialization.push_str(&host_and_after);
        } else if self.byte_at(self.username_end) == b':' {
            // If there is a password to remove
            let has_username_or_password = self.byte_at(self.host_start - 1) == b'@';
            debug_assert!(has_username_or_password);
            let username_start = self.scheme_end + 3;
            let empty_username = username_start == self.username_end;
            let start = self.username_end; // Remove the ':'
            let end = if empty_username {
                self.host_start // Remove the '@' as well
            } else {
                self.host_start - 1 // Keep the '@' to separate the username from the host
            };
            self.serialization.drain(start as usize..end as usize);
            let offset = end - start;
            self.host_start -= offset;
            self.host_end -= offset;
            self.path_start -= offset;
            if let Some(ref mut index) = self.query_start {
                *index -= offset
            }
            if let Some(ref mut index) = self.fragment_start {
                *index -= offset
            }
        }
        Ok(())
    }

    /// Change this URL’s username.
    ///
    /// If this URL is cannot-be-a-base or does not have a host, do nothing and return `Err`.
    /// # Examples
    ///
    /// Cannot setup username from mailto(cannot-be-base)
    ///
    /// ```rust
    /// use url::{Url, ParseError};
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("mailto:rmz@example.com")?;
    /// let result = url.set_username("user1");
    /// assert_eq!(url.as_str(), "mailto:rmz@example.com");
    /// assert!(result.is_err());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// Setup username to user1
    ///
    /// ```rust
    /// use url::{Url, ParseError};
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("ftp://:secre1@example.com/")?;
    /// let result = url.set_username("user1");
    /// assert!(result.is_ok());
    /// assert_eq!(url.username(), "user1");
    /// assert_eq!(url.as_str(), "ftp://user1:secre1@example.com/");
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    #[allow(clippy::result_unit_err)]
    pub fn set_username(&mut self, username: &str) -> Result<(), ()> {
        // has_host implies !cannot_be_a_base
        if !self.has_host() || self.host() == Some(Host::Domain("")) || self.scheme() == "file" {
            return Err(());
        }
        let username_start = self.scheme_end + 3;
        debug_assert!(self.slice(self.scheme_end..username_start) == "://");
        if self.slice(username_start..self.username_end) == username {
            return Ok(());
        }
        let after_username = self.slice(self.username_end..).to_owned();
        self.serialization.truncate(username_start as usize);
        self.serialization
            .extend(utf8_percent_encode(username, USERINFO));

        let mut removed_bytes = self.username_end;
        self.username_end = to_u32(self.serialization.len()).unwrap();
        let mut added_bytes = self.username_end;

        let new_username_is_empty = self.username_end == username_start;
        match (new_username_is_empty, after_username.chars().next()) {
            (true, Some('@')) => {
                removed_bytes += 1;
                self.serialization.push_str(&after_username[1..]);
            }
            (false, Some('@')) | (_, Some(':')) | (true, _) => {
                self.serialization.push_str(&after_username);
            }
            (false, _) => {
                added_bytes += 1;
                self.serialization.push('@');
                self.serialization.push_str(&after_username);
            }
        }

        let adjust = |index: &mut u32| {
            *index -= removed_bytes;
            *index += added_bytes;
        };
        adjust(&mut self.host_start);
        adjust(&mut self.host_end);
        adjust(&mut self.path_start);
        if let Some(ref mut index) = self.query_start {
            adjust(index)
        }
        if let Some(ref mut index) = self.fragment_start {
            adjust(index)
        }
        Ok(())
    }

    /// Change this URL’s scheme.
    ///
    /// Do nothing and return `Err` under the following circumstances:
    ///
    /// * If the new scheme is not in `[a-zA-Z][a-zA-Z0-9+.-]+`
    /// * If this URL is cannot-be-a-base and the new scheme is one of
    ///   `http`, `https`, `ws`, `wss` or `ftp`
    /// * If either the old or new scheme is `http`, `https`, `ws`,
    ///   `wss` or `ftp` and the other is not one of these
    /// * If the new scheme is `file` and this URL includes credentials
    ///   or has a non-null port
    /// * If this URL's scheme is `file` and its host is empty or null
    ///
    /// See also [the URL specification's section on legal scheme state
    /// overrides](https://url.spec.whatwg.org/#scheme-state).
    ///
    /// # Examples
    ///
    /// Change the URL’s scheme from `https` to `http`:
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("https://example.net")?;
    /// let result = url.set_scheme("http");
    /// assert_eq!(url.as_str(), "http://example.net/");
    /// assert!(result.is_ok());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    /// Change the URL’s scheme from `foo` to `bar`:
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("foo://example.net")?;
    /// let result = url.set_scheme("bar");
    /// assert_eq!(url.as_str(), "bar://example.net");
    /// assert!(result.is_ok());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// Cannot change URL’s scheme from `https` to `foõ`:
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("https://example.net")?;
    /// let result = url.set_scheme("foõ");
    /// assert_eq!(url.as_str(), "https://example.net/");
    /// assert!(result.is_err());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    ///
    /// Cannot change URL’s scheme from `mailto` (cannot-be-a-base) to `https`:
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("mailto:rms@example.net")?;
    /// let result = url.set_scheme("https");
    /// assert_eq!(url.as_str(), "mailto:rms@example.net");
    /// assert!(result.is_err());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    /// Cannot change the URL’s scheme from `foo` to `https`:
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("foo://example.net")?;
    /// let result = url.set_scheme("https");
    /// assert_eq!(url.as_str(), "foo://example.net");
    /// assert!(result.is_err());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    /// Cannot change the URL’s scheme from `http` to `foo`:
    ///
    /// ```
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("http://example.net")?;
    /// let result = url.set_scheme("foo");
    /// assert_eq!(url.as_str(), "http://example.net/");
    /// assert!(result.is_err());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    #[allow(clippy::result_unit_err, clippy::suspicious_operation_groupings)]
    pub fn set_scheme(&mut self, scheme: &str) -> Result<(), ()> {
        let mut parser = Parser::for_setter(String::new());
        let remaining = parser.parse_scheme(parser::Input::new_no_trim(scheme))?;
        let new_scheme_type = SchemeType::from(&parser.serialization);
        let old_scheme_type = SchemeType::from(self.scheme());
        // If url’s scheme is a special scheme and buffer is not a special scheme, then return.
        if (new_scheme_type.is_special() && !old_scheme_type.is_special()) ||
            // If url’s scheme is not a special scheme and buffer is a special scheme, then return.
            (!new_scheme_type.is_special() && old_scheme_type.is_special()) ||
            // If url includes credentials or has a non-null port, and buffer is "file", then return.
            // If url’s scheme is "file" and its host is an empty host or null, then return.
            (new_scheme_type.is_file() && self.has_authority())
        {
            return Err(());
        }

        if !remaining.is_empty() || (!self.has_host() && new_scheme_type.is_special()) {
            return Err(());
        }
        let old_scheme_end = self.scheme_end;
        let new_scheme_end = to_u32(parser.serialization.len()).unwrap();
        let adjust = |index: &mut u32| {
            *index -= old_scheme_end;
            *index += new_scheme_end;
        };

        self.scheme_end = new_scheme_end;
        adjust(&mut self.username_end);
        adjust(&mut self.host_start);
        adjust(&mut self.host_end);
        adjust(&mut self.path_start);
        if let Some(ref mut index) = self.query_start {
            adjust(index)
        }
        if let Some(ref mut index) = self.fragment_start {
            adjust(index)
        }

        parser.serialization.push_str(self.slice(old_scheme_end..));
        self.serialization = parser.serialization;

        // Update the port so it can be removed
        // If it is the scheme's default
        // we don't mind it silently failing
        // if there was no port in the first place
        let previous_port = self.port();
        let _ = self.set_port(previous_port);

        Ok(())
    }

    /// Convert a file name as `std::path::Path` into an URL in the `file` scheme.
    ///
    /// This returns `Err` if the given path is not absolute or,
    /// on Windows, if the prefix is not a disk prefix (e.g. `C:`) or a UNC prefix (`\\`).
    ///
    /// # Examples
    ///
    /// On Unix-like platforms:
    ///
    /// ```
    /// # if cfg!(unix) {
    /// use url::Url;
    ///
    /// # fn run() -> Result<(), ()> {
    /// let url = Url::from_file_path("/tmp/foo.txt")?;
    /// assert_eq!(url.as_str(), "file:///tmp/foo.txt");
    ///
    /// let url = Url::from_file_path("../foo.txt");
    /// assert!(url.is_err());
    ///
    /// let url = Url::from_file_path("https://google.com/");
    /// assert!(url.is_err());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// # }
    /// ```
    ///
    /// This method is only available if the `std` Cargo feature is enabled.
    #[cfg(all(
        feature = "std",
        any(
            unix,
            windows,
            target_os = "redox",
            target_os = "wasi",
            target_os = "hermit"
        )
    ))]
    #[allow(clippy::result_unit_err)]
    pub fn from_file_path<P: AsRef<std::path::Path>>(path: P) -> Result<Url, ()> {
        let mut serialization = "file://".to_owned();
        let host_start = serialization.len() as u32;
        let (host_end, host) = path_to_file_url_segments(path.as_ref(), &mut serialization)?;
        Ok(Url {
            serialization,
            scheme_end: "file".len() as u32,
            username_end: host_start,
            host_start,
            host_end,
            host,
            port: None,
            path_start: host_end,
            query_start: None,
            fragment_start: None,
        })
    }

    /// Convert a directory name as `std::path::Path` into an URL in the `file` scheme.
    ///
    /// This returns `Err` if the given path is not absolute or,
    /// on Windows, if the prefix is not a disk prefix (e.g. `C:`) or a UNC prefix (`\\`).
    ///
    /// Compared to `from_file_path`, this ensure that URL’s the path has a trailing slash
    /// so that the entire path is considered when using this URL as a base URL.
    ///
    /// For example:
    ///
    /// * `"index.html"` parsed with `Url::from_directory_path(Path::new("/var/www"))`
    ///   as the base URL is `file:///var/www/index.html`
    /// * `"index.html"` parsed with `Url::from_file_path(Path::new("/var/www"))`
    ///   as the base URL is `file:///var/index.html`, which might not be what was intended.
    ///
    /// Note that `std::path` does not consider trailing slashes significant
    /// and usually does not include them (e.g. in `Path::parent()`).
    ///
    /// This method is only available if the `std` Cargo feature is enabled.
    #[cfg(all(
        feature = "std",
        any(
            unix,
            windows,
            target_os = "redox",
            target_os = "wasi",
            target_os = "hermit"
        )
    ))]
    #[allow(clippy::result_unit_err)]
    pub fn from_directory_path<P: AsRef<std::path::Path>>(path: P) -> Result<Url, ()> {
        let mut url = Url::from_file_path(path)?;
        if !url.serialization.ends_with('/') {
            url.serialization.push('/')
        }
        Ok(url)
    }

    /// Serialize with Serde using the internal representation of the `Url` struct.
    ///
    /// The corresponding `deserialize_internal` method sacrifices some invariant-checking
    /// for speed, compared to the `Deserialize` trait impl.
    ///
    /// This method is only available if the `serde` Cargo feature is enabled.
    #[cfg(feature = "serde")]
    #[deny(unused)]
    pub fn serialize_internal<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::Serialize;
        // Destructuring first lets us ensure that adding or removing fields forces this method
        // to be updated
        let Url {
            ref serialization,
            ref scheme_end,
            ref username_end,
            ref host_start,
            ref host_end,
            ref host,
            ref port,
            ref path_start,
            ref query_start,
            ref fragment_start,
        } = *self;
        (
            serialization,
            scheme_end,
            username_end,
            host_start,
            host_end,
            host,
            port,
            path_start,
            query_start,
            fragment_start,
        )
            .serialize(serializer)
    }

    /// Serialize with Serde using the internal representation of the `Url` struct.
    ///
    /// The corresponding `deserialize_internal` method sacrifices some invariant-checking
    /// for speed, compared to the `Deserialize` trait impl.
    ///
    /// This method is only available if the `serde` Cargo feature is enabled.
    #[cfg(feature = "serde")]
    #[deny(unused)]
    pub fn deserialize_internal<'de, D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Deserialize, Error};
        let (
            serialization,
            scheme_end,
            username_end,
            host_start,
            host_end,
            host,
            port,
            path_start,
            query_start,
            fragment_start,
        ) = Deserialize::deserialize(deserializer)?;
        let url = Url {
            serialization,
            scheme_end,
            username_end,
            host_start,
            host_end,
            host,
            port,
            path_start,
            query_start,
            fragment_start,
        };
        if cfg!(debug_assertions) {
            url.check_invariants()
                .map_err(|reason| Error::custom(reason))?
        }
        Ok(url)
    }

    /// Assuming the URL is in the `file` scheme or similar,
    /// convert its path to an absolute `std::path::Path`.
    ///
    /// **Note:** This does not actually check the URL’s `scheme`,
    /// and may give nonsensical results for other schemes.
    /// It is the user’s responsibility to check the URL’s scheme before calling this.
    ///
    /// ```
    /// # use url::Url;
    /// # let url = Url::parse("file:///etc/passwd").unwrap();
    /// let path = url.to_file_path();
    /// ```
    ///
    /// Returns `Err` if the host is neither empty nor `"localhost"` (except on Windows, where
    /// `file:` URLs may have a non-local host),
    /// or if `Path::new_opt()` returns `None`.
    /// (That is, if the percent-decoded path contains a NUL byte or,
    /// for a Windows path, is not UTF-8.)
    ///
    /// This method is only available if the `std` Cargo feature is enabled.
    #[inline]
    #[cfg(all(
        feature = "std",
        any(
            unix,
            windows,
            target_os = "redox",
            target_os = "wasi",
            target_os = "hermit"
        )
    ))]
    #[allow(clippy::result_unit_err)]
    pub fn to_file_path(&self) -> Result<PathBuf, ()> {
        if let Some(segments) = self.path_segments() {
            let host = match self.host() {
                None | Some(Host::Domain("localhost")) => None,
                Some(_) if cfg!(windows) && self.scheme() == "file" => {
                    Some(&self.serialization[self.host_start as usize..self.host_end as usize])
                }
                _ => return Err(()),
            };

            return file_url_segments_to_pathbuf(host, segments);
        }
        Err(())
    }

    // Private helper methods:

    #[inline]
    fn slice<R>(&self, range: R) -> &str
    where
        R: RangeArg,
    {
        range.slice_of(&self.serialization)
    }

    #[inline]
    fn byte_at(&self, i: u32) -> u8 {
        self.serialization.as_bytes()[i as usize]
    }
}

/// Parse a string as an URL, without a base URL or encoding override.
impl str::FromStr for Url {
    type Err = ParseError;

    #[inline]
    fn from_str(input: &str) -> Result<Url, crate::ParseError> {
        Url::parse(input)
    }
}

impl<'a> TryFrom<&'a str> for Url {
    type Error = ParseError;

    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        Url::parse(s)
    }
}

/// Display the serialization of this URL.
impl fmt::Display for Url {
    #[inline]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.serialization, formatter)
    }
}

/// String conversion.
impl From<Url> for String {
    fn from(value: Url) -> String {
        value.serialization
    }
}

/// Debug the serialization of this URL.
impl fmt::Debug for Url {
    #[inline]
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("Url")
            .field("scheme", &self.scheme())
            .field("cannot_be_a_base", &self.cannot_be_a_base())
            .field("username", &self.username())
            .field("password", &self.password())
            .field("host", &self.host())
            .field("port", &self.port())
            .field("path", &self.path())
            .field("query", &self.query())
            .field("fragment", &self.fragment())
            .finish()
    }
}

/// URLs compare like their serialization.
impl Eq for Url {}

/// URLs compare like their serialization.
impl PartialEq for Url {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.serialization == other.serialization
    }
}

/// URLs compare like their serialization.
impl Ord for Url {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.serialization.cmp(&other.serialization)
    }
}

/// URLs compare like their serialization.
impl PartialOrd for Url {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// URLs hash like their serialization.
impl hash::Hash for Url {
    #[inline]
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        hash::Hash::hash(&self.serialization, state)
    }
}

/// Return the serialization of this URL.
impl AsRef<str> for Url {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.serialization
    }
}

trait RangeArg {
    fn slice_of<'a>(&self, s: &'a str) -> &'a str;
}

impl RangeArg for Range<u32> {
    #[inline]
    fn slice_of<'a>(&self, s: &'a str) -> &'a str {
        &s[self.start as usize..self.end as usize]
    }
}

impl RangeArg for RangeFrom<u32> {
    #[inline]
    fn slice_of<'a>(&self, s: &'a str) -> &'a str {
        &s[self.start as usize..]
    }
}

impl RangeArg for RangeTo<u32> {
    #[inline]
    fn slice_of<'a>(&self, s: &'a str) -> &'a str {
        &s[..self.end as usize]
    }
}

/// Serializes this URL into a `serde` stream.
///
/// This implementation is only available if the `serde` Cargo feature is enabled.
#[cfg(feature = "serde")]
impl serde::Serialize for Url {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

/// Deserializes this URL from a `serde` stream.
///
/// This implementation is only available if the `serde` Cargo feature is enabled.
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Url {
    fn deserialize<D>(deserializer: D) -> Result<Url, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Error, Visitor};

        struct UrlVisitor;

        impl<'de> Visitor<'de> for UrlVisitor {
            type Value = Url;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string representing an URL")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Url::parse(s).map_err(|err| Error::custom(format!("{}: {:?}", err, s)))
            }
        }

        deserializer.deserialize_str(UrlVisitor)
    }
}

#[cfg(all(
    feature = "std",
    any(unix, target_os = "redox", target_os = "wasi", target_os = "hermit")
))]
fn path_to_file_url_segments(
    path: &Path,
    serialization: &mut String,
) -> Result<(u32, HostInternal), ()> {
    use parser::SPECIAL_PATH_SEGMENT;
    use percent_encoding::percent_encode;
    #[cfg(target_os = "hermit")]
    use std::os::hermit::ffi::OsStrExt;
    #[cfg(any(unix, target_os = "redox"))]
    use std::os::unix::prelude::OsStrExt;
    if !path.is_absolute() {
        return Err(());
    }
    let host_end = to_u32(serialization.len()).unwrap();
    let mut empty = true;
    // skip the root component
    for component in path.components().skip(1) {
        empty = false;
        serialization.push('/');
        #[cfg(not(target_os = "wasi"))]
        serialization.extend(percent_encode(
            component.as_os_str().as_bytes(),
            SPECIAL_PATH_SEGMENT,
        ));
        #[cfg(target_os = "wasi")]
        serialization.extend(percent_encode(
            component.as_os_str().to_string_lossy().as_bytes(),
            SPECIAL_PATH_SEGMENT,
        ));
    }
    if empty {
        // An URL’s path must not be empty.
        serialization.push('/');
    }
    Ok((host_end, HostInternal::None))
}

#[cfg(all(feature = "std", windows))]
fn path_to_file_url_segments(
    path: &Path,
    serialization: &mut String,
) -> Result<(u32, HostInternal), ()> {
    path_to_file_url_segments_windows(path, serialization)
}

// Build this unconditionally to alleviate https://github.com/servo/rust-url/issues/102
#[cfg(feature = "std")]
#[cfg_attr(not(windows), allow(dead_code))]
fn path_to_file_url_segments_windows(
    path: &Path,
    serialization: &mut String,
) -> Result<(u32, HostInternal), ()> {
    use crate::parser::PATH_SEGMENT;
    use percent_encoding::percent_encode;
    use std::path::{Component, Prefix};
    if !path.is_absolute() {
        return Err(());
    }
    let mut components = path.components();

    let host_start = serialization.len() + 1;
    let host_end;
    let host_internal;

    match components.next() {
        Some(Component::Prefix(ref p)) => match p.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                host_end = to_u32(serialization.len()).unwrap();
                host_internal = HostInternal::None;
                serialization.push('/');
                serialization.push(letter as char);
                serialization.push(':');
            }
            Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
                let host = Host::parse(server.to_str().ok_or(())?).map_err(|_| ())?;
                write!(serialization, "{}", host).unwrap();
                host_end = to_u32(serialization.len()).unwrap();
                host_internal = host.into();
                serialization.push('/');
                let share = share.to_str().ok_or(())?;
                serialization.extend(percent_encode(share.as_bytes(), PATH_SEGMENT));
            }
            _ => return Err(()),
        },
        _ => return Err(()),
    }

    let mut path_only_has_prefix = true;
    for component in components {
        if component == Component::RootDir {
            continue;
        }

        path_only_has_prefix = false;
        // FIXME: somehow work with non-unicode?
        let component = component.as_os_str().to_str().ok_or(())?;

        serialization.push('/');
        serialization.extend(percent_encode(component.as_bytes(), PATH_SEGMENT));
    }

    // A windows drive letter must end with a slash.
    if serialization.len() > host_start
        && parser::is_windows_drive_letter(&serialization[host_start..])
        && path_only_has_prefix
    {
        serialization.push('/');
    }

    Ok((host_end, host_internal))
}

#[cfg(all(
    feature = "std",
    any(unix, target_os = "redox", target_os = "wasi", target_os = "hermit")
))]
fn file_url_segments_to_pathbuf(
    host: Option<&str>,
    segments: str::Split<'_, char>,
) -> Result<PathBuf, ()> {
    use alloc::vec::Vec;
    use percent_encoding::percent_decode;
    #[cfg(not(target_os = "wasi"))]
    use std::ffi::OsStr;
    #[cfg(target_os = "hermit")]
    use std::os::hermit::ffi::OsStrExt;
    #[cfg(any(unix, target_os = "redox"))]
    use std::os::unix::prelude::OsStrExt;
    use std::path::PathBuf;

    if host.is_some() {
        return Err(());
    }

    let mut bytes = if cfg!(target_os = "redox") {
        b"file:".to_vec()
    } else {
        Vec::new()
    };

    for segment in segments {
        bytes.push(b'/');
        bytes.extend(percent_decode(segment.as_bytes()));
    }

    // A windows drive letter must end with a slash.
    if bytes.len() > 2
        && bytes[bytes.len() - 2].is_ascii_alphabetic()
        && matches!(bytes[bytes.len() - 1], b':' | b'|')
    {
        bytes.push(b'/');
    }

    #[cfg(not(target_os = "wasi"))]
    let path = PathBuf::from(OsStr::from_bytes(&bytes));
    #[cfg(target_os = "wasi")]
    let path = String::from_utf8(bytes)
        .map(|path| PathBuf::from(path))
        .map_err(|_| ())?;

    debug_assert!(
        path.is_absolute(),
        "to_file_path() failed to produce an absolute Path"
    );

    Ok(path)
}

#[cfg(all(feature = "std", windows))]
fn file_url_segments_to_pathbuf(
    host: Option<&str>,
    segments: str::Split<char>,
) -> Result<PathBuf, ()> {
    file_url_segments_to_pathbuf_windows(host, segments)
}

// Build this unconditionally to alleviate https://github.com/servo/rust-url/issues/102
#[cfg(feature = "std")]
#[cfg_attr(not(windows), allow(dead_code))]
fn file_url_segments_to_pathbuf_windows(
    host: Option<&str>,
    mut segments: str::Split<'_, char>,
) -> Result<PathBuf, ()> {
    use percent_encoding::percent_decode;
    let mut string = if let Some(host) = host {
        r"\\".to_owned() + host
    } else {
        let first = segments.next().ok_or(())?;

        match first.len() {
            2 => {
                if !first.starts_with(parser::ascii_alpha) || first.as_bytes()[1] != b':' {
                    return Err(());
                }

                first.to_owned()
            }

            4 => {
                if !first.starts_with(parser::ascii_alpha) {
                    return Err(());
                }
                let bytes = first.as_bytes();
                if bytes[1] != b'%' || bytes[2] != b'3' || (bytes[3] != b'a' && bytes[3] != b'A') {
                    return Err(());
                }

                first[0..1].to_owned() + ":"
            }

            _ => return Err(()),
        }
    };

    for segment in segments {
        string.push('\\');

        // Currently non-unicode windows paths cannot be represented
        match String::from_utf8(percent_decode(segment.as_bytes()).collect()) {
            Ok(s) => string.push_str(&s),
            Err(..) => return Err(()),
        }
    }
    let path = PathBuf::from(string);
    debug_assert!(
        path.is_absolute(),
        "to_file_path() failed to produce an absolute Path"
    );
    Ok(path)
}

/// Implementation detail of `Url::query_pairs_mut`. Typically not used directly.
#[derive(Debug)]
pub struct UrlQuery<'a> {
    url: Option<&'a mut Url>,
    fragment: Option<String>,
}

// `as_mut_string` string here exposes the internal serialization of an `Url`,
// which should not be exposed to users.
// We achieve that by not giving users direct access to `UrlQuery`:
// * Its fields are private
//   (and so can not be constructed with struct literal syntax outside of this crate),
// * It has no constructor
// * It is only visible (on the type level) to users in the return type of
//   `Url::query_pairs_mut` which is `Serializer<UrlQuery>`
// * `Serializer` keeps its target in a private field
// * Unlike in other `Target` impls, `UrlQuery::finished` does not return `Self`.
impl<'a> form_urlencoded::Target for UrlQuery<'a> {
    fn as_mut_string(&mut self) -> &mut String {
        &mut self.url.as_mut().unwrap().serialization
    }

    fn finish(mut self) -> &'a mut Url {
        let url = self.url.take().unwrap();
        url.restore_already_parsed_fragment(self.fragment.take());
        url
    }

    type Finished = &'a mut Url;
}

impl<'a> Drop for UrlQuery<'a> {
    fn drop(&mut self) {
        if let Some(url) = self.url.take() {
            url.restore_already_parsed_fragment(self.fragment.take())
        }
    }
}
