/*!
Types and routines specific to dense DFAs.

This module is the home of [`dense::DFA`](DFA).

This module also contains a [`dense::Builder`](Builder) and a
[`dense::Config`](Config) for building and configuring a dense DFA.
*/

#[cfg(feature = "dfa-build")]
use core::cmp;
use core::{fmt, iter, mem::size_of, slice};

#[cfg(feature = "dfa-build")]
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec,
    vec::Vec,
};

#[cfg(feature = "dfa-build")]
use crate::{
    dfa::{
        accel::Accel, determinize, minimize::Minimizer, remapper::Remapper,
        sparse,
    },
    nfa::thompson,
    util::{look::LookMatcher, search::MatchKind},
};
use crate::{
    dfa::{
        accel::Accels,
        automaton::{fmt_state_indicator, Automaton, StartError},
        special::Special,
        start::StartKind,
        DEAD,
    },
    util::{
        alphabet::{self, ByteClasses, ByteSet},
        int::{Pointer, Usize},
        prefilter::Prefilter,
        primitives::{PatternID, StateID},
        search::Anchored,
        start::{self, Start, StartByteMap},
        wire::{self, DeserializeError, Endian, SerializeError},
    },
};

/// The label that is pre-pended to a serialized DFA.
const LABEL: &str = "rust-regex-automata-dfa-dense";

/// The format version of dense regexes. This version gets incremented when a
/// change occurs. A change may not necessarily be a breaking change, but the
/// version does permit good error messages in the case where a breaking change
/// is made.
const VERSION: u32 = 2;

/// The configuration used for compiling a dense DFA.
///
/// As a convenience, [`DFA::config`] is an alias for [`Config::new`]. The
/// advantage of the former is that it often lets you avoid importing the
/// `Config` type directly.
///
/// A dense DFA configuration is a simple data object that is typically used
/// with [`dense::Builder::configure`](self::Builder::configure).
///
/// The default configuration guarantees that a search will never return
/// a "quit" error, although it is possible for a search to fail if
/// [`Config::starts_for_each_pattern`] wasn't enabled (which it is
/// not by default) and an [`Anchored::Pattern`] mode is requested via
/// [`Input`](crate::Input).
#[cfg(feature = "dfa-build")]
#[derive(Clone, Debug, Default)]
pub struct Config {
    // As with other configuration types in this crate, we put all our knobs
    // in options so that we can distinguish between "default" and "not set."
    // This makes it possible to easily combine multiple configurations
    // without default values overwriting explicitly specified values. See the
    // 'overwrite' method.
    //
    // For docs on the fields below, see the corresponding method setters.
    accelerate: Option<bool>,
    pre: Option<Option<Prefilter>>,
    minimize: Option<bool>,
    match_kind: Option<MatchKind>,
    start_kind: Option<StartKind>,
    starts_for_each_pattern: Option<bool>,
    byte_classes: Option<bool>,
    unicode_word_boundary: Option<bool>,
    quitset: Option<ByteSet>,
    specialize_start_states: Option<bool>,
    dfa_size_limit: Option<Option<usize>>,
    determinize_size_limit: Option<Option<usize>>,
}

#[cfg(feature = "dfa-build")]
impl Config {
    /// Return a new default dense DFA compiler configuration.
    pub fn new() -> Config {
        Config::default()
    }

    /// Enable state acceleration.
    ///
    /// When enabled, DFA construction will analyze each state to determine
    /// whether it is eligible for simple acceleration. Acceleration typically
    /// occurs when most of a state's transitions loop back to itself, leaving
    /// only a select few bytes that will exit the state. When this occurs,
    /// other routines like `memchr` can be used to look for those bytes which
    /// may be much faster than traversing the DFA.
    ///
    /// Callers may elect to disable this if consistent performance is more
    /// desirable than variable performance. Namely, acceleration can sometimes
    /// make searching slower than it otherwise would be if the transitions
    /// that leave accelerated states are traversed frequently.
    ///
    /// See [`Automaton::accelerator`] for an example.
    ///
    /// This is enabled by default.
    pub fn accelerate(mut self, yes: bool) -> Config {
        self.accelerate = Some(yes);
        self
    }

    /// Set a prefilter to be used whenever a start state is entered.
    ///
    /// A [`Prefilter`] in this context is meant to accelerate searches by
    /// looking for literal prefixes that every match for the corresponding
    /// pattern (or patterns) must start with. Once a prefilter produces a
    /// match, the underlying search routine continues on to try and confirm
    /// the match.
    ///
    /// Be warned that setting a prefilter does not guarantee that the search
    /// will be faster. While it's usually a good bet, if the prefilter
    /// produces a lot of false positive candidates (i.e., positions matched
    /// by the prefilter but not by the regex), then the overall result can
    /// be slower than if you had just executed the regex engine without any
    /// prefilters.
    ///
    /// Note that unless [`Config::specialize_start_states`] has been
    /// explicitly set, then setting this will also enable (when `pre` is
    /// `Some`) or disable (when `pre` is `None`) start state specialization.
    /// This occurs because without start state specialization, a prefilter
    /// is likely to be less effective. And without a prefilter, start state
    /// specialization is usually pointless.
    ///
    /// **WARNING:** Note that prefilters are not preserved as part of
    /// serialization. Serializing a DFA will drop its prefilter.
    ///
    /// By default no prefilter is set.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{dense::DFA, Automaton},
    ///     util::prefilter::Prefilter,
    ///     Input, HalfMatch, MatchKind,
    /// };
    ///
    /// let pre = Prefilter::new(MatchKind::LeftmostFirst, &["foo", "bar"]);
    /// let re = DFA::builder()
    ///     .configure(DFA::config().prefilter(pre))
    ///     .build(r"(foo|bar)[a-z]+")?;
    /// let input = Input::new("foo1 barfox bar");
    /// assert_eq!(
    ///     Some(HalfMatch::must(0, 11)),
    ///     re.try_search_fwd(&input)?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Be warned though that an incorrect prefilter can lead to incorrect
    /// results!
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{dense::DFA, Automaton},
    ///     util::prefilter::Prefilter,
    ///     Input, HalfMatch, MatchKind,
    /// };
    ///
    /// let pre = Prefilter::new(MatchKind::LeftmostFirst, &["foo", "car"]);
    /// let re = DFA::builder()
    ///     .configure(DFA::config().prefilter(pre))
    ///     .build(r"(foo|bar)[a-z]+")?;
    /// let input = Input::new("foo1 barfox bar");
    /// assert_eq!(
    ///     // No match reported even though there clearly is one!
    ///     None,
    ///     re.try_search_fwd(&input)?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn prefilter(mut self, pre: Option<Prefilter>) -> Config {
        self.pre = Some(pre);
        if self.specialize_start_states.is_none() {
            self.specialize_start_states =
                Some(self.get_prefilter().is_some());
        }
        self
    }

    /// Minimize the DFA.
    ///
    /// When enabled, the DFA built will be minimized such that it is as small
    /// as possible.
    ///
    /// Whether one enables minimization or not depends on the types of costs
    /// you're willing to pay and how much you care about its benefits. In
    /// particular, minimization has worst case `O(n*k*logn)` time and `O(k*n)`
    /// space, where `n` is the number of DFA states and `k` is the alphabet
    /// size. In practice, minimization can be quite costly in terms of both
    /// space and time, so it should only be done if you're willing to wait
    /// longer to produce a DFA. In general, you might want a minimal DFA in
    /// the following circumstances:
    ///
    /// 1. You would like to optimize for the size of the automaton. This can
    ///    manifest in one of two ways. Firstly, if you're converting the
    ///    DFA into Rust code (or a table embedded in the code), then a minimal
    ///    DFA will translate into a corresponding reduction in code  size, and
    ///    thus, also the final compiled binary size. Secondly, if you are
    ///    building many DFAs and putting them on the heap, you'll be able to
    ///    fit more if they are smaller. Note though that building a minimal
    ///    DFA itself requires additional space; you only realize the space
    ///    savings once the minimal DFA is constructed (at which point, the
    ///    space used for minimization is freed).
    /// 2. You've observed that a smaller DFA results in faster match
    ///    performance. Naively, this isn't guaranteed since there is no
    ///    inherent difference between matching with a bigger-than-minimal
    ///    DFA and a minimal DFA. However, a smaller DFA may make use of your
    ///    CPU's cache more efficiently.
    /// 3. You are trying to establish an equivalence between regular
    ///    languages. The standard method for this is to build a minimal DFA
    ///    for each language and then compare them. If the DFAs are equivalent
    ///    (up to state renaming), then the languages are equivalent.
    ///
    /// Typically, minimization only makes sense as an offline process. That
    /// is, one might minimize a DFA before serializing it to persistent
    /// storage. In practical terms, minimization can take around an order of
    /// magnitude more time than compiling the initial DFA via determinization.
    ///
    /// This option is disabled by default.
    pub fn minimize(mut self, yes: bool) -> Config {
        self.minimize = Some(yes);
        self
    }

    /// Set the desired match semantics.
    ///
    /// The default is [`MatchKind::LeftmostFirst`], which corresponds to the
    /// match semantics of Perl-like regex engines. That is, when multiple
    /// patterns would match at the same leftmost position, the pattern that
    /// appears first in the concrete syntax is chosen.
    ///
    /// Currently, the only other kind of match semantics supported is
    /// [`MatchKind::All`]. This corresponds to classical DFA construction
    /// where all possible matches are added to the DFA.
    ///
    /// Typically, `All` is used when one wants to execute an overlapping
    /// search and `LeftmostFirst` otherwise. In particular, it rarely makes
    /// sense to use `All` with the various "leftmost" find routines, since the
    /// leftmost routines depend on the `LeftmostFirst` automata construction
    /// strategy. Specifically, `LeftmostFirst` adds dead states to the DFA
    /// as a way to terminate the search and report a match. `LeftmostFirst`
    /// also supports non-greedy matches using this strategy where as `All`
    /// does not.
    ///
    /// # Example: overlapping search
    ///
    /// This example shows the typical use of `MatchKind::All`, which is to
    /// report overlapping matches.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{
    ///     dfa::{Automaton, OverlappingState, dense},
    ///     HalfMatch, Input, MatchKind,
    /// };
    ///
    /// let dfa = dense::Builder::new()
    ///     .configure(dense::Config::new().match_kind(MatchKind::All))
    ///     .build_many(&[r"\w+$", r"\S+$"])?;
    /// let input = Input::new("@foo");
    /// let mut state = OverlappingState::start();
    ///
    /// let expected = Some(HalfMatch::must(1, 4));
    /// dfa.try_search_overlapping_fwd(&input, &mut state)?;
    /// assert_eq!(expected, state.get_match());
    ///
    /// // The first pattern also matches at the same position, so re-running
    /// // the search will yield another match. Notice also that the first
    /// // pattern is returned after the second. This is because the second
    /// // pattern begins its match before the first, is therefore an earlier
    /// // match and is thus reported first.
    /// let expected = Some(HalfMatch::must(0, 4));
    /// dfa.try_search_overlapping_fwd(&input, &mut state)?;
    /// assert_eq!(expected, state.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: reverse automaton to find start of match
    ///
    /// Another example for using `MatchKind::All` is for constructing a
    /// reverse automaton to find the start of a match. `All` semantics are
    /// used for this in order to find the longest possible match, which
    /// corresponds to the leftmost starting position.
    ///
    /// Note that if you need the starting position then
    /// [`dfa::regex::Regex`](crate::dfa::regex::Regex) will handle this for
    /// you, so it's usually not necessary to do this yourself.
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{dense, Automaton, StartKind},
    ///     nfa::thompson::NFA,
    ///     Anchored, HalfMatch, Input, MatchKind,
    /// };
    ///
    /// let haystack = "123foobar456".as_bytes();
    /// let pattern = r"[a-z]+r";
    ///
    /// let dfa_fwd = dense::DFA::new(pattern)?;
    /// let dfa_rev = dense::Builder::new()
    ///     .thompson(NFA::config().reverse(true))
    ///     .configure(dense::Config::new()
    ///         // This isn't strictly necessary since both anchored and
    ///         // unanchored searches are supported by default. But since
    ///         // finding the start-of-match only requires anchored searches,
    ///         // we can get rid of the unanchored configuration and possibly
    ///         // slim down our DFA considerably.
    ///         .start_kind(StartKind::Anchored)
    ///         .match_kind(MatchKind::All)
    ///     )
    ///     .build(pattern)?;
    /// let expected_fwd = HalfMatch::must(0, 9);
    /// let expected_rev = HalfMatch::must(0, 3);
    /// let got_fwd = dfa_fwd.try_search_fwd(&Input::new(haystack))?.unwrap();
    /// // Here we don't specify the pattern to search for since there's only
    /// // one pattern and we're doing a leftmost search. But if this were an
    /// // overlapping search, you'd need to specify the pattern that matched
    /// // in the forward direction. (Otherwise, you might wind up finding the
    /// // starting position of a match of some other pattern.) That in turn
    /// // requires building the reverse automaton with starts_for_each_pattern
    /// // enabled. Indeed, this is what Regex does internally.
    /// let input = Input::new(haystack)
    ///     .range(..got_fwd.offset())
    ///     .anchored(Anchored::Yes);
    /// let got_rev = dfa_rev.try_search_rev(&input)?.unwrap();
    /// assert_eq!(expected_fwd, got_fwd);
    /// assert_eq!(expected_rev, got_rev);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn match_kind(mut self, kind: MatchKind) -> Config {
        self.match_kind = Some(kind);
        self
    }

    /// The type of starting state configuration to use for a DFA.
    ///
    /// By default, the starting state configuration is [`StartKind::Both`].
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{dense::DFA, Automaton, StartKind},
    ///     Anchored, HalfMatch, Input,
    /// };
    ///
    /// let haystack = "quux foo123";
    /// let expected = HalfMatch::must(0, 11);
    ///
    /// // By default, DFAs support both anchored and unanchored searches.
    /// let dfa = DFA::new(r"[0-9]+")?;
    /// let input = Input::new(haystack);
    /// assert_eq!(Some(expected), dfa.try_search_fwd(&input)?);
    ///
    /// // But if we only need anchored searches, then we can build a DFA
    /// // that only supports anchored searches. This leads to a smaller DFA
    /// // (potentially significantly smaller in some cases), but a DFA that
    /// // will panic if you try to use it with an unanchored search.
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().start_kind(StartKind::Anchored))
    ///     .build(r"[0-9]+")?;
    /// let input = Input::new(haystack)
    ///     .range(8..)
    ///     .anchored(Anchored::Yes);
    /// assert_eq!(Some(expected), dfa.try_search_fwd(&input)?);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn start_kind(mut self, kind: StartKind) -> Config {
        self.start_kind = Some(kind);
        self
    }

    /// Whether to compile a separate start state for each pattern in the
    /// automaton.
    ///
    /// When enabled, a separate **anchored** start state is added for each
    /// pattern in the DFA. When this start state is used, then the DFA will
    /// only search for matches for the pattern specified, even if there are
    /// other patterns in the DFA.
    ///
    /// The main downside of this option is that it can potentially increase
    /// the size of the DFA and/or increase the time it takes to build the DFA.
    ///
    /// There are a few reasons one might want to enable this (it's disabled
    /// by default):
    ///
    /// 1. When looking for the start of an overlapping match (using a
    /// reverse DFA), doing it correctly requires starting the reverse search
    /// using the starting state of the pattern that matched in the forward
    /// direction. Indeed, when building a [`Regex`](crate::dfa::regex::Regex),
    /// it will automatically enable this option when building the reverse DFA
    /// internally.
    /// 2. When you want to use a DFA with multiple patterns to both search
    /// for matches of any pattern or to search for anchored matches of one
    /// particular pattern while using the same DFA. (Otherwise, you would need
    /// to compile a new DFA for each pattern.)
    /// 3. Since the start states added for each pattern are anchored, if you
    /// compile an unanchored DFA with one pattern while also enabling this
    /// option, then you can use the same DFA to perform anchored or unanchored
    /// searches. The latter you get with the standard search APIs. The former
    /// you get from the various `_at` search methods that allow you specify a
    /// pattern ID to search for.
    ///
    /// By default this is disabled.
    ///
    /// # Example
    ///
    /// This example shows how to use this option to permit the same DFA to
    /// run both anchored and unanchored searches for a single pattern.
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{dense, Automaton},
    ///     Anchored, HalfMatch, PatternID, Input,
    /// };
    ///
    /// let dfa = dense::Builder::new()
    ///     .configure(dense::Config::new().starts_for_each_pattern(true))
    ///     .build(r"foo[0-9]+")?;
    /// let haystack = "quux foo123";
    ///
    /// // Here's a normal unanchored search. Notice that we use 'None' for the
    /// // pattern ID. Since the DFA was built as an unanchored machine, it
    /// // use its default unanchored starting state.
    /// let expected = HalfMatch::must(0, 11);
    /// let input = Input::new(haystack);
    /// assert_eq!(Some(expected), dfa.try_search_fwd(&input)?);
    /// // But now if we explicitly specify the pattern to search ('0' being
    /// // the only pattern in the DFA), then it will use the starting state
    /// // for that specific pattern which is always anchored. Since the
    /// // pattern doesn't have a match at the beginning of the haystack, we
    /// // find nothing.
    /// let input = Input::new(haystack)
    ///     .anchored(Anchored::Pattern(PatternID::must(0)));
    /// assert_eq!(None, dfa.try_search_fwd(&input)?);
    /// // And finally, an anchored search is not the same as putting a '^' at
    /// // beginning of the pattern. An anchored search can only match at the
    /// // beginning of the *search*, which we can change:
    /// let input = Input::new(haystack)
    ///     .anchored(Anchored::Pattern(PatternID::must(0)))
    ///     .range(5..);
    /// assert_eq!(Some(expected), dfa.try_search_fwd(&input)?);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn starts_for_each_pattern(mut self, yes: bool) -> Config {
        self.starts_for_each_pattern = Some(yes);
        self
    }

    /// Whether to attempt to shrink the size of the DFA's alphabet or not.
    ///
    /// This option is enabled by default and should never be disabled unless
    /// one is debugging a generated DFA.
    ///
    /// When enabled, the DFA will use a map from all possible bytes to their
    /// corresponding equivalence class. Each equivalence class represents a
    /// set of bytes that does not discriminate between a match and a non-match
    /// in the DFA. For example, the pattern `[ab]+` has at least two
    /// equivalence classes: a set containing `a` and `b` and a set containing
    /// every byte except for `a` and `b`. `a` and `b` are in the same
    /// equivalence class because they never discriminate between a match and a
    /// non-match.
    ///
    /// The advantage of this map is that the size of the transition table
    /// can be reduced drastically from `#states * 256 * sizeof(StateID)` to
    /// `#states * k * sizeof(StateID)` where `k` is the number of equivalence
    /// classes (rounded up to the nearest power of 2). As a result, total
    /// space usage can decrease substantially. Moreover, since a smaller
    /// alphabet is used, DFA compilation becomes faster as well.
    ///
    /// **WARNING:** This is only useful for debugging DFAs. Disabling this
    /// does not yield any speed advantages. Namely, even when this is
    /// disabled, a byte class map is still used while searching. The only
    /// difference is that every byte will be forced into its own distinct
    /// equivalence class. This is useful for debugging the actual generated
    /// transitions because it lets one see the transitions defined on actual
    /// bytes instead of the equivalence classes.
    pub fn byte_classes(mut self, yes: bool) -> Config {
        self.byte_classes = Some(yes);
        self
    }

    /// Heuristically enable Unicode word boundaries.
    ///
    /// When set, this will attempt to implement Unicode word boundaries as if
    /// they were ASCII word boundaries. This only works when the search input
    /// is ASCII only. If a non-ASCII byte is observed while searching, then a
    /// [`MatchError::quit`](crate::MatchError::quit) error is returned.
    ///
    /// A possible alternative to enabling this option is to simply use an
    /// ASCII word boundary, e.g., via `(?-u:\b)`. The main reason to use this
    /// option is if you absolutely need Unicode support. This option lets one
    /// use a fast search implementation (a DFA) for some potentially very
    /// common cases, while providing the option to fall back to some other
    /// regex engine to handle the general case when an error is returned.
    ///
    /// If the pattern provided has no Unicode word boundary in it, then this
    /// option has no effect. (That is, quitting on a non-ASCII byte only
    /// occurs when this option is enabled _and_ a Unicode word boundary is
    /// present in the pattern.)
    ///
    /// This is almost equivalent to setting all non-ASCII bytes to be quit
    /// bytes. The only difference is that this will cause non-ASCII bytes to
    /// be quit bytes _only_ when a Unicode word boundary is present in the
    /// pattern.
    ///
    /// When enabling this option, callers _must_ be prepared to handle
    /// a [`MatchError`](crate::MatchError) error during search.
    /// When using a [`Regex`](crate::dfa::regex::Regex), this corresponds
    /// to using the `try_` suite of methods. Alternatively, if
    /// callers can guarantee that their input is ASCII only, then a
    /// [`MatchError::quit`](crate::MatchError::quit) error will never be
    /// returned while searching.
    ///
    /// This is disabled by default.
    ///
    /// # Example
    ///
    /// This example shows how to heuristically enable Unicode word boundaries
    /// in a pattern. It also shows what happens when a search comes across a
    /// non-ASCII byte.
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{Automaton, dense},
    ///     HalfMatch, Input, MatchError,
    /// };
    ///
    /// let dfa = dense::Builder::new()
    ///     .configure(dense::Config::new().unicode_word_boundary(true))
    ///     .build(r"\b[0-9]+\b")?;
    ///
    /// // The match occurs before the search ever observes the snowman
    /// // character, so no error occurs.
    /// let haystack = "foo 123  ☃".as_bytes();
    /// let expected = Some(HalfMatch::must(0, 7));
    /// let got = dfa.try_search_fwd(&Input::new(haystack))?;
    /// assert_eq!(expected, got);
    ///
    /// // Notice that this search fails, even though the snowman character
    /// // occurs after the ending match offset. This is because search
    /// // routines read one byte past the end of the search to account for
    /// // look-around, and indeed, this is required here to determine whether
    /// // the trailing \b matches.
    /// let haystack = "foo 123 ☃".as_bytes();
    /// let expected = MatchError::quit(0xE2, 8);
    /// let got = dfa.try_search_fwd(&Input::new(haystack));
    /// assert_eq!(Err(expected), got);
    ///
    /// // Another example is executing a search where the span of the haystack
    /// // we specify is all ASCII, but there is non-ASCII just before it. This
    /// // correctly also reports an error.
    /// let input = Input::new("β123").range(2..);
    /// let expected = MatchError::quit(0xB2, 1);
    /// let got = dfa.try_search_fwd(&input);
    /// assert_eq!(Err(expected), got);
    ///
    /// // And similarly for the trailing word boundary.
    /// let input = Input::new("123β").range(..3);
    /// let expected = MatchError::quit(0xCE, 3);
    /// let got = dfa.try_search_fwd(&input);
    /// assert_eq!(Err(expected), got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn unicode_word_boundary(mut self, yes: bool) -> Config {
        // We have a separate option for this instead of just setting the
        // appropriate quit bytes here because we don't want to set quit bytes
        // for every regex. We only want to set them when the regex contains a
        // Unicode word boundary.
        self.unicode_word_boundary = Some(yes);
        self
    }

    /// Add a "quit" byte to the DFA.
    ///
    /// When a quit byte is seen during search time, then search will return
    /// a [`MatchError::quit`](crate::MatchError::quit) error indicating the
    /// offset at which the search stopped.
    ///
    /// A quit byte will always overrule any other aspects of a regex. For
    /// example, if the `x` byte is added as a quit byte and the regex `\w` is
    /// used, then observing `x` will cause the search to quit immediately
    /// despite the fact that `x` is in the `\w` class.
    ///
    /// This mechanism is primarily useful for heuristically enabling certain
    /// features like Unicode word boundaries in a DFA. Namely, if the input
    /// to search is ASCII, then a Unicode word boundary can be implemented
    /// via an ASCII word boundary with no change in semantics. Thus, a DFA
    /// can attempt to match a Unicode word boundary but give up as soon as it
    /// observes a non-ASCII byte. Indeed, if callers set all non-ASCII bytes
    /// to be quit bytes, then Unicode word boundaries will be permitted when
    /// building DFAs. Of course, callers should enable
    /// [`Config::unicode_word_boundary`] if they want this behavior instead.
    /// (The advantage being that non-ASCII quit bytes will only be added if a
    /// Unicode word boundary is in the pattern.)
    ///
    /// When enabling this option, callers _must_ be prepared to handle a
    /// [`MatchError`](crate::MatchError) error during search. When using a
    /// [`Regex`](crate::dfa::regex::Regex), this corresponds to using the
    /// `try_` suite of methods.
    ///
    /// By default, there are no quit bytes set.
    ///
    /// # Panics
    ///
    /// This panics if heuristic Unicode word boundaries are enabled and any
    /// non-ASCII byte is removed from the set of quit bytes. Namely, enabling
    /// Unicode word boundaries requires setting every non-ASCII byte to a quit
    /// byte. So if the caller attempts to undo any of that, then this will
    /// panic.
    ///
    /// # Example
    ///
    /// This example shows how to cause a search to terminate if it sees a
    /// `\n` byte. This could be useful if, for example, you wanted to prevent
    /// a user supplied pattern from matching across a line boundary.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{dfa::{Automaton, dense}, Input, MatchError};
    ///
    /// let dfa = dense::Builder::new()
    ///     .configure(dense::Config::new().quit(b'\n', true))
    ///     .build(r"foo\p{any}+bar")?;
    ///
    /// let haystack = "foo\nbar".as_bytes();
    /// // Normally this would produce a match, since \p{any} contains '\n'.
    /// // But since we instructed the automaton to enter a quit state if a
    /// // '\n' is observed, this produces a match error instead.
    /// let expected = MatchError::quit(b'\n', 3);
    /// let got = dfa.try_search_fwd(&Input::new(haystack)).unwrap_err();
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn quit(mut self, byte: u8, yes: bool) -> Config {
        if self.get_unicode_word_boundary() && !byte.is_ascii() && !yes {
            panic!(
                "cannot set non-ASCII byte to be non-quit when \
                 Unicode word boundaries are enabled"
            );
        }
        if self.quitset.is_none() {
            self.quitset = Some(ByteSet::empty());
        }
        if yes {
            self.quitset.as_mut().unwrap().add(byte);
        } else {
            self.quitset.as_mut().unwrap().remove(byte);
        }
        self
    }

    /// Enable specializing start states in the DFA.
    ///
    /// When start states are specialized, an implementor of a search routine
    /// using a lazy DFA can tell when the search has entered a starting state.
    /// When start states aren't specialized, then it is impossible to know
    /// whether the search has entered a start state.
    ///
    /// Ideally, this option wouldn't need to exist and we could always
    /// specialize start states. The problem is that start states can be quite
    /// active. This in turn means that an efficient search routine is likely
    /// to ping-pong between a heavily optimized hot loop that handles most
    /// states and to a less optimized specialized handling of start states.
    /// This causes branches to get heavily mispredicted and overall can
    /// materially decrease throughput. Therefore, specializing start states
    /// should only be enabled when it is needed.
    ///
    /// Knowing whether a search is in a start state is typically useful when a
    /// prefilter is active for the search. A prefilter is typically only run
    /// when in a start state and a prefilter can greatly accelerate a search.
    /// Therefore, the possible cost of specializing start states is worth it
    /// in this case. Otherwise, if you have no prefilter, there is likely no
    /// reason to specialize start states.
    ///
    /// This is disabled by default, but note that it is automatically
    /// enabled (or disabled) if [`Config::prefilter`] is set. Namely, unless
    /// `specialize_start_states` has already been set, [`Config::prefilter`]
    /// will automatically enable or disable it based on whether a prefilter
    /// is present or not, respectively. This is done because a prefilter's
    /// effectiveness is rooted in being executed whenever the DFA is in a
    /// start state, and that's only possible to do when they are specialized.
    ///
    /// Note that it is plausibly reasonable to _disable_ this option
    /// explicitly while _enabling_ a prefilter. In that case, a prefilter
    /// will still be run at the beginning of a search, but never again. This
    /// in theory could strike a good balance if you're in a situation where a
    /// prefilter is likely to produce many false positive candidates.
    ///
    /// # Example
    ///
    /// This example shows how to enable start state specialization and then
    /// shows how to check whether a state is a start state or not.
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense::DFA}, Input};
    ///
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().specialize_start_states(true))
    ///     .build(r"[a-z]+")?;
    ///
    /// let haystack = "123 foobar 4567".as_bytes();
    /// let sid = dfa.start_state_forward(&Input::new(haystack))?;
    /// // The ID returned by 'start_state_forward' will always be tagged as
    /// // a start state when start state specialization is enabled.
    /// assert!(dfa.is_special_state(sid));
    /// assert!(dfa.is_start_state(sid));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Compare the above with the default DFA configuration where start states
    /// are _not_ specialized. In this case, the start state is not tagged at
    /// all:
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense::DFA}, Input};
    ///
    /// let dfa = DFA::new(r"[a-z]+")?;
    ///
    /// let haystack = "123 foobar 4567";
    /// let sid = dfa.start_state_forward(&Input::new(haystack))?;
    /// // Start states are not special in the default configuration!
    /// assert!(!dfa.is_special_state(sid));
    /// assert!(!dfa.is_start_state(sid));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn specialize_start_states(mut self, yes: bool) -> Config {
        self.specialize_start_states = Some(yes);
        self
    }

    /// Set a size limit on the total heap used by a DFA.
    ///
    /// This size limit is expressed in bytes and is applied during
    /// determinization of an NFA into a DFA. If the DFA's heap usage, and only
    /// the DFA, exceeds this configured limit, then determinization is stopped
    /// and an error is returned.
    ///
    /// This limit does not apply to auxiliary storage used during
    /// determinization that isn't part of the generated DFA.
    ///
    /// This limit is only applied during determinization. Currently, there is
    /// no way to post-pone this check to after minimization if minimization
    /// was enabled.
    ///
    /// The total limit on heap used during determinization is the sum of the
    /// DFA and determinization size limits.
    ///
    /// The default is no limit.
    ///
    /// # Example
    ///
    /// This example shows a DFA that fails to build because of a configured
    /// size limit. This particular example also serves as a cautionary tale
    /// demonstrating just how big DFAs with large Unicode character classes
    /// can get.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{dfa::{dense, Automaton}, Input};
    ///
    /// // 6MB isn't enough!
    /// dense::Builder::new()
    ///     .configure(dense::Config::new().dfa_size_limit(Some(6_000_000)))
    ///     .build(r"\w{20}")
    ///     .unwrap_err();
    ///
    /// // ... but 7MB probably is!
    /// // (Note that DFA sizes aren't necessarily stable between releases.)
    /// let dfa = dense::Builder::new()
    ///     .configure(dense::Config::new().dfa_size_limit(Some(7_000_000)))
    ///     .build(r"\w{20}")?;
    /// let haystack = "A".repeat(20).into_bytes();
    /// assert!(dfa.try_search_fwd(&Input::new(&haystack))?.is_some());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// While one needs a little more than 6MB to represent `\w{20}`, it
    /// turns out that you only need a little more than 6KB to represent
    /// `(?-u:\w{20})`. So only use Unicode if you need it!
    ///
    /// As with [`Config::determinize_size_limit`], the size of a DFA is
    /// influenced by other factors, such as what start state configurations
    /// to support. For example, if you only need unanchored searches and not
    /// anchored searches, then configuring the DFA to only support unanchored
    /// searches can reduce its size. By default, DFAs support both unanchored
    /// and anchored searches.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{dfa::{dense, Automaton, StartKind}, Input};
    ///
    /// // 3MB isn't enough!
    /// dense::Builder::new()
    ///     .configure(dense::Config::new()
    ///         .dfa_size_limit(Some(3_000_000))
    ///         .start_kind(StartKind::Unanchored)
    ///     )
    ///     .build(r"\w{20}")
    ///     .unwrap_err();
    ///
    /// // ... but 4MB probably is!
    /// // (Note that DFA sizes aren't necessarily stable between releases.)
    /// let dfa = dense::Builder::new()
    ///     .configure(dense::Config::new()
    ///         .dfa_size_limit(Some(4_000_000))
    ///         .start_kind(StartKind::Unanchored)
    ///     )
    ///     .build(r"\w{20}")?;
    /// let haystack = "A".repeat(20).into_bytes();
    /// assert!(dfa.try_search_fwd(&Input::new(&haystack))?.is_some());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn dfa_size_limit(mut self, bytes: Option<usize>) -> Config {
        self.dfa_size_limit = Some(bytes);
        self
    }

    /// Set a size limit on the total heap used by determinization.
    ///
    /// This size limit is expressed in bytes and is applied during
    /// determinization of an NFA into a DFA. If the heap used for auxiliary
    /// storage during determinization (memory that is not in the DFA but
    /// necessary for building the DFA) exceeds this configured limit, then
    /// determinization is stopped and an error is returned.
    ///
    /// This limit does not apply to heap used by the DFA itself.
    ///
    /// The total limit on heap used during determinization is the sum of the
    /// DFA and determinization size limits.
    ///
    /// The default is no limit.
    ///
    /// # Example
    ///
    /// This example shows a DFA that fails to build because of a
    /// configured size limit on the amount of heap space used by
    /// determinization. This particular example complements the example for
    /// [`Config::dfa_size_limit`] by demonstrating that not only does Unicode
    /// potentially make DFAs themselves big, but it also results in more
    /// auxiliary storage during determinization. (Although, auxiliary storage
    /// is still not as much as the DFA itself.)
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// # if !cfg!(target_pointer_width = "64") { return Ok(()); } // see #1039
    /// use regex_automata::{dfa::{dense, Automaton}, Input};
    ///
    /// // 700KB isn't enough!
    /// dense::Builder::new()
    ///     .configure(dense::Config::new()
    ///         .determinize_size_limit(Some(700_000))
    ///     )
    ///     .build(r"\w{20}")
    ///     .unwrap_err();
    ///
    /// // ... but 800KB probably is!
    /// // (Note that auxiliary storage sizes aren't necessarily stable between
    /// // releases.)
    /// let dfa = dense::Builder::new()
    ///     .configure(dense::Config::new()
    ///         .determinize_size_limit(Some(800_000))
    ///     )
    ///     .build(r"\w{20}")?;
    /// let haystack = "A".repeat(20).into_bytes();
    /// assert!(dfa.try_search_fwd(&Input::new(&haystack))?.is_some());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Note that some parts of the configuration on a DFA can have a
    /// big impact on how big the DFA is, and thus, how much memory is
    /// used. For example, the default setting for [`Config::start_kind`] is
    /// [`StartKind::Both`]. But if you only need an anchored search, for
    /// example, then it can be much cheaper to build a DFA that only supports
    /// anchored searches. (Running an unanchored search with it would panic.)
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// # if !cfg!(target_pointer_width = "64") { return Ok(()); } // see #1039
    /// use regex_automata::{
    ///     dfa::{dense, Automaton, StartKind},
    ///     Anchored, Input,
    /// };
    ///
    /// // 200KB isn't enough!
    /// dense::Builder::new()
    ///     .configure(dense::Config::new()
    ///         .determinize_size_limit(Some(200_000))
    ///         .start_kind(StartKind::Anchored)
    ///     )
    ///     .build(r"\w{20}")
    ///     .unwrap_err();
    ///
    /// // ... but 300KB probably is!
    /// // (Note that auxiliary storage sizes aren't necessarily stable between
    /// // releases.)
    /// let dfa = dense::Builder::new()
    ///     .configure(dense::Config::new()
    ///         .determinize_size_limit(Some(300_000))
    ///         .start_kind(StartKind::Anchored)
    ///     )
    ///     .build(r"\w{20}")?;
    /// let haystack = "A".repeat(20).into_bytes();
    /// let input = Input::new(&haystack).anchored(Anchored::Yes);
    /// assert!(dfa.try_search_fwd(&input)?.is_some());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn determinize_size_limit(mut self, bytes: Option<usize>) -> Config {
        self.determinize_size_limit = Some(bytes);
        self
    }

    /// Returns whether this configuration has enabled simple state
    /// acceleration.
    pub fn get_accelerate(&self) -> bool {
        self.accelerate.unwrap_or(true)
    }

    /// Returns the prefilter attached to this configuration, if any.
    pub fn get_prefilter(&self) -> Option<&Prefilter> {
        self.pre.as_ref().unwrap_or(&None).as_ref()
    }

    /// Returns whether this configuration has enabled the expensive process
    /// of minimizing a DFA.
    pub fn get_minimize(&self) -> bool {
        self.minimize.unwrap_or(false)
    }

    /// Returns the match semantics set in this configuration.
    pub fn get_match_kind(&self) -> MatchKind {
        self.match_kind.unwrap_or(MatchKind::LeftmostFirst)
    }

    /// Returns the starting state configuration for a DFA.
    pub fn get_starts(&self) -> StartKind {
        self.start_kind.unwrap_or(StartKind::Both)
    }

    /// Returns whether this configuration has enabled anchored starting states
    /// for every pattern in the DFA.
    pub fn get_starts_for_each_pattern(&self) -> bool {
        self.starts_for_each_pattern.unwrap_or(false)
    }

    /// Returns whether this configuration has enabled byte classes or not.
    /// This is typically a debugging oriented option, as disabling it confers
    /// no speed benefit.
    pub fn get_byte_classes(&self) -> bool {
        self.byte_classes.unwrap_or(true)
    }

    /// Returns whether this configuration has enabled heuristic Unicode word
    /// boundary support. When enabled, it is possible for a search to return
    /// an error.
    pub fn get_unicode_word_boundary(&self) -> bool {
        self.unicode_word_boundary.unwrap_or(false)
    }

    /// Returns whether this configuration will instruct the DFA to enter a
    /// quit state whenever the given byte is seen during a search. When at
    /// least one byte has this enabled, it is possible for a search to return
    /// an error.
    pub fn get_quit(&self, byte: u8) -> bool {
        self.quitset.map_or(false, |q| q.contains(byte))
    }

    /// Returns whether this configuration will instruct the DFA to
    /// "specialize" start states. When enabled, the DFA will mark start states
    /// as "special" so that search routines using the DFA can detect when
    /// it's in a start state and do some kind of optimization (like run a
    /// prefilter).
    pub fn get_specialize_start_states(&self) -> bool {
        self.specialize_start_states.unwrap_or(false)
    }

    /// Returns the DFA size limit of this configuration if one was set.
    /// The size limit is total number of bytes on the heap that a DFA is
    /// permitted to use. If the DFA exceeds this limit during construction,
    /// then construction is stopped and an error is returned.
    pub fn get_dfa_size_limit(&self) -> Option<usize> {
        self.dfa_size_limit.unwrap_or(None)
    }

    /// Returns the determinization size limit of this configuration if one
    /// was set. The size limit is total number of bytes on the heap that
    /// determinization is permitted to use. If determinization exceeds this
    /// limit during construction, then construction is stopped and an error is
    /// returned.
    ///
    /// This is different from the DFA size limit in that this only applies to
    /// the auxiliary storage used during determinization. Once determinization
    /// is complete, this memory is freed.
    ///
    /// The limit on the total heap memory used is the sum of the DFA and
    /// determinization size limits.
    pub fn get_determinize_size_limit(&self) -> Option<usize> {
        self.determinize_size_limit.unwrap_or(None)
    }

    /// Overwrite the default configuration such that the options in `o` are
    /// always used. If an option in `o` is not set, then the corresponding
    /// option in `self` is used. If it's not set in `self` either, then it
    /// remains not set.
    pub(crate) fn overwrite(&self, o: Config) -> Config {
        Config {
            accelerate: o.accelerate.or(self.accelerate),
            pre: o.pre.or_else(|| self.pre.clone()),
            minimize: o.minimize.or(self.minimize),
            match_kind: o.match_kind.or(self.match_kind),
            start_kind: o.start_kind.or(self.start_kind),
            starts_for_each_pattern: o
                .starts_for_each_pattern
                .or(self.starts_for_each_pattern),
            byte_classes: o.byte_classes.or(self.byte_classes),
            unicode_word_boundary: o
                .unicode_word_boundary
                .or(self.unicode_word_boundary),
            quitset: o.quitset.or(self.quitset),
            specialize_start_states: o
                .specialize_start_states
                .or(self.specialize_start_states),
            dfa_size_limit: o.dfa_size_limit.or(self.dfa_size_limit),
            determinize_size_limit: o
                .determinize_size_limit
                .or(self.determinize_size_limit),
        }
    }
}

