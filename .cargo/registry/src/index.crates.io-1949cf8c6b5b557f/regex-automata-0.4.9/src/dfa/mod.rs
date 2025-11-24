/*!
A module for building and searching with deterministic finite automata (DFAs).

Like other modules in this crate, DFAs support a rich regex syntax with Unicode
features. DFAs also have extensive options for configuring the best space vs
time trade off for your use case and provides support for cheap deserialization
of automata for use in `no_std` environments.

If you're looking for lazy DFAs that build themselves incrementally during
search, then please see the top-level [`hybrid` module](crate::hybrid).

# Overview

This section gives a brief overview of the primary types in this module:

* A [`regex::Regex`] provides a way to search for matches of a regular
expression using DFAs. This includes iterating over matches with both the start
and end positions of each match.
* A [`dense::DFA`] provides low level access to a DFA that uses a dense
representation (uses lots of space, but fast searching).
* A [`sparse::DFA`] provides the same API as a `dense::DFA`, but uses a sparse
representation (uses less space, but slower searching).
* An [`Automaton`] trait that defines an interface that both dense and sparse
DFAs implement. (A `regex::Regex` is generic over this trait.)
* Both dense DFAs and sparse DFAs support serialization to raw bytes (e.g.,
[`dense::DFA::to_bytes_little_endian`]) and cheap deserialization (e.g.,
[`dense::DFA::from_bytes`]).

There is also a [`onepass`] module that provides a [one-pass
DFA](onepass::DFA). The unique advantage of this DFA is that, for the class
of regexes it can be built with, it supports reporting the spans of matching
capturing groups. It is the only DFA in this crate capable of such a thing.

# Example: basic regex searching

This example shows how to compile a regex using the default configuration
and then use it to find matches in a byte string:

```
use regex_automata::{Match, dfa::regex::Regex};

let re = Regex::new(r"[0-9]{4}-[0-9]{2}-[0-9]{2}")?;
let text = b"2018-12-24 2016-10-08";
let matches: Vec<Match> = re.find_iter(text).collect();
assert_eq!(matches, vec![
    Match::must(0, 0..10),
    Match::must(0, 11..21),
]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

# Example: searching with regex sets

The DFAs in this module all fully support searching with multiple regexes
simultaneously. You can use this support with standard leftmost-first style
searching to find non-overlapping matches:

```
# if cfg!(miri) { return Ok(()); } // miri takes too long
use regex_automata::{Match, dfa::regex::Regex};

let re = Regex::new_many(&[r"\w+", r"\S+"])?;
let text = b"@foo bar";
let matches: Vec<Match> = re.find_iter(text).collect();
assert_eq!(matches, vec![
    Match::must(1, 0..4),
    Match::must(0, 5..8),
]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

# Example: use sparse DFAs

By default, compiling a regex will use dense DFAs internally. This uses more
memory, but executes searches more quickly. If you can abide slower searches
(somewhere around 3-5x), then sparse DFAs might make more sense since they can
use significantly less space.

Using sparse DFAs is as easy as using `Regex::new_sparse` instead of
`Regex::new`:

```
use regex_automata::{Match, dfa::regex::Regex};

let re = Regex::new_sparse(r"[0-9]{4}-[0-9]{2}-[0-9]{2}").unwrap();
let text = b"2018-12-24 2016-10-08";
let matches: Vec<Match> = re.find_iter(text).collect();
assert_eq!(matches, vec![
    Match::must(0, 0..10),
    Match::must(0, 11..21),
]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

If you already have dense DFAs for some reason, they can be converted to sparse
DFAs and used to build a new `Regex`. For example:

```
use regex_automata::{Match, dfa::regex::Regex};

let dense_re = Regex::new(r"[0-9]{4}-[0-9]{2}-[0-9]{2}").unwrap();
let sparse_re = Regex::builder().build_from_dfas(
    dense_re.forward().to_sparse()?,
    dense_re.reverse().to_sparse()?,
);
let text = b"2018-12-24 2016-10-08";
let matches: Vec<Match> = sparse_re.find_iter(text).collect();
assert_eq!(matches, vec![
    Match::must(0, 0..10),
    Match::must(0, 11..21),
]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

# Example: deserialize a DFA

This shows how to first serialize a DFA into raw bytes, and then deserialize
those raw bytes back into a DFA. While this particular example is a
bit contrived, this same technique can be used in your program to
deserialize a DFA at start up time or by memory mapping a file.

```
use regex_automata::{Match, dfa::{dense, regex::Regex}};

let re1 = Regex::new(r"[0-9]{4}-[0-9]{2}-[0-9]{2}").unwrap();
// serialize both the forward and reverse DFAs, see note below
let (fwd_bytes, fwd_pad) = re1.forward().to_bytes_native_endian();
let (rev_bytes, rev_pad) = re1.reverse().to_bytes_native_endian();
// now deserialize both---we need to specify the correct type!
let fwd: dense::DFA<&[u32]> = dense::DFA::from_bytes(&fwd_bytes[fwd_pad..])?.0;
let rev: dense::DFA<&[u32]> = dense::DFA::from_bytes(&rev_bytes[rev_pad..])?.0;
// finally, reconstruct our regex
let re2 = Regex::builder().build_from_dfas(fwd, rev);

// we can use it like normal
let text = b"2018-12-24 2016-10-08";
let matches: Vec<Match> = re2.find_iter(text).collect();
assert_eq!(matches, vec![
    Match::must(0, 0..10),
    Match::must(0, 11..21),
]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

There are a few points worth noting here:

* We need to extract the raw DFAs used by the regex and serialize those. You
can build the DFAs manually yourself using [`dense::Builder`], but using
the DFAs from a `Regex` guarantees that the DFAs are built correctly. (In
particular, a `Regex` constructs a reverse DFA for finding the starting
location of matches.)
* To convert the DFA to raw bytes, we use the `to_bytes_native_endian` method.
In practice, you'll want to use either [`dense::DFA::to_bytes_little_endian`]
or [`dense::DFA::to_bytes_big_endian`], depending on which platform you're
deserializing your DFA from. If you intend to deserialize on either platform,
then you'll need to serialize both and deserialize the right one depending on
your target's endianness.
* Safely deserializing a DFA requires verifying the raw bytes, particularly if
they are untrusted, since an invalid DFA could cause logical errors, panics
or even undefined behavior. This verification step requires visiting all of
the transitions in the DFA, which can be costly. If cheaper verification is
desired, then [`dense::DFA::from_bytes_unchecked`] is available that only does
verification that can be performed in constant time. However, one can only use
this routine if the caller can guarantee that the bytes provided encoded a
valid DFA.

The same process can be achieved with sparse DFAs as well:

```
use regex_automata::{Match, dfa::{sparse, regex::Regex}};

let re1 = Regex::new(r"[0-9]{4}-[0-9]{2}-[0-9]{2}").unwrap();
// serialize both
let fwd_bytes = re1.forward().to_sparse()?.to_bytes_native_endian();
let rev_bytes = re1.reverse().to_sparse()?.to_bytes_native_endian();
// now deserialize both---we need to specify the correct type!
let fwd: sparse::DFA<&[u8]> = sparse::DFA::from_bytes(&fwd_bytes)?.0;
let rev: sparse::DFA<&[u8]> = sparse::DFA::from_bytes(&rev_bytes)?.0;
// finally, reconstruct our regex
let re2 = Regex::builder().build_from_dfas(fwd, rev);

// we can use it like normal
let text = b"2018-12-24 2016-10-08";
let matches: Vec<Match> = re2.find_iter(text).collect();
assert_eq!(matches, vec![
    Match::must(0, 0..10),
    Match::must(0, 11..21),
]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Note that unlike dense DFAs, sparse DFAs have no alignment requirements.
Conversely, dense DFAs must be aligned to the same alignment as a
[`StateID`](crate::util::primitives::StateID).

# Support for `no_std` and `alloc`-only

This crate comes with `alloc` and `std` features that are enabled by default.
When the `alloc` or `std` features are enabled, the API of this module will
include the facilities necessary for compiling, serializing, deserializing
and searching with DFAs. When only the `alloc` feature is enabled, then
implementations of the `std::error::Error` trait are dropped, but everything
else generally remains the same. When both the `alloc` and `std` features are
disabled, the API of this module will shrink such that it only includes the
facilities necessary for deserializing and searching with DFAs.

The intended workflow for `no_std` environments is thus as follows:

* Write a program with the `alloc` or `std` features that compiles and
serializes a regular expression. You may need to serialize both little and big
endian versions of each DFA. (So that's 4 DFAs in total for each regex.)
* In your `no_std` environment, follow the examples above for deserializing
your previously serialized DFAs into regexes. You can then search with them as
you would any regex.

Deserialization can happen anywhere. For example, with bytes embedded into a
binary or with a file memory mapped at runtime.

The `regex-cli` command (found in the same repository as this crate) can be
used to serialize DFAs to files and generate Rust code to read them.

# Syntax

This module supports the same syntax as the `regex` crate, since they share the
same parser. You can find an exhaustive list of supported syntax in the
[documentation for the `regex` crate](https://docs.rs/regex/1/regex/#syntax).

There are two things that are not supported by the DFAs in this module:

* Capturing groups. The DFAs (and [`Regex`](regex::Regex)es built on top
of them) can only find the offsets of an entire match, but cannot resolve
the offsets of each capturing group. This is because DFAs do not have the
expressive power necessary.
* Unicode word boundaries. These present particularly difficult challenges for
DFA construction and would result in an explosion in the number of states.
One can enable [`dense::Config::unicode_word_boundary`] though, which provides
heuristic support for Unicode word boundaries that only works on ASCII text.
Otherwise, one can use `(?-u:\b)` for an ASCII word boundary, which will work
on any input.

There are no plans to lift either of these limitations.

Note that these restrictions are identical to the restrictions on lazy DFAs.

# Differences with general purpose regexes

The main goal of the [`regex`](https://docs.rs/regex) crate is to serve as a
general purpose regular expression engine. It aims to automatically balance low
compile times, fast search times and low memory usage, while also providing
a convenient API for users. In contrast, this module provides a lower level
regular expression interface based exclusively on DFAs that is a bit less
convenient while providing more explicit control over memory usage and search
times.

Here are some specific negative differences:

* **Compilation can take an exponential amount of time and space** in the size
of the regex pattern. While most patterns do not exhibit worst case exponential
time, such patterns do exist. For example, `[01]*1[01]{N}` will build a DFA
with approximately `2^(N+2)` states. For this reason, untrusted patterns should
not be compiled with this module. (In the future, the API may expose an option
to return an error if the DFA gets too big.)
* This module does not support sub-match extraction via capturing groups, which
can be achieved with the regex crate's "captures" API.
* While the regex crate doesn't necessarily sport fast compilation times,
the regexes in this module are almost universally slow to compile, especially
when they contain large Unicode character classes. For example, on my system,
compiling `\w{50}` takes about 1 second and almost 15MB of memory! (Compiling
a sparse regex takes about the same time but only uses about 1.2MB of
memory.) Conversely, compiling the same regex without Unicode support, e.g.,
`(?-u)\w{50}`, takes under 1 millisecond and about 15KB of memory. For this
reason, you should only use Unicode character classes if you absolutely need
them! (They are enabled by default though.)
* This module does not support Unicode word boundaries. ASCII word bondaries
may be used though by disabling Unicode or selectively doing so in the syntax,
e.g., `(?-u:\b)`. There is also an option to
[heuristically enable Unicode word boundaries](crate::dfa::dense::Config::unicode_word_boundary),
where the corresponding DFA will give up if any non-ASCII byte is seen.
* As a lower level API, this module does not do literal optimizations
automatically. Although it does provide hooks in its API to make use of the
[`Prefilter`](crate::util::prefilter::Prefilter) trait. Missing literal
optimizations means that searches may run much slower than what you're
accustomed to, although, it does provide more predictable and consistent
performance.
* There is no `&str` API like in the regex crate. In this module, all APIs
operate on `&[u8]`. By default, match indices are
guaranteed to fall on UTF-8 boundaries, unless either of
[`syntax::Config::utf8`](crate::util::syntax::Config::utf8) or
[`thompson::Config::utf8`](crate::nfa::thompson::Config::utf8) are disabled.

With some of the downsides out of the way, here are some positive differences:

* Both dense and sparse DFAs can be serialized to raw bytes, and then cheaply
deserialized. Deserialization can be done in constant time with the unchecked
APIs, since searching can be performed directly on the raw serialized bytes of
a DFA.
* This module was specifically designed so that the searching phase of a
DFA has minimal runtime requirements, and can therefore be used in `no_std`
environments. While `no_std` environments cannot compile regexes, they can
deserialize pre-compiled regexes.
* Since this module builds DFAs ahead of time, it will generally out-perform
the `regex` crate on equivalent tasks. The performance difference is likely
not large. However, because of a complex set of optimizations in the regex
crate (like literal optimizations), an accurate performance comparison may be
difficult to do.
* Sparse DFAs provide a way to build a DFA ahead of time that sacrifices search
performance a small amount, but uses much less storage space. Potentially even
less than what the regex crate uses.
* This module exposes DFAs directly, such as [`dense::DFA`] and
[`sparse::DFA`], which enables one to do less work in some cases. For example,
if you only need the end of a match and not the start of a match, then you can
use a DFA directly without building a `Regex`, which always requires a second
DFA to find the start of a match.
* This module provides more control over memory usage. Aside from choosing
between dense and sparse DFAs, one can also choose a smaller state identifier
representation to use less space. Also, one can enable DFA minimization
via [`dense::Config::minimize`], but it can increase compilation times
dramatically.
*/

