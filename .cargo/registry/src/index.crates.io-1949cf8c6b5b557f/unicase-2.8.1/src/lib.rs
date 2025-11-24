#![cfg_attr(test, deny(missing_docs))]
#![cfg_attr(test, deny(warnings))]
#![cfg_attr(feature = "nightly", feature(test))]
#![no_std]

//! # UniCase
//!
//! UniCase provides a way of specifying strings that are case-insensitive.
//!
//! UniCase supports full [Unicode case
//! folding](https://www.w3.org/International/wiki/Case_folding). It can also
//! utilize faster ASCII case comparisons, if both strings are ASCII.
//!
//! Using the `UniCase::new()` constructor will check the string to see if it
//! is all ASCII. When a `UniCase` is compared against another, if both are
//! ASCII, it will use the faster comparison.
//!
//! There also exists the `Ascii` type in this crate, which will always assume
//! to use the ASCII case comparisons, if the encoding is already known.
//!
//! ## Example
//!
//! ```rust
//! use unicase::UniCase;
//!
//! let a = UniCase::new("Maße");
//! let b = UniCase::new("MASSE");
//! let c = UniCase::new("mase");
//!
//! assert_eq!(a, b);
//! assert!(b != c);
//! ```
//!
//! ## Ascii
//!
//! ```rust
//! use unicase::Ascii;
//!
//! let a = Ascii::new("foobar");
//! let b = Ascii::new("FoObAr");
//!
//! assert_eq!(a, b);
//! ```

#[cfg(test)]
extern crate std;
#[cfg(feature = "nightly")]
extern crate test;

extern crate alloc;
use alloc::string::String;

use alloc::borrow::Cow;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ops::{Deref, DerefMut};
use core::str::FromStr;

use self::unicode::Unicode;

mod ascii;
mod unicode;

/// Case Insensitive wrapper of strings.
#[derive(Clone, Copy)]
pub struct UniCase<S>(Encoding<S>);

/// Case Insensitive wrapper of Ascii strings.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ascii<S>(S);

/// Compare two string-like types for case-less equality, using unicode folding.
///
/// Equivalent to `UniCase::new(left) == UniCase::new(right)`.
///
/// Note: This will perform a scan for ASCII characters before doing the
/// the comparison. See `UniCase` for more information.
#[inline]
pub fn eq<S: AsRef<str> + ?Sized>(left: &S, right: &S) -> bool {
    UniCase::new(left) == UniCase::new(right)
}

/// Compare two string-like types for case-less equality, ignoring ASCII case.
///
/// Equivalent to `Ascii::new(left) == Ascii::new(right)`.
#[inline]
pub fn eq_ascii<S: AsRef<str> + ?Sized>(left: &S, right: &S) -> bool {
    Ascii(left) == Ascii(right)
}

#[derive(Clone, Copy, Debug)]
enum Encoding<S> {
    Ascii(Ascii<S>),
    Unicode(Unicode<S>),
}

macro_rules! inner {
    (mut $e:expr) => {{
        match &mut $e {
            &mut Encoding::Ascii(ref mut s) => &mut s.0,
            &mut Encoding::Unicode(ref mut s) => &mut s.0,
        }
    }};
    ($e:expr) => {{
        match &$e {
            &Encoding::Ascii(ref s) => &s.0,
            &Encoding::Unicode(ref s) => &s.0,
        }
    }};
}

impl<S: AsRef<str> + Default> Default for UniCase<S> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<S: AsRef<str>> UniCase<S> {
    /// Creates a new `UniCase`.
    ///
    /// Note: This scans the text to determine if it is all ASCII or not.
    pub fn new(s: S) -> UniCase<S> {
        if s.as_ref().is_ascii() {
            UniCase(Encoding::Ascii(Ascii(s)))
        } else {
            UniCase(Encoding::Unicode(Unicode(s)))
        }
    }

    /// Returns a copy of this string where each character is mapped to its
    /// Unicode CaseFolding equivalent.
    ///
    /// # Note
    ///
    /// Unicode Case Folding is meant for string storage and matching, not for
    /// display.
    pub fn to_folded_case(&self) -> String {
        match self.0 {
            Encoding::Ascii(ref s) => s.0.as_ref().to_ascii_lowercase(),
            Encoding::Unicode(ref s) => s.to_folded_case(),
        }
    }
}