/// A builder for constructing a deterministic finite automaton from regular
/// expressions.
///
/// This builder provides two main things:
///
/// 1. It provides a few different `build` routines for actually constructing
/// a DFA from different kinds of inputs. The most convenient is
/// [`Builder::build`], which builds a DFA directly from a pattern string. The
/// most flexible is [`Builder::build_from_nfa`], which builds a DFA straight
/// from an NFA.
/// 2. The builder permits configuring a number of things.
/// [`Builder::configure`] is used with [`Config`] to configure aspects of
/// the DFA and the construction process itself. [`Builder::syntax`] and
/// [`Builder::thompson`] permit configuring the regex parser and Thompson NFA
/// construction, respectively. The syntax and thompson configurations only
/// apply when building from a pattern string.
///
/// This builder always constructs a *single* DFA. As such, this builder
/// can only be used to construct regexes that either detect the presence
/// of a match or find the end location of a match. A single DFA cannot
/// produce both the start and end of a match. For that information, use a
/// [`Regex`](crate::dfa::regex::Regex), which can be similarly configured
/// using [`regex::Builder`](crate::dfa::regex::Builder). The main reason to
/// use a DFA directly is if the end location of a match is enough for your use
/// case. Namely, a `Regex` will construct two DFAs instead of one, since a
/// second reverse DFA is needed to find the start of a match.
///
/// Note that if one wants to build a sparse DFA, you must first build a dense
/// DFA and convert that to a sparse DFA. There is no way to build a sparse
/// DFA without first building a dense DFA.
///
/// # Example
///
/// This example shows how to build a minimized DFA that completely disables
/// Unicode. That is:
///
/// * Things such as `\w`, `.` and `\b` are no longer Unicode-aware. `\w`
///   and `\b` are ASCII-only while `.` matches any byte except for `\n`
///   (instead of any UTF-8 encoding of a Unicode scalar value except for
///   `\n`). Things that are Unicode only, such as `\pL`, are not allowed.
/// * The pattern itself is permitted to match invalid UTF-8. For example,
///   things like `[^a]` that match any byte except for `a` are permitted.
///
/// ```
/// use regex_automata::{
///     dfa::{Automaton, dense},
///     util::syntax,
///     HalfMatch, Input,
/// };
///
/// let dfa = dense::Builder::new()
///     .configure(dense::Config::new().minimize(false))
///     .syntax(syntax::Config::new().unicode(false).utf8(false))
///     .build(r"foo[^b]ar.*")?;
///
/// let haystack = b"\xFEfoo\xFFar\xE2\x98\xFF\n";
/// let expected = Some(HalfMatch::must(0, 10));
/// let got = dfa.try_search_fwd(&Input::new(haystack))?;
/// assert_eq!(expected, got);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[cfg(feature = "dfa-build")]
#[derive(Clone, Debug)]
pub struct Builder {
    config: Config,
    #[cfg(feature = "syntax")]
    thompson: thompson::Compiler,
}

#[cfg(feature = "dfa-build")]
impl Builder {
    /// Create a new dense DFA builder with the default configuration.
    pub fn new() -> Builder {
        Builder {
            config: Config::default(),
            #[cfg(feature = "syntax")]
            thompson: thompson::Compiler::new(),
        }
    }

    /// Build a DFA from the given pattern.
    ///
    /// If there was a problem parsing or compiling the pattern, then an error
    /// is returned.
    #[cfg(feature = "syntax")]
    pub fn build(&self, pattern: &str) -> Result<OwnedDFA, BuildError> {
        self.build_many(&[pattern])
    }

    /// Build a DFA from the given patterns.
    ///
    /// When matches are returned, the pattern ID corresponds to the index of
    /// the pattern in the slice given.
    #[cfg(feature = "syntax")]
    pub fn build_many<P: AsRef<str>>(
        &self,
        patterns: &[P],
    ) -> Result<OwnedDFA, BuildError> {
        let nfa = self
            .thompson
            .clone()
            // We can always forcefully disable captures because DFAs do not
            // support them.
            .configure(
                thompson::Config::new()
                    .which_captures(thompson::WhichCaptures::None),
            )
            .build_many(patterns)
            .map_err(BuildError::nfa)?;
        self.build_from_nfa(&nfa)
    }

    /// Build a DFA from the given NFA.
    ///
    /// # Example
    ///
    /// This example shows how to build a DFA if you already have an NFA in
    /// hand.
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{Automaton, dense},
    ///     nfa::thompson::NFA,
    ///     HalfMatch, Input,
    /// };
    ///
    /// let haystack = "foo123bar".as_bytes();
    ///
    /// // This shows how to set non-default options for building an NFA.
    /// let nfa = NFA::compiler()
    ///     .configure(NFA::config().shrink(true))
    ///     .build(r"[0-9]+")?;
    /// let dfa = dense::Builder::new().build_from_nfa(&nfa)?;
    /// let expected = Some(HalfMatch::must(0, 6));
    /// let got = dfa.try_search_fwd(&Input::new(haystack))?;
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn build_from_nfa(
        &self,
        nfa: &thompson::NFA,
    ) -> Result<OwnedDFA, BuildError> {
        let mut quitset = self.config.quitset.unwrap_or(ByteSet::empty());
        if self.config.get_unicode_word_boundary()
            && nfa.look_set_any().contains_word_unicode()
        {
            for b in 0x80..=0xFF {
                quitset.add(b);
            }
        }
        let classes = if !self.config.get_byte_classes() {
            // DFAs will always use the equivalence class map, but enabling
            // this option is useful for debugging. Namely, this will cause all
            // transitions to be defined over their actual bytes instead of an
            // opaque equivalence class identifier. The former is much easier
            // to grok as a human.
            ByteClasses::singletons()
        } else {
            let mut set = nfa.byte_class_set().clone();
            // It is important to distinguish any "quit" bytes from all other
            // bytes. Otherwise, a non-quit byte may end up in the same
            // class as a quit byte, and thus cause the DFA to stop when it
            // shouldn't.
            //
            // Test case:
            //
            //   regex-cli find match dense --unicode-word-boundary \
            //     -p '^#' -p '\b10\.55\.182\.100\b' -y @conn.json.1000x.log
            if !quitset.is_empty() {
                set.add_set(&quitset);
            }
            set.byte_classes()
        };

        let mut dfa = DFA::initial(
            classes,
            nfa.pattern_len(),
            self.config.get_starts(),
            nfa.look_matcher(),
            self.config.get_starts_for_each_pattern(),
            self.config.get_prefilter().map(|p| p.clone()),
            quitset,
            Flags::from_nfa(&nfa),
        )?;
        determinize::Config::new()
            .match_kind(self.config.get_match_kind())
            .quit(quitset)
            .dfa_size_limit(self.config.get_dfa_size_limit())
            .determinize_size_limit(self.config.get_determinize_size_limit())
            .run(nfa, &mut dfa)?;
        if self.config.get_minimize() {
            dfa.minimize();
        }
        if self.config.get_accelerate() {
            dfa.accelerate();
        }
        // The state shuffling done before this point always assumes that start
        // states should be marked as "special," even though it isn't the
        // default configuration. State shuffling is complex enough as it is,
        // so it's simpler to just "fix" our special state ID ranges to not
        // include starting states after-the-fact.
        if !self.config.get_specialize_start_states() {
            dfa.special.set_no_special_start_states();
        }
        // Look for and set the universal starting states.
        dfa.set_universal_starts();
        Ok(dfa)
    }

