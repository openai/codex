/*!
A module for building and searching with lazy deterministic finite automata
(DFAs).

Like other modules in this crate, lazy DFAs support a rich regex syntax with
Unicode features. The key feature of a lazy DFA is that it builds itself
incrementally during search, and never uses more than a configured capacity of
memory. Thus, when searching with a lazy DFA, one must supply a mutable "cache"
in which the actual DFA's transition table is stored.

If you're looking for fully compiled DFAs, then please see the top-level
[`dfa` module](crate::dfa).

# Overview

This section gives a brief overview of the primary types in this module:

* A [`regex::Regex`] provides a way to search for matches of a regular
expression using lazy DFAs. This includes iterating over matches with both the
start and end positions of each match.
* A [`dfa::DFA`] provides direct low level access to a lazy DFA.

# Example: basic regex searching

This example shows how to compile a regex using the default configuration
and then use it to find matches in a byte string:

```
use regex_automata::{hybrid::regex::Regex, Match};

let re = Regex::new(r"[0-9]{4}-[0-9]{2}-[0-9]{2}")?;
let mut cache = re.create_cache();

let haystack = "2018-12-24 2016-10-08";
let matches: Vec<Match> = re.find_iter(&mut cache, haystack).collect();
assert_eq!(matches, vec![
    Match::must(0, 0..10),
    Match::must(0, 11..21),
]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

# Example: searching with multiple regexes

The lazy DFAs in this module all fully support searching with multiple regexes
simultaneously. You can use this support with standard leftmost-first style
searching to find non-overlapping matches:

```
# if cfg!(miri) { return Ok(()); } // miri takes too long
use regex_automata::{hybrid::regex::Regex, Match};

let re = Regex::new_many(&[r"\w+", r"\S+"])?;
let mut cache = re.create_cache();

let haystack = "@foo bar";
let matches: Vec<Match> = re.find_iter(&mut cache, haystack).collect();
assert_eq!(matches, vec![
    Match::must(1, 0..4),
    Match::must(0, 5..8),
]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

# When should I use this?

Generally speaking, if you can abide the use of mutable state during search,
and you don't need things like capturing groups or Unicode word boundary
support in non-ASCII text, then a lazy DFA is likely a robust choice with
respect to both search speed and memory usage. Note however that its speed
may be worse than a general purpose regex engine if you don't select a good
[prefilter](crate::util::prefilter).

If you know ahead of time that your pattern would result in a very large DFA
if it was fully compiled, it may be better to use an NFA simulation instead
of a lazy DFA. Either that, or increase the cache capacity of your lazy DFA
to something that is big enough to hold the state machine (likely through
experimentation). The issue here is that if the cache is too small, then it
could wind up being reset too frequently and this might decrease searching
speed significantly.

# Differences with fully compiled DFAs

A [`hybrid::regex::Regex`](crate::hybrid::regex::Regex) and a
[`dfa::regex::Regex`](crate::dfa::regex::Regex) both have the same capabilities
(and similarly for their underlying DFAs), but they achieve them through
different means. The main difference is that a hybrid or "lazy" regex builds
its DFA lazily during search, where as a fully compiled regex will build its
DFA at construction time. While building a DFA at search time might sound like
it's slow, it tends to work out where most bytes seen during a search will
reuse pre-built parts of the DFA and thus can be almost as fast as a fully
compiled DFA. The main downside is that searching requires mutable space to
store the DFA, and, in the worst case, a search can result in a new state being
created for each byte seen, which would make searching quite a bit slower.

A fully compiled DFA never has to worry about searches being slower once
it's built. (Aside from, say, the transition table being so large that it
is subject to harsh CPU cache effects.) However, of course, building a full
DFA can be quite time consuming and memory hungry. Particularly when large
Unicode character classes are used, which tend to translate into very large
DFAs.

A lazy DFA strikes a nice balance _in practice_, particularly in the
presence of Unicode mode, by only building what is needed. It avoids the
worst case exponential time complexity of DFA compilation by guaranteeing that
it will only build at most one state per byte searched. While the worst
case here can lead to a very high constant, it will never be exponential.

# Syntax

This module supports the same syntax as the `regex` crate, since they share the
same parser. You can find an exhaustive list of supported syntax in the
[documentation for the `regex` crate](https://docs.rs/regex/1/regex/#syntax).

There are two things that are not supported by the lazy DFAs in this module:

* Capturing groups. The DFAs (and [`Regex`](regex::Regex)es built on top
of them) can only find the offsets of an entire match, but cannot resolve
the offsets of each capturing group. This is because DFAs do not have the
expressive power necessary. Note that it is okay to build a lazy DFA from an
NFA that contains capture groups. The capture groups will simply be ignored.
* Unicode word boundaries. These present particularly difficult challenges for
DFA construction and would result in an explosion in the number of states.
One can enable [`dfa::Config::unicode_word_boundary`] though, which provides
heuristic support for Unicode word boundaries that only works on ASCII text.
Otherwise, one can use `(?-u:\b)` for an ASCII word boundary, which will work
on any input.

There are no plans to lift either of these limitations.

Note that these restrictions are identical to the restrictions on fully
compiled DFAs.
*/

pub use self::{
    error::{BuildError, CacheError, StartError},
    id::LazyStateID,
};

pub mod dfa;
mod error;
mod id;
pub mod regex;
mod search;
