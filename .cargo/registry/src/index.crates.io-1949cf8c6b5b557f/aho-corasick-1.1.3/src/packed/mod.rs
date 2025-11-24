/*!
Provides packed multiple substring search, principally for a small number of
patterns.

This sub-module provides vectorized routines for quickly finding
matches of a small number of patterns. In general, users of this crate
shouldn't need to interface with this module directly, as the primary
[`AhoCorasick`](crate::AhoCorasick) searcher will use these routines
automatically as a prefilter when applicable. However, in some cases, callers
may want to bypass the Aho-Corasick machinery entirely and use this vectorized
searcher directly.

# Overview

The primary types in this sub-module are:

* [`Searcher`] executes the actual search algorithm to report matches in a
haystack.
* [`Builder`] accumulates patterns incrementally and can construct a
`Searcher`.
* [`Config`] permits tuning the searcher, and itself will produce a `Builder`
(which can then be used to build a `Searcher`). Currently, the only tuneable
knob are the match semantics, but this may be expanded in the future.

# Examples

This example shows how to create a searcher from an iterator of patterns.
By default, leftmost-first match semantics are used. (See the top-level
[`MatchKind`] type for more details about match semantics, which apply
similarly to packed substring search.)

```
use aho_corasick::{packed::{MatchKind, Searcher}, PatternID};

# fn example() -> Option<()> {
let searcher = Searcher::new(["foobar", "foo"].iter().cloned())?;
let matches: Vec<PatternID> = searcher
    .find_iter("foobar")
    .map(|mat| mat.pattern())
    .collect();
assert_eq!(vec![PatternID::ZERO], matches);
# Some(()) }
# if cfg!(all(feature = "std", any(
#     target_arch = "x86_64", target_arch = "aarch64",
# ))) {
#     example().unwrap()
# } else {
#     assert!(example().is_none());
# }
```

This example shows how to use [`Config`] to change the match semantics to
leftmost-longest:

```
use aho_corasick::{packed::{Config, MatchKind}, PatternID};

# fn example() -> Option<()> {
let searcher = Config::new()
    .match_kind(MatchKind::LeftmostLongest)
    .builder()
    .add("foo")
    .add("foobar")
    .build()?;
let matches: Vec<PatternID> = searcher
    .find_iter("foobar")
    .map(|mat| mat.pattern())
    .collect();
assert_eq!(vec![PatternID::must(1)], matches);
# Some(()) }
# if cfg!(all(feature = "std", any(
#     target_arch = "x86_64", target_arch = "aarch64",
# ))) {
#     example().unwrap()
# } else {
#     assert!(example().is_none());
# }
```

# Packed substring searching

Packed substring searching refers to the use of SIMD (Single Instruction,
Multiple Data) to accelerate the detection of matches in a haystack. Unlike
conventional algorithms, such as Aho-Corasick, SIMD algorithms for substring
search tend to do better with a small number of patterns, where as Aho-Corasick
generally maintains reasonably consistent performance regardless of the number
of patterns you give it. Because of this, the vectorized searcher in this
sub-module cannot be used as a general purpose searcher, since building the
searcher may fail even when given a small number of patterns. However, in
exchange, when searching for a small number of patterns, searching can be quite
a bit faster than Aho-Corasick (sometimes by an order of magnitude).

The key take away here is that constructing a searcher from a list of patterns
is a fallible operation with no clear rules for when it will fail. While the
precise conditions under which building a searcher can fail is specifically an
implementation detail, here are some common reasons:

* Too many patterns were given. Typically, the limit is on the order of 100 or
  so, but this limit may fluctuate based on available CPU features.
* The available packed algorithms require CPU features that aren't available.
  For example, currently, this crate only provides packed algorithms for
  `x86_64` and `aarch64`. Therefore, constructing a packed searcher on any
  other target will always fail.
* Zero patterns were given, or one of the patterns given was empty. Packed
  searchers require at least one pattern and that all patterns are non-empty.
* Something else about the nature of the patterns (typically based on
  heuristics) suggests that a packed searcher would perform very poorly, so
  no searcher is built.
*/

pub use crate::packed::api::{Builder, Config, FindIter, MatchKind, Searcher};

mod api;
mod ext;
mod pattern;
mod rabinkarp;
mod teddy;
#[cfg(all(feature = "std", test))]
mod tests;
mod vector;