    /// Apply the given dense DFA configuration options to this builder.
    pub fn configure(&mut self, config: Config) -> &mut Builder {
        self.config = self.config.overwrite(config);
        self
    }

    /// Set the syntax configuration for this builder using
    /// [`syntax::Config`](crate::util::syntax::Config).
    ///
    /// This permits setting things like case insensitivity, Unicode and multi
    /// line mode.
    ///
    /// These settings only apply when constructing a DFA directly from a
    /// pattern.
    #[cfg(feature = "syntax")]
    pub fn syntax(
        &mut self,
        config: crate::util::syntax::Config,
    ) -> &mut Builder {
        self.thompson.syntax(config);
        self
    }

    /// Set the Thompson NFA configuration for this builder using
    /// [`nfa::thompson::Config`](crate::nfa::thompson::Config).
    ///
    /// This permits setting things like whether the DFA should match the regex
    /// in reverse or if additional time should be spent shrinking the size of
    /// the NFA.
    ///
    /// These settings only apply when constructing a DFA directly from a
    /// pattern.
    #[cfg(feature = "syntax")]
    pub fn thompson(&mut self, config: thompson::Config) -> &mut Builder {
        self.thompson.configure(config);
        self
    }
}

#[cfg(feature = "dfa-build")]
impl Default for Builder {
    fn default() -> Builder {
        Builder::new()
    }
}

/// A convenience alias for an owned DFA. We use this particular instantiation
/// a lot in this crate, so it's worth giving it a name. This instantiation
/// is commonly used for mutable APIs on the DFA while building it. The main
/// reason for making DFAs generic is no_std support, and more generally,
/// making it possible to load a DFA from an arbitrary slice of bytes.
#[cfg(feature = "alloc")]
pub(crate) type OwnedDFA = DFA<alloc::vec::Vec<u32>>;

/// A dense table-based deterministic finite automaton (DFA).
///
/// All dense DFAs have one or more start states, zero or more match states
/// and a transition table that maps the current state and the current byte
/// of input to the next state. A DFA can use this information to implement
/// fast searching. In particular, the use of a dense DFA generally makes the
/// trade off that match speed is the most valuable characteristic, even if
/// building the DFA may take significant time *and* space. (More concretely,
/// building a DFA takes time and space that is exponential in the size of the
/// pattern in the worst case.) As such, the processing of every byte of input
/// is done with a small constant number of operations that does not vary with
/// the pattern, its size or the size of the alphabet. If your needs don't line
/// up with this trade off, then a dense DFA may not be an adequate solution to
/// your problem.
///
/// In contrast, a [`sparse::DFA`] makes the opposite
/// trade off: it uses less space but will execute a variable number of
/// instructions per byte at match time, which makes it slower for matching.
/// (Note that space usage is still exponential in the size of the pattern in
/// the worst case.)
///
/// A DFA can be built using the default configuration via the
/// [`DFA::new`] constructor. Otherwise, one can
/// configure various aspects via [`dense::Builder`](Builder).
///
/// A single DFA fundamentally supports the following operations:
///
/// 1. Detection of a match.
/// 2. Location of the end of a match.
/// 3. In the case of a DFA with multiple patterns, which pattern matched is
///    reported as well.
///
/// A notable absence from the above list of capabilities is the location of
/// the *start* of a match. In order to provide both the start and end of
/// a match, *two* DFAs are required. This functionality is provided by a
/// [`Regex`](crate::dfa::regex::Regex).
///
/// # Type parameters
///
/// A `DFA` has one type parameter, `T`, which is used to represent state IDs,
/// pattern IDs and accelerators. `T` is typically a `Vec<u32>` or a `&[u32]`.
///
/// # The `Automaton` trait
///
/// This type implements the [`Automaton`] trait, which means it can be used
/// for searching. For example:
///
/// ```
/// use regex_automata::{dfa::{Automaton, dense::DFA}, HalfMatch, Input};
///
/// let dfa = DFA::new("foo[0-9]+")?;
/// let expected = HalfMatch::must(0, 8);
/// assert_eq!(Some(expected), dfa.try_search_fwd(&Input::new("foo12345"))?);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct DFA<T> {
    /// The transition table for this DFA. This includes the transitions
    /// themselves, along with the stride, number of states and the equivalence
    /// class mapping.
    tt: TransitionTable<T>,
    /// The set of starting state identifiers for this DFA. The starting state
    /// IDs act as pointers into the transition table. The specific starting
    /// state chosen for each search is dependent on the context at which the
    /// search begins.
    st: StartTable<T>,
    /// The set of match states and the patterns that match for each
    /// corresponding match state.
    ///
    /// This structure is technically only needed because of support for
    /// multi-regexes. Namely, multi-regexes require answering not just whether
    /// a match exists, but _which_ patterns match. So we need to store the
    /// matching pattern IDs for each match state. We do this even when there
    /// is only one pattern for the sake of simplicity. In practice, this uses
    /// up very little space for the case of one pattern.
    ms: MatchStates<T>,
    /// Information about which states are "special." Special states are states
    /// that are dead, quit, matching, starting or accelerated. For more info,
    /// see the docs for `Special`.
    special: Special,
    /// The accelerators for this DFA.
    ///
    /// If a state is accelerated, then there exist only a small number of
    /// bytes that can cause the DFA to leave the state. This permits searching
    /// to use optimized routines to find those specific bytes instead of using
    /// the transition table.
    ///
    /// All accelerated states exist in a contiguous range in the DFA's
    /// transition table. See dfa/special.rs for more details on how states are
    /// arranged.
    accels: Accels<T>,
    /// Any prefilter attached to this DFA.
    ///
    /// Note that currently prefilters are not serialized. When deserializing
    /// a DFA from bytes, this is always set to `None`.
    pre: Option<Prefilter>,
    /// The set of "quit" bytes for this DFA.
    ///
    /// This is only used when computing the start state for a particular
    /// position in a haystack. Namely, in the case where there is a quit
    /// byte immediately before the start of the search, this set needs to be
    /// explicitly consulted. In all other cases, quit bytes are detected by
    /// the DFA itself, by transitioning all quit bytes to a special "quit
    /// state."
    quitset: ByteSet,
    /// Various flags describing the behavior of this DFA.
    flags: Flags,
}

#[cfg(feature = "dfa-build")]
impl OwnedDFA {
    /// Parse the given regular expression using a default configuration and
    /// return the corresponding DFA.
    ///
    /// If you want a non-default configuration, then use the
    /// [`dense::Builder`](Builder) to set your own configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense}, HalfMatch, Input};
    ///
    /// let dfa = dense::DFA::new("foo[0-9]+bar")?;
    /// let expected = Some(HalfMatch::must(0, 11));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345bar"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "syntax")]
    pub fn new(pattern: &str) -> Result<OwnedDFA, BuildError> {
        Builder::new().build(pattern)
    }

    /// Parse the given regular expressions using a default configuration and
    /// return the corresponding multi-DFA.
    ///
    /// If you want a non-default configuration, then use the
    /// [`dense::Builder`](Builder) to set your own configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense}, HalfMatch, Input};
    ///
    /// let dfa = dense::DFA::new_many(&["[0-9]+", "[a-z]+"])?;
    /// let expected = Some(HalfMatch::must(1, 3));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345bar"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "syntax")]
    pub fn new_many<P: AsRef<str>>(
        patterns: &[P],
    ) -> Result<OwnedDFA, BuildError> {
        Builder::new().build_many(patterns)
    }
}

#[cfg(feature = "dfa-build")]
impl OwnedDFA {
    /// Create a new DFA that matches every input.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense}, HalfMatch, Input};
    ///
    /// let dfa = dense::DFA::always_match()?;
    ///
    /// let expected = Some(HalfMatch::must(0, 0));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new(""))?);
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn always_match() -> Result<OwnedDFA, BuildError> {
        let nfa = thompson::NFA::always_match();
        Builder::new().build_from_nfa(&nfa)
    }

    /// Create a new DFA that never matches any input.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense}, Input};
    ///
    /// let dfa = dense::DFA::never_match()?;
    /// assert_eq!(None, dfa.try_search_fwd(&Input::new(""))?);
    /// assert_eq!(None, dfa.try_search_fwd(&Input::new("foo"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn never_match() -> Result<OwnedDFA, BuildError> {
        let nfa = thompson::NFA::never_match();
        Builder::new().build_from_nfa(&nfa)
    }

    /// Create an initial DFA with the given equivalence classes, pattern
    /// length and whether anchored starting states are enabled for each
    /// pattern. An initial DFA can be further mutated via determinization.
    fn initial(
        classes: ByteClasses,
        pattern_len: usize,
        starts: StartKind,
        lookm: &LookMatcher,
        starts_for_each_pattern: bool,
        pre: Option<Prefilter>,
        quitset: ByteSet,
        flags: Flags,
    ) -> Result<OwnedDFA, BuildError> {
        let start_pattern_len =
            if starts_for_each_pattern { Some(pattern_len) } else { None };
        Ok(DFA {
            tt: TransitionTable::minimal(classes),
            st: StartTable::dead(starts, lookm, start_pattern_len)?,
            ms: MatchStates::empty(pattern_len),
            special: Special::new(),
            accels: Accels::empty(),
            pre,
            quitset,
            flags,
        })
    }
}

#[cfg(feature = "dfa-build")]
impl DFA<&[u32]> {
    /// Return a new default dense DFA compiler configuration.
    ///
    /// This is a convenience routine to avoid needing to import the [`Config`]
    /// type when customizing the construction of a dense DFA.
    pub fn config() -> Config {
        Config::new()
    }

    /// Create a new dense DFA builder with the default configuration.
    ///
    /// This is a convenience routine to avoid needing to import the
    /// [`Builder`] type in common cases.
    pub fn builder() -> Builder {
        Builder::new()
    }
}

impl<T: AsRef<[u32]>> DFA<T> {
    /// Cheaply return a borrowed version of this dense DFA. Specifically,
    /// the DFA returned always uses `&[u32]` for its transition table.
    pub fn as_ref(&self) -> DFA<&'_ [u32]> {
        DFA {
            tt: self.tt.as_ref(),
            st: self.st.as_ref(),
            ms: self.ms.as_ref(),
            special: self.special,
            accels: self.accels(),
            pre: self.pre.clone(),
            quitset: self.quitset,
            flags: self.flags,
        }
    }

    /// Return an owned version of this sparse DFA. Specifically, the DFA
    /// returned always uses `Vec<u32>` for its transition table.
    ///
    /// Effectively, this returns a dense DFA whose transition table lives on
    /// the heap.
    #[cfg(feature = "alloc")]
    pub fn to_owned(&self) -> OwnedDFA {
        DFA {
            tt: self.tt.to_owned(),
            st: self.st.to_owned(),
            ms: self.ms.to_owned(),
            special: self.special,
            accels: self.accels().to_owned(),
            pre: self.pre.clone(),
            quitset: self.quitset,
            flags: self.flags,
        }
    }

    /// Returns the starting state configuration for this DFA.
    ///
    /// The default is [`StartKind::Both`], which means the DFA supports both
    /// unanchored and anchored searches. However, this can generally lead to
    /// bigger DFAs. Therefore, a DFA might be compiled with support for just
    /// unanchored or anchored searches. In that case, running a search with
    /// an unsupported configuration will panic.
    pub fn start_kind(&self) -> StartKind {
        self.st.kind
    }

    /// Returns the start byte map used for computing the `Start` configuration
    /// at the beginning of a search.
    pub(crate) fn start_map(&self) -> &StartByteMap {
        &self.st.start_map
    }

    /// Returns true only if this DFA has starting states for each pattern.
    ///
    /// When a DFA has starting states for each pattern, then a search with the
    /// DFA can be configured to only look for anchored matches of a specific
    /// pattern. Specifically, APIs like [`Automaton::try_search_fwd`] can
    /// accept a non-None `pattern_id` if and only if this method returns true.
    /// Otherwise, calling `try_search_fwd` will panic.
    ///
    /// Note that if the DFA has no patterns, this always returns false.
    pub fn starts_for_each_pattern(&self) -> bool {
        self.st.pattern_len.is_some()
    }

    /// Returns the equivalence classes that make up the alphabet for this DFA.
    ///
    /// Unless [`Config::byte_classes`] was disabled, it is possible that
    /// multiple distinct bytes are grouped into the same equivalence class
    /// if it is impossible for them to discriminate between a match and a
    /// non-match. This has the effect of reducing the overall alphabet size
    /// and in turn potentially substantially reducing the size of the DFA's
    /// transition table.
    ///
    /// The downside of using equivalence classes like this is that every state
    /// transition will automatically use this map to convert an arbitrary
    /// byte to its corresponding equivalence class. In practice this has a
    /// negligible impact on performance.
    pub fn byte_classes(&self) -> &ByteClasses {
        &self.tt.classes
    }

    /// Returns the total number of elements in the alphabet for this DFA.
    ///
    /// That is, this returns the total number of transitions that each state
    /// in this DFA must have. Typically, a normal byte oriented DFA would
    /// always have an alphabet size of 256, corresponding to the number of
    /// unique values in a single byte. However, this implementation has two
    /// peculiarities that impact the alphabet length:
    ///
    /// * Every state has a special "EOI" transition that is only followed
    /// after the end of some haystack is reached. This EOI transition is
    /// necessary to account for one byte of look-ahead when implementing
    /// things like `\b` and `$`.
    /// * Bytes are grouped into equivalence classes such that no two bytes in
    /// the same class can distinguish a match from a non-match. For example,
    /// in the regex `^[a-z]+$`, the ASCII bytes `a-z` could all be in the
    /// same equivalence class. This leads to a massive space savings.
    ///
    /// Note though that the alphabet length does _not_ necessarily equal the
    /// total stride space taken up by a single DFA state in the transition
    /// table. Namely, for performance reasons, the stride is always the
    /// smallest power of two that is greater than or equal to the alphabet
    /// length. For this reason, [`DFA::stride`] or [`DFA::stride2`] are
    /// often more useful. The alphabet length is typically useful only for
    /// informational purposes.
    pub fn alphabet_len(&self) -> usize {
        self.tt.alphabet_len()
    }

    /// Returns the total stride for every state in this DFA, expressed as the
    /// exponent of a power of 2. The stride is the amount of space each state
    /// takes up in the transition table, expressed as a number of transitions.
    /// (Unused transitions map to dead states.)
    ///
    /// The stride of a DFA is always equivalent to the smallest power of 2
    /// that is greater than or equal to the DFA's alphabet length. This
    /// definition uses extra space, but permits faster translation between
    /// premultiplied state identifiers and contiguous indices (by using shifts
    /// instead of relying on integer division).
    ///
    /// For example, if the DFA's stride is 16 transitions, then its `stride2`
    /// is `4` since `2^4 = 16`.
    ///
    /// The minimum `stride2` value is `1` (corresponding to a stride of `2`)
    /// while the maximum `stride2` value is `9` (corresponding to a stride of
    /// `512`). The maximum is not `8` since the maximum alphabet size is `257`
    /// when accounting for the special EOI transition. However, an alphabet
    /// length of that size is exceptionally rare since the alphabet is shrunk
    /// into equivalence classes.
    pub fn stride2(&self) -> usize {
        self.tt.stride2
    }

    /// Returns the total stride for every state in this DFA. This corresponds
    /// to the total number of transitions used by each state in this DFA's
    /// transition table.
    ///
    /// Please see [`DFA::stride2`] for more information. In particular, this
    /// returns the stride as the number of transitions, where as `stride2`
    /// returns it as the exponent of a power of 2.
    pub fn stride(&self) -> usize {
        self.tt.stride()
    }

    /// Returns the memory usage, in bytes, of this DFA.
    ///
    /// The memory usage is computed based on the number of bytes used to
    /// represent this DFA.
    ///
    /// This does **not** include the stack size used up by this DFA. To
    /// compute that, use `std::mem::size_of::<dense::DFA>()`.
    pub fn memory_usage(&self) -> usize {
        self.tt.memory_usage()
            + self.st.memory_usage()
            + self.ms.memory_usage()
            + self.accels.memory_usage()
    }
}

/// Routines for converting a dense DFA to other representations, such as
/// sparse DFAs or raw bytes suitable for persistent storage.
impl<T: AsRef<[u32]>> DFA<T> {
    /// Convert this dense DFA to a sparse DFA.
    ///
    /// If a `StateID` is too small to represent all states in the sparse
    /// DFA, then this returns an error. In most cases, if a dense DFA is
    /// constructable with `StateID` then a sparse DFA will be as well.
    /// However, it is not guaranteed.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense}, HalfMatch, Input};
    ///
    /// let dense = dense::DFA::new("foo[0-9]+")?;
    /// let sparse = dense.to_sparse()?;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, sparse.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "dfa-build")]
    pub fn to_sparse(&self) -> Result<sparse::DFA<Vec<u8>>, BuildError> {
        sparse::DFA::from_dense(self)
    }