impl<S> UniCase<S> {
    /// Creates a new `UniCase`, skipping the ASCII check.
    pub const fn unicode(s: S) -> UniCase<S> {
        UniCase(Encoding::Unicode(Unicode(s)))
    }

    /// Creates a new `UniCase` which performs only ASCII case folding.
    pub const fn ascii(s: S) -> UniCase<S> {
        UniCase(Encoding::Ascii(Ascii(s)))
    }

    /// Return `true` if this instance will only perform ASCII case folding.
    pub fn is_ascii(&self) -> bool {
        match self.0 {
            Encoding::Ascii(_) => true,
            Encoding::Unicode(_) => false,
        }
    }

    /// Unwraps the inner value held by this `UniCase`.
    #[inline]
    pub fn into_inner(self) -> S {
        match self.0 {
            Encoding::Ascii(s) => s.0,
            Encoding::Unicode(s) => s.0,
        }
    }
}

impl<S> Deref for UniCase<S> {
    type Target = S;
    #[inline]
    fn deref<'a>(&'a self) -> &'a S {
        inner!(self.0)
    }
}

impl<S> DerefMut for UniCase<S> {
    #[inline]
    fn deref_mut<'a>(&'a mut self) -> &'a mut S {
        inner!(mut self.0)
    }
}

impl<S: AsRef<str>> AsRef<str> for UniCase<S> {
    #[inline]
    fn as_ref(&self) -> &str {
        inner!(self.0).as_ref()
    }
}

impl<S: fmt::Debug> fmt::Debug for UniCase<S> {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(inner!(self.0), fmt)
    }
}

impl<S: fmt::Display> fmt::Display for UniCase<S> {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(inner!(self.0), fmt)
    }
}

impl<S1: AsRef<str>, S2: AsRef<str>> PartialEq<UniCase<S2>> for UniCase<S1> {
    #[inline]
    fn eq(&self, other: &UniCase<S2>) -> bool {
        match (&self.0, &other.0) {
            (&Encoding::Ascii(ref x), &Encoding::Ascii(ref y)) => x == y,
            (&Encoding::Unicode(ref x), &Encoding::Unicode(ref y)) => x == y,
            (&Encoding::Ascii(ref x), &Encoding::Unicode(ref y)) => &Unicode(x.as_ref()) == y,
            (&Encoding::Unicode(ref x), &Encoding::Ascii(ref y)) => x == &Unicode(y.as_ref()),
        }
    }
}

impl<S: AsRef<str>> Eq for UniCase<S> {}

impl<S: AsRef<str>> Hash for UniCase<S> {
    #[inline]
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        match self.0 {
            Encoding::Ascii(ref s) => s.hash(hasher),
            Encoding::Unicode(ref s) => s.hash(hasher),
        }
    }
}

impl<S> From<Ascii<S>> for UniCase<S> {
    fn from(ascii: Ascii<S>) -> Self {
        UniCase(Encoding::Ascii(ascii))
    }
}

macro_rules! from_impl {
    ($from:ty => $to:ty; $by:ident) => (
        impl<'a> From<$from> for UniCase<$to> {
            fn from(s: $from) -> Self {
                UniCase::unicode(s.$by())
            }
        }
    );
    ($from:ty => $to:ty) => ( from_impl!($from => $to; into); )
}

macro_rules! into_impl {
    ($to:ty) => {
        impl<'a> Into<$to> for UniCase<$to> {
            fn into(self) -> $to {
                self.into_inner()
            }
        }
    };
}

impl<S: AsRef<str>> From<S> for UniCase<S> {
    fn from(s: S) -> Self {
        UniCase::unicode(s)
    }
}