#[cfg(feature = "dfa-search")]
pub use crate::dfa::{
    automaton::{Automaton, OverlappingState, StartError},
    start::StartKind,
};

/// This is an alias for a state ID of zero. It has special significance
/// because it always corresponds to the first state in a DFA, and the first
/// state in a DFA is always "dead." That is, the dead state always has all
/// of its transitions set to itself. Moreover, the dead state is used as a
/// sentinel for various things. e.g., In search, reaching a dead state means
/// that the search must stop.
const DEAD: crate::util::primitives::StateID =
    crate::util::primitives::StateID::ZERO;

#[cfg(feature = "dfa-search")]
pub mod dense;
#[cfg(feature = "dfa-onepass")]
pub mod onepass;
#[cfg(feature = "dfa-search")]
pub mod regex;
#[cfg(feature = "dfa-search")]
pub mod sparse;

#[cfg(feature = "dfa-search")]
pub(crate) mod accel;
#[cfg(feature = "dfa-search")]
mod automaton;
#[cfg(feature = "dfa-build")]
mod determinize;
#[cfg(feature = "dfa-build")]
mod minimize;
#[cfg(any(feature = "dfa-build", feature = "dfa-onepass"))]
mod remapper;
#[cfg(feature = "dfa-search")]
mod search;
#[cfg(feature = "dfa-search")]
mod special;
#[cfg(feature = "dfa-search")]
mod start;