    /// Serialize this DFA as raw bytes to a `Vec<u8>` in little endian
    /// format. Upon success, the `Vec<u8>` and the initial padding length are
    /// returned.
    ///
    /// The written bytes are guaranteed to be deserialized correctly and
    /// without errors in a semver compatible release of this crate by a
    /// `DFA`'s deserialization APIs (assuming all other criteria for the
    /// deserialization APIs has been satisfied):
    ///
    /// * [`DFA::from_bytes`]
    /// * [`DFA::from_bytes_unchecked`]
    ///
    /// The padding returned is non-zero if the returned `Vec<u8>` starts at
    /// an address that does not have the same alignment as `u32`. The padding
    /// corresponds to the number of leading bytes written to the returned
    /// `Vec<u8>`.
    ///
    /// # Example
    ///
    /// This example shows how to serialize and deserialize a DFA:
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense::DFA}, HalfMatch, Input};
    ///
    /// // Compile our original DFA.
    /// let original_dfa = DFA::new("foo[0-9]+")?;
    ///
    /// // N.B. We use native endianness here to make the example work, but
    /// // using to_bytes_little_endian would work on a little endian target.
    /// let (buf, _) = original_dfa.to_bytes_native_endian();
    /// // Even if buf has initial padding, DFA::from_bytes will automatically
    /// // ignore it.
    /// let dfa: DFA<&[u32]> = DFA::from_bytes(&buf)?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "dfa-build")]
    pub fn to_bytes_little_endian(&self) -> (Vec<u8>, usize) {
        self.to_bytes::<wire::LE>()
    }

    /// Serialize this DFA as raw bytes to a `Vec<u8>` in big endian
    /// format. Upon success, the `Vec<u8>` and the initial padding length are
    /// returned.
    ///
    /// The written bytes are guaranteed to be deserialized correctly and
    /// without errors in a semver compatible release of this crate by a
    /// `DFA`'s deserialization APIs (assuming all other criteria for the
    /// deserialization APIs has been satisfied):
    ///
    /// * [`DFA::from_bytes`]
    /// * [`DFA::from_bytes_unchecked`]
    ///
    /// The padding returned is non-zero if the returned `Vec<u8>` starts at
    /// an address that does not have the same alignment as `u32`. The padding
    /// corresponds to the number of leading bytes written to the returned
    /// `Vec<u8>`.
    ///
    /// # Example
    ///
    /// This example shows how to serialize and deserialize a DFA:
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense::DFA}, HalfMatch, Input};
    ///
    /// // Compile our original DFA.
    /// let original_dfa = DFA::new("foo[0-9]+")?;
    ///
    /// // N.B. We use native endianness here to make the example work, but
    /// // using to_bytes_big_endian would work on a big endian target.
    /// let (buf, _) = original_dfa.to_bytes_native_endian();
    /// // Even if buf has initial padding, DFA::from_bytes will automatically
    /// // ignore it.
    /// let dfa: DFA<&[u32]> = DFA::from_bytes(&buf)?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "dfa-build")]
    pub fn to_bytes_big_endian(&self) -> (Vec<u8>, usize) {
        self.to_bytes::<wire::BE>()
    }

    /// Serialize this DFA as raw bytes to a `Vec<u8>` in native endian
    /// format. Upon success, the `Vec<u8>` and the initial padding length are
    /// returned.
    ///
    /// The written bytes are guaranteed to be deserialized correctly and
    /// without errors in a semver compatible release of this crate by a
    /// `DFA`'s deserialization APIs (assuming all other criteria for the
    /// deserialization APIs has been satisfied):
    ///
    /// * [`DFA::from_bytes`]
    /// * [`DFA::from_bytes_unchecked`]
    ///
    /// The padding returned is non-zero if the returned `Vec<u8>` starts at
    /// an address that does not have the same alignment as `u32`. The padding
    /// corresponds to the number of leading bytes written to the returned
    /// `Vec<u8>`.
    ///
    /// Generally speaking, native endian format should only be used when
    /// you know that the target you're compiling the DFA for matches the
    /// endianness of the target on which you're compiling DFA. For example,
    /// if serialization and deserialization happen in the same process or on
    /// the same machine. Otherwise, when serializing a DFA for use in a
    /// portable environment, you'll almost certainly want to serialize _both_
    /// a little endian and a big endian version and then load the correct one
    /// based on the target's configuration.
    ///
    /// # Example
    ///
    /// This example shows how to serialize and deserialize a DFA:
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense::DFA}, HalfMatch, Input};
    ///
    /// // Compile our original DFA.
    /// let original_dfa = DFA::new("foo[0-9]+")?;
    ///
    /// let (buf, _) = original_dfa.to_bytes_native_endian();
    /// // Even if buf has initial padding, DFA::from_bytes will automatically
    /// // ignore it.
    /// let dfa: DFA<&[u32]> = DFA::from_bytes(&buf)?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "dfa-build")]
    pub fn to_bytes_native_endian(&self) -> (Vec<u8>, usize) {
        self.to_bytes::<wire::NE>()
    }

    /// The implementation of the public `to_bytes` serialization methods,
    /// which is generic over endianness.
    #[cfg(feature = "dfa-build")]
    fn to_bytes<E: Endian>(&self) -> (Vec<u8>, usize) {
        let len = self.write_to_len();
        let (mut buf, padding) = wire::alloc_aligned_buffer::<u32>(len);
        // This should always succeed since the only possible serialization
        // error is providing a buffer that's too small, but we've ensured that
        // `buf` is big enough here.
        self.as_ref().write_to::<E>(&mut buf[padding..]).unwrap();
        (buf, padding)
    }

    /// Serialize this DFA as raw bytes to the given slice, in little endian
    /// format. Upon success, the total number of bytes written to `dst` is
    /// returned.
    ///
    /// The written bytes are guaranteed to be deserialized correctly and
    /// without errors in a semver compatible release of this crate by a
    /// `DFA`'s deserialization APIs (assuming all other criteria for the
    /// deserialization APIs has been satisfied):
    ///
    /// * [`DFA::from_bytes`]
    /// * [`DFA::from_bytes_unchecked`]
    ///
    /// Note that unlike the various `to_byte_*` routines, this does not write
    /// any padding. Callers are responsible for handling alignment correctly.
    ///
    /// # Errors
    ///
    /// This returns an error if the given destination slice is not big enough
    /// to contain the full serialized DFA. If an error occurs, then nothing
    /// is written to `dst`.
    ///
    /// # Example
    ///
    /// This example shows how to serialize and deserialize a DFA without
    /// dynamic memory allocation.
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense::DFA}, HalfMatch, Input};
    ///
    /// // Compile our original DFA.
    /// let original_dfa = DFA::new("foo[0-9]+")?;
    ///
    /// // Create a 4KB buffer on the stack to store our serialized DFA. We
    /// // need to use a special type to force the alignment of our [u8; N]
    /// // array to be aligned to a 4 byte boundary. Otherwise, deserializing
    /// // the DFA may fail because of an alignment mismatch.
    /// #[repr(C)]
    /// struct Aligned<B: ?Sized> {
    ///     _align: [u32; 0],
    ///     bytes: B,
    /// }
    /// let mut buf = Aligned { _align: [], bytes: [0u8; 4 * (1<<10)] };
    /// // N.B. We use native endianness here to make the example work, but
    /// // using write_to_little_endian would work on a little endian target.
    /// let written = original_dfa.write_to_native_endian(&mut buf.bytes)?;
    /// let dfa: DFA<&[u32]> = DFA::from_bytes(&buf.bytes[..written])?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn write_to_little_endian(
        &self,
        dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        self.as_ref().write_to::<wire::LE>(dst)
    }

    /// Serialize this DFA as raw bytes to the given slice, in big endian
    /// format. Upon success, the total number of bytes written to `dst` is
    /// returned.
    ///
    /// The written bytes are guaranteed to be deserialized correctly and
    /// without errors in a semver compatible release of this crate by a
    /// `DFA`'s deserialization APIs (assuming all other criteria for the
    /// deserialization APIs has been satisfied):
    ///
    /// * [`DFA::from_bytes`]
    /// * [`DFA::from_bytes_unchecked`]
    ///
    /// Note that unlike the various `to_byte_*` routines, this does not write
    /// any padding. Callers are responsible for handling alignment correctly.
    ///
    /// # Errors
    ///
    /// This returns an error if the given destination slice is not big enough
    /// to contain the full serialized DFA. If an error occurs, then nothing
    /// is written to `dst`.
    ///
    /// # Example
    ///
    /// This example shows how to serialize and deserialize a DFA without
    /// dynamic memory allocation.
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense::DFA}, HalfMatch, Input};
    ///
    /// // Compile our original DFA.
    /// let original_dfa = DFA::new("foo[0-9]+")?;
    ///
    /// // Create a 4KB buffer on the stack to store our serialized DFA. We
    /// // need to use a special type to force the alignment of our [u8; N]
    /// // array to be aligned to a 4 byte boundary. Otherwise, deserializing
    /// // the DFA may fail because of an alignment mismatch.
    /// #[repr(C)]
    /// struct Aligned<B: ?Sized> {
    ///     _align: [u32; 0],
    ///     bytes: B,
    /// }
    /// let mut buf = Aligned { _align: [], bytes: [0u8; 4 * (1<<10)] };
    /// // N.B. We use native endianness here to make the example work, but
    /// // using write_to_big_endian would work on a big endian target.
    /// let written = original_dfa.write_to_native_endian(&mut buf.bytes)?;
    /// let dfa: DFA<&[u32]> = DFA::from_bytes(&buf.bytes[..written])?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn write_to_big_endian(
        &self,
        dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        self.as_ref().write_to::<wire::BE>(dst)
    }

    /// Serialize this DFA as raw bytes to the given slice, in native endian
    /// format. Upon success, the total number of bytes written to `dst` is
    /// returned.
    ///
    /// The written bytes are guaranteed to be deserialized correctly and
    /// without errors in a semver compatible release of this crate by a
    /// `DFA`'s deserialization APIs (assuming all other criteria for the
    /// deserialization APIs has been satisfied):
    ///
    /// * [`DFA::from_bytes`]
    /// * [`DFA::from_bytes_unchecked`]
    ///
    /// Generally speaking, native endian format should only be used when
    /// you know that the target you're compiling the DFA for matches the
    /// endianness of the target on which you're compiling DFA. For example,
    /// if serialization and deserialization happen in the same process or on
    /// the same machine. Otherwise, when serializing a DFA for use in a
    /// portable environment, you'll almost certainly want to serialize _both_
    /// a little endian and a big endian version and then load the correct one
    /// based on the target's configuration.
    ///
    /// Note that unlike the various `to_byte_*` routines, this does not write
    /// any padding. Callers are responsible for handling alignment correctly.
    ///
    /// # Errors
    ///
    /// This returns an error if the given destination slice is not big enough
    /// to contain the full serialized DFA. If an error occurs, then nothing
    /// is written to `dst`.
    ///
    /// # Example
    ///
    /// This example shows how to serialize and deserialize a DFA without
    /// dynamic memory allocation.
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense::DFA}, HalfMatch, Input};
    ///
    /// // Compile our original DFA.
    /// let original_dfa = DFA::new("foo[0-9]+")?;
    ///
    /// // Create a 4KB buffer on the stack to store our serialized DFA. We
    /// // need to use a special type to force the alignment of our [u8; N]
    /// // array to be aligned to a 4 byte boundary. Otherwise, deserializing
    /// // the DFA may fail because of an alignment mismatch.
    /// #[repr(C)]
    /// struct Aligned<B: ?Sized> {
    ///     _align: [u32; 0],
    ///     bytes: B,
    /// }
    /// let mut buf = Aligned { _align: [], bytes: [0u8; 4 * (1<<10)] };
    /// let written = original_dfa.write_to_native_endian(&mut buf.bytes)?;
    /// let dfa: DFA<&[u32]> = DFA::from_bytes(&buf.bytes[..written])?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn write_to_native_endian(
        &self,
        dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        self.as_ref().write_to::<wire::NE>(dst)
    }

    /// Return the total number of bytes required to serialize this DFA.
    ///
    /// This is useful for determining the size of the buffer required to pass
    /// to one of the serialization routines:
    ///
    /// * [`DFA::write_to_little_endian`]
    /// * [`DFA::write_to_big_endian`]
    /// * [`DFA::write_to_native_endian`]
    ///
    /// Passing a buffer smaller than the size returned by this method will
    /// result in a serialization error. Serialization routines are guaranteed
    /// to succeed when the buffer is big enough.
    ///
    /// # Example
    ///
    /// This example shows how to dynamically allocate enough room to serialize
    /// a DFA.
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense::DFA}, HalfMatch, Input};
    ///
    /// let original_dfa = DFA::new("foo[0-9]+")?;
    ///
    /// let mut buf = vec![0; original_dfa.write_to_len()];
    /// // This is guaranteed to succeed, because the only serialization error
    /// // that can occur is when the provided buffer is too small. But
    /// // write_to_len guarantees a correct size.
    /// let written = original_dfa.write_to_native_endian(&mut buf).unwrap();
    /// // But this is not guaranteed to succeed! In particular,
    /// // deserialization requires proper alignment for &[u32], but our buffer
    /// // was allocated as a &[u8] whose required alignment is smaller than
    /// // &[u32]. However, it's likely to work in practice because of how most
    /// // allocators work. So if you write code like this, make sure to either
    /// // handle the error correctly and/or run it under Miri since Miri will
    /// // likely provoke the error by returning Vec<u8> buffers with alignment
    /// // less than &[u32].
    /// let dfa: DFA<&[u32]> = match DFA::from_bytes(&buf[..written]) {
    ///     // As mentioned above, it is legal for an error to be returned
    ///     // here. It is quite difficult to get a Vec<u8> with a guaranteed
    ///     // alignment equivalent to Vec<u32>.
    ///     Err(_) => return Ok(()),
    ///     Ok((dfa, _)) => dfa,
    /// };
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Note that this example isn't actually guaranteed to work! In
    /// particular, if `buf` is not aligned to a 4-byte boundary, then the
    /// `DFA::from_bytes` call will fail. If you need this to work, then you
    /// either need to deal with adding some initial padding yourself, or use
    /// one of the `to_bytes` methods, which will do it for you.
    pub fn write_to_len(&self) -> usize {
        wire::write_label_len(LABEL)
        + wire::write_endianness_check_len()
        + wire::write_version_len()
        + size_of::<u32>() // unused, intended for future flexibility
        + self.flags.write_to_len()
        + self.tt.write_to_len()
        + self.st.write_to_len()
        + self.ms.write_to_len()
        + self.special.write_to_len()
        + self.accels.write_to_len()
        + self.quitset.write_to_len()
    }
}

impl<'a> DFA<&'a [u32]> {
    /// Safely deserialize a DFA with a specific state identifier
    /// representation. Upon success, this returns both the deserialized DFA
    /// and the number of bytes read from the given slice. Namely, the contents
    /// of the slice beyond the DFA are not read.
    ///
    /// Deserializing a DFA using this routine will never allocate heap memory.
    /// For safety purposes, the DFA's transition table will be verified such
    /// that every transition points to a valid state. If this verification is
    /// too costly, then a [`DFA::from_bytes_unchecked`] API is provided, which
    /// will always execute in constant time.
    ///
    /// The bytes given must be generated by one of the serialization APIs
    /// of a `DFA` using a semver compatible release of this crate. Those
    /// include:
    ///
    /// * [`DFA::to_bytes_little_endian`]
    /// * [`DFA::to_bytes_big_endian`]
    /// * [`DFA::to_bytes_native_endian`]
    /// * [`DFA::write_to_little_endian`]
    /// * [`DFA::write_to_big_endian`]
    /// * [`DFA::write_to_native_endian`]
    ///
    /// The `to_bytes` methods allocate and return a `Vec<u8>` for you, along
    /// with handling alignment correctly. The `write_to` methods do not
    /// allocate and write to an existing slice (which may be on the stack).
    /// Since deserialization always uses the native endianness of the target
    /// platform, the serialization API you use should match the endianness of
    /// the target platform. (It's often a good idea to generate serialized
    /// DFAs for both forms of endianness and then load the correct one based
    /// on endianness.)
    ///
    /// # Errors
    ///
    /// Generally speaking, it's easier to state the conditions in which an
    /// error is _not_ returned. All of the following must be true:
    ///
    /// * The bytes given must be produced by one of the serialization APIs
    ///   on this DFA, as mentioned above.
    /// * The endianness of the target platform matches the endianness used to
    ///   serialized the provided DFA.
    /// * The slice given must have the same alignment as `u32`.
    ///
    /// If any of the above are not true, then an error will be returned.
    ///
    /// # Panics
    ///
    /// This routine will never panic for any input.
    ///
    /// # Example
    ///
    /// This example shows how to serialize a DFA to raw bytes, deserialize it
    /// and then use it for searching.
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense::DFA}, HalfMatch, Input};
    ///
    /// let initial = DFA::new("foo[0-9]+")?;
    /// let (bytes, _) = initial.to_bytes_native_endian();
    /// let dfa: DFA<&[u32]> = DFA::from_bytes(&bytes)?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: dealing with alignment and padding
    ///
    /// In the above example, we used the `to_bytes_native_endian` method to
    /// serialize a DFA, but we ignored part of its return value corresponding
    /// to padding added to the beginning of the serialized DFA. This is OK
    /// because deserialization will skip this initial padding. What matters
    /// is that the address immediately following the padding has an alignment
    /// that matches `u32`. That is, the following is an equivalent but
    /// alternative way to write the above example:
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense::DFA}, HalfMatch, Input};
    ///
    /// let initial = DFA::new("foo[0-9]+")?;
    /// // Serialization returns the number of leading padding bytes added to
    /// // the returned Vec<u8>.
    /// let (bytes, pad) = initial.to_bytes_native_endian();
    /// let dfa: DFA<&[u32]> = DFA::from_bytes(&bytes[pad..])?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This padding is necessary because Rust's standard library does
    /// not expose any safe and robust way of creating a `Vec<u8>` with a
    /// guaranteed alignment other than 1. Now, in practice, the underlying
    /// allocator is likely to provide a `Vec<u8>` that meets our alignment
    /// requirements, which means `pad` is zero in practice most of the time.
    ///
    /// The purpose of exposing the padding like this is flexibility for the
    /// caller. For example, if one wants to embed a serialized DFA into a
    /// compiled program, then it's important to guarantee that it starts at a
    /// `u32`-aligned address. The simplest way to do this is to discard the
    /// padding bytes and set it up so that the serialized DFA itself begins at
    /// a properly aligned address. We can show this in two parts. The first
    /// part is serializing the DFA to a file:
    ///
    /// ```no_run
    /// use regex_automata::dfa::dense::DFA;
    ///
    /// let dfa = DFA::new("foo[0-9]+")?;
    ///
    /// let (bytes, pad) = dfa.to_bytes_big_endian();
    /// // Write the contents of the DFA *without* the initial padding.
    /// std::fs::write("foo.bigendian.dfa", &bytes[pad..])?;
    ///
    /// // Do it again, but this time for little endian.
    /// let (bytes, pad) = dfa.to_bytes_little_endian();
    /// std::fs::write("foo.littleendian.dfa", &bytes[pad..])?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// And now the second part is embedding the DFA into the compiled program
    /// and deserializing it at runtime on first use. We use conditional
    /// compilation to choose the correct endianness.
    ///
    /// ```no_run
    /// use regex_automata::{
    ///     dfa::{Automaton, dense::DFA},
    ///     util::{lazy::Lazy, wire::AlignAs},
    ///     HalfMatch, Input,
    /// };
    ///
    /// // This crate provides its own "lazy" type, kind of like
    /// // lazy_static! or once_cell::sync::Lazy. But it works in no-alloc
    /// // no-std environments and let's us write this using completely
    /// // safe code.
    /// static RE: Lazy<DFA<&'static [u32]>> = Lazy::new(|| {
    ///     # const _: &str = stringify! {
    ///     // This assignment is made possible (implicitly) via the
    ///     // CoerceUnsized trait. This is what guarantees that our
    ///     // bytes are stored in memory on a 4 byte boundary. You
    ///     // *must* do this or something equivalent for correct
    ///     // deserialization.
    ///     static ALIGNED: &AlignAs<[u8], u32> = &AlignAs {
    ///         _align: [],
    ///         #[cfg(target_endian = "big")]
    ///         bytes: *include_bytes!("foo.bigendian.dfa"),
    ///         #[cfg(target_endian = "little")]
    ///         bytes: *include_bytes!("foo.littleendian.dfa"),
    ///     };
    ///     # };
    ///     # static ALIGNED: &AlignAs<[u8], u32> = &AlignAs {
    ///     #     _align: [],
    ///     #     bytes: [],
    ///     # };
    ///
    ///     let (dfa, _) = DFA::from_bytes(&ALIGNED.bytes)
    ///         .expect("serialized DFA should be valid");
    ///     dfa
    /// });
    ///
    /// let expected = Ok(Some(HalfMatch::must(0, 8)));
    /// assert_eq!(expected, RE.try_search_fwd(&Input::new("foo12345")));
    /// ```
    ///
    /// An alternative to [`util::lazy::Lazy`](crate::util::lazy::Lazy)
    /// is [`lazy_static`](https://crates.io/crates/lazy_static) or
    /// [`once_cell`](https://crates.io/crates/once_cell), which provide
    /// stronger guarantees (like the initialization function only being
    /// executed once). And `once_cell` in particular provides a more
    /// expressive API. But a `Lazy` value from this crate is likely just fine
    /// in most circumstances.
    ///
    /// Note that regardless of which initialization method you use, you
    /// will still need to use the [`AlignAs`](crate::util::wire::AlignAs)
    /// trick above to force correct alignment, but this is safe to do and
    /// `from_bytes` will return an error if you get it wrong.
    pub fn from_bytes(
        slice: &'a [u8],
    ) -> Result<(DFA<&'a [u32]>, usize), DeserializeError> {
        // SAFETY: This is safe because we validate the transition table, start
        // table, match states and accelerators below. If any validation fails,
        // then we return an error.
        let (dfa, nread) = unsafe { DFA::from_bytes_unchecked(slice)? };
        dfa.tt.validate(&dfa)?;
        dfa.st.validate(&dfa)?;
        dfa.ms.validate(&dfa)?;
        dfa.accels.validate()?;
        // N.B. dfa.special doesn't have a way to do unchecked deserialization,
        // so it has already been validated.
        for state in dfa.states() {
            // If the state is an accel state, then it must have a non-empty
            // accelerator.
            if dfa.is_accel_state(state.id()) {
                let index = dfa.accelerator_index(state.id());
                if index >= dfa.accels.len() {
                    return Err(DeserializeError::generic(
                        "found DFA state with invalid accelerator index",
                    ));
                }
                let needles = dfa.accels.needles(index);
                if !(1 <= needles.len() && needles.len() <= 3) {
                    return Err(DeserializeError::generic(
                        "accelerator needles has invalid length",
                    ));
                }
            }
        }
        Ok((dfa, nread))
    }

    /// Deserialize a DFA with a specific state identifier representation in
    /// constant time by omitting the verification of the validity of the
    /// transition table and other data inside the DFA.
    ///
    /// This is just like [`DFA::from_bytes`], except it can potentially return
    /// a DFA that exhibits undefined behavior if its transition table contains
    /// invalid state identifiers.
    ///
    /// This routine is useful if you need to deserialize a DFA cheaply
    /// and cannot afford the transition table validation performed by
    /// `from_bytes`.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense::DFA}, HalfMatch, Input};
    ///
    /// let initial = DFA::new("foo[0-9]+")?;
    /// let (bytes, _) = initial.to_bytes_native_endian();
    /// // SAFETY: This is guaranteed to be safe since the bytes given come
    /// // directly from a compatible serialization routine.
    /// let dfa: DFA<&[u32]> = unsafe { DFA::from_bytes_unchecked(&bytes)?.0 };
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub unsafe fn from_bytes_unchecked(
        slice: &'a [u8],
    ) -> Result<(DFA<&'a [u32]>, usize), DeserializeError> {
        let mut nr = 0;

        nr += wire::skip_initial_padding(slice);
        wire::check_alignment::<StateID>(&slice[nr..])?;
        nr += wire::read_label(&slice[nr..], LABEL)?;
        nr += wire::read_endianness_check(&slice[nr..])?;
        nr += wire::read_version(&slice[nr..], VERSION)?;

        let _unused = wire::try_read_u32(&slice[nr..], "unused space")?;
        nr += size_of::<u32>();

        let (flags, nread) = Flags::from_bytes(&slice[nr..])?;
        nr += nread;

        let (tt, nread) = TransitionTable::from_bytes_unchecked(&slice[nr..])?;
        nr += nread;

        let (st, nread) = StartTable::from_bytes_unchecked(&slice[nr..])?;
        nr += nread;

        let (ms, nread) = MatchStates::from_bytes_unchecked(&slice[nr..])?;
        nr += nread;

        let (special, nread) = Special::from_bytes(&slice[nr..])?;
        nr += nread;
        special.validate_state_len(tt.len(), tt.stride2)?;

        let (accels, nread) = Accels::from_bytes_unchecked(&slice[nr..])?;
        nr += nread;

        let (quitset, nread) = ByteSet::from_bytes(&slice[nr..])?;
        nr += nread;

        // Prefilters don't support serialization, so they're always absent.
        let pre = None;
        Ok((DFA { tt, st, ms, special, accels, pre, quitset, flags }, nr))
    }

    /// The implementation of the public `write_to` serialization methods,
    /// which is generic over endianness.
    ///
    /// This is defined only for &[u32] to reduce binary size/compilation time.
    fn write_to<E: Endian>(
        &self,
        mut dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("dense DFA"));
        }
        dst = &mut dst[..nwrite];

        let mut nw = 0;
        nw += wire::write_label(LABEL, &mut dst[nw..])?;
        nw += wire::write_endianness_check::<E>(&mut dst[nw..])?;
        nw += wire::write_version::<E>(VERSION, &mut dst[nw..])?;
        nw += {
            // Currently unused, intended for future flexibility
            E::write_u32(0, &mut dst[nw..]);
            size_of::<u32>()
        };
        nw += self.flags.write_to::<E>(&mut dst[nw..])?;
        nw += self.tt.write_to::<E>(&mut dst[nw..])?;
        nw += self.st.write_to::<E>(&mut dst[nw..])?;
        nw += self.ms.write_to::<E>(&mut dst[nw..])?;
        nw += self.special.write_to::<E>(&mut dst[nw..])?;
        nw += self.accels.write_to::<E>(&mut dst[nw..])?;
        nw += self.quitset.write_to::<E>(&mut dst[nw..])?;
        Ok(nw)
    }
}

// The following methods implement mutable routines on the internal
// representation of a DFA. As such, we must fix the first type parameter to a
// `Vec<u32>` since a generic `T: AsRef<[u32]>` does not permit mutation. We
// can get away with this because these methods are internal to the crate and
// are exclusively used during construction of the DFA.
#[cfg(feature = "dfa-build")]
impl OwnedDFA {
    /// Add a start state of this DFA.
    pub(crate) fn set_start_state(
        &mut self,
        anchored: Anchored,
        start: Start,
        id: StateID,
    ) {
        assert!(self.tt.is_valid(id), "invalid start state");
        self.st.set_start(anchored, start, id);
    }

    /// Set the given transition to this DFA. Both the `from` and `to` states
    /// must already exist.
    pub(crate) fn set_transition(
        &mut self,
        from: StateID,
        byte: alphabet::Unit,
        to: StateID,
    ) {
        self.tt.set(from, byte, to);
    }

    /// An empty state (a state where all transitions lead to a dead state)
    /// and return its identifier. The identifier returned is guaranteed to
    /// not point to any other existing state.
    ///
    /// If adding a state would exceed `StateID::LIMIT`, then this returns an
    /// error.
    pub(crate) fn add_empty_state(&mut self) -> Result<StateID, BuildError> {
        self.tt.add_empty_state()
    }

    /// Swap the two states given in the transition table.
    ///
    /// This routine does not do anything to check the correctness of this
    /// swap. Callers must ensure that other states pointing to id1 and id2 are
    /// updated appropriately.
    pub(crate) fn swap_states(&mut self, id1: StateID, id2: StateID) {
        self.tt.swap(id1, id2);
    }

