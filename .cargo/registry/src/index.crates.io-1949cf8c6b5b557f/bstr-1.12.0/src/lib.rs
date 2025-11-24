/*!
A byte string library.

Byte strings are just like standard Unicode strings with one very important
difference: byte strings are only *conventionally* UTF-8 while Rust's standard
Unicode strings are *guaranteed* to be valid UTF-8. The primary motivation for
byte strings is for handling arbitrary bytes that are mostly UTF-8.

# Overview

This crate provides two important traits that provide string oriented methods
on `&[u8]` and `Vec<u8>` types:

* [`ByteSlice`](trait.ByteSlice.html) extends the `[u8]` type with additional
  string oriented methods.
* [`ByteVec`](trait.ByteVec.html) extends the `Vec<u8>` type with additional
  string oriented methods.

Additionally, this crate provides two concrete byte string types that deref to
`[u8]` and `Vec<u8>`. These are useful for storing byte string types, and come
with convenient `std::fmt::Debug` implementations:

* [`BStr`](struct.BStr.html) is a byte string slice, analogous to `str`.
* [`BString`](struct.BString.html) is an owned growable byte string buffer,
  analogous to `String`.

Additionally, the free function [`B`](fn.B.html) serves as a convenient short
hand for writing byte string literals.

# Quick examples

Byte strings build on the existing APIs for `Vec<u8>` and `&[u8]`, with
additional string oriented methods. Operations such as iterating over
graphemes, searching for substrings, replacing substrings, trimming and case
conversion are examples of things not provided on the standard library `&[u8]`
APIs but are provided by this crate. For example, this code iterates over all
of occurrences of a substring:

```
use bstr::ByteSlice;

let s = b"foo bar foo foo quux foo";

let mut matches = vec![];
for start in s.find_iter("foo") {
    matches.push(start);
}
assert_eq!(matches, [0, 8, 12, 21]);
```

Here's another example showing how to do a search and replace (and also showing
use of the `B` function):

```
# #[cfg(feature = "alloc")] {
use bstr::{B, ByteSlice};

let old = B("foo ☃☃☃ foo foo quux foo");
let new = old.replace("foo", "hello");
assert_eq!(new, B("hello ☃☃☃ hello hello quux hello"));
# }
```

And here's an example that shows case conversion, even in the presence of
invalid UTF-8:

```
# #[cfg(all(feature = "alloc", feature = "unicode"))] {
use bstr::{ByteSlice, ByteVec};

let mut lower = Vec::from("hello β");
lower[0] = b'\xFF';
// lowercase β is uppercased to Β
assert_eq!(lower.to_uppercase(), b"\xFFELLO \xCE\x92");
# }
```

# Convenient debug representation

When working with byte strings, it is often useful to be able to print them
as if they were byte strings and not sequences of integers. While this crate
cannot affect the `std::fmt::Debug` implementations for `[u8]` and `Vec<u8>`,
this crate does provide the `BStr` and `BString` types which have convenient
`std::fmt::Debug` implementations.

For example, this

```
use bstr::ByteSlice;

let mut bytes = Vec::from("hello β");
bytes[0] = b'\xFF';

println!("{:?}", bytes.as_bstr());
```

will output `"\xFFello β"`.

This example works because the
[`ByteSlice::as_bstr`](trait.ByteSlice.html#method.as_bstr)
method converts any `&[u8]` to a `&BStr`.

# When should I use byte strings?

This library reflects my belief that UTF-8 by convention is a better trade
off in some circumstances than guaranteed UTF-8.

The first time this idea hit me was in the implementation of Rust's regex
engine. In particular, very little of the internal implementation cares at all
about searching valid UTF-8 encoded strings. Indeed, internally, the
implementation converts `&str` from the API to `&[u8]` fairly quickly and
just deals with raw bytes. UTF-8 match boundaries are then guaranteed by the
finite state machine itself rather than any specific string type. This makes it
possible to not only run regexes on `&str` values, but also on `&[u8]` values.

Why would you ever want to run a regex on a `&[u8]` though? Well, `&[u8]` is
the fundamental way at which one reads data from all sorts of streams, via the
standard library's [`Read`](https://doc.rust-lang.org/std/io/trait.Read.html)
trait. In particular, there is no platform independent way to determine whether
what you're reading from is some binary file or a human readable text file.
Therefore, if you're writing a program to search files, you probably need to
deal with `&[u8]` directly unless you're okay with first converting it to a
`&str` and dropping any bytes that aren't valid UTF-8. (Or otherwise determine
the encoding---which is often impractical---and perform a transcoding step.)
Often, the simplest and most robust way to approach this is to simply treat the
contents of a file as if it were mostly valid UTF-8 and pass through invalid
UTF-8 untouched. This may not be the most correct approach though!

One case in particular exacerbates these issues, and that's memory mapping
a file. When you memory map a file, that file may be gigabytes big, but all
you get is a `&[u8]`. Converting that to a `&str` all in one go is generally
not a good idea because of the costs associated with doing so, and also
because it generally causes one to do two passes over the data instead of
one, which is quite undesirable. It is of course usually possible to do it an
incremental way by only parsing chunks at a time, but this is often complex to
do or impractical. For example, many regex engines only accept one contiguous
sequence of bytes at a time with no way to perform incremental matching.

# `bstr` in public APIs

This library is past version `1` and is expected to remain at version `1` for
the foreseeable future. Therefore, it is encouraged to put types from `bstr`
(like `BStr` and `BString`) in your public API if that makes sense for your
crate.

With that said, in general, it should be possible to avoid putting anything
in this crate into your public APIs. Namely, you should never need to use the
`ByteSlice` or `ByteVec` traits as bounds on public APIs, since their only
purpose is to extend the methods on the concrete types `[u8]` and `Vec<u8>`,
respectively. Similarly, it should not be necessary to put either the `BStr` or
`BString` types into public APIs. If you want to use them internally, then they
can be converted to/from `[u8]`/`Vec<u8>` as needed. The conversions are free.

So while it shouldn't ever be 100% necessary to make `bstr` a public
dependency, there may be cases where it is convenient to do so. This is an
explicitly supported use case of `bstr`, and as such, major version releases
should be exceptionally rare.


# Differences with standard strings

The primary difference between `[u8]` and `str` is that the former is
conventionally UTF-8 while the latter is guaranteed to be UTF-8. The phrase
"conventionally UTF-8" means that a `[u8]` may contain bytes that do not form
a valid UTF-8 sequence, but operations defined on the type in this crate are
generally most useful on valid UTF-8 sequences. For example, iterating over
Unicode codepoints or grapheme clusters is an operation that is only defined
on valid UTF-8. Therefore, when invalid UTF-8 is encountered, the Unicode
replacement codepoint is substituted. Thus, a byte string that is not UTF-8 at
all is of limited utility when using these crate.

However, not all operations on byte strings are specifically Unicode aware. For
example, substring search has no specific Unicode semantics ascribed to it. It
works just as well for byte strings that are completely valid UTF-8 as for byte
strings that contain no valid UTF-8 at all. Similarly for replacements and
various other operations that do not need any Unicode specific tailoring.

Aside from the difference in how UTF-8 is handled, the APIs between `[u8]` and
`str` (and `Vec<u8>` and `String`) are intentionally very similar, including
maintaining the same behavior for corner cases in things like substring
splitting. There are, however, some differences:

* Substring search is not done with `matches`, but instead, `find_iter`.
  In general, this crate does not define any generic
  [`Pattern`](https://doc.rust-lang.org/std/str/pattern/trait.Pattern.html)
  infrastructure, and instead prefers adding new methods for different
  argument types. For example, `matches` can search by a `char` or a `&str`,
  where as `find_iter` can only search by a byte string. `find_char` can be
  used for searching by a `char`.
* Since `SliceConcatExt` in the standard library is unstable, it is not
  possible to reuse that to implement `join` and `concat` methods. Instead,
  [`join`](fn.join.html) and [`concat`](fn.concat.html) are provided as free
  functions that perform a similar task.
* This library bundles in a few more Unicode operations, such as grapheme,
  word and sentence iterators. More operations, such as normalization and
  case folding, may be provided in the future.
* Some `String`/`str` APIs will panic if a particular index was not on a valid
  UTF-8 code unit sequence boundary. Conversely, no such checking is performed
  in this crate, as is consistent with treating byte strings as a sequence of
  bytes. This means callers are responsible for maintaining a UTF-8 invariant
  if that's important.
* Some routines provided by this crate, such as `starts_with_str`, have a
  `_str` suffix to differentiate them from similar routines already defined
  on the `[u8]` type. The difference is that `starts_with` requires its
  parameter to be a `&[u8]`, where as `starts_with_str` permits its parameter
  to by anything that implements `AsRef<[u8]>`, which is more flexible. This
  means you can write `bytes.starts_with_str("☃")` instead of
  `bytes.starts_with("☃".as_bytes())`.

Otherwise, you should find most of the APIs between this crate and the standard
library string APIs to be very similar, if not identical.

# Handling of invalid UTF-8

Since byte strings are only *conventionally* UTF-8, there is no guarantee
that byte strings contain valid UTF-8. Indeed, it is perfectly legal for a
byte string to contain arbitrary bytes. However, since this library defines
a *string* type, it provides many operations specified by Unicode. These
operations are typically only defined over codepoints, and thus have no real
meaning on bytes that are invalid UTF-8 because they do not map to a particular
codepoint.

For this reason, whenever operations defined only on codepoints are used, this
library will automatically convert invalid UTF-8 to the Unicode replacement
codepoint, `U+FFFD`, which looks like this: `�`. For example, an
[iterator over codepoints](struct.Chars.html) will yield a Unicode
replacement codepoint whenever it comes across bytes that are not valid UTF-8:

```
use bstr::ByteSlice;

let bs = b"a\xFF\xFFz";
let chars: Vec<char> = bs.chars().collect();
assert_eq!(vec!['a', '\u{FFFD}', '\u{FFFD}', 'z'], chars);
```

There are a few ways in which invalid bytes can be substituted with a Unicode
replacement codepoint. One way, not used by this crate, is to replace every
individual invalid byte with a single replacement codepoint. In contrast, the
approach this crate uses is called the "substitution of maximal subparts," as
specified by the Unicode Standard (Chapter 3, Section 9). (This approach is
also used by [W3C's Encoding Standard](https://www.w3.org/TR/encoding/).) In
this strategy, a replacement codepoint is inserted whenever a byte is found
that cannot possibly lead to a valid UTF-8 code unit sequence. If there were
previous bytes that represented a *prefix* of a well-formed UTF-8 code unit
sequence, then all of those bytes (up to 3) are substituted with a single
replacement codepoint. For example:

```
use bstr::ByteSlice;

let bs = b"a\xF0\x9F\x87z";
let chars: Vec<char> = bs.chars().collect();
// The bytes \xF0\x9F\x87 could lead to a valid UTF-8 sequence, but 3 of them
// on their own are invalid. Only one replacement codepoint is substituted,
// which demonstrates the "substitution of maximal subparts" strategy.
assert_eq!(vec!['a', '\u{FFFD}', 'z'], chars);
```

If you do need to access the raw bytes for some reason in an iterator like
`Chars`, then you should use the iterator's "indices" variant, which gives
the byte offsets containing the invalid UTF-8 bytes that were substituted with
the replacement codepoint. For example:

```
use bstr::{B, ByteSlice};

let bs = b"a\xE2\x98z";
let chars: Vec<(usize, usize, char)> = bs.char_indices().collect();
// Even though the replacement codepoint is encoded as 3 bytes itself, the
// byte range given here is only two bytes, corresponding to the original
// raw bytes.
assert_eq!(vec![(0, 1, 'a'), (1, 3, '\u{FFFD}'), (3, 4, 'z')], chars);

// Thus, getting the original raw bytes is as simple as slicing the original
// byte string:
let chars: Vec<&[u8]> = bs.char_indices().map(|(s, e, _)| &bs[s..e]).collect();
assert_eq!(vec![B("a"), B(b"\xE2\x98"), B("z")], chars);
```

# File paths and OS strings

One of the premiere features of Rust's standard library is how it handles file
paths. In particular, it makes it very hard to write incorrect code while
simultaneously providing a correct cross platform abstraction for manipulating
file paths. The key challenge that one faces with file paths across platforms
is derived from the following observations:

* On most Unix-like systems, file paths are an arbitrary sequence of bytes.
* On Windows, file paths are an arbitrary sequence of 16-bit integers.

(In both cases, certain sequences aren't allowed. For example a `NUL` byte is
not allowed in either case. But we can ignore this for the purposes of this
section.)

Byte strings, like the ones provided in this crate, line up really well with
file paths on Unix like systems, which are themselves just arbitrary sequences
of bytes. It turns out that if you treat them as "mostly UTF-8," then things
work out pretty well. On the contrary, byte strings _don't_ really work
that well on Windows because it's not possible to correctly roundtrip file
paths between 16-bit integers and something that looks like UTF-8 _without_
explicitly defining an encoding to do this for you, which is anathema to byte
strings, which are just bytes.

Rust's standard library elegantly solves this problem by specifying an
internal encoding for file paths that's only used on Windows called
[WTF-8](https://simonsapin.github.io/wtf-8/). Its key properties are that they
permit losslessly roundtripping file paths on Windows by extending UTF-8 to
support an encoding of surrogate codepoints, while simultaneously supporting
zero-cost conversion from Rust's Unicode strings to file paths. (Since UTF-8 is
a proper subset of WTF-8.)

The fundamental point at which the above strategy fails is when you want to
treat file paths as things that look like strings in a zero cost way. In most
cases, this is actually the wrong thing to do, but some cases call for it,
for example, glob or regex matching on file paths. This is because WTF-8 is
treated as an internal implementation detail, and there is no way to access
those bytes via a public API. Therefore, such consumers are limited in what
they can do:

1. One could re-implement WTF-8 and re-encode file paths on Windows to WTF-8
   by accessing their underlying 16-bit integer representation. Unfortunately,
   this isn't zero cost (it introduces a second WTF-8 decoding step) and it's
   not clear this is a good thing to do, since WTF-8 should ideally remain an
   internal implementation detail. This is roughly the approach taken by the
   [`os_str_bytes`](https://crates.io/crates/os_str_bytes) crate.
2. One could instead declare that they will not handle paths on Windows that
   are not valid UTF-16, and return an error when one is encountered.
3. Like (2), but instead of returning an error, lossily decode the file path
   on Windows that isn't valid UTF-16 into UTF-16 by replacing invalid bytes
   with the Unicode replacement codepoint.

While this library may provide facilities for (1) in the future, currently,
this library only provides facilities for (2) and (3). In particular, a suite
of conversion functions are provided that permit converting between byte
strings, OS strings and file paths. For owned byte strings, they are:

* [`ByteVec::from_os_string`](trait.ByteVec.html#method.from_os_string)
* [`ByteVec::from_os_str_lossy`](trait.ByteVec.html#method.from_os_str_lossy)
* [`ByteVec::from_path_buf`](trait.ByteVec.html#method.from_path_buf)
* [`ByteVec::from_path_lossy`](trait.ByteVec.html#method.from_path_lossy)
* [`ByteVec::into_os_string`](trait.ByteVec.html#method.into_os_string)
* [`ByteVec::into_os_string_lossy`](trait.ByteVec.html#method.into_os_string_lossy)
* [`ByteVec::into_path_buf`](trait.ByteVec.html#method.into_path_buf)
* [`ByteVec::into_path_buf_lossy`](trait.ByteVec.html#method.into_path_buf_lossy)

For byte string slices, they are:

* [`ByteSlice::from_os_str`](trait.ByteSlice.html#method.from_os_str)
* [`ByteSlice::from_path`](trait.ByteSlice.html#method.from_path)
* [`ByteSlice::to_os_str`](trait.ByteSlice.html#method.to_os_str)
* [`ByteSlice::to_os_str_lossy`](trait.ByteSlice.html#method.to_os_str_lossy)
* [`ByteSlice::to_path`](trait.ByteSlice.html#method.to_path)
* [`ByteSlice::to_path_lossy`](trait.ByteSlice.html#method.to_path_lossy)

On Unix, all of these conversions are rigorously zero cost, which gives one
a way to ergonomically deal with raw file paths exactly as they are using
normal string-related functions. On Windows, these conversion routines perform
a UTF-8 check and either return an error or lossily decode the file path
into valid UTF-8, depending on which function you use. This means that you
cannot roundtrip all file paths on Windows correctly using these conversion
routines. However, this may be an acceptable downside since such file paths
are exceptionally rare. Moreover, roundtripping isn't always necessary, for
example, if all you're doing is filtering based on file paths.

The reason why using byte strings for this is potentially superior than the
standard library's approach is that a lot of Rust code is already lossily
converting file paths to Rust's Unicode strings, which are required to be valid
UTF-8, and thus contain latent bugs on Unix where paths with invalid UTF-8 are
not terribly uncommon. If you instead use byte strings, then you're guaranteed
to write correct code for Unix, at the cost of getting a corner case wrong on
Windows.

# Cargo features

This crates comes with a few features that control standard library, serde
and Unicode support.

* `std` - **Enabled** by default. This provides APIs that require the standard
  library, such as `Vec<u8>` and `PathBuf`. Enabling this feature also enables
  the `alloc` feature and any other relevant `std` features for dependencies.
* `alloc` - **Enabled** by default. This provides APIs that require allocations
  via the `alloc` crate, such as `Vec<u8>`.
* `unicode` - **Enabled** by default. This provides APIs that require sizable
  Unicode data compiled into the binary. This includes, but is not limited to,
  grapheme/word/sentence segmenters. When this is disabled, basic support such
  as UTF-8 decoding is still included. Note that currently, enabling this
  feature also requires enabling the `std` feature. It is expected that this
  limitation will be lifted at some point.
* `serde` - Enables implementations of serde traits for `BStr`, and also
  `BString` when `alloc` is enabled.
*/

