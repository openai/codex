/*!
Provides a regex matcher that composes several other regex matchers
automatically.

This module is home to a meta [`Regex`], which provides a convenient high
level API for executing regular expressions in linear time.

# Comparison with the `regex` crate

A meta `Regex` is the implementation used directly by the `regex` crate.
Indeed, the `regex` crate API is essentially just a light wrapper over a meta
`Regex`. This means that if you need the full flexibility offered by this
API, then you should be able to switch to using this API directly without
any changes in match semantics or syntax. However, there are some API level
differences:

* The `regex` crate API returns match objects that include references to the
haystack itself, which in turn makes it easy to access the matching strings
without having to slice the haystack yourself. In contrast, a meta `Regex`
returns match objects that only have offsets in them.
* At time of writing, a meta `Regex` doesn't have some of the convenience
routines that the `regex` crate has, such as replacements. Note though that
[`Captures::interpolate_string`](crate::util::captures::Captures::interpolate_string)
will handle the replacement string interpolation for you.
* A meta `Regex` supports the [`Input`](crate::Input) abstraction, which
provides a way to configure a search in more ways than is supported by the
`regex` crate. For example, [`Input::anchored`](crate::Input::anchored) can
be used to run an anchored search, regardless of whether the pattern is itself
anchored with a `^`.
* A meta `Regex` supports multi-pattern searching everywhere.
Indeed, every [`Match`](crate::Match) returned by the search APIs
include a [`PatternID`](crate::PatternID) indicating which pattern
matched. In the single pattern case, all matches correspond to
[`PatternID::ZERO`](crate::PatternID::ZERO). In contrast, the `regex` crate
has distinct `Regex` and a `RegexSet` APIs. The former only supports a single
pattern, while the latter supports multiple patterns but cannot report the
offsets of a match.
* A meta `Regex` provides the explicit capability of bypassing its internal
memory pool for automatically acquiring mutable scratch space required by its
internal regex engines. Namely, a [`Cache`] can be explicitly provided to lower
level routines such as [`Regex::search_with`].

*/

pub use self::{
    error::BuildError,
    regex::{
        Builder, Cache, CapturesMatches, Config, FindMatches, Regex, Split,
        SplitN,
    },
};

mod error;
#[cfg(any(feature = "dfa-build", feature = "hybrid"))]
mod limited;
mod literal;
mod regex;
mod reverse_inner;
#[cfg(any(feature = "dfa-build", feature = "hybrid"))]
mod stopat;
mod strategy;
mod wrappers;