    /// Remap all of the state identifiers in this DFA according to the map
    /// function given. This includes all transitions and all starting state
    /// identifiers.
    pub(crate) fn remap(&mut self, map: impl Fn(StateID) -> StateID) {
        // We could loop over each state ID and call 'remap_state' here, but
        // this is more direct: just map every transition directly. This
        // technically might do a little extra work since the alphabet length
        // is likely less than the stride, but if that is indeed an issue we
        // should benchmark it and fix it.
        for sid in self.tt.table_mut().iter_mut() {
            *sid = map(*sid);
        }
        for sid in self.st.table_mut().iter_mut() {
            *sid = map(*sid);
        }
    }

    /// Remap the transitions for the state given according to the function
    /// given. This applies the given map function to every transition in the
    /// given state and changes the transition in place to the result of the
    /// map function for that transition.
    pub(crate) fn remap_state(
        &mut self,
        id: StateID,
        map: impl Fn(StateID) -> StateID,
    ) {
        self.tt.remap(id, map);
    }

    /// Truncate the states in this DFA to the given length.
    ///
    /// This routine does not do anything to check the correctness of this
    /// truncation. Callers must ensure that other states pointing to truncated
    /// states are updated appropriately.
    pub(crate) fn truncate_states(&mut self, len: usize) {
        self.tt.truncate(len);
    }

    /// Minimize this DFA in place using Hopcroft's algorithm.
    pub(crate) fn minimize(&mut self) {
        Minimizer::new(self).run();
    }

    /// Updates the match state pattern ID map to use the one provided.
    ///
    /// This is useful when it's convenient to manipulate matching states
    /// (and their corresponding pattern IDs) as a map. In particular, the
    /// representation used by a DFA for this map is not amenable to mutation,
    /// so if things need to be changed (like when shuffling states), it's
    /// often easier to work with the map form.
    pub(crate) fn set_pattern_map(
        &mut self,
        map: &BTreeMap<StateID, Vec<PatternID>>,
    ) -> Result<(), BuildError> {
        self.ms = self.ms.new_with_map(map)?;
        Ok(())
    }

    /// Find states that have a small number of non-loop transitions and mark
    /// them as candidates for acceleration during search.
    pub(crate) fn accelerate(&mut self) {
        // dead and quit states can never be accelerated.
        if self.state_len() <= 2 {
            return;
        }

        // Go through every state and record their accelerator, if possible.
        let mut accels = BTreeMap::new();
        // Count the number of accelerated match, start and non-match/start
        // states.
        let (mut cmatch, mut cstart, mut cnormal) = (0, 0, 0);
        for state in self.states() {
            if let Some(accel) = state.accelerate(self.byte_classes()) {
                debug!(
                    "accelerating full DFA state {}: {:?}",
                    state.id().as_usize(),
                    accel,
                );
                accels.insert(state.id(), accel);
                if self.is_match_state(state.id()) {
                    cmatch += 1;
                } else if self.is_start_state(state.id()) {
                    cstart += 1;
                } else {
                    assert!(!self.is_dead_state(state.id()));
                    assert!(!self.is_quit_state(state.id()));
                    cnormal += 1;
                }
            }
        }
        // If no states were able to be accelerated, then we're done.
        if accels.is_empty() {
            return;
        }
        let original_accels_len = accels.len();

        // A remapper keeps track of state ID changes. Once we're done
        // shuffling, the remapper is used to rewrite all transitions in the
        // DFA based on the new positions of states.
        let mut remapper = Remapper::new(self);

        // As we swap states, if they are match states, we need to swap their
        // pattern ID lists too (for multi-regexes). We do this by converting
        // the lists to an easily swappable map, and then convert back to
        // MatchStates once we're done.
        let mut new_matches = self.ms.to_map(self);

        // There is at least one state that gets accelerated, so these are
        // guaranteed to get set to sensible values below.
        self.special.min_accel = StateID::MAX;
        self.special.max_accel = StateID::ZERO;
        let update_special_accel =
            |special: &mut Special, accel_id: StateID| {
                special.min_accel = cmp::min(special.min_accel, accel_id);
                special.max_accel = cmp::max(special.max_accel, accel_id);
            };

        // Start by shuffling match states. Any match states that are
        // accelerated get moved to the end of the match state range.
        if cmatch > 0 && self.special.matches() {
            // N.B. special.{min,max}_match do not need updating, since the
            // range/number of match states does not change. Only the ordering
            // of match states may change.
            let mut next_id = self.special.max_match;
            let mut cur_id = next_id;
            while cur_id >= self.special.min_match {
                if let Some(accel) = accels.remove(&cur_id) {
                    accels.insert(next_id, accel);
                    update_special_accel(&mut self.special, next_id);

                    // No need to do any actual swapping for equivalent IDs.
                    if cur_id != next_id {
                        remapper.swap(self, cur_id, next_id);

                        // Swap pattern IDs for match states.
                        let cur_pids = new_matches.remove(&cur_id).unwrap();
                        let next_pids = new_matches.remove(&next_id).unwrap();
                        new_matches.insert(cur_id, next_pids);
                        new_matches.insert(next_id, cur_pids);
                    }
                    next_id = self.tt.prev_state_id(next_id);
                }
                cur_id = self.tt.prev_state_id(cur_id);
            }
        }

        // This is where it gets tricky. Without acceleration, start states
        // normally come right after match states. But we want accelerated
        // states to be a single contiguous range (to make it very fast
        // to determine whether a state *is* accelerated), while also keeping
        // match and starting states as contiguous ranges for the same reason.
        // So what we do here is shuffle states such that it looks like this:
        //
        //     DQMMMMAAAAASSSSSSNNNNNNN
        //         |         |
        //         |---------|
        //      accelerated states
        //
        // Where:
        //   D - dead state
        //   Q - quit state
        //   M - match state (may be accelerated)
        //   A - normal state that is accelerated
        //   S - start state (may be accelerated)
        //   N - normal state that is NOT accelerated
        //
        // We implement this by shuffling states, which is done by a sequence
        // of pairwise swaps. We start by looking at all normal states to be
        // accelerated. When we find one, we swap it with the earliest starting
        // state, and then swap that with the earliest normal state. This
        // preserves the contiguous property.
        //
        // Once we're done looking for accelerated normal states, now we look
        // for accelerated starting states by moving them to the beginning
        // of the starting state range (just like we moved accelerated match
        // states to the end of the matching state range).
        //
        // For a more detailed/different perspective on this, see the docs
        // in dfa/special.rs.
        if cnormal > 0 {
            // our next available starting and normal states for swapping.
            let mut next_start_id = self.special.min_start;
            let mut cur_id = self.to_state_id(self.state_len() - 1);
            // This is guaranteed to exist since cnormal > 0.
            let mut next_norm_id =
                self.tt.next_state_id(self.special.max_start);
            while cur_id >= next_norm_id {
                if let Some(accel) = accels.remove(&cur_id) {
                    remapper.swap(self, next_start_id, cur_id);
                    remapper.swap(self, next_norm_id, cur_id);
                    // Keep our accelerator map updated with new IDs if the
                    // states we swapped were also accelerated.
                    if let Some(accel2) = accels.remove(&next_norm_id) {
                        accels.insert(cur_id, accel2);
                    }
                    if let Some(accel2) = accels.remove(&next_start_id) {
                        accels.insert(next_norm_id, accel2);
                    }
                    accels.insert(next_start_id, accel);
                    update_special_accel(&mut self.special, next_start_id);
                    // Our start range shifts one to the right now.
                    self.special.min_start =
                        self.tt.next_state_id(self.special.min_start);
                    self.special.max_start =
                        self.tt.next_state_id(self.special.max_start);
                    next_start_id = self.tt.next_state_id(next_start_id);
                    next_norm_id = self.tt.next_state_id(next_norm_id);
                }
                // This is pretty tricky, but if our 'next_norm_id' state also
                // happened to be accelerated, then the result is that it is
                // now in the position of cur_id, so we need to consider it
                // again. This loop is still guaranteed to terminate though,
                // because when accels contains cur_id, we're guaranteed to
                // increment next_norm_id even if cur_id remains unchanged.
                if !accels.contains_key(&cur_id) {
                    cur_id = self.tt.prev_state_id(cur_id);
                }
            }
        }
        // Just like we did for match states, but we want to move accelerated
        // start states to the beginning of the range instead of the end.
        if cstart > 0 {
            // N.B. special.{min,max}_start do not need updating, since the
            // range/number of start states does not change at this point. Only
            // the ordering of start states may change.
            let mut next_id = self.special.min_start;
            let mut cur_id = next_id;
            while cur_id <= self.special.max_start {
                if let Some(accel) = accels.remove(&cur_id) {
                    remapper.swap(self, cur_id, next_id);
                    accels.insert(next_id, accel);
                    update_special_accel(&mut self.special, next_id);
                    next_id = self.tt.next_state_id(next_id);
                }
                cur_id = self.tt.next_state_id(cur_id);
            }
        }

        // Remap all transitions in our DFA and assert some things.
        remapper.remap(self);
        // This unwrap is OK because acceleration never changes the number of
        // match states or patterns in those match states. Since acceleration
        // runs after the pattern map has been set at least once, we know that
        // our match states cannot error.
        self.set_pattern_map(&new_matches).unwrap();
        self.special.set_max();
        self.special.validate().expect("special state ranges should validate");
        self.special
            .validate_state_len(self.state_len(), self.stride2())
            .expect(
                "special state ranges should be consistent with state length",
            );
        assert_eq!(
            self.special.accel_len(self.stride()),
            // We record the number of accelerated states initially detected
            // since the accels map is itself mutated in the process above.
            // If mutated incorrectly, its size may change, and thus can't be
            // trusted as a source of truth of how many accelerated states we
            // expected there to be.
            original_accels_len,
            "mismatch with expected number of accelerated states",
        );

        // And finally record our accelerators. We kept our accels map updated
        // as we shuffled states above, so the accelerators should now
        // correspond to a contiguous range in the state ID space. (Which we
        // assert.)
        let mut prev: Option<StateID> = None;
        for (id, accel) in accels {
            assert!(prev.map_or(true, |p| self.tt.next_state_id(p) == id));
            prev = Some(id);
            self.accels.add(accel);
        }
    }

    /// Shuffle the states in this DFA so that starting states, match
    /// states and accelerated states are all contiguous.
    ///
    /// See dfa/special.rs for more details.
    pub(crate) fn shuffle(
        &mut self,
        mut matches: BTreeMap<StateID, Vec<PatternID>>,
    ) -> Result<(), BuildError> {
        // The determinizer always adds a quit state and it is always second.
        self.special.quit_id = self.to_state_id(1);
        // If all we have are the dead and quit states, then we're done and
        // the DFA will never produce a match.
        if self.state_len() <= 2 {
            self.special.set_max();
            return Ok(());
        }

        // Collect all our non-DEAD start states into a convenient set and
        // confirm there is no overlap with match states. In the classicl DFA
        // construction, start states can be match states. But because of
        // look-around, we delay all matches by a byte, which prevents start
        // states from being match states.
        let mut is_start: BTreeSet<StateID> = BTreeSet::new();
        for (start_id, _, _) in self.starts() {
            // If a starting configuration points to a DEAD state, then we
            // don't want to shuffle it. The DEAD state is always the first
            // state with ID=0. So we can just leave it be.
            if start_id == DEAD {
                continue;
            }
            assert!(
                !matches.contains_key(&start_id),
                "{:?} is both a start and a match state, which is not allowed",
                start_id,
            );
            is_start.insert(start_id);
        }

        // We implement shuffling by a sequence of pairwise swaps of states.
        // Since we have a number of things referencing states via their
        // IDs and swapping them changes their IDs, we need to record every
        // swap we make so that we can remap IDs. The remapper handles this
        // book-keeping for us.
        let mut remapper = Remapper::new(self);

        // Shuffle matching states.
        if matches.is_empty() {
            self.special.min_match = DEAD;
            self.special.max_match = DEAD;
        } else {
            // The determinizer guarantees that the first two states are the
            // dead and quit states, respectively. We want our match states to
            // come right after quit.
            let mut next_id = self.to_state_id(2);
            let mut new_matches = BTreeMap::new();
            self.special.min_match = next_id;
            for (id, pids) in matches {
                remapper.swap(self, next_id, id);
                new_matches.insert(next_id, pids);
                // If we swapped a start state, then update our set.
                if is_start.contains(&next_id) {
                    is_start.remove(&next_id);
                    is_start.insert(id);
                }
                next_id = self.tt.next_state_id(next_id);
            }
            matches = new_matches;
            self.special.max_match = cmp::max(
                self.special.min_match,
                self.tt.prev_state_id(next_id),
            );
        }

        // Shuffle starting states.
        {
            let mut next_id = self.to_state_id(2);
            if self.special.matches() {
                next_id = self.tt.next_state_id(self.special.max_match);
            }
            self.special.min_start = next_id;
            for id in is_start {
                remapper.swap(self, next_id, id);
                next_id = self.tt.next_state_id(next_id);
            }
            self.special.max_start = cmp::max(
                self.special.min_start,
                self.tt.prev_state_id(next_id),
            );
        }

        // Finally remap all transitions in our DFA.
        remapper.remap(self);
        self.set_pattern_map(&matches)?;
        self.special.set_max();
        self.special.validate().expect("special state ranges should validate");
        self.special
            .validate_state_len(self.state_len(), self.stride2())
            .expect(
                "special state ranges should be consistent with state length",
            );
        Ok(())
    }

    /// Checks whether there are universal start states (both anchored and
    /// unanchored), and if so, sets the relevant fields to the start state
    /// IDs.
    ///
    /// Universal start states occur precisely when the all patterns in the
    /// DFA have no look-around assertions in their prefix.
    fn set_universal_starts(&mut self) {
        assert_eq!(6, Start::len(), "expected 6 start configurations");

        let start_id = |dfa: &mut OwnedDFA,
                        anchored: Anchored,
                        start: Start| {
            // This OK because we only call 'start' under conditions
            // in which we know it will succeed.
            dfa.st.start(anchored, start).expect("valid Input configuration")
        };
        if self.start_kind().has_unanchored() {
            let anchor = Anchored::No;
            let sid = start_id(self, anchor, Start::NonWordByte);
            if sid == start_id(self, anchor, Start::WordByte)
                && sid == start_id(self, anchor, Start::Text)
                && sid == start_id(self, anchor, Start::LineLF)
                && sid == start_id(self, anchor, Start::LineCR)
                && sid == start_id(self, anchor, Start::CustomLineTerminator)
            {
                self.st.universal_start_unanchored = Some(sid);
            }
        }
        if self.start_kind().has_anchored() {
            let anchor = Anchored::Yes;
            let sid = start_id(self, anchor, Start::NonWordByte);
            if sid == start_id(self, anchor, Start::WordByte)
                && sid == start_id(self, anchor, Start::Text)
                && sid == start_id(self, anchor, Start::LineLF)
                && sid == start_id(self, anchor, Start::LineCR)
                && sid == start_id(self, anchor, Start::CustomLineTerminator)
            {
                self.st.universal_start_anchored = Some(sid);
            }
        }
    }
}

// A variety of generic internal methods for accessing DFA internals.
impl<T: AsRef<[u32]>> DFA<T> {
    /// Return the info about special states.
    pub(crate) fn special(&self) -> &Special {
        &self.special
    }

    /// Return the info about special states as a mutable borrow.
    #[cfg(feature = "dfa-build")]
    pub(crate) fn special_mut(&mut self) -> &mut Special {
        &mut self.special
    }

    /// Returns the quit set (may be empty) used by this DFA.
    pub(crate) fn quitset(&self) -> &ByteSet {
        &self.quitset
    }

    /// Returns the flags for this DFA.
    pub(crate) fn flags(&self) -> &Flags {
        &self.flags
    }

    /// Returns an iterator over all states in this DFA.
    ///
    /// This iterator yields a tuple for each state. The first element of the
    /// tuple corresponds to a state's identifier, and the second element
    /// corresponds to the state itself (comprised of its transitions).
    pub(crate) fn states(&self) -> StateIter<'_, T> {
        self.tt.states()
    }

    /// Return the total number of states in this DFA. Every DFA has at least
    /// 1 state, even the empty DFA.
    pub(crate) fn state_len(&self) -> usize {
        self.tt.len()
    }

    /// Return an iterator over all pattern IDs for the given match state.
    ///
    /// If the given state is not a match state, then this panics.
    #[cfg(feature = "dfa-build")]
    pub(crate) fn pattern_id_slice(&self, id: StateID) -> &[PatternID] {
        assert!(self.is_match_state(id));
        self.ms.pattern_id_slice(self.match_state_index(id))
    }

    /// Return the total number of pattern IDs for the given match state.
    ///
    /// If the given state is not a match state, then this panics.
    pub(crate) fn match_pattern_len(&self, id: StateID) -> usize {
        assert!(self.is_match_state(id));
        self.ms.pattern_len(self.match_state_index(id))
    }

    /// Returns the total number of patterns matched by this DFA.
    pub(crate) fn pattern_len(&self) -> usize {
        self.ms.pattern_len
    }

    /// Returns a map from match state ID to a list of pattern IDs that match
    /// in that state.
    #[cfg(feature = "dfa-build")]
    pub(crate) fn pattern_map(&self) -> BTreeMap<StateID, Vec<PatternID>> {
        self.ms.to_map(self)
    }

    /// Returns the ID of the quit state for this DFA.
    #[cfg(feature = "dfa-build")]
    pub(crate) fn quit_id(&self) -> StateID {
        self.to_state_id(1)
    }

    /// Convert the given state identifier to the state's index. The state's
    /// index corresponds to the position in which it appears in the transition
    /// table. When a DFA is NOT premultiplied, then a state's identifier is
    /// also its index. When a DFA is premultiplied, then a state's identifier
    /// is equal to `index * alphabet_len`. This routine reverses that.
    pub(crate) fn to_index(&self, id: StateID) -> usize {
        self.tt.to_index(id)
    }

    /// Convert an index to a state (in the range 0..self.state_len()) to an
    /// actual state identifier.
    ///
    /// This is useful when using a `Vec<T>` as an efficient map keyed by state
    /// to some other information (such as a remapped state ID).
    #[cfg(feature = "dfa-build")]
    pub(crate) fn to_state_id(&self, index: usize) -> StateID {
        self.tt.to_state_id(index)
    }

    /// Return the table of state IDs for this DFA's start states.
    pub(crate) fn starts(&self) -> StartStateIter<'_> {
        self.st.iter()
    }

    /// Returns the index of the match state for the given ID. If the
    /// given ID does not correspond to a match state, then this may
    /// panic or produce an incorrect result.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn match_state_index(&self, id: StateID) -> usize {
        debug_assert!(self.is_match_state(id));
        // This is one of the places where we rely on the fact that match
        // states are contiguous in the transition table. Namely, that the
        // first match state ID always corresponds to dfa.special.min_match.
        // From there, since we know the stride, we can compute the overall
        // index of any match state given the match state's ID.
        let min = self.special().min_match.as_usize();
        // CORRECTNESS: We're allowed to produce an incorrect result or panic,
        // so both the subtraction and the unchecked StateID construction is
        // OK.
        self.to_index(StateID::new_unchecked(id.as_usize() - min))
    }

    /// Returns the index of the accelerator state for the given ID. If the
    /// given ID does not correspond to an accelerator state, then this may
    /// panic or produce an incorrect result.
    fn accelerator_index(&self, id: StateID) -> usize {
        let min = self.special().min_accel.as_usize();
        // CORRECTNESS: We're allowed to produce an incorrect result or panic,
        // so both the subtraction and the unchecked StateID construction is
        // OK.
        self.to_index(StateID::new_unchecked(id.as_usize() - min))
    }

    /// Return the accelerators for this DFA.
    fn accels(&self) -> Accels<&[u32]> {
        self.accels.as_ref()
    }

    /// Return this DFA's transition table as a slice.
    fn trans(&self) -> &[StateID] {
        self.tt.table()
    }
}

impl<T: AsRef<[u32]>> fmt::Debug for DFA<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "dense::DFA(")?;
        for state in self.states() {
            fmt_state_indicator(f, self, state.id())?;
            let id = if f.alternate() {
                state.id().as_usize()
            } else {
                self.to_index(state.id())
            };
            write!(f, "{:06?}: ", id)?;
            state.fmt(f)?;
            write!(f, "\n")?;
        }
        writeln!(f, "")?;
        for (i, (start_id, anchored, sty)) in self.starts().enumerate() {
            let id = if f.alternate() {
                start_id.as_usize()
            } else {
                self.to_index(start_id)
            };
            if i % self.st.stride == 0 {
                match anchored {
                    Anchored::No => writeln!(f, "START-GROUP(unanchored)")?,
                    Anchored::Yes => writeln!(f, "START-GROUP(anchored)")?,
                    Anchored::Pattern(pid) => {
                        writeln!(f, "START_GROUP(pattern: {:?})", pid)?
                    }
                }
            }
            writeln!(f, "  {:?} => {:06?}", sty, id)?;
        }
        if self.pattern_len() > 1 {
            writeln!(f, "")?;
            for i in 0..self.ms.len() {
                let id = self.ms.match_state_id(self, i);
                let id = if f.alternate() {
                    id.as_usize()
                } else {
                    self.to_index(id)
                };
                write!(f, "MATCH({:06?}): ", id)?;
                for (i, &pid) in self.ms.pattern_id_slice(i).iter().enumerate()
                {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:?}", pid)?;
                }
                writeln!(f, "")?;
            }
        }
        writeln!(f, "state length: {:?}", self.state_len())?;
        writeln!(f, "pattern length: {:?}", self.pattern_len())?;
        writeln!(f, "flags: {:?}", self.flags)?;
        writeln!(f, ")")?;
        Ok(())
    }
}

