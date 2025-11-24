/*!
A library for finding occurrences of many patterns at once. This library
provides multiple pattern search principally through an implementation of the
[Aho-Corasick algorithm](https://en.wikipedia.org/wiki/Aho%E2%80%93Corasick_algorithm),
which builds a fast finite state machine for executing searches in linear time.

Additionally, this library provides a number of configuration options for
building the automaton that permit controlling the space versus time trade
off. Other features include simple ASCII case insensitive matching, finding
overlapping matches, replacements, searching streams and even searching and
replacing text in streams.

Finally, unlike most other Aho-Corasick implementations, this one
supports enabling [leftmost-first](MatchKind::LeftmostFirst) or
[leftmost-longest](MatchKind::LeftmostLongest) match semantics, using a
(seemingly) novel alternative construction algorithm. For more details on what
match semantics means, see the [`MatchKind`] type.

# Overview

This section gives a brief overview of the primary types in this crate:

* [`AhoCorasick`] is the primary type and represents an Aho-Corasick automaton.
This is the type you use to execute searches.
* [`AhoCorasickBuilder`] can be used to build an Aho-Corasick automaton, and
supports configuring a number of options.
* [`Match`] represents a single match reported by an Aho-Corasick automaton.
Each match has two pieces of information: the pattern that matched and the
start and end byte offsets corresponding to the position in the haystack at
which it matched.

# Example: basic searching

This example shows how to search for occurrences of multiple patterns
simultaneously. Each match includes the pattern that matched along with the
byte offsets of the match.

```
use aho_corasick::{AhoCorasick, PatternID};

let patterns = &["apple", "maple", "Snapple"];
let haystack = "Nobody likes maple in their apple flavored Snapple.";

let ac = AhoCorasick::new(patterns).unwrap();
let mut matches = vec![];
for mat in ac.find_iter(haystack) {
    matches.push((mat.pattern(), mat.start(), mat.end()));
}
assert_eq!(matches, vec![
    (PatternID::must(1), 13, 18),
    (PatternID::must(0), 28, 33),
    (PatternID::must(2), 43, 50),
]);
```

# Example: case insensitivity

This is like the previous example, but matches `Snapple` case insensitively
using `AhoCorasickBuilder`:

```
use aho_corasick::{AhoCorasick, PatternID};

let patterns = &["apple", "maple", "snapple"];
let haystack = "Nobody likes maple in their apple flavored Snapple.";

let ac = AhoCorasick::builder()
    .ascii_case_insensitive(true)
    .build(patterns)
    .unwrap();
let mut matches = vec![];
for mat in ac.find_iter(haystack) {
    matches.push((mat.pattern(), mat.start(), mat.end()));
}
assert_eq!(matches, vec![
    (PatternID::must(1), 13, 18),
    (PatternID::must(0), 28, 33),
    (PatternID::must(2), 43, 50),
]);
```

# Example: replacing matches in a stream

This example shows how to execute a search and replace on a stream without
loading the entire stream into memory first.

```
# #[cfg(feature = "std")] {
use aho_corasick::AhoCorasick;

# fn example() -> Result<(), std::io::Error> {
let patterns = &["fox", "brown", "quick"];
let replace_with = &["sloth", "grey", "slow"];

// In a real example, these might be `std::fs::File`s instead. All you need to
// do is supply a pair of `std::io::Read` and `std::io::Write` implementations.
let rdr = "The quick brown fox.";
let mut wtr = vec![];

let ac = AhoCorasick::new(patterns).unwrap();
ac.try_stream_replace_all(rdr.as_bytes(), &mut wtr, replace_with)?;
assert_eq!(b"The slow grey sloth.".to_vec(), wtr);
# Ok(()) }; example().unwrap()
# }
```

# Example: finding the leftmost first match

In the textbook description of Aho-Corasick, its formulation is typically
structured such that it reports all possible matches, even when they overlap
with another. In many cases, overlapping matches may not be desired, such as
the case of finding all successive non-overlapping matches like you might with
a standard regular expression.

Unfortunately the "obvious" way to modify the Aho-Corasick algorithm to do
this doesn't always work in the expected way, since it will report matches as
soon as they are seen. For example, consider matching the regex `Samwise|Sam`
against the text `Samwise`. Most regex engines (that are Perl-like, or
non-POSIX) will report `Samwise` as a match, but the standard Aho-Corasick
algorithm modified for reporting non-overlapping matches will report `Sam`.

A novel contribution of this library is the ability to change the match
semantics of Aho-Corasick (without additional search time overhead) such that
`Samwise` is reported instead. For example, here's the standard approach:

```
use aho_corasick::AhoCorasick;

let patterns = &["Samwise", "Sam"];
let haystack = "Samwise";

let ac = AhoCorasick::new(patterns).unwrap();
let mat = ac.find(haystack).expect("should have a match");
assert_eq!("Sam", &haystack[mat.start()..mat.end()]);
```

And now here's the leftmost-first version, which matches how a Perl-like
regex will work:

```
use aho_corasick::{AhoCorasick, MatchKind};

let patterns = &["Samwise", "Sam"];
let haystack = "Samwise";

let ac = AhoCorasick::builder()
    .match_kind(MatchKind::LeftmostFirst)
    .build(patterns)
    .unwrap();
let mat = ac.find(haystack).expect("should have a match");
assert_eq!("Samwise", &haystack[mat.start()..mat.end()]);
```

In addition to leftmost-first semantics, this library also supports
leftmost-longest semantics, which match the POSIX behavior of a regular
expression alternation. See [`MatchKind`] for more details.

# Prefilters

While an Aho-Corasick automaton can perform admirably when compared to more
naive solutions, it is generally slower than more specialized algorithms that
are accelerated using vector instructions such as SIMD.

For that reason, this library will internally use a "prefilter" to attempt
to accelerate searches when possible. Currently, this library has several
different algorithms it might use depending on the patterns provided. Once the
number of patterns gets too big, prefilters are no longer used.

While a prefilter is generally good to have on by default since it works
well in the common case, it can lead to less predictable or even sub-optimal
performance in some cases. For that reason, prefilters can be explicitly
disabled via [`AhoCorasickBuilder::prefilter`].

# Lower level APIs

This crate also provides several sub-modules that collectively expose many of
the implementation details of the main [`AhoCorasick`] type. Most users of this
library can completely ignore the submodules and their contents, but if you
needed finer grained control, some parts of them may be useful to you. Here is
a brief overview of each and why you might want to use them:

* The [`packed`] sub-module contains a lower level API for using fast
vectorized routines for finding a small number of patterns in a haystack.
You might want to use this API when you want to completely side-step using
Aho-Corasick automata. Otherwise, the fast vectorized routines are used
automatically as prefilters for `AhoCorasick` searches whenever possible.
* The [`automaton`] sub-module provides a lower level finite state
machine interface that the various Aho-Corasick implementations in
this crate implement. This sub-module's main contribution is the
[`Automaton`](automaton::Automaton) trait, which permits manually walking the
state transitions of an Aho-Corasick automaton.
* The [`dfa`] and [`nfa`] sub-modules provide DFA and NFA implementations of
the aforementioned `Automaton` trait. The main reason one might want to use
these sub-modules is to get access to a type that implements the `Automaton`
trait. (The top-level `AhoCorasick` type does not implement the `Automaton`
trait.)

As mentioned above, if you aren't sure whether you need these sub-modules,
you should be able to safely ignore them and just focus on the [`AhoCorasick`]
type.

# Crate features

This crate exposes a few features for controlling dependency usage and whether
this crate can be used without the standard library.

* **std** -
  Enables support for the standard library. This feature is enabled by
  default. When disabled, only `core` and `alloc` are used. At an API
  level, enabling `std` enables `std::error::Error` trait impls for the
  various error types, and higher level stream search routines such as
  [`AhoCorasick::try_stream_find_iter`]. But the `std` feature is also required
  to enable vectorized prefilters. Prefilters can greatly accelerate searches,
  but generally only apply when the number of patterns is small (less than
  ~100).
* **perf-literal** -
  Enables support for literal prefilters that use vectorized routines from
  external crates. This feature is enabled by default. If you're only using
  Aho-Corasick for large numbers of patterns or otherwise can abide lower
  throughput when searching with a small number of patterns, then it is
  reasonable to disable this feature.
* **logging** -
  Enables a dependency on the `log` crate and emits messages to aide in
  diagnostics. This feature is disabled by default.
*/