// #![cfg_attr(not(any(feature = "std", test)), no_std)]
#![no_std]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[cfg(any(test, feature = "std"))]
extern crate std;

#[cfg(any(test, feature = "alloc"))]
extern crate alloc;

pub use crate::bstr::BStr;
#[cfg(feature = "alloc")]
pub use crate::bstring::BString;
pub use crate::escape_bytes::EscapeBytes;
#[cfg(feature = "unicode")]
pub use crate::ext_slice::Fields;
pub use crate::ext_slice::{
    ByteSlice, Bytes, FieldsWith, Find, FindReverse, Finder, FinderReverse,
    Lines, LinesWithTerminator, Split, SplitN, SplitNReverse, SplitReverse, B,
};
#[cfg(feature = "alloc")]
pub use crate::ext_vec::{concat, join, ByteVec, DrainBytes, FromUtf8Error};
#[cfg(feature = "unicode")]
pub use crate::unicode::{
    GraphemeIndices, Graphemes, SentenceIndices, Sentences, WordIndices,
    Words, WordsWithBreakIndices, WordsWithBreaks,
};
pub use crate::utf8::{
    decode as decode_utf8, decode_last as decode_last_utf8, CharIndices,
    Chars, Utf8Chunk, Utf8Chunks, Utf8Error,
};