// SAFETY: We assert that our implementation of each method is correct.
unsafe impl<T: AsRef<[u32]>> Automaton for DFA<T> {
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn is_special_state(&self, id: StateID) -> bool {
        self.special.is_special_state(id)
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn is_dead_state(&self, id: StateID) -> bool {
        self.special.is_dead_state(id)
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn is_quit_state(&self, id: StateID) -> bool {
        self.special.is_quit_state(id)
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn is_match_state(&self, id: StateID) -> bool {
        self.special.is_match_state(id)
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn is_start_state(&self, id: StateID) -> bool {
        self.special.is_start_state(id)
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn is_accel_state(&self, id: StateID) -> bool {
        self.special.is_accel_state(id)
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn next_state(&self, current: StateID, input: u8) -> StateID {
        let input = self.byte_classes().get(input);
        let o = current.as_usize() + usize::from(input);
        self.trans()[o]
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    unsafe fn next_state_unchecked(
        &self,
        current: StateID,
        byte: u8,
    ) -> StateID {
        // We don't (or shouldn't) need an unchecked variant for the byte
        // class mapping, since bound checks should be omitted automatically
        // by virtue of its representation. If this ends up not being true as
        // confirmed by codegen, please file an issue. ---AG
        let class = self.byte_classes().get(byte);
        let o = current.as_usize() + usize::from(class);
        let next = *self.trans().get_unchecked(o);
        next
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn next_eoi_state(&self, current: StateID) -> StateID {
        let eoi = self.byte_classes().eoi().as_usize();
        let o = current.as_usize() + eoi;
        self.trans()[o]
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn pattern_len(&self) -> usize {
        self.ms.pattern_len
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn match_len(&self, id: StateID) -> usize {
        self.match_pattern_len(id)
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn match_pattern(&self, id: StateID, match_index: usize) -> PatternID {
        // This is an optimization for the very common case of a DFA with a
        // single pattern. This conditional avoids a somewhat more costly path
        // that finds the pattern ID from the state machine, which requires
        // a bit of slicing/pointer-chasing. This optimization tends to only
        // matter when matches are frequent.
        if self.ms.pattern_len == 1 {
            return PatternID::ZERO;
        }
        let state_index = self.match_state_index(id);
        self.ms.pattern_id(state_index, match_index)
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn has_empty(&self) -> bool {
        self.flags.has_empty
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn is_utf8(&self) -> bool {
        self.flags.is_utf8
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn is_always_start_anchored(&self) -> bool {
        self.flags.is_always_start_anchored
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn start_state(
        &self,
        config: &start::Config,
    ) -> Result<StateID, StartError> {
        let anchored = config.get_anchored();
        let start = match config.get_look_behind() {
            None => Start::Text,
            Some(byte) => {
                if !self.quitset.is_empty() && self.quitset.contains(byte) {
                    return Err(StartError::quit(byte));
                }
                self.st.start_map.get(byte)
            }
        };
        self.st.start(anchored, start)
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn universal_start_state(&self, mode: Anchored) -> Option<StateID> {
        match mode {
            Anchored::No => self.st.universal_start_unanchored,
            Anchored::Yes => self.st.universal_start_anchored,
            Anchored::Pattern(_) => None,
        }
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn accelerator(&self, id: StateID) -> &[u8] {
        if !self.is_accel_state(id) {
            return &[];
        }
        self.accels.needles(self.accelerator_index(id))
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn get_prefilter(&self) -> Option<&Prefilter> {
        self.pre.as_ref()
    }
}

/// The transition table portion of a dense DFA.
///
/// The transition table is the core part of the DFA in that it describes how
/// to move from one state to another based on the input sequence observed.
#[derive(Clone)]
pub(crate) struct TransitionTable<T> {
    /// A contiguous region of memory representing the transition table in
    /// row-major order. The representation is dense. That is, every state
    /// has precisely the same number of transitions. The maximum number of
    /// transitions per state is 257 (256 for each possible byte value, plus 1
    /// for the special EOI transition). If a DFA has been instructed to use
    /// byte classes (the default), then the number of transitions is usually
    /// substantially fewer.
    ///
    /// In practice, T is either `Vec<u32>` or `&[u32]`.
    table: T,
    /// A set of equivalence classes, where a single equivalence class
    /// represents a set of bytes that never discriminate between a match
    /// and a non-match in the DFA. Each equivalence class corresponds to a
    /// single character in this DFA's alphabet, where the maximum number of
    /// characters is 257 (each possible value of a byte plus the special
    /// EOI transition). Consequently, the number of equivalence classes
    /// corresponds to the number of transitions for each DFA state. Note
    /// though that the *space* used by each DFA state in the transition table
    /// may be larger. The total space used by each DFA state is known as the
    /// stride.
    ///
    /// The only time the number of equivalence classes is fewer than 257 is if
    /// the DFA's kind uses byte classes (which is the default). Equivalence
    /// classes should generally only be disabled when debugging, so that
    /// the transitions themselves aren't obscured. Disabling them has no
    /// other benefit, since the equivalence class map is always used while
    /// searching. In the vast majority of cases, the number of equivalence
    /// classes is substantially smaller than 257, particularly when large
    /// Unicode classes aren't used.
    classes: ByteClasses,
    /// The stride of each DFA state, expressed as a power-of-two exponent.
    ///
    /// The stride of a DFA corresponds to the total amount of space used by
    /// each DFA state in the transition table. This may be bigger than the
    /// size of a DFA's alphabet, since the stride is always the smallest
    /// power of two greater than or equal to the alphabet size.
    ///
    /// While this wastes space, this avoids the need for integer division
    /// to convert between premultiplied state IDs and their corresponding
    /// indices. Instead, we can use simple bit-shifts.
    ///
    /// See the docs for the `stride2` method for more details.
    ///
    /// The minimum `stride2` value is `1` (corresponding to a stride of `2`)
    /// while the maximum `stride2` value is `9` (corresponding to a stride of
    /// `512`). The maximum is not `8` since the maximum alphabet size is `257`
    /// when accounting for the special EOI transition. However, an alphabet
    /// length of that size is exceptionally rare since the alphabet is shrunk
    /// into equivalence classes.
    stride2: usize,
}

impl<'a> TransitionTable<&'a [u32]> {
    /// Deserialize a transition table starting at the beginning of `slice`.
    /// Upon success, return the total number of bytes read along with the
    /// transition table.
    ///
    /// If there was a problem deserializing any part of the transition table,
    /// then this returns an error. Notably, if the given slice does not have
    /// the same alignment as `StateID`, then this will return an error (among
    /// other possible errors).
    ///
    /// This is guaranteed to execute in constant time.
    ///
    /// # Safety
    ///
    /// This routine is not safe because it does not check the validity of the
    /// transition table itself. In particular, the transition table can be
    /// quite large, so checking its validity can be somewhat expensive. An
    /// invalid transition table is not safe because other code may rely on the
    /// transition table being correct (such as explicit bounds check elision).
    /// Therefore, an invalid transition table can lead to undefined behavior.
    ///
    /// Callers that use this function must either pass on the safety invariant
    /// or guarantee that the bytes given contain a valid transition table.
    /// This guarantee is upheld by the bytes written by `write_to`.
    unsafe fn from_bytes_unchecked(
        mut slice: &'a [u8],
    ) -> Result<(TransitionTable<&'a [u32]>, usize), DeserializeError> {
        let slice_start = slice.as_ptr().as_usize();

        let (state_len, nr) =
            wire::try_read_u32_as_usize(slice, "state length")?;
        slice = &slice[nr..];

        let (stride2, nr) = wire::try_read_u32_as_usize(slice, "stride2")?;
        slice = &slice[nr..];

        let (classes, nr) = ByteClasses::from_bytes(slice)?;
        slice = &slice[nr..];

        // The alphabet length (determined by the byte class map) cannot be
        // bigger than the stride (total space used by each DFA state).
        if stride2 > 9 {
            return Err(DeserializeError::generic(
                "dense DFA has invalid stride2 (too big)",
            ));
        }
        // It also cannot be zero, since even a DFA that never matches anything
        // has a non-zero number of states with at least two equivalence
        // classes: one for all 256 byte values and another for the EOI
        // sentinel.
        if stride2 < 1 {
            return Err(DeserializeError::generic(
                "dense DFA has invalid stride2 (too small)",
            ));
        }
        // This is OK since 1 <= stride2 <= 9.
        let stride =
            1usize.checked_shl(u32::try_from(stride2).unwrap()).unwrap();
        if classes.alphabet_len() > stride {
            return Err(DeserializeError::generic(
                "alphabet size cannot be bigger than transition table stride",
            ));
        }

        let trans_len =
            wire::shl(state_len, stride2, "dense table transition length")?;
        let table_bytes_len = wire::mul(
            trans_len,
            StateID::SIZE,
            "dense table state byte length",
        )?;
        wire::check_slice_len(slice, table_bytes_len, "transition table")?;
        wire::check_alignment::<StateID>(slice)?;
        let table_bytes = &slice[..table_bytes_len];
        slice = &slice[table_bytes_len..];
        // SAFETY: Since StateID is always representable as a u32, all we need
        // to do is ensure that we have the proper length and alignment. We've
        // checked both above, so the cast below is safe.
        //
        // N.B. This is the only not-safe code in this function.
        let table = core::slice::from_raw_parts(
            table_bytes.as_ptr().cast::<u32>(),
            trans_len,
        );
        let tt = TransitionTable { table, classes, stride2 };
        Ok((tt, slice.as_ptr().as_usize() - slice_start))
    }
}

#[cfg(feature = "dfa-build")]
impl TransitionTable<Vec<u32>> {
    /// Create a minimal transition table with just two states: a dead state
    /// and a quit state. The alphabet length and stride of the transition
    /// table is determined by the given set of equivalence classes.
    fn minimal(classes: ByteClasses) -> TransitionTable<Vec<u32>> {
        let mut tt = TransitionTable {
            table: vec![],
            classes,
            stride2: classes.stride2(),
        };
        // Two states, regardless of alphabet size, can always fit into u32.
        tt.add_empty_state().unwrap(); // dead state
        tt.add_empty_state().unwrap(); // quit state
        tt
    }

    /// Set a transition in this table. Both the `from` and `to` states must
    /// already exist, otherwise this panics. `unit` should correspond to the
    /// transition out of `from` to set to `to`.
    fn set(&mut self, from: StateID, unit: alphabet::Unit, to: StateID) {
        assert!(self.is_valid(from), "invalid 'from' state");
        assert!(self.is_valid(to), "invalid 'to' state");
        self.table[from.as_usize() + self.classes.get_by_unit(unit)] =
            to.as_u32();
    }

    /// Add an empty state (a state where all transitions lead to a dead state)
    /// and return its identifier. The identifier returned is guaranteed to
    /// not point to any other existing state.
    ///
    /// If adding a state would exhaust the state identifier space, then this
    /// returns an error.
    fn add_empty_state(&mut self) -> Result<StateID, BuildError> {
        // Normally, to get a fresh state identifier, we would just
        // take the index of the next state added to the transition
        // table. However, we actually perform an optimization here
        // that premultiplies state IDs by the stride, such that they
        // point immediately at the beginning of their transitions in
        // the transition table. This avoids an extra multiplication
        // instruction for state lookup at search time.
        //
        // Premultiplied identifiers means that instead of your matching
        // loop looking something like this:
        //
        //   state = dfa.start
        //   for byte in haystack:
        //       next = dfa.transitions[state * stride + byte]
        //       if dfa.is_match(next):
        //           return true
        //   return false
        //
        // it can instead look like this:
        //
        //   state = dfa.start
        //   for byte in haystack:
        //       next = dfa.transitions[state + byte]
        //       if dfa.is_match(next):
        //           return true
        //   return false
        //
        // In other words, we save a multiplication instruction in the
        // critical path. This turns out to be a decent performance win.
        // The cost of using premultiplied state ids is that they can
        // require a bigger state id representation. (And they also make
        // the code a bit more complex, especially during minimization and
        // when reshuffling states, as one needs to convert back and forth
        // between state IDs and state indices.)
        //
        // To do this, we simply take the index of the state into the
        // entire transition table, rather than the index of the state
        // itself. e.g., If the stride is 64, then the ID of the 3rd state
        // is 192, not 2.
        let next = self.table.len();
        let id =
            StateID::new(next).map_err(|_| BuildError::too_many_states())?;
        self.table.extend(iter::repeat(0).take(self.stride()));
        Ok(id)
    }

    /// Swap the two states given in this transition table.
    ///
    /// This routine does not do anything to check the correctness of this
    /// swap. Callers must ensure that other states pointing to id1 and id2 are
    /// updated appropriately.
    ///
    /// Both id1 and id2 must point to valid states, otherwise this panics.
    fn swap(&mut self, id1: StateID, id2: StateID) {
        assert!(self.is_valid(id1), "invalid 'id1' state: {:?}", id1);
        assert!(self.is_valid(id2), "invalid 'id2' state: {:?}", id2);
        // We only need to swap the parts of the state that are used. So if the
        // stride is 64, but the alphabet length is only 33, then we save a lot
        // of work.
        for b in 0..self.classes.alphabet_len() {
            self.table.swap(id1.as_usize() + b, id2.as_usize() + b);
        }
    }

    /// Remap the transitions for the state given according to the function
    /// given. This applies the given map function to every transition in the
    /// given state and changes the transition in place to the result of the
    /// map function for that transition.
    fn remap(&mut self, id: StateID, map: impl Fn(StateID) -> StateID) {
        for byte in 0..self.alphabet_len() {
            let i = id.as_usize() + byte;
            let next = self.table()[i];
            self.table_mut()[id.as_usize() + byte] = map(next);
        }
    }

    /// Truncate the states in this transition table to the given length.
    ///
    /// This routine does not do anything to check the correctness of this
    /// truncation. Callers must ensure that other states pointing to truncated
    /// states are updated appropriately.
    fn truncate(&mut self, len: usize) {
        self.table.truncate(len << self.stride2);
    }
}

impl<T: AsRef<[u32]>> TransitionTable<T> {
    /// Writes a serialized form of this transition table to the buffer given.
    /// If the buffer is too small, then an error is returned. To determine
    /// how big the buffer must be, use `write_to_len`.
    fn write_to<E: Endian>(
        &self,
        mut dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("transition table"));
        }
        dst = &mut dst[..nwrite];

        // write state length
        // Unwrap is OK since number of states is guaranteed to fit in a u32.
        E::write_u32(u32::try_from(self.len()).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];

        // write state stride (as power of 2)
        // Unwrap is OK since stride2 is guaranteed to be <= 9.
        E::write_u32(u32::try_from(self.stride2).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];

        // write byte class map
        let n = self.classes.write_to(dst)?;
        dst = &mut dst[n..];

        // write actual transitions
        for &sid in self.table() {
            let n = wire::write_state_id::<E>(sid, &mut dst);
            dst = &mut dst[n..];
        }
        Ok(nwrite)
    }

    /// Returns the number of bytes the serialized form of this transition
    /// table will use.
    fn write_to_len(&self) -> usize {
        size_of::<u32>()   // state length
        + size_of::<u32>() // stride2
        + self.classes.write_to_len()
        + (self.table().len() * StateID::SIZE)
    }

    /// Validates that every state ID in this transition table is valid.
    ///
    /// That is, every state ID can be used to correctly index a state in this
    /// table.
    fn validate(&self, dfa: &DFA<T>) -> Result<(), DeserializeError> {
        let sp = &dfa.special;
        for state in self.states() {
            // We check that the ID itself is well formed. That is, if it's
            // a special state then it must actually be a quit, dead, accel,
            // match or start state.
            if sp.is_special_state(state.id()) {
                let is_actually_special = sp.is_dead_state(state.id())
                    || sp.is_quit_state(state.id())
                    || sp.is_match_state(state.id())
                    || sp.is_start_state(state.id())
                    || sp.is_accel_state(state.id());
                if !is_actually_special {
                    // This is kind of a cryptic error message...
                    return Err(DeserializeError::generic(
                        "found dense state tagged as special but \
                         wasn't actually special",
                    ));
                }
                if sp.is_match_state(state.id())
                    && dfa.match_len(state.id()) == 0
                {
                    return Err(DeserializeError::generic(
                        "found match state with zero pattern IDs",
                    ));
                }
            }
            for (_, to) in state.transitions() {
                if !self.is_valid(to) {
                    return Err(DeserializeError::generic(
                        "found invalid state ID in transition table",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Converts this transition table to a borrowed value.
    fn as_ref(&self) -> TransitionTable<&'_ [u32]> {
        TransitionTable {
            table: self.table.as_ref(),
            classes: self.classes.clone(),
            stride2: self.stride2,
        }
    }

    /// Converts this transition table to an owned value.
    #[cfg(feature = "alloc")]
    fn to_owned(&self) -> TransitionTable<alloc::vec::Vec<u32>> {
        TransitionTable {
            table: self.table.as_ref().to_vec(),
            classes: self.classes.clone(),
            stride2: self.stride2,
        }
    }

    /// Return the state for the given ID. If the given ID is not valid, then
    /// this panics.
    fn state(&self, id: StateID) -> State<'_> {
        assert!(self.is_valid(id));

        let i = id.as_usize();
        State {
            id,
            stride2: self.stride2,
            transitions: &self.table()[i..i + self.alphabet_len()],
        }
    }

    /// Returns an iterator over all states in this transition table.
    ///
    /// This iterator yields a tuple for each state. The first element of the
    /// tuple corresponds to a state's identifier, and the second element
    /// corresponds to the state itself (comprised of its transitions).
    fn states(&self) -> StateIter<'_, T> {
        StateIter {
            tt: self,
            it: self.table().chunks(self.stride()).enumerate(),
        }
    }

    /// Convert a state identifier to an index to a state (in the range
    /// 0..self.len()).
    ///
    /// This is useful when using a `Vec<T>` as an efficient map keyed by state
    /// to some other information (such as a remapped state ID).
    ///
    /// If the given ID is not valid, then this may panic or produce an
    /// incorrect index.
    fn to_index(&self, id: StateID) -> usize {
        id.as_usize() >> self.stride2
    }

    /// Convert an index to a state (in the range 0..self.len()) to an actual
    /// state identifier.
    ///
    /// This is useful when using a `Vec<T>` as an efficient map keyed by state
    /// to some other information (such as a remapped state ID).
    ///
    /// If the given index is not in the specified range, then this may panic
    /// or produce an incorrect state ID.
    fn to_state_id(&self, index: usize) -> StateID {
        // CORRECTNESS: If the given index is not valid, then it is not
        // required for this to panic or return a valid state ID.
        StateID::new_unchecked(index << self.stride2)
    }

    /// Returns the state ID for the state immediately following the one given.
    ///
    /// This does not check whether the state ID returned is invalid. In fact,
    /// if the state ID given is the last state in this DFA, then the state ID
    /// returned is guaranteed to be invalid.
    #[cfg(feature = "dfa-build")]
    fn next_state_id(&self, id: StateID) -> StateID {
        self.to_state_id(self.to_index(id).checked_add(1).unwrap())
    }

    /// Returns the state ID for the state immediately preceding the one given.
    ///
    /// If the dead ID given (which is zero), then this panics.
    #[cfg(feature = "dfa-build")]
    fn prev_state_id(&self, id: StateID) -> StateID {
        self.to_state_id(self.to_index(id).checked_sub(1).unwrap())
    }

    /// Returns the table as a slice of state IDs.
    fn table(&self) -> &[StateID] {
        wire::u32s_to_state_ids(self.table.as_ref())
    }

    /// Returns the total number of states in this transition table.
    ///
    /// Note that a DFA always has at least two states: the dead and quit
    /// states. In particular, the dead state always has ID 0 and is
    /// correspondingly always the first state. The dead state is never a match
    /// state.
    fn len(&self) -> usize {
        self.table().len() >> self.stride2
    }

    /// Returns the total stride for every state in this DFA. This corresponds
    /// to the total number of transitions used by each state in this DFA's
    /// transition table.
    fn stride(&self) -> usize {
        1 << self.stride2
    }

    /// Returns the total number of elements in the alphabet for this
    /// transition table. This is always less than or equal to `self.stride()`.
    /// It is only equal when the alphabet length is a power of 2. Otherwise,
    /// it is always strictly less.
    fn alphabet_len(&self) -> usize {
        self.classes.alphabet_len()
    }

    /// Returns true if and only if the given state ID is valid for this
    /// transition table. Validity in this context means that the given ID can
    /// be used as a valid offset with `self.stride()` to index this transition
    /// table.
    fn is_valid(&self, id: StateID) -> bool {
        let id = id.as_usize();
        id < self.table().len() && id % self.stride() == 0
    }

    /// Return the memory usage, in bytes, of this transition table.
    ///
    /// This does not include the size of a `TransitionTable` value itself.
    fn memory_usage(&self) -> usize {
        self.table().len() * StateID::SIZE
    }
}

#[cfg(feature = "dfa-build")]
impl<T: AsMut<[u32]>> TransitionTable<T> {
    /// Returns the table as a slice of state IDs.
    fn table_mut(&mut self) -> &mut [StateID] {
        wire::u32s_to_state_ids_mut(self.table.as_mut())
    }
}

/// The set of all possible starting states in a DFA.
///
/// The set of starting states corresponds to the possible choices one can make
/// in terms of starting a DFA. That is, before following the first transition,
/// you first need to select the state that you start in.
///
/// Normally, a DFA converted from an NFA that has a single starting state
/// would itself just have one starting state. However, our support for look
/// around generally requires more starting states. The correct starting state
/// is chosen based on certain properties of the position at which we begin
/// our search.
///
/// Before listing those properties, we first must define two terms:
///
/// * `haystack` - The bytes to execute the search. The search always starts
///   at the beginning of `haystack` and ends before or at the end of
///   `haystack`.
/// * `context` - The (possibly empty) bytes surrounding `haystack`. `haystack`
///   must be contained within `context` such that `context` is at least as big
///   as `haystack`.
///
/// This split is crucial for dealing with look-around. For example, consider
/// the context `foobarbaz`, the haystack `bar` and the regex `^bar$`. This
/// regex should _not_ match the haystack since `bar` does not appear at the
/// beginning of the input. Similarly, the regex `\Bbar\B` should match the
/// haystack because `bar` is not surrounded by word boundaries. But a search
/// that does not take context into account would not permit `\B` to match
/// since the beginning of any string matches a word boundary. Similarly, a
/// search that does not take context into account when searching `^bar$` in
/// the haystack `bar` would produce a match when it shouldn't.
///
/// Thus, it follows that the starting state is chosen based on the following
/// criteria, derived from the position at which the search starts in the
/// `context` (corresponding to the start of `haystack`):
///
/// 1. If the search starts at the beginning of `context`, then the `Text`
///    start state is used. (Since `^` corresponds to
///    `hir::Anchor::Start`.)
/// 2. If the search starts at a position immediately following a line
///    terminator, then the `Line` start state is used. (Since `(?m:^)`
///    corresponds to `hir::Anchor::StartLF`.)
/// 3. If the search starts at a position immediately following a byte
///    classified as a "word" character (`[_0-9a-zA-Z]`), then the `WordByte`
///    start state is used. (Since `(?-u:\b)` corresponds to a word boundary.)
/// 4. Otherwise, if the search starts at a position immediately following
///    a byte that is not classified as a "word" character (`[^_0-9a-zA-Z]`),
///    then the `NonWordByte` start state is used. (Since `(?-u:\B)`
///    corresponds to a not-word-boundary.)
///
/// (N.B. Unicode word boundaries are not supported by the DFA because they
/// require multi-byte look-around and this is difficult to support in a DFA.)
///
/// To further complicate things, we also support constructing individual
/// anchored start states for each pattern in the DFA. (Which is required to
/// implement overlapping regexes correctly, but is also generally useful.)
/// Thus, when individual start states for each pattern are enabled, then the
/// total number of start states represented is `4 + (4 * #patterns)`, where
/// the 4 comes from each of the 4 possibilities above. The first 4 represents
/// the starting states for the entire DFA, which support searching for
/// multiple patterns simultaneously (possibly unanchored).
///
/// If individual start states are disabled, then this will only store 4
/// start states. Typically, individual start states are only enabled when
/// constructing the reverse DFA for regex matching. But they are also useful
/// for building DFAs that can search for a specific pattern or even to support
/// both anchored and unanchored searches with the same DFA.
///
/// Note though that while the start table always has either `4` or
/// `4 + (4 * #patterns)` starting state *ids*, the total number of states
/// might be considerably smaller. That is, many of the IDs may be duplicative.
/// (For example, if a regex doesn't have a `\b` sub-pattern, then there's no
/// reason to generate a unique starting state for handling word boundaries.
/// Similarly for start/end anchors.)
#[derive(Clone)]
pub(crate) struct StartTable<T> {
    /// The initial start state IDs.
    ///
    /// In practice, T is either `Vec<u32>` or `&[u32]`.
    ///
    /// The first `2 * stride` (currently always 8) entries always correspond
    /// to the starts states for the entire DFA, with the first 4 entries being
    /// for unanchored searches and the second 4 entries being for anchored
    /// searches. To keep things simple, we always use 8 entries even if the
    /// `StartKind` is not both.
    ///
    /// After that, there are `stride * patterns` state IDs, where `patterns`
    /// may be zero in the case of a DFA with no patterns or in the case where
    /// the DFA was built without enabling starting states for each pattern.
    table: T,
    /// The starting state configuration supported. When 'both', both
    /// unanchored and anchored searches work. When 'unanchored', anchored
    /// searches panic. When 'anchored', unanchored searches panic.
    kind: StartKind,
    /// The start state configuration for every possible byte.
    start_map: StartByteMap,
    /// The number of starting state IDs per pattern.
    stride: usize,
    /// The total number of patterns for which starting states are encoded.
    /// This is `None` for DFAs that were built without start states for each
    /// pattern. Thus, one cannot use this field to say how many patterns
    /// are in the DFA in all cases. It is specific to how many patterns are
    /// represented in this start table.
    pattern_len: Option<usize>,
    /// The universal starting state for unanchored searches. This is only
    /// present when the DFA supports unanchored searches and when all starting
    /// state IDs for an unanchored search are equivalent.
    universal_start_unanchored: Option<StateID>,
    /// The universal starting state for anchored searches. This is only
    /// present when the DFA supports anchored searches and when all starting
    /// state IDs for an anchored search are equivalent.
    universal_start_anchored: Option<StateID>,
}

#[cfg(feature = "dfa-build")]
impl StartTable<Vec<u32>> {
    /// Create a valid set of start states all pointing to the dead state.
    ///
    /// When the corresponding DFA is constructed with start states for each
    /// pattern, then `patterns` should be the number of patterns. Otherwise,
    /// it should be zero.
    ///
    /// If the total table size could exceed the allocatable limit, then this
    /// returns an error. In practice, this is unlikely to be able to occur,
    /// since it's likely that allocation would have failed long before it got
    /// to this point.
    fn dead(
        kind: StartKind,
        lookm: &LookMatcher,
        pattern_len: Option<usize>,
    ) -> Result<StartTable<Vec<u32>>, BuildError> {
        if let Some(len) = pattern_len {
            assert!(len <= PatternID::LIMIT);
        }
        let stride = Start::len();
        // OK because 2*4 is never going to overflow anything.
        let starts_len = stride.checked_mul(2).unwrap();
        let pattern_starts_len =
            match stride.checked_mul(pattern_len.unwrap_or(0)) {
                Some(x) => x,
                None => return Err(BuildError::too_many_start_states()),
            };
        let table_len = match starts_len.checked_add(pattern_starts_len) {
            Some(x) => x,
            None => return Err(BuildError::too_many_start_states()),
        };
        if let Err(_) = isize::try_from(table_len) {
            return Err(BuildError::too_many_start_states());
        }
        let table = vec![DEAD.as_u32(); table_len];
        let start_map = StartByteMap::new(lookm);
        Ok(StartTable {
            table,
            kind,
            start_map,
            stride,
            pattern_len,
            universal_start_unanchored: None,
            universal_start_anchored: None,
        })
    }
}

impl<'a> StartTable<&'a [u32]> {
    /// Deserialize a table of start state IDs starting at the beginning of
    /// `slice`. Upon success, return the total number of bytes read along with
    /// the table of starting state IDs.
    ///
    /// If there was a problem deserializing any part of the starting IDs,
    /// then this returns an error. Notably, if the given slice does not have
    /// the same alignment as `StateID`, then this will return an error (among
    /// other possible errors).
    ///
    /// This is guaranteed to execute in constant time.
    ///
    /// # Safety
    ///
    /// This routine is not safe because it does not check the validity of the
    /// starting state IDs themselves. In particular, the number of starting
    /// IDs can be of variable length, so it's possible that checking their
    /// validity cannot be done in constant time. An invalid starting state
    /// ID is not safe because other code may rely on the starting IDs being
    /// correct (such as explicit bounds check elision). Therefore, an invalid
    /// start ID can lead to undefined behavior.
    ///
    /// Callers that use this function must either pass on the safety invariant
    /// or guarantee that the bytes given contain valid starting state IDs.
    /// This guarantee is upheld by the bytes written by `write_to`.
    unsafe fn from_bytes_unchecked(
        mut slice: &'a [u8],
    ) -> Result<(StartTable<&'a [u32]>, usize), DeserializeError> {
        let slice_start = slice.as_ptr().as_usize();

        let (kind, nr) = StartKind::from_bytes(slice)?;
        slice = &slice[nr..];

        let (start_map, nr) = StartByteMap::from_bytes(slice)?;
        slice = &slice[nr..];

        let (stride, nr) =
            wire::try_read_u32_as_usize(slice, "start table stride")?;
        slice = &slice[nr..];
        if stride != Start::len() {
            return Err(DeserializeError::generic(
                "invalid starting table stride",
            ));
        }

        let (maybe_pattern_len, nr) =
            wire::try_read_u32_as_usize(slice, "start table patterns")?;
        slice = &slice[nr..];
        let pattern_len = if maybe_pattern_len.as_u32() == u32::MAX {
            None
        } else {
            Some(maybe_pattern_len)
        };
        if pattern_len.map_or(false, |len| len > PatternID::LIMIT) {
            return Err(DeserializeError::generic(
                "invalid number of patterns",
            ));
        }

        let (universal_unanchored, nr) =
            wire::try_read_u32(slice, "universal unanchored start")?;
        slice = &slice[nr..];
        let universal_start_unanchored = if universal_unanchored == u32::MAX {
            None
        } else {
            Some(StateID::try_from(universal_unanchored).map_err(|e| {
                DeserializeError::state_id_error(
                    e,
                    "universal unanchored start",
                )
            })?)
        };

        let (universal_anchored, nr) =
            wire::try_read_u32(slice, "universal anchored start")?;
        slice = &slice[nr..];
        let universal_start_anchored = if universal_anchored == u32::MAX {
            None
        } else {
            Some(StateID::try_from(universal_anchored).map_err(|e| {
                DeserializeError::state_id_error(e, "universal anchored start")
            })?)
        };

        let pattern_table_size = wire::mul(
            stride,
            pattern_len.unwrap_or(0),
            "invalid pattern length",
        )?;
        // Our start states always start with a two stride of start states for
        // the entire automaton. The first stride is for unanchored starting
        // states and the second stride is for anchored starting states. What
        // follows it are an optional set of start states for each pattern.
        let start_state_len = wire::add(
            wire::mul(2, stride, "start state stride too big")?,
            pattern_table_size,
            "invalid 'any' pattern starts size",
        )?;
        let table_bytes_len = wire::mul(
            start_state_len,
            StateID::SIZE,
            "pattern table bytes length",
        )?;
        wire::check_slice_len(slice, table_bytes_len, "start ID table")?;
        wire::check_alignment::<StateID>(slice)?;
        let table_bytes = &slice[..table_bytes_len];
        slice = &slice[table_bytes_len..];
        // SAFETY: Since StateID is always representable as a u32, all we need
        // to do is ensure that we have the proper length and alignment. We've
        // checked both above, so the cast below is safe.
        //
        // N.B. This is the only not-safe code in this function.
        let table = core::slice::from_raw_parts(
            table_bytes.as_ptr().cast::<u32>(),
            start_state_len,
        );
        let st = StartTable {
            table,
            kind,
            start_map,
            stride,
            pattern_len,
            universal_start_unanchored,
            universal_start_anchored,
        };
        Ok((st, slice.as_ptr().as_usize() - slice_start))
    }
}

impl<T: AsRef<[u32]>> StartTable<T> {
    /// Writes a serialized form of this start table to the buffer given. If
    /// the buffer is too small, then an error is returned. To determine how
    /// big the buffer must be, use `write_to_len`.
    fn write_to<E: Endian>(
        &self,
        mut dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small(
                "starting table ids",
            ));
        }
        dst = &mut dst[..nwrite];

        // write start kind
        let nw = self.kind.write_to::<E>(dst)?;
        dst = &mut dst[nw..];
        // write start byte map
        let nw = self.start_map.write_to(dst)?;
        dst = &mut dst[nw..];
        // write stride
        // Unwrap is OK since the stride is always 4 (currently).
        E::write_u32(u32::try_from(self.stride).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];
        // write pattern length
        // Unwrap is OK since number of patterns is guaranteed to fit in a u32.
        E::write_u32(
            u32::try_from(self.pattern_len.unwrap_or(0xFFFF_FFFF)).unwrap(),
            dst,
        );
        dst = &mut dst[size_of::<u32>()..];
        // write universal start unanchored state id, u32::MAX if absent
        E::write_u32(
            self.universal_start_unanchored
                .map_or(u32::MAX, |sid| sid.as_u32()),
            dst,
        );
        dst = &mut dst[size_of::<u32>()..];
        // write universal start anchored state id, u32::MAX if absent
        E::write_u32(
            self.universal_start_anchored.map_or(u32::MAX, |sid| sid.as_u32()),
            dst,
        );
        dst = &mut dst[size_of::<u32>()..];
        // write start IDs
        for &sid in self.table() {
            let n = wire::write_state_id::<E>(sid, &mut dst);
            dst = &mut dst[n..];
        }
        Ok(nwrite)
    }

    /// Returns the number of bytes the serialized form of this start ID table
    /// will use.
    fn write_to_len(&self) -> usize {
        self.kind.write_to_len()
        + self.start_map.write_to_len()
        + size_of::<u32>() // stride
        + size_of::<u32>() // # patterns
        + size_of::<u32>() // universal unanchored start
        + size_of::<u32>() // universal anchored start
        + (self.table().len() * StateID::SIZE)
    }

    /// Validates that every state ID in this start table is valid by checking
    /// it against the given transition table (which must be for the same DFA).
    ///
    /// That is, every state ID can be used to correctly index a state.
    fn validate(&self, dfa: &DFA<T>) -> Result<(), DeserializeError> {
        let tt = &dfa.tt;
        if !self.universal_start_unanchored.map_or(true, |s| tt.is_valid(s)) {
            return Err(DeserializeError::generic(
                "found invalid universal unanchored starting state ID",
            ));
        }
        if !self.universal_start_anchored.map_or(true, |s| tt.is_valid(s)) {
            return Err(DeserializeError::generic(
                "found invalid universal anchored starting state ID",
            ));
        }
        for &id in self.table() {
            if !tt.is_valid(id) {
                return Err(DeserializeError::generic(
                    "found invalid starting state ID",
                ));
            }
        }
        Ok(())
    }

    /// Converts this start list to a borrowed value.
    fn as_ref(&self) -> StartTable<&'_ [u32]> {
        StartTable {
            table: self.table.as_ref(),
            kind: self.kind,
            start_map: self.start_map.clone(),
            stride: self.stride,
            pattern_len: self.pattern_len,
            universal_start_unanchored: self.universal_start_unanchored,
            universal_start_anchored: self.universal_start_anchored,
        }
    }

    /// Converts this start list to an owned value.
    #[cfg(feature = "alloc")]
    fn to_owned(&self) -> StartTable<alloc::vec::Vec<u32>> {
        StartTable {
            table: self.table.as_ref().to_vec(),
            kind: self.kind,
            start_map: self.start_map.clone(),
            stride: self.stride,
            pattern_len: self.pattern_len,
            universal_start_unanchored: self.universal_start_unanchored,
            universal_start_anchored: self.universal_start_anchored,
        }
    }

    /// Return the start state for the given input and starting configuration.
    /// This returns an error if the input configuration is not supported by
    /// this DFA. For example, requesting an unanchored search when the DFA was
    /// not built with unanchored starting states. Or asking for an anchored
    /// pattern search with an invalid pattern ID or on a DFA that was not
    /// built with start states for each pattern.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn start(
        &self,
        anchored: Anchored,
        start: Start,
    ) -> Result<StateID, StartError> {
        let start_index = start.as_usize();
        let index = match anchored {
            Anchored::No => {
                if !self.kind.has_unanchored() {
                    return Err(StartError::unsupported_anchored(anchored));
                }
                start_index
            }
            Anchored::Yes => {
                if !self.kind.has_anchored() {
                    return Err(StartError::unsupported_anchored(anchored));
                }
                self.stride + start_index
            }
            Anchored::Pattern(pid) => {
                let len = match self.pattern_len {
                    None => {
                        return Err(StartError::unsupported_anchored(anchored))
                    }
                    Some(len) => len,
                };
                if pid.as_usize() >= len {
                    return Ok(DEAD);
                }
                (2 * self.stride)
                    + (self.stride * pid.as_usize())
                    + start_index
            }
        };
        Ok(self.table()[index])
    }

    /// Returns an iterator over all start state IDs in this table.
    ///
    /// Each item is a triple of: start state ID, the start state type and the
    /// pattern ID (if any).
    fn iter(&self) -> StartStateIter<'_> {
        StartStateIter { st: self.as_ref(), i: 0 }
    }

    /// Returns the table as a slice of state IDs.
    fn table(&self) -> &[StateID] {
        wire::u32s_to_state_ids(self.table.as_ref())
    }

    /// Return the memory usage, in bytes, of this start list.
    ///
    /// This does not include the size of a `StartList` value itself.
    fn memory_usage(&self) -> usize {
        self.table().len() * StateID::SIZE
    }
}

#[cfg(feature = "dfa-build")]
impl<T: AsMut<[u32]>> StartTable<T> {
    /// Set the start state for the given index and pattern.
    ///
    /// If the pattern ID or state ID are not valid, then this will panic.
    fn set_start(&mut self, anchored: Anchored, start: Start, id: StateID) {
        let start_index = start.as_usize();
        let index = match anchored {
            Anchored::No => start_index,
            Anchored::Yes => self.stride + start_index,
            Anchored::Pattern(pid) => {
                let pid = pid.as_usize();
                let len = self
                    .pattern_len
                    .expect("start states for each pattern enabled");
                assert!(pid < len, "invalid pattern ID {:?}", pid);
                self.stride
                    .checked_mul(pid)
                    .unwrap()
                    .checked_add(self.stride.checked_mul(2).unwrap())
                    .unwrap()
                    .checked_add(start_index)
                    .unwrap()
            }
        };
        self.table_mut()[index] = id;
    }

    /// Returns the table as a mutable slice of state IDs.
    fn table_mut(&mut self) -> &mut [StateID] {
        wire::u32s_to_state_ids_mut(self.table.as_mut())
    }
}

/// An iterator over start state IDs.
///
/// This iterator yields a triple of start state ID, the anchored mode and the
/// start state type. If a pattern ID is relevant, then the anchored mode will
/// contain it. Start states with an anchored mode containing a pattern ID will
/// only occur when the DFA was compiled with start states for each pattern
/// (which is disabled by default).
pub(crate) struct StartStateIter<'a> {
    st: StartTable<&'a [u32]>,
    i: usize,
}

impl<'a> Iterator for StartStateIter<'a> {
    type Item = (StateID, Anchored, Start);

    fn next(&mut self) -> Option<(StateID, Anchored, Start)> {
        let i = self.i;
        let table = self.st.table();
        if i >= table.len() {
            return None;
        }
        self.i += 1;

        // This unwrap is okay since the stride of the starting state table
        // must always match the number of start state types.
        let start_type = Start::from_usize(i % self.st.stride).unwrap();
        let anchored = if i < self.st.stride {
            Anchored::No
        } else if i < (2 * self.st.stride) {
            Anchored::Yes
        } else {
            let pid = (i - (2 * self.st.stride)) / self.st.stride;
            Anchored::Pattern(PatternID::new(pid).unwrap())
        };
        Some((table[i], anchored, start_type))
    }
}

/// This type represents that patterns that should be reported whenever a DFA
/// enters a match state. This structure exists to support DFAs that search for
/// matches for multiple regexes.
///
/// This structure relies on the fact that all match states in a DFA occur
/// contiguously in the DFA's transition table. (See dfa/special.rs for a more
/// detailed breakdown of the representation.) Namely, when a match occurs, we
/// know its state ID. Since we know the start and end of the contiguous region
/// of match states, we can use that to compute the position at which the match
/// state occurs. That in turn is used as an offset into this structure.
#[derive(Clone, Debug)]
struct MatchStates<T> {
    /// Slices is a flattened sequence of pairs, where each pair points to a
    /// sub-slice of pattern_ids. The first element of the pair is an offset
    /// into pattern_ids and the second element of the pair is the number
    /// of 32-bit pattern IDs starting at that position. That is, each pair
    /// corresponds to a single DFA match state and its corresponding match
    /// IDs. The number of pairs always corresponds to the number of distinct
    /// DFA match states.
    ///
    /// In practice, T is either Vec<u32> or &[u32].
    slices: T,
    /// A flattened sequence of pattern IDs for each DFA match state. The only
    /// way to correctly read this sequence is indirectly via `slices`.
    ///
    /// In practice, T is either Vec<u32> or &[u32].
    pattern_ids: T,
    /// The total number of unique patterns represented by these match states.
    pattern_len: usize,
}

impl<'a> MatchStates<&'a [u32]> {
    unsafe fn from_bytes_unchecked(
        mut slice: &'a [u8],
    ) -> Result<(MatchStates<&'a [u32]>, usize), DeserializeError> {
        let slice_start = slice.as_ptr().as_usize();

        // Read the total number of match states.
        let (state_len, nr) =
            wire::try_read_u32_as_usize(slice, "match state length")?;
        slice = &slice[nr..];

        // Read the slice start/length pairs.
        let pair_len = wire::mul(2, state_len, "match state offset pairs")?;
        let slices_bytes_len = wire::mul(
            pair_len,
            PatternID::SIZE,
            "match state slice offset byte length",
        )?;
        wire::check_slice_len(slice, slices_bytes_len, "match state slices")?;
        wire::check_alignment::<PatternID>(slice)?;
        let slices_bytes = &slice[..slices_bytes_len];
        slice = &slice[slices_bytes_len..];
        // SAFETY: Since PatternID is always representable as a u32, all we
        // need to do is ensure that we have the proper length and alignment.
        // We've checked both above, so the cast below is safe.
        //
        // N.B. This is one of the few not-safe snippets in this function,
        // so we mark it explicitly to call it out.
        let slices = core::slice::from_raw_parts(
            slices_bytes.as_ptr().cast::<u32>(),
            pair_len,
        );

        // Read the total number of unique pattern IDs (which is always 1 more
        // than the maximum pattern ID in this automaton, since pattern IDs are
        // handed out contiguously starting at 0).
        let (pattern_len, nr) =
            wire::try_read_u32_as_usize(slice, "pattern length")?;
        slice = &slice[nr..];

        // Now read the pattern ID length. We don't need to store this
        // explicitly, but we need it to know how many pattern IDs to read.
        let (idlen, nr) =
            wire::try_read_u32_as_usize(slice, "pattern ID length")?;
        slice = &slice[nr..];

        // Read the actual pattern IDs.
        let pattern_ids_len =
            wire::mul(idlen, PatternID::SIZE, "pattern ID byte length")?;
        wire::check_slice_len(slice, pattern_ids_len, "match pattern IDs")?;
        wire::check_alignment::<PatternID>(slice)?;
        let pattern_ids_bytes = &slice[..pattern_ids_len];
        slice = &slice[pattern_ids_len..];
        // SAFETY: Since PatternID is always representable as a u32, all we
        // need to do is ensure that we have the proper length and alignment.
        // We've checked both above, so the cast below is safe.
        //
        // N.B. This is one of the few not-safe snippets in this function,
        // so we mark it explicitly to call it out.
        let pattern_ids = core::slice::from_raw_parts(
            pattern_ids_bytes.as_ptr().cast::<u32>(),
            idlen,
        );

        let ms = MatchStates { slices, pattern_ids, pattern_len };
        Ok((ms, slice.as_ptr().as_usize() - slice_start))
    }
}

#[cfg(feature = "dfa-build")]
impl MatchStates<Vec<u32>> {
    fn empty(pattern_len: usize) -> MatchStates<Vec<u32>> {
        assert!(pattern_len <= PatternID::LIMIT);
        MatchStates { slices: vec![], pattern_ids: vec![], pattern_len }
    }

    fn new(
        matches: &BTreeMap<StateID, Vec<PatternID>>,
        pattern_len: usize,
    ) -> Result<MatchStates<Vec<u32>>, BuildError> {
        let mut m = MatchStates::empty(pattern_len);
        for (_, pids) in matches.iter() {
            let start = PatternID::new(m.pattern_ids.len())
                .map_err(|_| BuildError::too_many_match_pattern_ids())?;
            m.slices.push(start.as_u32());
            // This is always correct since the number of patterns in a single
            // match state can never exceed maximum number of allowable
            // patterns. Why? Because a pattern can only appear once in a
            // particular match state, by construction. (And since our pattern
            // ID limit is one less than u32::MAX, we're guaranteed that the
            // length fits in a u32.)
            m.slices.push(u32::try_from(pids.len()).unwrap());
            for &pid in pids {
                m.pattern_ids.push(pid.as_u32());
            }
        }
        m.pattern_len = pattern_len;
        Ok(m)
    }

    fn new_with_map(
        &self,
        matches: &BTreeMap<StateID, Vec<PatternID>>,
    ) -> Result<MatchStates<Vec<u32>>, BuildError> {
        MatchStates::new(matches, self.pattern_len)
    }
}

impl<T: AsRef<[u32]>> MatchStates<T> {
    /// Writes a serialized form of these match states to the buffer given. If
    /// the buffer is too small, then an error is returned. To determine how
    /// big the buffer must be, use `write_to_len`.
    fn write_to<E: Endian>(
        &self,
        mut dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("match states"));
        }
        dst = &mut dst[..nwrite];

        // write state ID length
        // Unwrap is OK since number of states is guaranteed to fit in a u32.
        E::write_u32(u32::try_from(self.len()).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];

        // write slice offset pairs
        for &pid in self.slices() {
            let n = wire::write_pattern_id::<E>(pid, &mut dst);
            dst = &mut dst[n..];
        }

        // write unique pattern ID length
        // Unwrap is OK since number of patterns is guaranteed to fit in a u32.
        E::write_u32(u32::try_from(self.pattern_len).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];

        // write pattern ID length
        // Unwrap is OK since we check at construction (and deserialization)
        // that the number of patterns is representable as a u32.
        E::write_u32(u32::try_from(self.pattern_ids().len()).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];

        // write pattern IDs
        for &pid in self.pattern_ids() {
            let n = wire::write_pattern_id::<E>(pid, &mut dst);
            dst = &mut dst[n..];
        }

        Ok(nwrite)
    }

    /// Returns the number of bytes the serialized form of these match states
    /// will use.
    fn write_to_len(&self) -> usize {
        size_of::<u32>()   // match state length
        + (self.slices().len() * PatternID::SIZE)
        + size_of::<u32>() // unique pattern ID length
        + size_of::<u32>() // pattern ID length
        + (self.pattern_ids().len() * PatternID::SIZE)
    }

    /// Valides that the match state info is itself internally consistent and
    /// consistent with the recorded match state region in the given DFA.
    fn validate(&self, dfa: &DFA<T>) -> Result<(), DeserializeError> {
        if self.len() != dfa.special.match_len(dfa.stride()) {
            return Err(DeserializeError::generic(
                "match state length mismatch",
            ));
        }
        for si in 0..self.len() {
            let start = self.slices()[si * 2].as_usize();
            let len = self.slices()[si * 2 + 1].as_usize();
            if start >= self.pattern_ids().len() {
                return Err(DeserializeError::generic(
                    "invalid pattern ID start offset",
                ));
            }
            if start + len > self.pattern_ids().len() {
                return Err(DeserializeError::generic(
                    "invalid pattern ID length",
                ));
            }
            for mi in 0..len {
                let pid = self.pattern_id(si, mi);
                if pid.as_usize() >= self.pattern_len {
                    return Err(DeserializeError::generic(
                        "invalid pattern ID",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Converts these match states back into their map form. This is useful
    /// when shuffling states, as the normal MatchStates representation is not
    /// amenable to easy state swapping. But with this map, to swap id1 and
    /// id2, all you need to do is:
    ///
    /// if let Some(pids) = map.remove(&id1) {
    ///     map.insert(id2, pids);
    /// }
    ///
    /// Once shuffling is done, use MatchStates::new to convert back.
    #[cfg(feature = "dfa-build")]
    fn to_map(&self, dfa: &DFA<T>) -> BTreeMap<StateID, Vec<PatternID>> {
        let mut map = BTreeMap::new();
        for i in 0..self.len() {
            let mut pids = vec![];
            for j in 0..self.pattern_len(i) {
                pids.push(self.pattern_id(i, j));
            }
            map.insert(self.match_state_id(dfa, i), pids);
        }
        map
    }

    /// Converts these match states to a borrowed value.
    fn as_ref(&self) -> MatchStates<&'_ [u32]> {
        MatchStates {
            slices: self.slices.as_ref(),
            pattern_ids: self.pattern_ids.as_ref(),
            pattern_len: self.pattern_len,
        }
    }

    /// Converts these match states to an owned value.
    #[cfg(feature = "alloc")]
    fn to_owned(&self) -> MatchStates<alloc::vec::Vec<u32>> {
        MatchStates {
            slices: self.slices.as_ref().to_vec(),
            pattern_ids: self.pattern_ids.as_ref().to_vec(),
            pattern_len: self.pattern_len,
        }
    }

    /// Returns the match state ID given the match state index. (Where the
    /// first match state corresponds to index 0.)
    ///
    /// This panics if there is no match state at the given index.
    fn match_state_id(&self, dfa: &DFA<T>, index: usize) -> StateID {
        assert!(dfa.special.matches(), "no match states to index");
        // This is one of the places where we rely on the fact that match
        // states are contiguous in the transition table. Namely, that the
        // first match state ID always corresponds to dfa.special.min_start.
        // From there, since we know the stride, we can compute the ID of any
        // match state given its index.
        let stride2 = u32::try_from(dfa.stride2()).unwrap();
        let offset = index.checked_shl(stride2).unwrap();
        let id = dfa.special.min_match.as_usize().checked_add(offset).unwrap();
        let sid = StateID::new(id).unwrap();
        assert!(dfa.is_match_state(sid));
        sid
    }

    /// Returns the pattern ID at the given match index for the given match
    /// state.
    ///
    /// The match state index is the state index minus the state index of the
    /// first match state in the DFA.
    ///
    /// The match index is the index of the pattern ID for the given state.
    /// The index must be less than `self.pattern_len(state_index)`.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn pattern_id(&self, state_index: usize, match_index: usize) -> PatternID {
        self.pattern_id_slice(state_index)[match_index]
    }

    /// Returns the number of patterns in the given match state.
    ///
    /// The match state index is the state index minus the state index of the
    /// first match state in the DFA.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn pattern_len(&self, state_index: usize) -> usize {
        self.slices()[state_index * 2 + 1].as_usize()
    }

    /// Returns all of the pattern IDs for the given match state index.
    ///
    /// The match state index is the state index minus the state index of the
    /// first match state in the DFA.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn pattern_id_slice(&self, state_index: usize) -> &[PatternID] {
        let start = self.slices()[state_index * 2].as_usize();
        let len = self.pattern_len(state_index);
        &self.pattern_ids()[start..start + len]
    }

    /// Returns the pattern ID offset slice of u32 as a slice of PatternID.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn slices(&self) -> &[PatternID] {
        wire::u32s_to_pattern_ids(self.slices.as_ref())
    }

    /// Returns the total number of match states.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn len(&self) -> usize {
        assert_eq!(0, self.slices().len() % 2);
        self.slices().len() / 2
    }

    /// Returns the pattern ID slice of u32 as a slice of PatternID.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn pattern_ids(&self) -> &[PatternID] {
        wire::u32s_to_pattern_ids(self.pattern_ids.as_ref())
    }

    /// Return the memory usage, in bytes, of these match pairs.
    fn memory_usage(&self) -> usize {
        (self.slices().len() + self.pattern_ids().len()) * PatternID::SIZE
    }
}

/// A common set of flags for both dense and sparse DFAs. This primarily
/// centralizes the serialization format of these flags at a bitset.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Flags {
    /// Whether the DFA can match the empty string. When this is false, all
    /// matches returned by this DFA are guaranteed to have non-zero length.
    pub(crate) has_empty: bool,
    /// Whether the DFA should only produce matches with spans that correspond
    /// to valid UTF-8. This also includes omitting any zero-width matches that
    /// split the UTF-8 encoding of a codepoint.
    pub(crate) is_utf8: bool,
    /// Whether the DFA is always anchored or not, regardless of `Input`
    /// configuration. This is useful for avoiding a reverse scan even when
    /// executing unanchored searches.
    pub(crate) is_always_start_anchored: bool,
}

impl Flags {
    /// Creates a set of flags for a DFA from an NFA.
    ///
    /// N.B. This constructor was defined at the time of writing because all
    /// of the flags are derived directly from the NFA. If this changes in the
    /// future, we might be more thoughtful about how the `Flags` value is
    /// itself built.
    #[cfg(feature = "dfa-build")]
    fn from_nfa(nfa: &thompson::NFA) -> Flags {
        Flags {
            has_empty: nfa.has_empty(),
            is_utf8: nfa.is_utf8(),
            is_always_start_anchored: nfa.is_always_start_anchored(),
        }
    }

    /// Deserializes the flags from the given slice. On success, this also
    /// returns the number of bytes read from the slice.
    pub(crate) fn from_bytes(
        slice: &[u8],
    ) -> Result<(Flags, usize), DeserializeError> {
        let (bits, nread) = wire::try_read_u32(slice, "flag bitset")?;
        let flags = Flags {
            has_empty: bits & (1 << 0) != 0,
            is_utf8: bits & (1 << 1) != 0,
            is_always_start_anchored: bits & (1 << 2) != 0,
        };
        Ok((flags, nread))
    }

    /// Writes these flags to the given byte slice. If the buffer is too small,
    /// then an error is returned. To determine how big the buffer must be,
    /// use `write_to_len`.
    pub(crate) fn write_to<E: Endian>(
        &self,
        dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        fn bool_to_int(b: bool) -> u32 {
            if b {
                1
            } else {
                0
            }
        }

        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("flag bitset"));
        }
        let bits = (bool_to_int(self.has_empty) << 0)
            | (bool_to_int(self.is_utf8) << 1)
            | (bool_to_int(self.is_always_start_anchored) << 2);
        E::write_u32(bits, dst);
        Ok(nwrite)
    }

    /// Returns the number of bytes the serialized form of these flags
    /// will use.
    pub(crate) fn write_to_len(&self) -> usize {
        size_of::<u32>()
    }
}

/// An iterator over all states in a DFA.
///
/// This iterator yields a tuple for each state. The first element of the
/// tuple corresponds to a state's identifier, and the second element
/// corresponds to the state itself (comprised of its transitions).
///
/// `'a` corresponding to the lifetime of original DFA, `T` corresponds to
/// the type of the transition table itself.
pub(crate) struct StateIter<'a, T> {
    tt: &'a TransitionTable<T>,
    it: iter::Enumerate<slice::Chunks<'a, StateID>>,
}

impl<'a, T: AsRef<[u32]>> Iterator for StateIter<'a, T> {
    type Item = State<'a>;

    fn next(&mut self) -> Option<State<'a>> {
        self.it.next().map(|(index, _)| {
            let id = self.tt.to_state_id(index);
            self.tt.state(id)
        })
    }
}

/// An immutable representation of a single DFA state.
///
/// `'a` correspondings to the lifetime of a DFA's transition table.
pub(crate) struct State<'a> {
    id: StateID,
    stride2: usize,
    transitions: &'a [StateID],
}

impl<'a> State<'a> {
    /// Return an iterator over all transitions in this state. This yields
    /// a number of transitions equivalent to the alphabet length of the
    /// corresponding DFA.
    ///
    /// Each transition is represented by a tuple. The first element is
    /// the input byte for that transition and the second element is the
    /// transitions itself.
    pub(crate) fn transitions(&self) -> StateTransitionIter<'_> {
        StateTransitionIter {
            len: self.transitions.len(),
            it: self.transitions.iter().enumerate(),
        }
    }

    /// Return an iterator over a sparse representation of the transitions in
    /// this state. Only non-dead transitions are returned.
    ///
    /// The "sparse" representation in this case corresponds to a sequence of
    /// triples. The first two elements of the triple comprise an inclusive
    /// byte range while the last element corresponds to the transition taken
    /// for all bytes in the range.
    ///
    /// This is somewhat more condensed than the classical sparse
    /// representation (where you have an element for every non-dead
    /// transition), but in practice, checking if a byte is in a range is very
    /// cheap and using ranges tends to conserve quite a bit more space.
    pub(crate) fn sparse_transitions(&self) -> StateSparseTransitionIter<'_> {
        StateSparseTransitionIter { dense: self.transitions(), cur: None }
    }

    /// Returns the identifier for this state.
    pub(crate) fn id(&self) -> StateID {
        self.id
    }

    /// Analyzes this state to determine whether it can be accelerated. If so,
    /// it returns an accelerator that contains at least one byte.
    #[cfg(feature = "dfa-build")]
    fn accelerate(&self, classes: &ByteClasses) -> Option<Accel> {
        // We just try to add bytes to our accelerator. Once adding fails
        // (because we've added too many bytes), then give up.
        let mut accel = Accel::new();
        for (class, id) in self.transitions() {
            if id == self.id() {
                continue;
            }
            for unit in classes.elements(class) {
                if let Some(byte) = unit.as_u8() {
                    if !accel.add(byte) {
                        return None;
                    }
                }
            }
        }
        if accel.is_empty() {
            None
        } else {
            Some(accel)
        }
    }
}

impl<'a> fmt::Debug for State<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, (start, end, sid)) in self.sparse_transitions().enumerate() {
            let id = if f.alternate() {
                sid.as_usize()
            } else {
                sid.as_usize() >> self.stride2
            };
            if i > 0 {
                write!(f, ", ")?;
            }
            if start == end {
                write!(f, "{:?} => {:?}", start, id)?;
            } else {
                write!(f, "{:?}-{:?} => {:?}", start, end, id)?;
            }
        }
        Ok(())
    }
}