from_impl!(&'a str => Cow<'a, str>);
from_impl!(String => Cow<'a, str>);
from_impl!(&'a str => String);
from_impl!(Cow<'a, str> => String; into_owned);
from_impl!(&'a String => &'a str; as_ref);

into_impl!(&'a str);
into_impl!(String);
into_impl!(Cow<'a, str>);

impl<T: AsRef<str>> PartialOrd for UniCase<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: AsRef<str>> Ord for UniCase<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.0, &other.0) {
            (&Encoding::Ascii(ref x), &Encoding::Ascii(ref y)) => x.cmp(y),
            (&Encoding::Unicode(ref x), &Encoding::Unicode(ref y)) => x.cmp(y),
            (&Encoding::Ascii(ref x), &Encoding::Unicode(ref y)) => {
                Unicode(x.as_ref()).cmp(&Unicode(y.0.as_ref()))
            }
            (&Encoding::Unicode(ref x), &Encoding::Ascii(ref y)) => {
                Unicode(x.0.as_ref()).cmp(&Unicode(y.as_ref()))
            }
        }
    }
}

impl<S: FromStr + AsRef<str>> FromStr for UniCase<S> {
    type Err = <S as FromStr>::Err;
    fn from_str(s: &str) -> Result<UniCase<S>, Self::Err> {
        s.parse().map(UniCase::new)
    }
}

#[cfg(test)]
mod tests {
    use super::UniCase;
    use std::borrow::ToOwned;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::string::String;

    fn hash<T: Hash>(t: &T) -> u64 {
        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        s.finish()
    }

    #[test]
    fn test_copy_for_refs() {
        fn foo<T>(_: UniCase<T>) {}

        let a = UniCase::new("foobar");
        foo(a);
        foo(a);
    }

    #[test]
    fn test_eq_ascii() {
        let a = UniCase::new("foobar");
        let b = UniCase::new("FOOBAR");
        let c = UniCase::ascii("FoObAr");

        assert_eq!(a, b);
        assert_eq!(b, a);
        assert_eq!(a, c);
        assert_eq!(c, a);
        assert_eq!(hash(&a), hash(&b));
        assert_eq!(hash(&a), hash(&c));
        assert!(a.is_ascii());
        assert!(b.is_ascii());
        assert!(c.is_ascii());
    }

    #[test]
    fn test_eq_unicode() {
        let a = UniCase::new("στιγμας");
        let b = UniCase::new("στιγμασ");
        assert_eq!(a, b);
        assert_eq!(b, a);
        assert_eq!(hash(&a), hash(&b));
    }

    #[test]
    fn test_eq_unicode_left_is_substring() {
        // https://github.com/seanmonstar/unicase/issues/38
        let a = UniCase::unicode("foo");
        let b = UniCase::unicode("foobar");

        assert!(a != b);
        assert!(b != a);
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_unicase_ascii(b: &mut ::test::Bencher) {
        b.bytes = b"foobar".len() as u64;
        let x = UniCase::new("foobar");
        let y = UniCase::new("FOOBAR");
        b.iter(|| assert_eq!(x, y));
    }

    #[cfg(feature = "nightly")]
    static SUBJECT: &'static [u8] = b"ffoo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz foo bar baz oo bar baz quux herp derp";

    #[cfg(feature = "nightly")]
    #[inline(never)]
    fn is_ascii(bytes: &[u8]) -> bool {
        #[allow(unused, deprecated)]
        use std::ascii::AsciiExt;
        bytes.is_ascii()
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_is_ascii(b: &mut ::test::Bencher) {
        b.iter(|| assert!(is_ascii(SUBJECT)));
    }

    #[cfg(feature = "nightly")]
    #[bench]
    fn bench_is_utf8(b: &mut ::test::Bencher) {
        b.iter(|| assert!(::std::str::from_utf8(SUBJECT).is_ok()));
    }

    #[test]
    fn test_case_cmp() {
        assert!(UniCase::new("a") < UniCase::new("B"));

        assert!(UniCase::new("A") < UniCase::new("b"));
        assert!(UniCase::new("aa") > UniCase::new("a"));

        assert!(UniCase::new("a") < UniCase::new("aa"));
        assert!(UniCase::new("a") < UniCase::new("AA"));
    }

    #[test]
    fn test_from_impls() {
        let view: &'static str = "foobar";
        let _: UniCase<&'static str> = view.into();
        let _: UniCase<&str> = view.into();
        let _: UniCase<String> = view.into();

        let owned: String = view.to_owned();
        let _: UniCase<&str> = (&owned).into();
        let _: UniCase<String> = owned.into();
    }

    #[test]
    fn test_into_impls() {
        let view: UniCase<&'static str> = UniCase::new("foobar");
        let _: &'static str = view.into();
        let _: &str = view.into();

        let owned: UniCase<String> = "foobar".into();
        let _: String = owned.clone().into();
        let _: &str = owned.as_ref();
    }

    #[test]
    fn test_unicase_unicode_const() {
        const _UNICASE: UniCase<&'static str> = UniCase::unicode("");
    }
}