mod ascii;
mod bstr;
#[cfg(feature = "alloc")]
mod bstring;
mod byteset;
mod escape_bytes;
mod ext_slice;
#[cfg(feature = "alloc")]
mod ext_vec;
mod impls;
#[cfg(feature = "std")]
pub mod io;
#[cfg(all(test, feature = "std"))]
mod tests;
#[cfg(feature = "unicode")]
mod unicode;
mod utf8;

#[cfg(all(test, feature = "std"))]
mod apitests {
    use crate::{
        bstr::BStr,
        bstring::BString,
        ext_slice::{Finder, FinderReverse},
    };

    #[test]
    fn oibits() {
        use std::panic::{RefUnwindSafe, UnwindSafe};

        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        fn assert_unwind_safe<T: RefUnwindSafe + UnwindSafe>() {}

        assert_send::<&BStr>();
        assert_sync::<&BStr>();
        assert_unwind_safe::<&BStr>();
        assert_send::<BString>();
        assert_sync::<BString>();
        assert_unwind_safe::<BString>();

        assert_send::<Finder<'_>>();
        assert_sync::<Finder<'_>>();
        assert_unwind_safe::<Finder<'_>>();
        assert_send::<FinderReverse<'_>>();
        assert_sync::<FinderReverse<'_>>();
        assert_unwind_safe::<FinderReverse<'_>>();
    }
}