/// An iterator over all transitions in a single DFA state. This yields
/// a number of transitions equivalent to the alphabet length of the
/// corresponding DFA.
///
/// Each transition is represented by a tuple. The first element is the input
/// byte for that transition and the second element is the transition itself.
#[derive(Debug)]
pub(crate) struct StateTransitionIter<'a> {
    len: usize,
    it: iter::Enumerate<slice::Iter<'a, StateID>>,
}

impl<'a> Iterator for StateTransitionIter<'a> {
    type Item = (alphabet::Unit, StateID);

    fn next(&mut self) -> Option<(alphabet::Unit, StateID)> {
        self.it.next().map(|(i, &id)| {
            let unit = if i + 1 == self.len {
                alphabet::Unit::eoi(i)
            } else {
                let b = u8::try_from(i)
                    .expect("raw byte alphabet is never exceeded");
                alphabet::Unit::u8(b)
            };
            (unit, id)
        })
    }
}

/// An iterator over all non-DEAD transitions in a single DFA state using a
/// sparse representation.
///
/// Each transition is represented by a triple. The first two elements of the
/// triple comprise an inclusive byte range while the last element corresponds
/// to the transition taken for all bytes in the range.
///
/// As a convenience, this always returns `alphabet::Unit` values of the same
/// type. That is, you'll never get a (byte, EOI) or a (EOI, byte). Only (byte,
/// byte) and (EOI, EOI) values are yielded.
#[derive(Debug)]
pub(crate) struct StateSparseTransitionIter<'a> {
    dense: StateTransitionIter<'a>,
    cur: Option<(alphabet::Unit, alphabet::Unit, StateID)>,
}