#![no_std]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

extern crate alloc;
#[cfg(any(test, feature = "std"))]
extern crate std;

#[cfg(doctest)]
doc_comment::doctest!("../README.md");

#[cfg(feature = "std")]
pub use crate::ahocorasick::StreamFindIter;
pub use crate::{
    ahocorasick::{
        AhoCorasick, AhoCorasickBuilder, AhoCorasickKind, FindIter,
        FindOverlappingIter,
    },
    util::{
        error::{BuildError, MatchError, MatchErrorKind},
        primitives::{PatternID, PatternIDError},
        search::{Anchored, Input, Match, MatchKind, Span, StartKind},
    },
};

#[macro_use]
mod macros;

mod ahocorasick;
pub mod automaton;
pub mod dfa;
pub mod nfa;
pub mod packed;
#[cfg(test)]
mod tests;
// I wrote out the module for implementing fst::Automaton only to later realize
// that this would make fst a public dependency and fst is not at 1.0 yet. I
// decided to just keep the code in tree, but build it only during tests.
//
// TODO: I think I've changed my mind again. I'm considering pushing it out
// into either a separate crate or into 'fst' directly as an optional feature.
// #[cfg(test)]
// #[allow(dead_code)]
// mod transducer;
pub(crate) mod util;

#[cfg(test)]
mod testoibits {
    use std::panic::{RefUnwindSafe, UnwindSafe};

    use super::*;

    fn assert_all<T: Send + Sync + UnwindSafe + RefUnwindSafe>() {}

    #[test]
    fn oibits_main() {
        assert_all::<AhoCorasick>();
        assert_all::<AhoCorasickBuilder>();
        assert_all::<AhoCorasickKind>();
        assert_all::<FindIter>();
        assert_all::<FindOverlappingIter>();

        assert_all::<BuildError>();
        assert_all::<MatchError>();
        assert_all::<MatchErrorKind>();

        assert_all::<Anchored>();
        assert_all::<Input>();
        assert_all::<Match>();
        assert_all::<MatchKind>();
        assert_all::<Span>();
        assert_all::<StartKind>();
    }

    #[test]
    fn oibits_automaton() {
        use crate::{automaton, dfa::DFA};

        assert_all::<automaton::FindIter<DFA>>();
        assert_all::<automaton::FindOverlappingIter<DFA>>();
        #[cfg(feature = "std")]
        assert_all::<automaton::StreamFindIter<DFA, std::io::Stdin>>();
        assert_all::<automaton::OverlappingState>();

        assert_all::<automaton::Prefilter>();
        assert_all::<automaton::Candidate>();
    }

    #[test]
    fn oibits_packed() {
        use crate::packed;

        assert_all::<packed::Config>();
        assert_all::<packed::Builder>();
        assert_all::<packed::Searcher>();
        assert_all::<packed::FindIter>();
        assert_all::<packed::MatchKind>();
    }
}