impl<'a> Iterator for StateSparseTransitionIter<'a> {
    type Item = (alphabet::Unit, alphabet::Unit, StateID);

    fn next(&mut self) -> Option<(alphabet::Unit, alphabet::Unit, StateID)> {
        while let Some((unit, next)) = self.dense.next() {
            let (prev_start, prev_end, prev_next) = match self.cur {
                Some(t) => t,
                None => {
                    self.cur = Some((unit, unit, next));
                    continue;
                }
            };
            if prev_next == next && !unit.is_eoi() {
                self.cur = Some((prev_start, unit, prev_next));
            } else {
                self.cur = Some((unit, unit, next));
                if prev_next != DEAD {
                    return Some((prev_start, prev_end, prev_next));
                }
            }
        }
        if let Some((start, end, next)) = self.cur.take() {
            if next != DEAD {
                return Some((start, end, next));
            }
        }
        None
    }
}

/// An error that occurred during the construction of a DFA.
///
/// This error does not provide many introspection capabilities. There are
/// generally only two things you can do with it:
///
/// * Obtain a human readable message via its `std::fmt::Display` impl.
/// * Access an underlying [`nfa::thompson::BuildError`](thompson::BuildError)
/// type from its `source` method via the `std::error::Error` trait. This error
/// only occurs when using convenience routines for building a DFA directly
/// from a pattern string.
///
/// When the `std` feature is enabled, this implements the `std::error::Error`
/// trait.
#[cfg(feature = "dfa-build")]
#[derive(Clone, Debug)]
pub struct BuildError {
    kind: BuildErrorKind,
}

#[cfg(feature = "dfa-build")]
impl BuildError {
    /// Returns true if and only if this error corresponds to an error with DFA
    /// construction that occurred because of exceeding a size limit.
    ///
    /// While this can occur when size limits like [`Config::dfa_size_limit`]
    /// or [`Config::determinize_size_limit`] are exceeded, this can also occur
    /// when the number of states or patterns exceeds a hard-coded maximum.
    /// (Where these maximums are derived based on the values representable by
    /// [`StateID`] and [`PatternID`].)
    ///
    /// This predicate is useful in contexts where you want to distinguish
    /// between errors related to something provided by an end user (for
    /// example, an invalid regex pattern) and errors related to configured
    /// heuristics. For example, building a DFA might be an optimization that
    /// you want to skip if construction fails because of an exceeded size
    /// limit, but where you want to bubble up an error if it fails for some
    /// other reason.
    ///
    /// # Example
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// # if !cfg!(target_pointer_width = "64") { return Ok(()); } // see #1039
    /// use regex_automata::{dfa::{dense, Automaton}, Input};
    ///
    /// let err = dense::Builder::new()
    ///     .configure(dense::Config::new()
    ///         .determinize_size_limit(Some(100_000))
    ///     )
    ///     .build(r"\w{20}")
    ///     .unwrap_err();
    /// // This error occurs because a size limit was exceeded.
    /// // But things are otherwise valid.
    /// assert!(err.is_size_limit_exceeded());
    ///
    /// let err = dense::Builder::new()
    ///     .build(r"\bxyz\b")
    ///     .unwrap_err();
    /// // This error occurs because a Unicode word boundary
    /// // was used without enabling heuristic support for it.
    /// // So... not related to size limits.
    /// assert!(!err.is_size_limit_exceeded());
    ///
    /// let err = dense::Builder::new()
    ///     .build(r"(xyz")
    ///     .unwrap_err();
    /// // This error occurs because the pattern is invalid.
    /// // So... not related to size limits.
    /// assert!(!err.is_size_limit_exceeded());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn is_size_limit_exceeded(&self) -> bool {
        use self::BuildErrorKind::*;

        match self.kind {
            NFA(_) | Unsupported(_) => false,
            TooManyStates
            | TooManyStartStates
            | TooManyMatchPatternIDs
            | DFAExceededSizeLimit { .. }
            | DeterminizeExceededSizeLimit { .. } => true,
        }
    }
}

/// The kind of error that occurred during the construction of a DFA.
///
/// Note that this error is non-exhaustive. Adding new variants is not
/// considered a breaking change.
#[cfg(feature = "dfa-build")]
#[derive(Clone, Debug)]
enum BuildErrorKind {
    /// An error that occurred while constructing an NFA as a precursor step
    /// before a DFA is compiled.
    NFA(thompson::BuildError),
    /// An error that occurred because an unsupported regex feature was used.
    /// The message string describes which unsupported feature was used.
    ///
    /// The primary regex feature that is unsupported by DFAs is the Unicode
    /// word boundary look-around assertion (`\b`). This can be worked around
    /// by either using an ASCII word boundary (`(?-u:\b)`) or by enabling
    /// Unicode word boundaries when building a DFA.
    Unsupported(&'static str),
    /// An error that occurs if too many states are produced while building a
    /// DFA.
    TooManyStates,
    /// An error that occurs if too many start states are needed while building
    /// a DFA.
    ///
    /// This is a kind of oddball error that occurs when building a DFA with
    /// start states enabled for each pattern and enough patterns to cause
    /// the table of start states to overflow `usize`.
    TooManyStartStates,
    /// This is another oddball error that can occur if there are too many
    /// patterns spread out across too many match states.
    TooManyMatchPatternIDs,
    /// An error that occurs if the DFA got too big during determinization.
    DFAExceededSizeLimit { limit: usize },
    /// An error that occurs if auxiliary storage (not the DFA) used during
    /// determinization got too big.
    DeterminizeExceededSizeLimit { limit: usize },
}

#[cfg(feature = "dfa-build")]
impl BuildError {
    /// Return the kind of this error.
    fn kind(&self) -> &BuildErrorKind {
        &self.kind
    }

    pub(crate) fn nfa(err: thompson::BuildError) -> BuildError {
        BuildError { kind: BuildErrorKind::NFA(err) }
    }

    pub(crate) fn unsupported_dfa_word_boundary_unicode() -> BuildError {
        let msg = "cannot build DFAs for regexes with Unicode word \
                   boundaries; switch to ASCII word boundaries, or \
                   heuristically enable Unicode word boundaries or use a \
                   different regex engine";
        BuildError { kind: BuildErrorKind::Unsupported(msg) }
    }

    pub(crate) fn too_many_states() -> BuildError {
        BuildError { kind: BuildErrorKind::TooManyStates }
    }

    pub(crate) fn too_many_start_states() -> BuildError {
        BuildError { kind: BuildErrorKind::TooManyStartStates }
    }

    pub(crate) fn too_many_match_pattern_ids() -> BuildError {
        BuildError { kind: BuildErrorKind::TooManyMatchPatternIDs }
    }

    pub(crate) fn dfa_exceeded_size_limit(limit: usize) -> BuildError {
        BuildError { kind: BuildErrorKind::DFAExceededSizeLimit { limit } }
    }

    pub(crate) fn determinize_exceeded_size_limit(limit: usize) -> BuildError {
        BuildError {
            kind: BuildErrorKind::DeterminizeExceededSizeLimit { limit },
        }
    }
}

#[cfg(all(feature = "std", feature = "dfa-build"))]
impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind() {
            BuildErrorKind::NFA(ref err) => Some(err),
            _ => None,
        }
    }
}

#[cfg(feature = "dfa-build")]
impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.kind() {
            BuildErrorKind::NFA(_) => write!(f, "error building NFA"),
            BuildErrorKind::Unsupported(ref msg) => {
                write!(f, "unsupported regex feature for DFAs: {}", msg)
            }
            BuildErrorKind::TooManyStates => write!(
                f,
                "number of DFA states exceeds limit of {}",
                StateID::LIMIT,
            ),
            BuildErrorKind::TooManyStartStates => {
                let stride = Start::len();
                // The start table has `stride` entries for starting states for
                // the entire DFA, and then `stride` entries for each pattern
                // if start states for each pattern are enabled (which is the
                // only way this error can occur). Thus, the total number of
                // patterns that can fit in the table is `stride` less than
                // what we can allocate.
                let max = usize::try_from(core::isize::MAX).unwrap();
                let limit = (max - stride) / stride;
                write!(
                    f,
                    "compiling DFA with start states exceeds pattern \
                     pattern limit of {}",
                    limit,
                )
            }
            BuildErrorKind::TooManyMatchPatternIDs => write!(
                f,
                "compiling DFA with total patterns in all match states \
                 exceeds limit of {}",
                PatternID::LIMIT,
            ),
            BuildErrorKind::DFAExceededSizeLimit { limit } => write!(
                f,
                "DFA exceeded size limit of {:?} during determinization",
                limit,
            ),
            BuildErrorKind::DeterminizeExceededSizeLimit { limit } => {
                write!(f, "determinization exceeded size limit of {:?}", limit)
            }
        }
    }
}

#[cfg(all(test, feature = "syntax", feature = "dfa-build"))]
mod tests {
    use crate::{Input, MatchError};

    use super::*;

    #[test]
    fn errors_with_unicode_word_boundary() {
        let pattern = r"\b";
        assert!(Builder::new().build(pattern).is_err());
    }

    #[test]
    fn roundtrip_never_match() {
        let dfa = DFA::never_match().unwrap();
        let (buf, _) = dfa.to_bytes_native_endian();
        let dfa: DFA<&[u32]> = DFA::from_bytes(&buf).unwrap().0;

        assert_eq!(None, dfa.try_search_fwd(&Input::new("foo12345")).unwrap());
    }

    #[test]
    fn roundtrip_always_match() {
        use crate::HalfMatch;

        let dfa = DFA::always_match().unwrap();
        let (buf, _) = dfa.to_bytes_native_endian();
        let dfa: DFA<&[u32]> = DFA::from_bytes(&buf).unwrap().0;

        assert_eq!(
            Some(HalfMatch::must(0, 0)),
            dfa.try_search_fwd(&Input::new("foo12345")).unwrap()
        );
    }

    // See the analogous test in src/hybrid/dfa.rs.
    #[test]
    fn heuristic_unicode_reverse() {
        let dfa = DFA::builder()
            .configure(DFA::config().unicode_word_boundary(true))
            .thompson(thompson::Config::new().reverse(true))
            .build(r"\b[0-9]+\b")
            .unwrap();

        let input = Input::new("β123").range(2..);
        let expected = MatchError::quit(0xB2, 1);
        let got = dfa.try_search_rev(&input);
        assert_eq!(Err(expected), got);

        let input = Input::new("123β").range(..3);
        let expected = MatchError::quit(0xCE, 3);
        let got = dfa.try_search_rev(&input);
        assert_eq!(Err(expected), got);
    }
}
