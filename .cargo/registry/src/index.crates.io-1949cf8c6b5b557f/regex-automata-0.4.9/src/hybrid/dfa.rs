/*!
Types and routines specific to lazy DFAs.

This module is the home of [`hybrid::dfa::DFA`](DFA).

This module also contains a [`hybrid::dfa::Builder`](Builder) and a
[`hybrid::dfa::Config`](Config) for configuring and building a lazy DFA.
*/

use core::{iter, mem::size_of};

use alloc::vec::Vec;

use crate::{
    hybrid::{
        error::{BuildError, CacheError, StartError},
        id::{LazyStateID, LazyStateIDError},
        search,
    },
    nfa::thompson,
    util::{
        alphabet::{self, ByteClasses, ByteSet},
        determinize::{self, State, StateBuilderEmpty, StateBuilderNFA},
        empty,
        prefilter::Prefilter,
        primitives::{PatternID, StateID as NFAStateID},
        search::{
            Anchored, HalfMatch, Input, MatchError, MatchKind, PatternSet,
        },
        sparse_set::SparseSets,
        start::{self, Start, StartByteMap},
    },
};

/// The minimum number of states that a lazy DFA's cache size must support.
///
/// This is checked at time of construction to ensure that at least some small
/// number of states can fit in the given capacity allotment. If we can't fit
/// at least this number of states, then the thinking is that it's pretty
/// senseless to use the lazy DFA. More to the point, parts of the code do
/// assume that the cache can fit at least some small number of states.
const MIN_STATES: usize = SENTINEL_STATES + 2;

/// The number of "sentinel" states that get added to every lazy DFA.
///
/// These are special states indicating status conditions of a search: unknown,
/// dead and quit. These states in particular also use zero NFA states, so
/// their memory usage is quite small. This is relevant for computing the
/// minimum memory needed for a lazy DFA cache.
const SENTINEL_STATES: usize = 3;

/// A hybrid NFA/DFA (also called a "lazy DFA") for regex searching.
///
/// A lazy DFA is a DFA that builds itself at search time. It otherwise has
/// very similar characteristics as a [`dense::DFA`](crate::dfa::dense::DFA).
/// Indeed, both support precisely the same regex features with precisely the
/// same semantics.
///
/// Where as a `dense::DFA` must be completely built to handle any input before
/// it may be used for search, a lazy DFA starts off effectively empty. During
/// a search, a lazy DFA will build itself depending on whether it has already
/// computed the next transition or not. If it has, then it looks a lot like
/// a `dense::DFA` internally: it does a very fast table based access to find
/// the next transition. Otherwise, if the state hasn't been computed, then it
/// does determinization _for that specific transition_ to compute the next DFA
/// state.
///
/// The main selling point of a lazy DFA is that, in practice, it has
/// the performance profile of a `dense::DFA` without the weakness of it
/// taking worst case exponential time to build. Indeed, for each byte of
/// input, the lazy DFA will construct as most one new DFA state. Thus, a
/// lazy DFA achieves worst case `O(mn)` time for regex search (where `m ~
/// pattern.len()` and `n ~ haystack.len()`).
///
/// The main downsides of a lazy DFA are:
///
/// 1. It requires mutable "cache" space during search. This is where the
/// transition table, among other things, is stored.
/// 2. In pathological cases (e.g., if the cache is too small), it will run
/// out of room and either require a bigger cache capacity or will repeatedly
/// clear the cache and thus repeatedly regenerate DFA states. Overall, this
/// will tend to be slower than a typical NFA simulation.
///
/// # Capabilities
///
/// Like a `dense::DFA`, a single lazy DFA fundamentally supports the following
/// operations:
///
/// 1. Detection of a match.
/// 2. Location of the end of a match.
/// 3. In the case of a lazy DFA with multiple patterns, which pattern matched
/// is reported as well.
///
/// A notable absence from the above list of capabilities is the location of
/// the *start* of a match. In order to provide both the start and end of
/// a match, *two* lazy DFAs are required. This functionality is provided by a
/// [`Regex`](crate::hybrid::regex::Regex).
///
/// # Example
///
/// This shows how to build a lazy DFA with the default configuration and
/// execute a search. Notice how, in contrast to a `dense::DFA`, we must create
/// a cache and pass it to our search routine.
///
/// ```
/// use regex_automata::{hybrid::dfa::DFA, HalfMatch, Input};
///
/// let dfa = DFA::new("foo[0-9]+")?;
/// let mut cache = dfa.create_cache();
///
/// let expected = Some(HalfMatch::must(0, 8));
/// assert_eq!(expected, dfa.try_search_fwd(
///     &mut cache, &Input::new("foo12345"))?,
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct DFA {
    config: Config,
    nfa: thompson::NFA,
    stride2: usize,
    start_map: StartByteMap,
    classes: ByteClasses,
    quitset: ByteSet,
    cache_capacity: usize,
}

impl DFA {
    /// Parse the given regular expression using a default configuration and
    /// return the corresponding lazy DFA.
    ///
    /// If you want a non-default configuration, then use the [`Builder`] to
    /// set your own configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{hybrid::dfa::DFA, HalfMatch, Input};
    ///
    /// let dfa = DFA::new("foo[0-9]+bar")?;
    /// let mut cache = dfa.create_cache();
    ///
    /// let expected = HalfMatch::must(0, 11);
    /// assert_eq!(
    ///     Some(expected),
    ///     dfa.try_search_fwd(&mut cache, &Input::new("foo12345bar"))?,
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "syntax")]
    pub fn new(pattern: &str) -> Result<DFA, BuildError> {
        DFA::builder().build(pattern)
    }

    /// Parse the given regular expressions using a default configuration and
    /// return the corresponding lazy multi-DFA.
    ///
    /// If you want a non-default configuration, then use the [`Builder`] to
    /// set your own configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{hybrid::dfa::DFA, HalfMatch, Input};
    ///
    /// let dfa = DFA::new_many(&["[0-9]+", "[a-z]+"])?;
    /// let mut cache = dfa.create_cache();
    ///
    /// let expected = HalfMatch::must(1, 3);
    /// assert_eq!(
    ///     Some(expected),
    ///     dfa.try_search_fwd(&mut cache, &Input::new("foo12345bar"))?,
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "syntax")]
    pub fn new_many<P: AsRef<str>>(patterns: &[P]) -> Result<DFA, BuildError> {
        DFA::builder().build_many(patterns)
    }

    /// Create a new lazy DFA that matches every input.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{hybrid::dfa::DFA, HalfMatch, Input};
    ///
    /// let dfa = DFA::always_match()?;
    /// let mut cache = dfa.create_cache();
    ///
    /// let expected = HalfMatch::must(0, 0);
    /// assert_eq!(Some(expected), dfa.try_search_fwd(
    ///     &mut cache, &Input::new(""))?,
    /// );
    /// assert_eq!(Some(expected), dfa.try_search_fwd(
    ///     &mut cache, &Input::new("foo"))?,
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn always_match() -> Result<DFA, BuildError> {
        let nfa = thompson::NFA::always_match();
        Builder::new().build_from_nfa(nfa)
    }

    /// Create a new lazy DFA that never matches any input.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{hybrid::dfa::DFA, Input};
    ///
    /// let dfa = DFA::never_match()?;
    /// let mut cache = dfa.create_cache();
    ///
    /// assert_eq!(None, dfa.try_search_fwd(&mut cache, &Input::new(""))?);
    /// assert_eq!(None, dfa.try_search_fwd(&mut cache, &Input::new("foo"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn never_match() -> Result<DFA, BuildError> {
        let nfa = thompson::NFA::never_match();
        Builder::new().build_from_nfa(nfa)
    }

    /// Return a default configuration for a `DFA`.
    ///
    /// This is a convenience routine to avoid needing to import the [`Config`]
    /// type when customizing the construction of a lazy DFA.
    ///
    /// # Example
    ///
    /// This example shows how to build a lazy DFA that heuristically supports
    /// Unicode word boundaries.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{hybrid::dfa::DFA, HalfMatch, MatchError, Input};
    ///
    /// let re = DFA::builder()
    ///     .configure(DFA::config().unicode_word_boundary(true))
    ///     .build(r"\b\w+\b")?;
    /// let mut cache = re.create_cache();
    ///
    /// // Since our haystack is all ASCII, the DFA search sees then and knows
    /// // it is legal to interpret Unicode word boundaries as ASCII word
    /// // boundaries.
    /// let input = Input::new("!!foo!!");
    /// let expected = HalfMatch::must(0, 5);
    /// assert_eq!(Some(expected), re.try_search_fwd(&mut cache, &input)?);
    ///
    /// // But if our haystack contains non-ASCII, then the search will fail
    /// // with an error.
    /// let input = Input::new("!!βββ!!");
    /// let expected = MatchError::quit(b'\xCE', 2);
    /// assert_eq!(Err(expected), re.try_search_fwd(&mut cache, &input));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn config() -> Config {
        Config::new()
    }

    /// Return a builder for configuring the construction of a `Regex`.
    ///
    /// This is a convenience routine to avoid needing to import the
    /// [`Builder`] type in common cases.
    ///
    /// # Example
    ///
    /// This example shows how to use the builder to disable UTF-8 mode
    /// everywhere for lazy DFAs.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{hybrid::dfa::DFA, util::syntax, HalfMatch, Input};
    ///
    /// let re = DFA::builder()
    ///     .syntax(syntax::Config::new().utf8(false))
    ///     .build(r"foo(?-u:[^b])ar.*")?;
    /// let mut cache = re.create_cache();
    ///
    /// let input = Input::new(b"\xFEfoo\xFFarzz\xE2\x98\xFF\n");
    /// let expected = Some(HalfMatch::must(0, 9));
    /// let got = re.try_search_fwd(&mut cache, &input)?;
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// Create a new cache for this lazy DFA.
    ///
    /// The cache returned should only be used for searches for this
    /// lazy DFA. If you want to reuse the cache for another DFA, then
    /// you must call [`Cache::reset`] with that DFA (or, equivalently,
    /// [`DFA::reset_cache`]).
    pub fn create_cache(&self) -> Cache {
        Cache::new(self)
    }

    /// Reset the given cache such that it can be used for searching with the
    /// this lazy DFA (and only this DFA).
    ///
    /// A cache reset permits reusing memory already allocated in this cache
    /// with a different lazy DFA.
    ///
    /// Resetting a cache sets its "clear count" to 0. This is relevant if the
    /// lazy DFA has been configured to "give up" after it has cleared the
    /// cache a certain number of times.
    ///
    /// Any lazy state ID generated by the cache prior to resetting it is
    /// invalid after the reset.
    ///
    /// # Example
    ///
    /// This shows how to re-purpose a cache for use with a different DFA.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{hybrid::dfa::DFA, HalfMatch, Input};
    ///
    /// let dfa1 = DFA::new(r"\w")?;
    /// let dfa2 = DFA::new(r"\W")?;
    ///
    /// let mut cache = dfa1.create_cache();
    /// assert_eq!(
    ///     Some(HalfMatch::must(0, 2)),
    ///     dfa1.try_search_fwd(&mut cache, &Input::new("Δ"))?,
    /// );
    ///
    /// // Using 'cache' with dfa2 is not allowed. It may result in panics or
    /// // incorrect results. In order to re-purpose the cache, we must reset
    /// // it with the DFA we'd like to use it with.
    /// //
    /// // Similarly, after this reset, using the cache with 'dfa1' is also not
    /// // allowed.
    /// dfa2.reset_cache(&mut cache);
    /// assert_eq!(
    ///     Some(HalfMatch::must(0, 3)),
    ///     dfa2.try_search_fwd(&mut cache, &Input::new("☃"))?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn reset_cache(&self, cache: &mut Cache) {
        Lazy::new(self, cache).reset_cache()
    }

    /// Returns the total number of patterns compiled into this lazy DFA.
    ///
    /// In the case of a DFA that contains no patterns, this returns `0`.
    ///
    /// # Example
    ///
    /// This example shows the pattern length for a DFA that never matches:
    ///
    /// ```
    /// use regex_automata::hybrid::dfa::DFA;
    ///
    /// let dfa = DFA::never_match()?;
    /// assert_eq!(dfa.pattern_len(), 0);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// And another example for a DFA that matches at every position:
    ///
    /// ```
    /// use regex_automata::hybrid::dfa::DFA;
    ///
    /// let dfa = DFA::always_match()?;
    /// assert_eq!(dfa.pattern_len(), 1);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// And finally, a DFA that was constructed from multiple patterns:
    ///
    /// ```
    /// use regex_automata::hybrid::dfa::DFA;
    ///
    /// let dfa = DFA::new_many(&["[0-9]+", "[a-z]+", "[A-Z]+"])?;
    /// assert_eq!(dfa.pattern_len(), 3);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn pattern_len(&self) -> usize {
        self.nfa.pattern_len()
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
        &self.classes
    }

    /// Returns this lazy DFA's configuration.
    pub fn get_config(&self) -> &Config {
        &self.config
    }

    /// Returns a reference to the underlying NFA.
    pub fn get_nfa(&self) -> &thompson::NFA {
        &self.nfa
    }

    /// Returns the stride, as a base-2 exponent, required for these
    /// equivalence classes.
    ///
    /// The stride is always the smallest power of 2 that is greater than or
    /// equal to the alphabet length. This is done so that converting between
    /// state IDs and indices can be done with shifts alone, which is much
    /// faster than integer division.
    fn stride2(&self) -> usize {
        self.stride2
    }

    /// Returns the total stride for every state in this lazy DFA. This
    /// corresponds to the total number of transitions used by each state in
    /// this DFA's transition table.
    fn stride(&self) -> usize {
        1 << self.stride2()
    }

    /// Returns the memory usage, in bytes, of this lazy DFA.
    ///
    /// This does **not** include the stack size used up by this lazy DFA. To
    /// compute that, use `std::mem::size_of::<DFA>()`. This also does not
    /// include the size of the `Cache` used.
    ///
    /// This also does not include any heap memory used by the NFA inside of
    /// this hybrid NFA/DFA. This is because the NFA's ownership is shared, and
    /// thus not owned by this hybrid NFA/DFA. More practically, several regex
    /// engines in this crate embed an NFA, and reporting the NFA's memory
    /// usage in all of them would likely result in reporting higher heap
    /// memory than is actually used.
    pub fn memory_usage(&self) -> usize {
        // The only thing that uses heap memory in a DFA is the NFA. But the
        // NFA has shared ownership, so reporting its memory as part of the
        // hybrid DFA is likely to lead to double-counting the NFA memory
        // somehow. In particular, this DFA does not really own an NFA, so
        // including it in the DFA's memory usage doesn't seem semantically
        // correct.
        0
    }
}

impl DFA {
    /// Executes a forward search and returns the end position of the leftmost
    /// match that is found. If no match exists, then `None` is returned.
    ///
    /// In particular, this method continues searching even after it enters
    /// a match state. The search only terminates once it has reached the
    /// end of the input or when it has entered a dead or quit state. Upon
    /// termination, the position of the last byte seen while still in a match
    /// state is returned.
    ///
    /// # Errors
    ///
    /// This routine errors if the search could not complete. This can occur
    /// in a number of circumstances:
    ///
    /// * The configuration of the lazy DFA may permit it to "quit" the search.
    /// For example, setting quit bytes or enabling heuristic support for
    /// Unicode word boundaries. The default configuration does not enable any
    /// option that could result in the lazy DFA quitting.
    /// * The configuration of the lazy DFA may also permit it to "give up"
    /// on a search if it makes ineffective use of its transition table
    /// cache. The default configuration does not enable this by default,
    /// although it is typically a good idea to.
    /// * When the provided `Input` configuration is not supported. For
    /// example, by providing an unsupported anchor mode.
    ///
    /// When a search returns an error, callers cannot know whether a match
    /// exists or not.
    ///
    /// # Example
    ///
    /// This example shows how to run a basic search.
    ///
    /// ```
    /// use regex_automata::{hybrid::dfa::DFA, HalfMatch, Input};
    ///
    /// let dfa = DFA::new("foo[0-9]+")?;
    /// let mut cache = dfa.create_cache();
    /// let expected = HalfMatch::must(0, 8);
    /// assert_eq!(Some(expected), dfa.try_search_fwd(
    ///     &mut cache, &Input::new("foo12345"))?,
    /// );
    ///
    /// // Even though a match is found after reading the first byte (`a`),
    /// // the leftmost first match semantics demand that we find the earliest
    /// // match that prefers earlier parts of the pattern over later parts.
    /// let dfa = DFA::new("abc|a")?;
    /// let mut cache = dfa.create_cache();
    /// let expected = HalfMatch::must(0, 3);
    /// assert_eq!(Some(expected), dfa.try_search_fwd(
    ///     &mut cache, &Input::new("abc"))?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: specific pattern search
    ///
    /// This example shows how to build a lazy multi-DFA that permits searching
    /// for specific patterns.
    ///
    /// ```
    /// use regex_automata::{
    ///     hybrid::dfa::DFA,
    ///     Anchored, HalfMatch, PatternID, Input,
    /// };
    ///
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().starts_for_each_pattern(true))
    ///     .build_many(&["[a-z0-9]{6}", "[a-z][a-z0-9]{5}"])?;
    /// let mut cache = dfa.create_cache();
    /// let haystack = "foo123";
    ///
    /// // Since we are using the default leftmost-first match and both
    /// // patterns match at the same starting position, only the first pattern
    /// // will be returned in this case when doing a search for any of the
    /// // patterns.
    /// let expected = Some(HalfMatch::must(0, 6));
    /// let got = dfa.try_search_fwd(&mut cache, &Input::new(haystack))?;
    /// assert_eq!(expected, got);
    ///
    /// // But if we want to check whether some other pattern matches, then we
    /// // can provide its pattern ID.
    /// let expected = Some(HalfMatch::must(1, 6));
    /// let input = Input::new(haystack)
    ///     .anchored(Anchored::Pattern(PatternID::must(1)));
    /// let got = dfa.try_search_fwd(&mut cache, &input)?;
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: specifying the bounds of a search
    ///
    /// This example shows how providing the bounds of a search can produce
    /// different results than simply sub-slicing the haystack.
    ///
    /// ```
    /// use regex_automata::{hybrid::dfa::DFA, HalfMatch, Input};
    ///
    /// // N.B. We disable Unicode here so that we use a simple ASCII word
    /// // boundary. Alternatively, we could enable heuristic support for
    /// // Unicode word boundaries since our haystack is pure ASCII.
    /// let dfa = DFA::new(r"(?-u)\b[0-9]{3}\b")?;
    /// let mut cache = dfa.create_cache();
    /// let haystack = "foo123bar";
    ///
    /// // Since we sub-slice the haystack, the search doesn't know about the
    /// // larger context and assumes that `123` is surrounded by word
    /// // boundaries. And of course, the match position is reported relative
    /// // to the sub-slice as well, which means we get `3` instead of `6`.
    /// let expected = Some(HalfMatch::must(0, 3));
    /// let got = dfa.try_search_fwd(
    ///     &mut cache,
    ///     &Input::new(&haystack[3..6]),
    /// )?;
    /// assert_eq!(expected, got);
    ///
    /// // But if we provide the bounds of the search within the context of the
    /// // entire haystack, then the search can take the surrounding context
    /// // into account. (And if we did find a match, it would be reported
    /// // as a valid offset into `haystack` instead of its sub-slice.)
    /// let expected = None;
    /// let got = dfa.try_search_fwd(
    ///     &mut cache,
    ///     &Input::new(haystack).range(3..6),
    /// )?;
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn try_search_fwd(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Result<Option<HalfMatch>, MatchError> {
        let utf8empty = self.get_nfa().has_empty() && self.get_nfa().is_utf8();
        let hm = match search::find_fwd(self, cache, input)? {
            None => return Ok(None),
            Some(hm) if !utf8empty => return Ok(Some(hm)),
            Some(hm) => hm,
        };
        // We get to this point when we know our DFA can match the empty string
        // AND when UTF-8 mode is enabled. In this case, we skip any matches
        // whose offset splits a codepoint. Such a match is necessarily a
        // zero-width match, because UTF-8 mode requires the underlying NFA
        // to be built such that all non-empty matches span valid UTF-8.
        // Therefore, any match that ends in the middle of a codepoint cannot
        // be part of a span of valid UTF-8 and thus must be an empty match.
        // In such cases, we skip it, so as not to report matches that split a
        // codepoint.
        //
        // Note that this is not a checked assumption. Callers *can* provide an
        // NFA with UTF-8 mode enabled but produces non-empty matches that span
        // invalid UTF-8. But doing so is documented to result in unspecified
        // behavior.
        empty::skip_splits_fwd(input, hm, hm.offset(), |input| {
            let got = search::find_fwd(self, cache, input)?;
            Ok(got.map(|hm| (hm, hm.offset())))
        })
    }

    /// Executes a reverse search and returns the start of the position of the
    /// leftmost match that is found. If no match exists, then `None` is
    /// returned.
    ///
    /// # Errors
    ///
    /// This routine errors if the search could not complete. This can occur
    /// in a number of circumstances:
    ///
    /// * The configuration of the lazy DFA may permit it to "quit" the search.
    /// For example, setting quit bytes or enabling heuristic support for
    /// Unicode word boundaries. The default configuration does not enable any
    /// option that could result in the lazy DFA quitting.
    /// * The configuration of the lazy DFA may also permit it to "give up"
    /// on a search if it makes ineffective use of its transition table
    /// cache. The default configuration does not enable this by default,
    /// although it is typically a good idea to.
    /// * When the provided `Input` configuration is not supported. For
    /// example, by providing an unsupported anchor mode.
    ///
    /// When a search returns an error, callers cannot know whether a match
    /// exists or not.
    ///
    /// # Example
    ///
    /// This routine is principally useful when used in
    /// conjunction with the
    /// [`nfa::thompson::Config::reverse`](crate::nfa::thompson::Config::reverse)
    /// configuration. In general, it's unlikely to be correct to use both
    /// `try_search_fwd` and `try_search_rev` with the same DFA since any
    /// particular DFA will only support searching in one direction with
    /// respect to the pattern.
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson,
    ///     hybrid::dfa::DFA,
    ///     HalfMatch, Input,
    /// };
    ///
    /// let dfa = DFA::builder()
    ///     .thompson(thompson::Config::new().reverse(true))
    ///     .build("foo[0-9]+")?;
    /// let mut cache = dfa.create_cache();
    /// let expected = HalfMatch::must(0, 0);
    /// assert_eq!(
    ///     Some(expected),
    ///     dfa.try_search_rev(&mut cache, &Input::new("foo12345"))?,
    /// );
    ///
    /// // Even though a match is found after reading the last byte (`c`),
    /// // the leftmost first match semantics demand that we find the earliest
    /// // match that prefers earlier parts of the pattern over latter parts.
    /// let dfa = DFA::builder()
    ///     .thompson(thompson::Config::new().reverse(true))
    ///     .build("abc|c")?;
    /// let mut cache = dfa.create_cache();
    /// let expected = HalfMatch::must(0, 0);
    /// assert_eq!(Some(expected), dfa.try_search_rev(
    ///     &mut cache, &Input::new("abc"))?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: UTF-8 mode
    ///
    /// This examples demonstrates that UTF-8 mode applies to reverse
    /// DFAs. When UTF-8 mode is enabled in the underlying NFA, then all
    /// matches reported must correspond to valid UTF-8 spans. This includes
    /// prohibiting zero-width matches that split a codepoint.
    ///
    /// UTF-8 mode is enabled by default. Notice below how the only zero-width
    /// matches reported are those at UTF-8 boundaries:
    ///
    /// ```
    /// use regex_automata::{
    ///     hybrid::dfa::DFA,
    ///     nfa::thompson,
    ///     HalfMatch, Input, MatchKind,
    /// };
    ///
    /// let dfa = DFA::builder()
    ///     .thompson(thompson::Config::new().reverse(true))
    ///     .build(r"")?;
    /// let mut cache = dfa.create_cache();
    ///
    /// // Run the reverse DFA to collect all matches.
    /// let mut input = Input::new("☃");
    /// let mut matches = vec![];
    /// loop {
    ///     match dfa.try_search_rev(&mut cache, &input)? {
    ///         None => break,
    ///         Some(hm) => {
    ///             matches.push(hm);
    ///             if hm.offset() == 0 || input.end() == 0 {
    ///                 break;
    ///             } else if hm.offset() < input.end() {
    ///                 input.set_end(hm.offset());
    ///             } else {
    ///                 // This is only necessary to handle zero-width
    ///                 // matches, which of course occur in this example.
    ///                 // Without this, the search would never advance
    ///                 // backwards beyond the initial match.
    ///                 input.set_end(input.end() - 1);
    ///             }
    ///         }
    ///     }
    /// }
    ///
    /// // No matches split a codepoint.
    /// let expected = vec![
    ///     HalfMatch::must(0, 3),
    ///     HalfMatch::must(0, 0),
    /// ];
    /// assert_eq!(expected, matches);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Now let's look at the same example, but with UTF-8 mode on the
    /// underlying NFA disabled:
    ///
    /// ```
    /// use regex_automata::{
    ///     hybrid::dfa::DFA,
    ///     nfa::thompson,
    ///     HalfMatch, Input, MatchKind,
    /// };
    ///
    /// let dfa = DFA::builder()
    ///     .thompson(thompson::Config::new().reverse(true).utf8(false))
    ///     .build(r"")?;
    /// let mut cache = dfa.create_cache();
    ///
    /// // Run the reverse DFA to collect all matches.
    /// let mut input = Input::new("☃");
    /// let mut matches = vec![];
    /// loop {
    ///     match dfa.try_search_rev(&mut cache, &input)? {
    ///         None => break,
    ///         Some(hm) => {
    ///             matches.push(hm);
    ///             if hm.offset() == 0 || input.end() == 0 {
    ///                 break;
    ///             } else if hm.offset() < input.end() {
    ///                 input.set_end(hm.offset());
    ///             } else {
    ///                 // This is only necessary to handle zero-width
    ///                 // matches, which of course occur in this example.
    ///                 // Without this, the search would never advance
    ///                 // backwards beyond the initial match.
    ///                 input.set_end(input.end() - 1);
    ///             }
    ///         }
    ///     }
    /// }
    ///
    /// // No matches split a codepoint.
    /// let expected = vec![
    ///     HalfMatch::must(0, 3),
    ///     HalfMatch::must(0, 2),
    ///     HalfMatch::must(0, 1),
    ///     HalfMatch::must(0, 0),
    /// ];
    /// assert_eq!(expected, matches);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn try_search_rev(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Result<Option<HalfMatch>, MatchError> {
        let utf8empty = self.get_nfa().has_empty() && self.get_nfa().is_utf8();
        let hm = match search::find_rev(self, cache, input)? {
            None => return Ok(None),
            Some(hm) if !utf8empty => return Ok(Some(hm)),
            Some(hm) => hm,
        };
        empty::skip_splits_rev(input, hm, hm.offset(), |input| {
            let got = search::find_rev(self, cache, input)?;
            Ok(got.map(|hm| (hm, hm.offset())))
        })
    }

    /// Executes an overlapping forward search and returns the end position of
    /// matches as they are found. If no match exists, then `None` is returned.
    ///
    /// This routine is principally only useful when searching for multiple
    /// patterns on inputs where multiple patterns may match the same regions
    /// of text. In particular, callers must preserve the automaton's search
    /// state from prior calls so that the implementation knows where the last
    /// match occurred.
    ///
    /// When using this routine to implement an iterator of overlapping
    /// matches, the `start` of the search should remain invariant throughout
    /// iteration. The `OverlappingState` given to the search will keep track
    /// of the current position of the search. (This is because multiple
    /// matches may be reported at the same position, so only the search
    /// implementation itself knows when to advance the position.)
    ///
    /// If for some reason you want the search to forget about its previous
    /// state and restart the search at a particular position, then setting the
    /// state to [`OverlappingState::start`] will accomplish that.
    ///
    /// # Errors
    ///
    /// This routine errors if the search could not complete. This can occur
    /// in a number of circumstances:
    ///
    /// * The configuration of the lazy DFA may permit it to "quit" the search.
    /// For example, setting quit bytes or enabling heuristic support for
    /// Unicode word boundaries. The default configuration does not enable any
    /// option that could result in the lazy DFA quitting.
    /// * The configuration of the lazy DFA may also permit it to "give up"
    /// on a search if it makes ineffective use of its transition table
    /// cache. The default configuration does not enable this by default,
    /// although it is typically a good idea to.
    /// * When the provided `Input` configuration is not supported. For
    /// example, by providing an unsupported anchor mode.
    ///
    /// When a search returns an error, callers cannot know whether a match
    /// exists or not.
    ///
    /// # Example
    ///
    /// This example shows how to run a basic overlapping search. Notice
    /// that we build the automaton with a `MatchKind::All` configuration.
    /// Overlapping searches are unlikely to work as one would expect when
    /// using the default `MatchKind::LeftmostFirst` match semantics, since
    /// leftmost-first matching is fundamentally incompatible with overlapping
    /// searches. Namely, overlapping searches need to report matches as they
    /// are seen, where as leftmost-first searches will continue searching even
    /// after a match has been observed in order to find the conventional end
    /// position of the match. More concretely, leftmost-first searches use
    /// dead states to terminate a search after a specific match can no longer
    /// be extended. Overlapping searches instead do the opposite by continuing
    /// the search to find totally new matches (potentially of other patterns).
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{
    ///     hybrid::dfa::{DFA, OverlappingState},
    ///     HalfMatch, Input, MatchKind,
    /// };
    ///
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().match_kind(MatchKind::All))
    ///     .build_many(&[r"\w+$", r"\S+$"])?;
    /// let mut cache = dfa.create_cache();
    ///
    /// let haystack = "@foo";
    /// let mut state = OverlappingState::start();
    ///
    /// let expected = Some(HalfMatch::must(1, 4));
    /// dfa.try_search_overlapping_fwd(
    ///     &mut cache, &Input::new(haystack), &mut state,
    /// )?;
    /// assert_eq!(expected, state.get_match());
    ///
    /// // The first pattern also matches at the same position, so re-running
    /// // the search will yield another match. Notice also that the first
    /// // pattern is returned after the second. This is because the second
    /// // pattern begins its match before the first, is therefore an earlier
    /// // match and is thus reported first.
    /// let expected = Some(HalfMatch::must(0, 4));
    /// dfa.try_search_overlapping_fwd(
    ///     &mut cache, &Input::new(haystack), &mut state,
    /// )?;
    /// assert_eq!(expected, state.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn try_search_overlapping_fwd(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        let utf8empty = self.get_nfa().has_empty() && self.get_nfa().is_utf8();
        search::find_overlapping_fwd(self, cache, input, state)?;
        match state.get_match() {
            None => Ok(()),
            Some(_) if !utf8empty => Ok(()),
            Some(_) => skip_empty_utf8_splits_overlapping(
                input,
                state,
                |input, state| {
                    search::find_overlapping_fwd(self, cache, input, state)
                },
            ),
        }
    }

    /// Executes a reverse overlapping search and returns the start of the
    /// position of the leftmost match that is found. If no match exists, then
    /// `None` is returned.
    ///
    /// When using this routine to implement an iterator of overlapping
    /// matches, the `start` of the search should remain invariant throughout
    /// iteration. The `OverlappingState` given to the search will keep track
    /// of the current position of the search. (This is because multiple
    /// matches may be reported at the same position, so only the search
    /// implementation itself knows when to advance the position.)
    ///
    /// If for some reason you want the search to forget about its previous
    /// state and restart the search at a particular position, then setting the
    /// state to [`OverlappingState::start`] will accomplish that.
    ///
    /// # Errors
    ///
    /// This routine errors if the search could not complete. This can occur
    /// in a number of circumstances:
    ///
    /// * The configuration of the lazy DFA may permit it to "quit" the search.
    /// For example, setting quit bytes or enabling heuristic support for
    /// Unicode word boundaries. The default configuration does not enable any
    /// option that could result in the lazy DFA quitting.
    /// * The configuration of the lazy DFA may also permit it to "give up"
    /// on a search if it makes ineffective use of its transition table
    /// cache. The default configuration does not enable this by default,
    /// although it is typically a good idea to.
    /// * When the provided `Input` configuration is not supported. For
    /// example, by providing an unsupported anchor mode.
    ///
    /// When a search returns an error, callers cannot know whether a match
    /// exists or not.
    ///
    /// # Example: UTF-8 mode
    ///
    /// This examples demonstrates that UTF-8 mode applies to reverse
    /// DFAs. When UTF-8 mode is enabled in the underlying NFA, then all
    /// matches reported must correspond to valid UTF-8 spans. This includes
    /// prohibiting zero-width matches that split a codepoint.
    ///
    /// UTF-8 mode is enabled by default. Notice below how the only zero-width
    /// matches reported are those at UTF-8 boundaries:
    ///
    /// ```
    /// use regex_automata::{
    ///     hybrid::dfa::{DFA, OverlappingState},
    ///     nfa::thompson,
    ///     HalfMatch, Input, MatchKind,
    /// };
    ///
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().match_kind(MatchKind::All))
    ///     .thompson(thompson::Config::new().reverse(true))
    ///     .build_many(&[r"", r"☃"])?;
    /// let mut cache = dfa.create_cache();
    ///
    /// // Run the reverse DFA to collect all matches.
    /// let input = Input::new("☃");
    /// let mut state = OverlappingState::start();
    /// let mut matches = vec![];
    /// loop {
    ///     dfa.try_search_overlapping_rev(&mut cache, &input, &mut state)?;
    ///     match state.get_match() {
    ///         None => break,
    ///         Some(hm) => matches.push(hm),
    ///     }
    /// }
    ///
    /// // No matches split a codepoint.
    /// let expected = vec![
    ///     HalfMatch::must(0, 3),
    ///     HalfMatch::must(1, 0),
    ///     HalfMatch::must(0, 0),
    /// ];
    /// assert_eq!(expected, matches);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Now let's look at the same example, but with UTF-8 mode on the
    /// underlying NFA disabled:
    ///
    /// ```
    /// use regex_automata::{
    ///     hybrid::dfa::{DFA, OverlappingState},
    ///     nfa::thompson,
    ///     HalfMatch, Input, MatchKind,
    /// };
    ///
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().match_kind(MatchKind::All))
    ///     .thompson(thompson::Config::new().reverse(true).utf8(false))
    ///     .build_many(&[r"", r"☃"])?;
    /// let mut cache = dfa.create_cache();
    ///
    /// // Run the reverse DFA to collect all matches.
    /// let input = Input::new("☃");
    /// let mut state = OverlappingState::start();
    /// let mut matches = vec![];
    /// loop {
    ///     dfa.try_search_overlapping_rev(&mut cache, &input, &mut state)?;
    ///     match state.get_match() {
    ///         None => break,
    ///         Some(hm) => matches.push(hm),
    ///     }
    /// }
    ///
    /// // Now *all* positions match, even within a codepoint,
    /// // because we lifted the requirement that matches
    /// // correspond to valid UTF-8 spans.
    /// let expected = vec![
    ///     HalfMatch::must(0, 3),
    ///     HalfMatch::must(0, 2),
    ///     HalfMatch::must(0, 1),
    ///     HalfMatch::must(1, 0),
    ///     HalfMatch::must(0, 0),
    /// ];
    /// assert_eq!(expected, matches);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn try_search_overlapping_rev(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        let utf8empty = self.get_nfa().has_empty() && self.get_nfa().is_utf8();
        search::find_overlapping_rev(self, cache, input, state)?;
        match state.get_match() {
            None => Ok(()),
            Some(_) if !utf8empty => Ok(()),
            Some(_) => skip_empty_utf8_splits_overlapping(
                input,
                state,
                |input, state| {
                    search::find_overlapping_rev(self, cache, input, state)
                },
            ),
        }
    }

    /// Writes the set of patterns that match anywhere in the given search
    /// configuration to `patset`. If multiple patterns match at the same
    /// position and the underlying DFA supports overlapping matches, then all
    /// matching patterns are written to the given set.
    ///
    /// Unless all of the patterns in this DFA are anchored, then generally
    /// speaking, this will visit every byte in the haystack.
    ///
    /// This search routine *does not* clear the pattern set. This gives some
    /// flexibility to the caller (e.g., running multiple searches with the
    /// same pattern set), but does make the API bug-prone if you're reusing
    /// the same pattern set for multiple searches but intended them to be
    /// independent.
    ///
    /// If a pattern ID matched but the given `PatternSet` does not have
    /// sufficient capacity to store it, then it is not inserted and silently
    /// dropped.
    ///
    /// # Errors
    ///
    /// This routine errors if the search could not complete. This can occur
    /// in a number of circumstances:
    ///
    /// * The configuration of the lazy DFA may permit it to "quit" the search.
    /// For example, setting quit bytes or enabling heuristic support for
    /// Unicode word boundaries. The default configuration does not enable any
    /// option that could result in the lazy DFA quitting.
    /// * The configuration of the lazy DFA may also permit it to "give up"
    /// on a search if it makes ineffective use of its transition table
    /// cache. The default configuration does not enable this by default,
    /// although it is typically a good idea to.
    /// * When the provided `Input` configuration is not supported. For
    /// example, by providing an unsupported anchor mode.
    ///
    /// When a search returns an error, callers cannot know whether a match
    /// exists or not.
    ///
    /// # Example
    ///
    /// This example shows how to find all matching patterns in a haystack,
    /// even when some patterns match at the same position as other patterns.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{
    ///     hybrid::dfa::DFA,
    ///     Input, MatchKind, PatternSet,
    /// };
    ///
    /// let patterns = &[
    ///     r"\w+", r"\d+", r"\pL+", r"foo", r"bar", r"barfoo", r"foobar",
    /// ];
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().match_kind(MatchKind::All))
    ///     .build_many(patterns)?;
    /// let mut cache = dfa.create_cache();
    ///
    /// let input = Input::new("foobar");
    /// let mut patset = PatternSet::new(dfa.pattern_len());
    /// dfa.try_which_overlapping_matches(&mut cache, &input, &mut patset)?;
    /// let expected = vec![0, 2, 3, 4, 6];
    /// let got: Vec<usize> = patset.iter().map(|p| p.as_usize()).collect();
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn try_which_overlapping_matches(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) -> Result<(), MatchError> {
        let mut state = OverlappingState::start();
        while let Some(m) = {
            self.try_search_overlapping_fwd(cache, input, &mut state)?;
            state.get_match()
        } {
            let _ = patset.try_insert(m.pattern());
            // There's nothing left to find, so we can stop. Or the caller
            // asked us to.
            if patset.is_full() || input.get_earliest() {
                break;
            }
        }
        Ok(())
    }
}

impl DFA {
    /// Transitions from the current state to the next state, given the next
    /// byte of input.
    ///
    /// The given cache is used to either reuse pre-computed state
    /// transitions, or to store this newly computed transition for future
    /// reuse. Thus, this routine guarantees that it will never return a state
    /// ID that has an "unknown" tag.
    ///
    /// # State identifier validity
    ///
    /// The only valid value for `current` is the lazy state ID returned
    /// by the most recent call to `next_state`, `next_state_untagged`,
    /// `next_state_untagged_unchecked`, `start_state_forward` or
    /// `state_state_reverse` for the given `cache`. Any state ID returned from
    /// prior calls to these routines (with the same `cache`) is considered
    /// invalid (even if it gives an appearance of working). State IDs returned
    /// from _any_ prior call for different `cache` values are also always
    /// invalid.
    ///
    /// The returned ID is always a valid ID when `current` refers to a valid
    /// ID. Moreover, this routine is defined for all possible values of
    /// `input`.
    ///
    /// These validity rules are not checked, even in debug mode. Callers are
    /// required to uphold these rules themselves.
    ///
    /// Violating these state ID validity rules will not sacrifice memory
    /// safety, but _may_ produce an incorrect result or a panic.
    ///
    /// # Panics
    ///
    /// If the given ID does not refer to a valid state, then this routine
    /// may panic but it also may not panic and instead return an invalid or
    /// incorrect ID.
    ///
    /// # Example
    ///
    /// This shows a simplistic example for walking a lazy DFA for a given
    /// haystack by using the `next_state` method.
    ///
    /// ```
    /// use regex_automata::{hybrid::dfa::DFA, Input};
    ///
    /// let dfa = DFA::new(r"[a-z]+r")?;
    /// let mut cache = dfa.create_cache();
    /// let haystack = "bar".as_bytes();
    ///
    /// // The start state is determined by inspecting the position and the
    /// // initial bytes of the haystack.
    /// let mut sid = dfa.start_state_forward(
    ///     &mut cache, &Input::new(haystack),
    /// )?;
    /// // Walk all the bytes in the haystack.
    /// for &b in haystack {
    ///     sid = dfa.next_state(&mut cache, sid, b)?;
    /// }
    /// // Matches are always delayed by 1 byte, so we must explicitly walk the
    /// // special "EOI" transition at the end of the search.
    /// sid = dfa.next_eoi_state(&mut cache, sid)?;
    /// assert!(sid.is_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn next_state(
        &self,
        cache: &mut Cache,
        current: LazyStateID,
        input: u8,
    ) -> Result<LazyStateID, CacheError> {
        let class = usize::from(self.classes.get(input));
        let offset = current.as_usize_untagged() + class;
        let sid = cache.trans[offset];
        if !sid.is_unknown() {
            return Ok(sid);
        }
        let unit = alphabet::Unit::u8(input);
        Lazy::new(self, cache).cache_next_state(current, unit)
    }

    /// Transitions from the current state to the next state, given the next
    /// byte of input and a state ID that is not tagged.
    ///
    /// The only reason to use this routine is performance. In particular, the
    /// `next_state` method needs to do some additional checks, among them is
    /// to account for identifiers to states that are not yet computed. In
    /// such a case, the transition is computed on the fly. However, if it is
    /// known that the `current` state ID is untagged, then these checks can be
    /// omitted.
    ///
    /// Since this routine does not compute states on the fly, it does not
    /// modify the cache and thus cannot return an error. Consequently, `cache`
    /// does not need to be mutable and it is possible for this routine to
    /// return a state ID corresponding to the special "unknown" state. In
    /// this case, it is the caller's responsibility to use the prior state
    /// ID and `input` with `next_state` in order to force the computation of
    /// the unknown transition. Otherwise, trying to use the "unknown" state
    /// ID will just result in transitioning back to itself, and thus never
    /// terminating. (This is technically a special exemption to the state ID
    /// validity rules, but is permissible since this routine is guarateed to
    /// never mutate the given `cache`, and thus the identifier is guaranteed
    /// to remain valid.)
    ///
    /// See [`LazyStateID`] for more details on what it means for a state ID
    /// to be tagged. Also, see
    /// [`next_state_untagged_unchecked`](DFA::next_state_untagged_unchecked)
    /// for this same idea, but with bounds checks forcefully elided.
    ///
    /// # State identifier validity
    ///
    /// The only valid value for `current` is an **untagged** lazy
    /// state ID returned by the most recent call to `next_state`,
    /// `next_state_untagged`, `next_state_untagged_unchecked`,
    /// `start_state_forward` or `state_state_reverse` for the given `cache`.
    /// Any state ID returned from prior calls to these routines (with the
    /// same `cache`) is considered invalid (even if it gives an appearance
    /// of working). State IDs returned from _any_ prior call for different
    /// `cache` values are also always invalid.
    ///
    /// The returned ID is always a valid ID when `current` refers to a valid
    /// ID, although it may be tagged. Moreover, this routine is defined for
    /// all possible values of `input`.
    ///
    /// Not all validity rules are checked, even in debug mode. Callers are
    /// required to uphold these rules themselves.
    ///
    /// Violating these state ID validity rules will not sacrifice memory
    /// safety, but _may_ produce an incorrect result or a panic.
    ///
    /// # Panics
    ///
    /// If the given ID does not refer to a valid state, then this routine
    /// may panic but it also may not panic and instead return an invalid or
    /// incorrect ID.
    ///
    /// # Example
    ///
    /// This shows a simplistic example for walking a lazy DFA for a given
    /// haystack by using the `next_state_untagged` method where possible.
    ///
    /// ```
    /// use regex_automata::{hybrid::dfa::DFA, Input};
    ///
    /// let dfa = DFA::new(r"[a-z]+r")?;
    /// let mut cache = dfa.create_cache();
    /// let haystack = "bar".as_bytes();
    ///
    /// // The start state is determined by inspecting the position and the
    /// // initial bytes of the haystack.
    /// let mut sid = dfa.start_state_forward(
    ///     &mut cache, &Input::new(haystack),
    /// )?;
    /// // Walk all the bytes in the haystack.
    /// let mut at = 0;
    /// while at < haystack.len() {
    ///     if sid.is_tagged() {
    ///         sid = dfa.next_state(&mut cache, sid, haystack[at])?;
    ///     } else {
    ///         let mut prev_sid = sid;
    ///         // We attempt to chew through as much as we can while moving
    ///         // through untagged state IDs. Thus, the transition function
    ///         // does less work on average per byte. (Unrolling this loop
    ///         // may help even more.)
    ///         while at < haystack.len() {
    ///             prev_sid = sid;
    ///             sid = dfa.next_state_untagged(
    ///                 &mut cache, sid, haystack[at],
    ///             );
    ///             at += 1;
    ///             if sid.is_tagged() {
    ///                 break;
    ///             }
    ///         }
    ///         // We must ensure that we never proceed to the next iteration
    ///         // with an unknown state ID. If we don't account for this
    ///         // case, then search isn't guaranteed to terminate since all
    ///         // transitions on unknown states loop back to itself.
    ///         if sid.is_unknown() {
    ///             sid = dfa.next_state(
    ///                 &mut cache, prev_sid, haystack[at - 1],
    ///             )?;
    ///         }
    ///     }
    /// }
    /// // Matches are always delayed by 1 byte, so we must explicitly walk the
    /// // special "EOI" transition at the end of the search.
    /// sid = dfa.next_eoi_state(&mut cache, sid)?;
    /// assert!(sid.is_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn next_state_untagged(
        &self,
        cache: &Cache,
        current: LazyStateID,
        input: u8,
    ) -> LazyStateID {
        debug_assert!(!current.is_tagged());
        let class = usize::from(self.classes.get(input));
        let offset = current.as_usize_unchecked() + class;
        cache.trans[offset]
    }

    /// Transitions from the current state to the next state, eliding bounds
    /// checks, given the next byte of input and a state ID that is not tagged.
    ///
    /// The only reason to use this routine is performance. In particular, the
    /// `next_state` method needs to do some additional checks, among them is
    /// to account for identifiers to states that are not yet computed. In
    /// such a case, the transition is computed on the fly. However, if it is
    /// known that the `current` state ID is untagged, then these checks can be
    /// omitted.
    ///
    /// Since this routine does not compute states on the fly, it does not
    /// modify the cache and thus cannot return an error. Consequently, `cache`
    /// does not need to be mutable and it is possible for this routine to
    /// return a state ID corresponding to the special "unknown" state. In
    /// this case, it is the caller's responsibility to use the prior state
    /// ID and `input` with `next_state` in order to force the computation of
    /// the unknown transition. Otherwise, trying to use the "unknown" state
    /// ID will just result in transitioning back to itself, and thus never
    /// terminating. (This is technically a special exemption to the state ID
    /// validity rules, but is permissible since this routine is guarateed to
    /// never mutate the given `cache`, and thus the identifier is guaranteed
    /// to remain valid.)
    ///
    /// See [`LazyStateID`] for more details on what it means for a state ID
    /// to be tagged. Also, see
    /// [`next_state_untagged`](DFA::next_state_untagged)
    /// for this same idea, but with memory safety guaranteed by retaining
    /// bounds checks.
    ///
    /// # State identifier validity
    ///
    /// The only valid value for `current` is an **untagged** lazy
    /// state ID returned by the most recent call to `next_state`,
    /// `next_state_untagged`, `next_state_untagged_unchecked`,
    /// `start_state_forward` or `state_state_reverse` for the given `cache`.
    /// Any state ID returned from prior calls to these routines (with the
    /// same `cache`) is considered invalid (even if it gives an appearance
    /// of working). State IDs returned from _any_ prior call for different
    /// `cache` values are also always invalid.
    ///
    /// The returned ID is always a valid ID when `current` refers to a valid
    /// ID, although it may be tagged. Moreover, this routine is defined for
    /// all possible values of `input`.
    ///
    /// Not all validity rules are checked, even in debug mode. Callers are
    /// required to uphold these rules themselves.
    ///
    /// Violating these state ID validity rules will not sacrifice memory
    /// safety, but _may_ produce an incorrect result or a panic.
    ///
    /// # Safety
    ///
    /// Callers of this method must guarantee that `current` refers to a valid
    /// state ID according to the rules described above. If `current` is not a
    /// valid state ID for this automaton, then calling this routine may result
    /// in undefined behavior.
    ///
    /// If `current` is valid, then the ID returned is valid for all possible
    /// values of `input`.
    #[inline]
    pub unsafe fn next_state_untagged_unchecked(
        &self,
        cache: &Cache,
        current: LazyStateID,
        input: u8,
    ) -> LazyStateID {
        debug_assert!(!current.is_tagged());
        let class = usize::from(self.classes.get(input));
        let offset = current.as_usize_unchecked() + class;
        *cache.trans.get_unchecked(offset)
    }

    /// Transitions from the current state to the next state for the special
    /// EOI symbol.
    ///
    /// The given cache is used to either reuse pre-computed state
    /// transitions, or to store this newly computed transition for future
    /// reuse. Thus, this routine guarantees that it will never return a state
    /// ID that has an "unknown" tag.
    ///
    /// This routine must be called at the end of every search in a correct
    /// implementation of search. Namely, lazy DFAs in this crate delay matches
    /// by one byte in order to support look-around operators. Thus, after
    /// reaching the end of a haystack, a search implementation must follow one
    /// last EOI transition.
    ///
    /// It is best to think of EOI as an additional symbol in the alphabet of a
    /// DFA that is distinct from every other symbol. That is, the alphabet of
    /// lazy DFAs in this crate has a logical size of 257 instead of 256, where
    /// 256 corresponds to every possible inhabitant of `u8`. (In practice, the
    /// physical alphabet size may be smaller because of alphabet compression
    /// via equivalence classes, but EOI is always represented somehow in the
    /// alphabet.)
    ///
    /// # State identifier validity
    ///
    /// The only valid value for `current` is the lazy state ID returned
    /// by the most recent call to `next_state`, `next_state_untagged`,
    /// `next_state_untagged_unchecked`, `start_state_forward` or
    /// `state_state_reverse` for the given `cache`. Any state ID returned from
    /// prior calls to these routines (with the same `cache`) is considered
    /// invalid (even if it gives an appearance of working). State IDs returned
    /// from _any_ prior call for different `cache` values are also always
    /// invalid.
    ///
    /// The returned ID is always a valid ID when `current` refers to a valid
    /// ID.
    ///
    /// These validity rules are not checked, even in debug mode. Callers are
    /// required to uphold these rules themselves.
    ///
    /// Violating these state ID validity rules will not sacrifice memory
    /// safety, but _may_ produce an incorrect result or a panic.
    ///
    /// # Panics
    ///
    /// If the given ID does not refer to a valid state, then this routine
    /// may panic but it also may not panic and instead return an invalid or
    /// incorrect ID.
    ///
    /// # Example
    ///
    /// This shows a simplistic example for walking a DFA for a given haystack,
    /// and then finishing the search with the final EOI transition.
    ///
    /// ```
    /// use regex_automata::{hybrid::dfa::DFA, Input};
    ///
    /// let dfa = DFA::new(r"[a-z]+r")?;
    /// let mut cache = dfa.create_cache();
    /// let haystack = "bar".as_bytes();
    ///
    /// // The start state is determined by inspecting the position and the
    /// // initial bytes of the haystack.
    /// let mut sid = dfa.start_state_forward(
    ///     &mut cache, &Input::new(haystack),
    /// )?;
    /// // Walk all the bytes in the haystack.
    /// for &b in haystack {
    ///     sid = dfa.next_state(&mut cache, sid, b)?;
    /// }
    /// // Matches are always delayed by 1 byte, so we must explicitly walk
    /// // the special "EOI" transition at the end of the search. Without this
    /// // final transition, the assert below will fail since the DFA will not
    /// // have entered a match state yet!
    /// sid = dfa.next_eoi_state(&mut cache, sid)?;
    /// assert!(sid.is_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn next_eoi_state(
        &self,
        cache: &mut Cache,
        current: LazyStateID,
    ) -> Result<LazyStateID, CacheError> {
        let eoi = self.classes.eoi().as_usize();
        let offset = current.as_usize_untagged() + eoi;
        let sid = cache.trans[offset];
        if !sid.is_unknown() {
            return Ok(sid);
        }
        let unit = self.classes.eoi();
        Lazy::new(self, cache).cache_next_state(current, unit)
    }

    /// Return the ID of the start state for this lazy DFA for the given
    /// starting configuration.
    ///
    /// Unlike typical DFA implementations, the start state for DFAs in this
    /// crate is dependent on a few different factors:
    ///
    /// * The [`Anchored`] mode of the search. Unanchored, anchored and
    /// anchored searches for a specific [`PatternID`] all use different start
    /// states.
    /// * Whether a "look-behind" byte exists. For example, the `^` anchor
    /// matches if and only if there is no look-behind byte.
    /// * The specific value of that look-behind byte. For example, a `(?m:^)`
    /// assertion only matches when there is either no look-behind byte, or
    /// when the look-behind byte is a line terminator.
    ///
    /// The [starting configuration](start::Config) provides the above
    /// information.
    ///
    /// This routine can be used for either forward or reverse searches.
    /// Although, as a convenience, if you have an [`Input`], then it
    /// may be more succinct to use [`DFA::start_state_forward`] or
    /// [`DFA::start_state_reverse`]. Note, for example, that the convenience
    /// routines return a [`MatchError`] on failure where as this routine
    /// returns a [`StartError`].
    ///
    /// # Errors
    ///
    /// This may return a [`StartError`] if the search needs to give up when
    /// determining the start state (for example, if it sees a "quit" byte
    /// or if the cache has become inefficient). This can also return an
    /// error if the given configuration contains an unsupported [`Anchored`]
    /// configuration.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub fn start_state(
        &self,
        cache: &mut Cache,
        config: &start::Config,
    ) -> Result<LazyStateID, StartError> {
        let lazy = LazyRef::new(self, cache);
        let anchored = config.get_anchored();
        let start = match config.get_look_behind() {
            None => Start::Text,
            Some(byte) => {
                if !self.quitset.is_empty() && self.quitset.contains(byte) {
                    return Err(StartError::quit(byte));
                }
                self.start_map.get(byte)
            }
        };
        let start_id = lazy.get_cached_start_id(anchored, start)?;
        if !start_id.is_unknown() {
            return Ok(start_id);
        }
        Lazy::new(self, cache).cache_start_group(anchored, start)
    }

    /// Return the ID of the start state for this lazy DFA when executing a
    /// forward search.
    ///
    /// This is a convenience routine for calling [`DFA::start_state`] that
    /// converts the given [`Input`] to a [start configuration](start::Config).
    /// Additionally, if an error occurs, it is converted from a [`StartError`]
    /// to a [`MatchError`] using the offset information in the given
    /// [`Input`].
    ///
    /// # Errors
    ///
    /// This may return a [`MatchError`] if the search needs to give up when
    /// determining the start state (for example, if it sees a "quit" byte or
    /// if the cache has become inefficient). This can also return an error if
    /// the given `Input` contains an unsupported [`Anchored`] configuration.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub fn start_state_forward(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Result<LazyStateID, MatchError> {
        let config = start::Config::from_input_forward(input);
        self.start_state(cache, &config).map_err(|err| match err {
            StartError::Cache { .. } => MatchError::gave_up(input.start()),
            StartError::Quit { byte } => {
                let offset = input
                    .start()
                    .checked_sub(1)
                    .expect("no quit in start without look-behind");
                MatchError::quit(byte, offset)
            }
            StartError::UnsupportedAnchored { mode } => {
                MatchError::unsupported_anchored(mode)
            }
        })
    }

    /// Return the ID of the start state for this lazy DFA when executing a
    /// reverse search.
    ///
    /// This is a convenience routine for calling [`DFA::start_state`] that
    /// converts the given [`Input`] to a [start configuration](start::Config).
    /// Additionally, if an error occurs, it is converted from a [`StartError`]
    /// to a [`MatchError`] using the offset information in the given
    /// [`Input`].
    ///
    /// # Errors
    ///
    /// This may return a [`MatchError`] if the search needs to give up when
    /// determining the start state (for example, if it sees a "quit" byte or
    /// if the cache has become inefficient). This can also return an error if
    /// the given `Input` contains an unsupported [`Anchored`] configuration.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub fn start_state_reverse(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Result<LazyStateID, MatchError> {
        let config = start::Config::from_input_reverse(input);
        self.start_state(cache, &config).map_err(|err| match err {
            StartError::Cache { .. } => MatchError::gave_up(input.end()),
            StartError::Quit { byte } => {
                let offset = input.end();
                MatchError::quit(byte, offset)
            }
            StartError::UnsupportedAnchored { mode } => {
                MatchError::unsupported_anchored(mode)
            }
        })
    }

    /// Returns the total number of patterns that match in this state.
    ///
    /// If the lazy DFA was compiled with one pattern, then this must
    /// necessarily always return `1` for all match states.
    ///
    /// A lazy DFA guarantees that [`DFA::match_pattern`] can be called with
    /// indices up to (but not including) the length returned by this routine
    /// without panicking.
    ///
    /// # Panics
    ///
    /// If the given state is not a match state, then this may either panic
    /// or return an incorrect result.
    ///
    /// # Example
    ///
    /// This example shows a simple instance of implementing overlapping
    /// matches. In particular, it shows not only how to determine how many
    /// patterns have matched in a particular state, but also how to access
    /// which specific patterns have matched.
    ///
    /// Notice that we must use [`MatchKind::All`] when building the DFA. If we
    /// used [`MatchKind::LeftmostFirst`] instead, then the DFA would not be
    /// constructed in a way that supports overlapping matches. (It would only
    /// report a single pattern that matches at any particular point in time.)
    ///
    /// Another thing to take note of is the patterns used and the order in
    /// which the pattern IDs are reported. In the example below, pattern `3`
    /// is yielded first. Why? Because it corresponds to the match that
    /// appears first. Namely, the `@` symbol is part of `\S+` but not part
    /// of any of the other patterns. Since the `\S+` pattern has a match that
    /// starts to the left of any other pattern, its ID is returned before any
    /// other.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{hybrid::dfa::DFA, Input, MatchKind};
    ///
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().match_kind(MatchKind::All))
    ///     .build_many(&[
    ///         r"\w+", r"[a-z]+", r"[A-Z]+", r"\S+",
    ///     ])?;
    /// let mut cache = dfa.create_cache();
    /// let haystack = "@bar".as_bytes();
    ///
    /// // The start state is determined by inspecting the position and the
    /// // initial bytes of the haystack.
    /// let mut sid = dfa.start_state_forward(
    ///     &mut cache, &Input::new(haystack),
    /// )?;
    /// // Walk all the bytes in the haystack.
    /// for &b in haystack {
    ///     sid = dfa.next_state(&mut cache, sid, b)?;
    /// }
    /// sid = dfa.next_eoi_state(&mut cache, sid)?;
    ///
    /// assert!(sid.is_match());
    /// assert_eq!(dfa.match_len(&mut cache, sid), 3);
    /// // The following calls are guaranteed to not panic since `match_len`
    /// // returned `3` above.
    /// assert_eq!(dfa.match_pattern(&mut cache, sid, 0).as_usize(), 3);
    /// assert_eq!(dfa.match_pattern(&mut cache, sid, 1).as_usize(), 0);
    /// assert_eq!(dfa.match_pattern(&mut cache, sid, 2).as_usize(), 1);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn match_len(&self, cache: &Cache, id: LazyStateID) -> usize {
        assert!(id.is_match());
        LazyRef::new(self, cache).get_cached_state(id).match_len()
    }

    /// Returns the pattern ID corresponding to the given match index in the
    /// given state.
    ///
    /// See [`DFA::match_len`] for an example of how to use this method
    /// correctly. Note that if you know your lazy DFA is configured with a
    /// single pattern, then this routine is never necessary since it will
    /// always return a pattern ID of `0` for an index of `0` when `id`
    /// corresponds to a match state.
    ///
    /// Typically, this routine is used when implementing an overlapping
    /// search, as the example for `DFA::match_len` does.
    ///
    /// # Panics
    ///
    /// If the state ID is not a match state or if the match index is out
    /// of bounds for the given state, then this routine may either panic
    /// or produce an incorrect result. If the state ID is correct and the
    /// match index is correct, then this routine always produces a valid
    /// `PatternID`.
    #[inline]
    pub fn match_pattern(
        &self,
        cache: &Cache,
        id: LazyStateID,
        match_index: usize,
    ) -> PatternID {
        // This is an optimization for the very common case of a DFA with a
        // single pattern. This conditional avoids a somewhat more costly path
        // that finds the pattern ID from the corresponding `State`, which
        // requires a bit of slicing/pointer-chasing. This optimization tends
        // to only matter when matches are frequent.
        if self.pattern_len() == 1 {
            return PatternID::ZERO;
        }
        LazyRef::new(self, cache)
            .get_cached_state(id)
            .match_pattern(match_index)
    }
}

/// A cache represents a partially computed DFA.
///
/// A cache is the key component that differentiates a classical DFA and a
/// hybrid NFA/DFA (also called a "lazy DFA"). Where a classical DFA builds a
/// complete transition table that can handle all possible inputs, a hybrid
/// NFA/DFA starts with an empty transition table and builds only the parts
/// required during search. The parts that are built are stored in a cache. For
/// this reason, a cache is a required parameter for nearly every operation on
/// a [`DFA`].
///
/// Caches can be created from their corresponding DFA via
/// [`DFA::create_cache`]. A cache can only be used with either the DFA that
/// created it, or the DFA that was most recently used to reset it with
/// [`Cache::reset`]. Using a cache with any other DFA may result in panics
/// or incorrect results.
#[derive(Clone, Debug)]
pub struct Cache {
    // N.B. If you're looking to understand how determinization works, it
    // is probably simpler to first grok src/dfa/determinize.rs, since that
    // doesn't have the "laziness" component.
    /// The transition table.
    ///
    /// Given a `current` LazyStateID and an `input` byte, the next state can
    /// be computed via `trans[untagged(current) + equiv_class(input)]`. Notice
    /// that no multiplication is used. That's because state identifiers are
    /// "premultiplied."
    ///
    /// Note that the next state may be the "unknown" state. In this case, the
    /// next state is not known and determinization for `current` on `input`
    /// must be performed.
    trans: Vec<LazyStateID>,
    /// The starting states for this DFA.
    ///
    /// These are computed lazily. Initially, these are all set to "unknown"
    /// lazy state IDs.
    ///
    /// When 'starts_for_each_pattern' is disabled (the default), then the size
    /// of this is constrained to the possible starting configurations based
    /// on the search parameters. (At time of writing, that's 4.) However,
    /// when starting states for each pattern is enabled, then there are N
    /// additional groups of starting states, where each group reflects the
    /// different possible configurations and N is the number of patterns.
    starts: Vec<LazyStateID>,
    /// A sequence of NFA/DFA powerset states that have been computed for this
    /// lazy DFA. This sequence is indexable by untagged LazyStateIDs. (Every
    /// tagged LazyStateID can be used to index this sequence by converting it
    /// to its untagged form.)
    states: Vec<State>,
    /// A map from states to their corresponding IDs. This map may be accessed
    /// via the raw byte representation of a state, which means that a `State`
    /// does not need to be allocated to determine whether it already exists
    /// in this map. Indeed, the existence of such a state is what determines
    /// whether we allocate a new `State` or not.
    ///
    /// The higher level idea here is that we do just enough determinization
    /// for a state to check whether we've already computed it. If we have,
    /// then we can save a little (albeit not much) work. The real savings is
    /// in memory usage. If we never checked for trivially duplicate states,
    /// then our memory usage would explode to unreasonable levels.
    states_to_id: StateMap,
    /// Sparse sets used to track which NFA states have been visited during
    /// various traversals.
    sparses: SparseSets,
    /// Scratch space for traversing the NFA graph. (We use space on the heap
    /// instead of the call stack.)
    stack: Vec<NFAStateID>,
    /// Scratch space for building a NFA/DFA powerset state. This is used to
    /// help amortize allocation since not every powerset state generated is
    /// added to the cache. In particular, if it already exists in the cache,
    /// then there is no need to allocate a new `State` for it.
    scratch_state_builder: StateBuilderEmpty,
    /// A simple abstraction for handling the saving of at most a single state
    /// across a cache clearing. This is required for correctness. Namely, if
    /// adding a new state after clearing the cache fails, then the caller
    /// must retain the ability to continue using the state ID given. The
    /// state corresponding to the state ID is what we preserve across cache
    /// clearings.
    state_saver: StateSaver,
    /// The memory usage, in bytes, used by 'states' and 'states_to_id'. We
    /// track this as new states are added since states use a variable amount
    /// of heap. Tracking this as we add states makes it possible to compute
    /// the total amount of memory used by the determinizer in constant time.
    memory_usage_state: usize,
    /// The number of times the cache has been cleared. When a minimum cache
    /// clear count is set, then the cache will return an error instead of
    /// clearing the cache if the count has been exceeded.
    clear_count: usize,
    /// The total number of bytes searched since the last time this cache was
    /// cleared, not including the current search.
    ///
    /// This can be added to the length of the current search to get the true
    /// total number of bytes searched.
    ///
    /// This is generally only non-zero when the
    /// `Cache::search_{start,update,finish}` APIs are used to track search
    /// progress.
    bytes_searched: usize,
    /// The progress of the current search.
    ///
    /// This is only non-`None` when callers utlize the `Cache::search_start`,
    /// `Cache::search_update` and `Cache::search_finish` APIs.
    ///
    /// The purpose of recording search progress is to be able to make a
    /// determination about the efficiency of the cache. Namely, by keeping
    /// track of the
    progress: Option<SearchProgress>,
}

impl Cache {
    /// Create a new cache for the given lazy DFA.
    ///
    /// The cache returned should only be used for searches for the given DFA.
    /// If you want to reuse the cache for another DFA, then you must call
    /// [`Cache::reset`] with that DFA.
    pub fn new(dfa: &DFA) -> Cache {
        let mut cache = Cache {
            trans: alloc::vec![],
            starts: alloc::vec![],
            states: alloc::vec![],
            states_to_id: StateMap::new(),
            sparses: SparseSets::new(dfa.get_nfa().states().len()),
            stack: alloc::vec![],
            scratch_state_builder: StateBuilderEmpty::new(),
            state_saver: StateSaver::none(),
            memory_usage_state: 0,
            clear_count: 0,
            bytes_searched: 0,
            progress: None,
        };
        debug!("pre-init lazy DFA cache size: {}", cache.memory_usage());
        Lazy { dfa, cache: &mut cache }.init_cache();
        debug!("post-init lazy DFA cache size: {}", cache.memory_usage());
        cache
    }

    /// Reset this cache such that it can be used for searching with the given
    /// lazy DFA (and only that DFA).
    ///
    /// A cache reset permits reusing memory already allocated in this cache
    /// with a different lazy DFA.
    ///
    /// Resetting a cache sets its "clear count" to 0. This is relevant if the
    /// lazy DFA has been configured to "give up" after it has cleared the
    /// cache a certain number of times.
    ///
    /// Any lazy state ID generated by the cache prior to resetting it is
    /// invalid after the reset.
    ///
    /// # Example
    ///
    /// This shows how to re-purpose a cache for use with a different DFA.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{hybrid::dfa::DFA, HalfMatch, Input};
    ///
    /// let dfa1 = DFA::new(r"\w")?;
    /// let dfa2 = DFA::new(r"\W")?;
    ///
    /// let mut cache = dfa1.create_cache();
    /// assert_eq!(
    ///     Some(HalfMatch::must(0, 2)),
    ///     dfa1.try_search_fwd(&mut cache, &Input::new("Δ"))?,
    /// );
    ///
    /// // Using 'cache' with dfa2 is not allowed. It may result in panics or
    /// // incorrect results. In order to re-purpose the cache, we must reset
    /// // it with the DFA we'd like to use it with.
    /// //
    /// // Similarly, after this reset, using the cache with 'dfa1' is also not
    /// // allowed.
    /// cache.reset(&dfa2);
    /// assert_eq!(
    ///     Some(HalfMatch::must(0, 3)),
    ///     dfa2.try_search_fwd(&mut cache, &Input::new("☃"))?,
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn reset(&mut self, dfa: &DFA) {
        Lazy::new(dfa, self).reset_cache()
    }

    /// Initializes a new search starting at the given position.
    ///
    /// If a previous search was unfinished, then it is finished automatically
    /// and a new search is begun.
    ///
    /// Note that keeping track of search progress is _not necessary_
    /// for correct implementations of search using a lazy DFA. Keeping
    /// track of search progress is only necessary if you want the
    /// [`Config::minimum_bytes_per_state`] configuration knob to work.
    #[inline]
    pub fn search_start(&mut self, at: usize) {
        // If a previous search wasn't marked as finished, then finish it
        // now automatically.
        if let Some(p) = self.progress.take() {
            self.bytes_searched += p.len();
        }
        self.progress = Some(SearchProgress { start: at, at });
    }

    /// Updates the current search to indicate that it has search to the
    /// current position.
    ///
    /// No special care needs to be taken for reverse searches. Namely, the
    /// position given may be _less than_ the starting position of the search.
    ///
    /// # Panics
    ///
    /// This panics if no search has been started by [`Cache::search_start`].
    #[inline]
    pub fn search_update(&mut self, at: usize) {
        let p =
            self.progress.as_mut().expect("no in-progress search to update");
        p.at = at;
    }

    /// Indicates that a search has finished at the given position.
    ///
    /// # Panics
    ///
    /// This panics if no search has been started by [`Cache::search_start`].
    #[inline]
    pub fn search_finish(&mut self, at: usize) {
        let mut p =
            self.progress.take().expect("no in-progress search to finish");
        p.at = at;
        self.bytes_searched += p.len();
    }

    /// Returns the total number of bytes that have been searched since this
    /// cache was last cleared.
    ///
    /// This is useful for determining the efficiency of the cache. For
    /// example, the lazy DFA uses this value in conjunction with the
    /// [`Config::minimum_bytes_per_state`] knob to help determine whether it
    /// should quit searching.
    ///
    /// This always returns `0` if search progress isn't being tracked. Note
    /// that the lazy DFA search routines in this crate always track search
    /// progress.
    pub fn search_total_len(&self) -> usize {
        self.bytes_searched + self.progress.as_ref().map_or(0, |p| p.len())
    }

    /// Returns the total number of times this cache has been cleared since it
    /// was either created or last reset.
    ///
    /// This is useful for informational purposes or if you want to change
    /// search strategies based on the number of times the cache has been
    /// cleared.
    pub fn clear_count(&self) -> usize {
        self.clear_count
    }

    /// Returns the heap memory usage, in bytes, of this cache.
    ///
    /// This does **not** include the stack size used up by this cache. To
    /// compute that, use `std::mem::size_of::<Cache>()`.
    pub fn memory_usage(&self) -> usize {
        const ID_SIZE: usize = size_of::<LazyStateID>();
        const STATE_SIZE: usize = size_of::<State>();

        // NOTE: If you make changes to the below, then
        // 'minimum_cache_capacity' should be updated correspondingly.

        self.trans.len() * ID_SIZE
        + self.starts.len() * ID_SIZE
        + self.states.len() * STATE_SIZE
        // Maps likely use more memory than this, but it's probably close.
        + self.states_to_id.len() * (STATE_SIZE + ID_SIZE)
        + self.sparses.memory_usage()
        + self.stack.capacity() * ID_SIZE
        + self.scratch_state_builder.capacity()
        // Heap memory used by 'State' in both 'states' and 'states_to_id'.
        + self.memory_usage_state
    }
}

/// Keeps track of the progress of the current search.
///
/// This is updated via the `Cache::search_{start,update,finish}` APIs to
/// record how many bytes have been searched. This permits computing a
/// heuristic that represents the efficiency of a cache, and thus helps inform
/// whether the lazy DFA should give up or not.
#[derive(Clone, Debug)]
struct SearchProgress {
    start: usize,
    at: usize,
}

impl SearchProgress {
    /// Returns the length, in bytes, of this search so far.
    ///
    /// This automatically handles the case of a reverse search, where `at`
    /// is likely to be less than `start`.
    fn len(&self) -> usize {
        if self.start <= self.at {
            self.at - self.start
        } else {
            self.start - self.at
        }
    }
}

/// A map from states to state identifiers. When using std, we use a standard
/// hashmap, since it's a bit faster for this use case. (Other maps, like
/// one's based on FNV, have not yet been benchmarked.)
///
/// The main purpose of this map is to reuse states where possible. This won't
/// fully minimize the DFA, but it works well in a lot of cases.
#[cfg(feature = "std")]
type StateMap = std::collections::HashMap<State, LazyStateID>;
#[cfg(not(feature = "std"))]
type StateMap = alloc::collections::BTreeMap<State, LazyStateID>;

/// A type that groups methods that require the base NFA/DFA and writable
/// access to the cache.
#[derive(Debug)]
struct Lazy<'i, 'c> {
    dfa: &'i DFA,
    cache: &'c mut Cache,
}

impl<'i, 'c> Lazy<'i, 'c> {
    /// Creates a new 'Lazy' wrapper for a DFA and its corresponding cache.
    fn new(dfa: &'i DFA, cache: &'c mut Cache) -> Lazy<'i, 'c> {
        Lazy { dfa, cache }
    }

    /// Return an immutable view by downgrading a writable cache to a read-only
    /// cache.
    fn as_ref<'a>(&'a self) -> LazyRef<'i, 'a> {
        LazyRef::new(self.dfa, self.cache)
    }

    /// This is marked as 'inline(never)' to avoid bloating methods on 'DFA'
    /// like 'next_state' and 'next_eoi_state' that are called in critical
    /// areas. The idea is to let the optimizer focus on the other areas of
    /// those methods as the hot path.
    ///
    /// Here's an example that justifies 'inline(never)'
    ///
    /// ```ignore
    /// regex-cli find match hybrid \
    ///   --cache-capacity 100000000 \
    ///   -p '\pL{100}'
    ///   all-codepoints-utf8-100x
    /// ```
    ///
    /// Where 'all-codepoints-utf8-100x' is the UTF-8 encoding of every
    /// codepoint, in sequence, repeated 100 times.
    ///
    /// With 'inline(never)' hyperfine reports 1.1s per run. With
    /// 'inline(always)', hyperfine reports 1.23s. So that's a 10% improvement.
    #[cold]
    #[inline(never)]
    fn cache_next_state(
        &mut self,
        mut current: LazyStateID,
        unit: alphabet::Unit,
    ) -> Result<LazyStateID, CacheError> {
        let stride2 = self.dfa.stride2();
        let empty_builder = self.get_state_builder();
        let builder = determinize::next(
            self.dfa.get_nfa(),
            self.dfa.get_config().get_match_kind(),
            &mut self.cache.sparses,
            &mut self.cache.stack,
            &self.cache.states[current.as_usize_untagged() >> stride2],
            unit,
            empty_builder,
        );
        let save_state = !self.as_ref().state_builder_fits_in_cache(&builder);
        if save_state {
            self.save_state(current);
        }
        let next = self.add_builder_state(builder, |sid| sid)?;
        if save_state {
            current = self.saved_state_id();
        }
        // This is the payoff. The next time 'next_state' is called with this
        // state and alphabet unit, it will find this transition and avoid
        // having to re-determinize this transition.
        self.set_transition(current, unit, next);
        Ok(next)
    }

    /// Compute and cache the starting state for the given pattern ID (if
    /// present) and the starting configuration.
    ///
    /// This panics if a pattern ID is given and the DFA isn't configured to
    /// build anchored start states for each pattern.
    ///
    /// This will never return an unknown lazy state ID.
    ///
    /// If caching this state would otherwise result in a cache that has been
    /// cleared too many times, then an error is returned.
    #[cold]
    #[inline(never)]
    fn cache_start_group(
        &mut self,
        anchored: Anchored,
        start: Start,
    ) -> Result<LazyStateID, StartError> {
        let nfa_start_id = match anchored {
            Anchored::No => self.dfa.get_nfa().start_unanchored(),
            Anchored::Yes => self.dfa.get_nfa().start_anchored(),
            Anchored::Pattern(pid) => {
                if !self.dfa.get_config().get_starts_for_each_pattern() {
                    return Err(StartError::unsupported_anchored(anchored));
                }
                match self.dfa.get_nfa().start_pattern(pid) {
                    None => return Ok(self.as_ref().dead_id()),
                    Some(sid) => sid,
                }
            }
        };

        let id = self
            .cache_start_one(nfa_start_id, start)
            .map_err(StartError::cache)?;
        self.set_start_state(anchored, start, id);
        Ok(id)
    }

    /// Compute and cache the starting state for the given NFA state ID and the
    /// starting configuration. The NFA state ID might be one of the following:
    ///
    /// 1) An unanchored start state to match any pattern.
    /// 2) An anchored start state to match any pattern.
    /// 3) An anchored start state for a particular pattern.
    ///
    /// This will never return an unknown lazy state ID.
    ///
    /// If caching this state would otherwise result in a cache that has been
    /// cleared too many times, then an error is returned.
    fn cache_start_one(
        &mut self,
        nfa_start_id: NFAStateID,
        start: Start,
    ) -> Result<LazyStateID, CacheError> {
        let mut builder_matches = self.get_state_builder().into_matches();
        determinize::set_lookbehind_from_start(
            self.dfa.get_nfa(),
            &start,
            &mut builder_matches,
        );
        self.cache.sparses.set1.clear();
        determinize::epsilon_closure(
            self.dfa.get_nfa(),
            nfa_start_id,
            builder_matches.look_have(),
            &mut self.cache.stack,
            &mut self.cache.sparses.set1,
        );
        let mut builder = builder_matches.into_nfa();
        determinize::add_nfa_states(
            &self.dfa.get_nfa(),
            &self.cache.sparses.set1,
            &mut builder,
        );
        let tag_starts = self.dfa.get_config().get_specialize_start_states();
        self.add_builder_state(builder, |id| {
            if tag_starts {
                id.to_start()
            } else {
                id
            }
        })
    }

    /// Either add the given builder state to this cache, or return an ID to an
    /// equivalent state already in this cache.
    ///
    /// In the case where no equivalent state exists, the idmap function given
    /// may be used to transform the identifier allocated. This is useful if
    /// the caller needs to tag the ID with additional information.
    ///
    /// This will never return an unknown lazy state ID.
    ///
    /// If caching this state would otherwise result in a cache that has been
    /// cleared too many times, then an error is returned.
    fn add_builder_state(
        &mut self,
        builder: StateBuilderNFA,
        idmap: impl Fn(LazyStateID) -> LazyStateID,
    ) -> Result<LazyStateID, CacheError> {
        if let Some(&cached_id) =
            self.cache.states_to_id.get(builder.as_bytes())
        {
            // Since we have a cached state, put the constructed state's
            // memory back into our scratch space, so that it can be reused.
            self.put_state_builder(builder);
            return Ok(cached_id);
        }
        let result = self.add_state(builder.to_state(), idmap);
        self.put_state_builder(builder);
        result
    }

    /// Allocate a new state ID and add the given state to this cache.
    ///
    /// The idmap function given may be used to transform the identifier
    /// allocated. This is useful if the caller needs to tag the ID with
    /// additional information.
    ///
    /// This will never return an unknown lazy state ID.
    ///
    /// If caching this state would otherwise result in a cache that has been
    /// cleared too many times, then an error is returned.
    fn add_state(
        &mut self,
        state: State,
        idmap: impl Fn(LazyStateID) -> LazyStateID,
    ) -> Result<LazyStateID, CacheError> {
        if !self.as_ref().state_fits_in_cache(&state) {
            self.try_clear_cache()?;
        }
        // It's important for this to come second, since the above may clear
        // the cache. If we clear the cache after ID generation, then the ID
        // is likely bunk since it would have been generated based on a larger
        // transition table.
        let mut id = idmap(self.next_state_id()?);
        if state.is_match() {
            id = id.to_match();
        }
        // Add room in the transition table. Since this is a fresh state, all
        // of its transitions are unknown.
        self.cache.trans.extend(
            iter::repeat(self.as_ref().unknown_id()).take(self.dfa.stride()),
        );
        // When we add a sentinel state, we never want to set any quit
        // transitions. Technically, this is harmless, since sentinel states
        // have all of their transitions set to loop back to themselves. But
        // when creating sentinel states before the quit sentinel state,
        // this will try to call 'set_transition' on a state ID that doesn't
        // actually exist yet, which isn't allowed. So we just skip doing so
        // entirely.
        if !self.dfa.quitset.is_empty() && !self.as_ref().is_sentinel(id) {
            let quit_id = self.as_ref().quit_id();
            for b in self.dfa.quitset.iter() {
                self.set_transition(id, alphabet::Unit::u8(b), quit_id);
            }
        }
        self.cache.memory_usage_state += state.memory_usage();
        self.cache.states.push(state.clone());
        self.cache.states_to_id.insert(state, id);
        Ok(id)
    }

    /// Allocate a new state ID.
    ///
    /// This will never return an unknown lazy state ID.
    ///
    /// If caching this state would otherwise result in a cache that has been
    /// cleared too many times, then an error is returned.
    fn next_state_id(&mut self) -> Result<LazyStateID, CacheError> {
        let sid = match LazyStateID::new(self.cache.trans.len()) {
            Ok(sid) => sid,
            Err(_) => {
                self.try_clear_cache()?;
                // This has to pass since we check that ID capacity at
                // construction time can fit at least MIN_STATES states.
                LazyStateID::new(self.cache.trans.len()).unwrap()
            }
        };
        Ok(sid)
    }

    /// Attempt to clear the cache used by this lazy DFA.
    ///
    /// If clearing the cache exceeds the minimum number of required cache
    /// clearings, then this will return a cache error. In this case,
    /// callers should bubble this up as the cache can't be used until it is
    /// reset. Implementations of search should convert this error into a
    /// [`MatchError::gave_up`].
    ///
    /// If 'self.state_saver' is set to save a state, then this state is
    /// persisted through cache clearing. Otherwise, the cache is returned to
    /// its state after initialization with two exceptions: its clear count
    /// is incremented and some of its memory likely has additional capacity.
    /// That is, clearing a cache does _not_ release memory.
    ///
    /// Otherwise, any lazy state ID generated by the cache prior to resetting
    /// it is invalid after the reset.
    fn try_clear_cache(&mut self) -> Result<(), CacheError> {
        let c = self.dfa.get_config();
        if let Some(min_count) = c.get_minimum_cache_clear_count() {
            if self.cache.clear_count >= min_count {
                if let Some(min_bytes_per) = c.get_minimum_bytes_per_state() {
                    let len = self.cache.search_total_len();
                    let min_bytes =
                        min_bytes_per.saturating_mul(self.cache.states.len());
                    // If we've searched 0 bytes then probably something has
                    // gone wrong and the lazy DFA search implementation isn't
                    // correctly updating the search progress state.
                    if len == 0 {
                        trace!(
                            "number of bytes searched is 0, but \
                             a minimum bytes per state searched ({}) is \
                             enabled, maybe Cache::search_update \
                             is not being used?",
                            min_bytes_per,
                        );
                    }
                    if len < min_bytes {
                        trace!(
                            "lazy DFA cache has been cleared {} times, \
                             which exceeds the limit of {}, \
                             AND its bytes searched per state is less \
                             than the configured minimum of {}, \
                             therefore lazy DFA is giving up \
                             (bytes searched since cache clear = {}, \
                              number of states = {})",
                            self.cache.clear_count,
                            min_count,
                            min_bytes_per,
                            len,
                            self.cache.states.len(),
                        );
                        return Err(CacheError::bad_efficiency());
                    } else {
                        trace!(
                            "lazy DFA cache has been cleared {} times, \
                             which exceeds the limit of {}, \
                             AND its bytes searched per state is greater \
                             than the configured minimum of {}, \
                             therefore lazy DFA is continuing! \
                             (bytes searched since cache clear = {}, \
                              number of states = {})",
                            self.cache.clear_count,
                            min_count,
                            min_bytes_per,
                            len,
                            self.cache.states.len(),
                        );
                    }
                } else {
                    trace!(
                        "lazy DFA cache has been cleared {} times, \
                         which exceeds the limit of {}, \
                         since there is no configured bytes per state \
                         minimum, lazy DFA is giving up",
                        self.cache.clear_count,
                        min_count,
                    );
                    return Err(CacheError::too_many_cache_clears());
                }
            }
        }
        self.clear_cache();
        Ok(())
    }

    /// Clears _and_ resets the cache. Resetting the cache means that no
    /// states are persisted and the clear count is reset to 0. No heap memory
    /// is released.
    ///
    /// Note that the caller may reset a cache with a different DFA than what
    /// it was created from. In which case, the cache can now be used with the
    /// new DFA (and not the old DFA).
    fn reset_cache(&mut self) {
        self.cache.state_saver = StateSaver::none();
        self.clear_cache();
        // If a new DFA is used, it might have a different number of NFA
        // states, so we need to make sure our sparse sets have the appropriate
        // size.
        self.cache.sparses.resize(self.dfa.get_nfa().states().len());
        self.cache.clear_count = 0;
        self.cache.progress = None;
    }

    /// Clear the cache used by this lazy DFA.
    ///
    /// If 'self.state_saver' is set to save a state, then this state is
    /// persisted through cache clearing. Otherwise, the cache is returned to
    /// its state after initialization with two exceptions: its clear count
    /// is incremented and some of its memory likely has additional capacity.
    /// That is, clearing a cache does _not_ release memory.
    ///
    /// Otherwise, any lazy state ID generated by the cache prior to resetting
    /// it is invalid after the reset.
    fn clear_cache(&mut self) {
        self.cache.trans.clear();
        self.cache.starts.clear();
        self.cache.states.clear();
        self.cache.states_to_id.clear();
        self.cache.memory_usage_state = 0;
        self.cache.clear_count += 1;
        self.cache.bytes_searched = 0;
        if let Some(ref mut progress) = self.cache.progress {
            progress.start = progress.at;
        }
        trace!(
            "lazy DFA cache has been cleared (count: {})",
            self.cache.clear_count
        );
        self.init_cache();
        // If the state we want to save is one of the sentinel
        // (unknown/dead/quit) states, then 'init_cache' adds those back, and
        // their identifier values remains invariant. So there's no need to add
        // it again. (And indeed, doing so would be incorrect!)
        if let Some((old_id, state)) = self.cache.state_saver.take_to_save() {
            // If the state is one of the special sentinel states, then it is
            // automatically added by cache initialization and its ID always
            // remains the same. With that said, this should never occur since
            // the sentinel states are all loop states back to themselves. So
            // we should never be in a position where we're attempting to save
            // a sentinel state since we never compute transitions out of a
            // sentinel state.
            assert!(
                !self.as_ref().is_sentinel(old_id),
                "cannot save sentinel state"
            );
            let new_id = self
                .add_state(state, |id| {
                    if old_id.is_start() {
                        // We don't need to consult the
                        // 'specialize_start_states' config knob here, because
                        // if it's disabled, old_id.is_start() will never
                        // return true.
                        id.to_start()
                    } else {
                        id
                    }
                })
                // The unwrap here is OK because lazy DFA creation ensures that
                // we have room in the cache to add MIN_STATES states. Since
                // 'init_cache' above adds 3, this adds a 4th.
                .expect("adding one state after cache clear must work");
            self.cache.state_saver = StateSaver::Saved(new_id);
        }
    }

    /// Initialize this cache from emptiness to a place where it can be used
    /// for search.
    ///
    /// This is called both at cache creation time and after the cache has been
    /// cleared.
    ///
    /// Primarily, this adds the three sentinel states and allocates some
    /// initial memory.
    fn init_cache(&mut self) {
        // Why multiply by 2 here? Because we make room for both the unanchored
        // and anchored start states. Unanchored is first and then anchored.
        let mut starts_len = Start::len().checked_mul(2).unwrap();
        // ... but if we also want start states for every pattern, we make room
        // for that too.
        if self.dfa.get_config().get_starts_for_each_pattern() {
            starts_len += Start::len() * self.dfa.pattern_len();
        }
        self.cache
            .starts
            .extend(iter::repeat(self.as_ref().unknown_id()).take(starts_len));
        // This is the set of NFA states that corresponds to each of our three
        // sentinel states: the empty set.
        let dead = State::dead();
        // This sets up some states that we use as sentinels that are present
        // in every DFA. While it would be technically possible to implement
        // this DFA without explicitly putting these states in the transition
        // table, this is convenient to do to make `next_state` correct for all
        // valid state IDs without needing explicit conditionals to special
        // case these sentinel states.
        //
        // All three of these states are "dead" states. That is, all of
        // them transition only to themselves. So once you enter one of
        // these states, it's impossible to leave them. Thus, any correct
        // search routine must explicitly check for these state types. (Sans
        // `unknown`, since that is only used internally to represent missing
        // states.)
        let unk_id =
            self.add_state(dead.clone(), |id| id.to_unknown()).unwrap();
        let dead_id = self.add_state(dead.clone(), |id| id.to_dead()).unwrap();
        let quit_id = self.add_state(dead.clone(), |id| id.to_quit()).unwrap();
        assert_eq!(unk_id, self.as_ref().unknown_id());
        assert_eq!(dead_id, self.as_ref().dead_id());
        assert_eq!(quit_id, self.as_ref().quit_id());
        // The idea here is that if you start in an unknown/dead/quit state and
        // try to transition on them, then you should end up where you started.
        self.set_all_transitions(unk_id, unk_id);
        self.set_all_transitions(dead_id, dead_id);
        self.set_all_transitions(quit_id, quit_id);
        // All of these states are technically equivalent from the FSM
        // perspective, so putting all three of them in the cache isn't
        // possible. (They are distinct merely because we use their
        // identifiers as sentinels to mean something, as indicated by the
        // names.) Moreover, we wouldn't want to do that. Unknown and quit
        // states are special in that they are artificial constructions
        // this implementation. But dead states are a natural part of
        // determinization. When you reach a point in the NFA where you cannot
        // go anywhere else, a dead state will naturally arise and we MUST
        // reuse the canonical dead state that we've created here. Why? Because
        // it is the state ID that tells the search routine whether a state is
        // dead or not, and thus, whether to stop the search. Having a bunch of
        // distinct dead states would be quite wasteful!
        self.cache.states_to_id.insert(dead, dead_id);
    }

    /// Save the state corresponding to the ID given such that the state
    /// persists through a cache clearing.
    ///
    /// While the state may persist, the ID may not. In order to discover the
    /// new state ID, one must call 'saved_state_id' after a cache clearing.
    fn save_state(&mut self, id: LazyStateID) {
        let state = self.as_ref().get_cached_state(id).clone();
        self.cache.state_saver = StateSaver::ToSave { id, state };
    }

    /// Returns the updated lazy state ID for a state that was persisted
    /// through a cache clearing.
    ///
    /// It is only correct to call this routine when both a state has been
    /// saved and the cache has just been cleared. Otherwise, this panics.
    fn saved_state_id(&mut self) -> LazyStateID {
        self.cache
            .state_saver
            .take_saved()
            .expect("state saver does not have saved state ID")
    }

    /// Set all transitions on the state 'from' to 'to'.
    fn set_all_transitions(&mut self, from: LazyStateID, to: LazyStateID) {
        for unit in self.dfa.classes.representatives(..) {
            self.set_transition(from, unit, to);
        }
    }

    /// Set the transition on 'from' for 'unit' to 'to'.
    ///
    /// This panics if either 'from' or 'to' is invalid.
    ///
    /// All unit values are OK.
    fn set_transition(
        &mut self,
        from: LazyStateID,
        unit: alphabet::Unit,
        to: LazyStateID,
    ) {
        assert!(self.as_ref().is_valid(from), "invalid 'from' id: {:?}", from);
        assert!(self.as_ref().is_valid(to), "invalid 'to' id: {:?}", to);
        let offset =
            from.as_usize_untagged() + self.dfa.classes.get_by_unit(unit);
        self.cache.trans[offset] = to;
    }

    /// Set the start ID for the given pattern ID (if given) and starting
    /// configuration to the ID given.
    ///
    /// This panics if 'id' is not valid or if a pattern ID is given and
    /// 'starts_for_each_pattern' is not enabled.
    fn set_start_state(
        &mut self,
        anchored: Anchored,
        start: Start,
        id: LazyStateID,
    ) {
        assert!(self.as_ref().is_valid(id));
        let start_index = start.as_usize();
        let index = match anchored {
            Anchored::No => start_index,
            Anchored::Yes => Start::len() + start_index,
            Anchored::Pattern(pid) => {
                assert!(
                    self.dfa.get_config().get_starts_for_each_pattern(),
                    "attempted to search for a specific pattern \
                     without enabling starts_for_each_pattern",
                );
                let pid = pid.as_usize();
                (2 * Start::len()) + (Start::len() * pid) + start_index
            }
        };
        self.cache.starts[index] = id;
    }

    /// Returns a state builder from this DFA that might have existing
    /// capacity. This helps avoid allocs in cases where a state is built that
    /// turns out to already be cached.
    ///
    /// Callers must put the state builder back with 'put_state_builder',
    /// otherwise the allocation reuse won't work.
    fn get_state_builder(&mut self) -> StateBuilderEmpty {
        core::mem::replace(
            &mut self.cache.scratch_state_builder,
            StateBuilderEmpty::new(),
        )
    }

    /// Puts the given state builder back into this DFA for reuse.
    ///
    /// Note that building a 'State' from a builder always creates a new alloc,
    /// so callers should always put the builder back.
    fn put_state_builder(&mut self, builder: StateBuilderNFA) {
        let _ = core::mem::replace(
            &mut self.cache.scratch_state_builder,
            builder.clear(),
        );
    }
}

/// A type that groups methods that require the base NFA/DFA and read-only
/// access to the cache.
#[derive(Debug)]
struct LazyRef<'i, 'c> {
    dfa: &'i DFA,
    cache: &'c Cache,
}

impl<'i, 'c> LazyRef<'i, 'c> {
    /// Creates a new 'Lazy' wrapper for a DFA and its corresponding cache.
    fn new(dfa: &'i DFA, cache: &'c Cache) -> LazyRef<'i, 'c> {
        LazyRef { dfa, cache }
    }

    /// Return the ID of the start state for the given configuration.
    ///
    /// If the start state has not yet been computed, then this returns an
    /// unknown lazy state ID.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn get_cached_start_id(
        &self,
        anchored: Anchored,
        start: Start,
    ) -> Result<LazyStateID, StartError> {
        let start_index = start.as_usize();
        let index = match anchored {
            Anchored::No => start_index,
            Anchored::Yes => Start::len() + start_index,
            Anchored::Pattern(pid) => {
                if !self.dfa.get_config().get_starts_for_each_pattern() {
                    return Err(StartError::unsupported_anchored(anchored));
                }
                if pid.as_usize() >= self.dfa.pattern_len() {
                    return Ok(self.dead_id());
                }
                (2 * Start::len())
                    + (Start::len() * pid.as_usize())
                    + start_index
            }
        };
        Ok(self.cache.starts[index])
    }

    /// Return the cached NFA/DFA powerset state for the given ID.
    ///
    /// This panics if the given ID does not address a valid state.
    fn get_cached_state(&self, sid: LazyStateID) -> &State {
        let index = sid.as_usize_untagged() >> self.dfa.stride2();
        &self.cache.states[index]
    }

    /// Returns true if and only if the given ID corresponds to a "sentinel"
    /// state.
    ///
    /// A sentinel state is a state that signifies a special condition of
    /// search, and where every transition maps back to itself. See LazyStateID
    /// for more details. Note that start and match states are _not_ sentinels
    /// since they may otherwise be real states with non-trivial transitions.
    /// The purposes of sentinel states is purely to indicate something. Their
    /// transitions are not meant to be followed.
    fn is_sentinel(&self, id: LazyStateID) -> bool {
        id == self.unknown_id() || id == self.dead_id() || id == self.quit_id()
    }

    /// Returns the ID of the unknown state for this lazy DFA.
    fn unknown_id(&self) -> LazyStateID {
        // This unwrap is OK since 0 is always a valid state ID.
        LazyStateID::new(0).unwrap().to_unknown()
    }

    /// Returns the ID of the dead state for this lazy DFA.
    fn dead_id(&self) -> LazyStateID {
        // This unwrap is OK since the maximum value here is 1 * 512 = 512,
        // which is <= 2047 (the maximum state ID on 16-bit systems). Where
        // 512 is the worst case for our equivalence classes (every byte is a
        // distinct class).
        LazyStateID::new(1 << self.dfa.stride2()).unwrap().to_dead()
    }

    /// Returns the ID of the quit state for this lazy DFA.
    fn quit_id(&self) -> LazyStateID {
        // This unwrap is OK since the maximum value here is 2 * 512 = 1024,
        // which is <= 2047 (the maximum state ID on 16-bit systems). Where
        // 512 is the worst case for our equivalence classes (every byte is a
        // distinct class).
        LazyStateID::new(2 << self.dfa.stride2()).unwrap().to_quit()
    }

    /// Returns true if and only if the given ID is valid.
    ///
    /// An ID is valid if it is both a valid index into the transition table
    /// and is a multiple of the DFA's stride.
    fn is_valid(&self, id: LazyStateID) -> bool {
        let id = id.as_usize_untagged();
        id < self.cache.trans.len() && id % self.dfa.stride() == 0
    }

    /// Returns true if adding the state given would fit in this cache.
    fn state_fits_in_cache(&self, state: &State) -> bool {
        let needed = self.cache.memory_usage()
            + self.memory_usage_for_one_more_state(state.memory_usage());
        trace!(
            "lazy DFA cache capacity check: {:?} ?<=? {:?}",
            needed,
            self.dfa.cache_capacity
        );
        needed <= self.dfa.cache_capacity
    }

    /// Returns true if adding the state to be built by the given builder would
    /// fit in this cache.
    fn state_builder_fits_in_cache(&self, state: &StateBuilderNFA) -> bool {
        let needed = self.cache.memory_usage()
            + self.memory_usage_for_one_more_state(state.as_bytes().len());
        needed <= self.dfa.cache_capacity
    }

    /// Returns the additional memory usage, in bytes, required to add one more
    /// state to this cache. The given size should be the heap size, in bytes,
    /// that would be used by the new state being added.
    fn memory_usage_for_one_more_state(
        &self,
        state_heap_size: usize,
    ) -> usize {
        const ID_SIZE: usize = size_of::<LazyStateID>();
        const STATE_SIZE: usize = size_of::<State>();

        self.dfa.stride() * ID_SIZE // additional space needed in trans table
        + STATE_SIZE // space in cache.states
        + (STATE_SIZE + ID_SIZE) // space in cache.states_to_id
        + state_heap_size // heap memory used by state itself
    }
}

/// A simple type that encapsulates the saving of a state ID through a cache
/// clearing.
///
/// A state ID can be marked for saving with ToSave, while a state ID can be
/// saved itself with Saved.
#[derive(Clone, Debug)]
enum StateSaver {
    /// An empty state saver. In this case, no states (other than the special
    /// sentinel states) are preserved after clearing the cache.
    None,
    /// An ID of a state (and the state itself) that should be preserved after
    /// the lazy DFA's cache has been cleared. After clearing, the updated ID
    /// is stored in 'Saved' since it may have changed.
    ToSave { id: LazyStateID, state: State },
    /// An ID that of a state that has been persisted through a lazy DFA
    /// cache clearing. The ID recorded here corresponds to an ID that was
    /// once marked as ToSave. The IDs are likely not equivalent even though
    /// the states they point to are.
    Saved(LazyStateID),
}

impl StateSaver {
    /// Create an empty state saver.
    fn none() -> StateSaver {
        StateSaver::None
    }

    /// Replace this state saver with an empty saver, and if this saver is a
    /// request to save a state, return that request.
    fn take_to_save(&mut self) -> Option<(LazyStateID, State)> {
        match core::mem::replace(self, StateSaver::None) {
            StateSaver::None | StateSaver::Saved(_) => None,
            StateSaver::ToSave { id, state } => Some((id, state)),
        }
    }

    /// Replace this state saver with an empty saver, and if this saver is a
    /// saved state (or a request to save a state), return that state's ID.
    ///
    /// The idea here is that a request to save a state isn't necessarily
    /// honored because it might not be needed. e.g., Some higher level code
    /// might request a state to be saved on the off chance that the cache gets
    /// cleared when a new state is added at a lower level. But if that new
    /// state is never added, then the cache is never cleared and the state and
    /// its ID remain unchanged.
    fn take_saved(&mut self) -> Option<LazyStateID> {
        match core::mem::replace(self, StateSaver::None) {
            StateSaver::None => None,
            StateSaver::Saved(id) | StateSaver::ToSave { id, .. } => Some(id),
        }
    }
}

/// The configuration used for building a lazy DFA.
///
/// As a convenience, [`DFA::config`] is an alias for [`Config::new`]. The
/// advantage of the former is that it often lets you avoid importing the
/// `Config` type directly.
///
/// A lazy DFA configuration is a simple data object that is typically used
/// with [`Builder::configure`].
///
/// The default configuration guarantees that a search will never return a
/// "gave up" or "quit" error, although it is possible for a search to fail
/// if [`Config::starts_for_each_pattern`] wasn't enabled (which it is not by
/// default) and an [`Anchored::Pattern`] mode is requested via [`Input`].
#[derive(Clone, Debug, Default)]
pub struct Config {
    // As with other configuration types in this crate, we put all our knobs
    // in options so that we can distinguish between "default" and "not set."
    // This makes it possible to easily combine multiple configurations
    // without default values overwriting explicitly specified values. See the
    // 'overwrite' method.
    //
    // For docs on the fields below, see the corresponding method setters.
    match_kind: Option<MatchKind>,
    pre: Option<Option<Prefilter>>,
    starts_for_each_pattern: Option<bool>,
    byte_classes: Option<bool>,
    unicode_word_boundary: Option<bool>,
    quitset: Option<ByteSet>,
    specialize_start_states: Option<bool>,
    cache_capacity: Option<usize>,
    skip_cache_capacity_check: Option<bool>,
    minimum_cache_clear_count: Option<Option<usize>>,
    minimum_bytes_per_state: Option<Option<usize>>,
}

impl Config {
    /// Return a new default lazy DFA builder configuration.
    pub fn new() -> Config {
        Config::default()
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
    /// where all possible matches are added to the lazy DFA.
    ///
    /// Typically, `All` is used when one wants to execute an overlapping
    /// search and `LeftmostFirst` otherwise. In particular, it rarely makes
    /// sense to use `All` with the various "leftmost" find routines, since the
    /// leftmost routines depend on the `LeftmostFirst` automata construction
    /// strategy. Specifically, `LeftmostFirst` adds dead states to the
    /// lazy DFA as a way to terminate the search and report a match.
    /// `LeftmostFirst` also supports non-greedy matches using this strategy
    /// where as `All` does not.
    ///
    /// # Example: overlapping search
    ///
    /// This example shows the typical use of `MatchKind::All`, which is to
    /// report overlapping matches.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{
    ///     hybrid::dfa::{DFA, OverlappingState},
    ///     HalfMatch, Input, MatchKind,
    /// };
    ///
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().match_kind(MatchKind::All))
    ///     .build_many(&[r"\w+$", r"\S+$"])?;
    /// let mut cache = dfa.create_cache();
    /// let haystack = "@foo";
    /// let mut state = OverlappingState::start();
    ///
    /// let expected = Some(HalfMatch::must(1, 4));
    /// dfa.try_search_overlapping_fwd(
    ///     &mut cache, &Input::new(haystack), &mut state,
    /// )?;
    /// assert_eq!(expected, state.get_match());
    ///
    /// // The first pattern also matches at the same position, so re-running
    /// // the search will yield another match. Notice also that the first
    /// // pattern is returned after the second. This is because the second
    /// // pattern begins its match before the first, is therefore an earlier
    /// // match and is thus reported first.
    /// let expected = Some(HalfMatch::must(0, 4));
    /// dfa.try_search_overlapping_fwd(
    ///     &mut cache, &Input::new(haystack), &mut state,
    /// )?;
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
    /// [`hybrid::regex::Regex`](crate::hybrid::regex::Regex) will handle this
    /// for you, so it's usually not necessary to do this yourself.
    ///
    /// ```
    /// use regex_automata::{
    ///     hybrid::dfa::DFA,
    ///     nfa::thompson::NFA,
    ///     Anchored, HalfMatch, Input, MatchKind,
    /// };
    ///
    /// let input = Input::new("123foobar456");
    /// let pattern = r"[a-z]+r";
    ///
    /// let dfa_fwd = DFA::new(pattern)?;
    /// let dfa_rev = DFA::builder()
    ///     .thompson(NFA::config().reverse(true))
    ///     .configure(DFA::config().match_kind(MatchKind::All))
    ///     .build(pattern)?;
    /// let mut cache_fwd = dfa_fwd.create_cache();
    /// let mut cache_rev = dfa_rev.create_cache();
    ///
    /// let expected_fwd = HalfMatch::must(0, 9);
    /// let expected_rev = HalfMatch::must(0, 3);
    /// let got_fwd = dfa_fwd.try_search_fwd(&mut cache_fwd, &input)?.unwrap();
    /// // Here we don't specify the pattern to search for since there's only
    /// // one pattern and we're doing a leftmost search. But if this were an
    /// // overlapping search, you'd need to specify the pattern that matched
    /// // in the forward direction. (Otherwise, you might wind up finding the
    /// // starting position of a match of some other pattern.) That in turn
    /// // requires building the reverse automaton with starts_for_each_pattern
    /// // enabled.
    /// let input = input
    ///     .clone()
    ///     .range(..got_fwd.offset())
    ///     .anchored(Anchored::Yes);
    /// let got_rev = dfa_rev.try_search_rev(&mut cache_rev, &input)?.unwrap();
    /// assert_eq!(expected_fwd, got_fwd);
    /// assert_eq!(expected_rev, got_rev);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn match_kind(mut self, kind: MatchKind) -> Config {
        self.match_kind = Some(kind);
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
    /// By default no prefilter is set.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{
    ///     hybrid::dfa::DFA,
    ///     util::prefilter::Prefilter,
    ///     Input, HalfMatch, MatchKind,
    /// };
    ///
    /// let pre = Prefilter::new(MatchKind::LeftmostFirst, &["foo", "bar"]);
    /// let re = DFA::builder()
    ///     .configure(DFA::config().prefilter(pre))
    ///     .build(r"(foo|bar)[a-z]+")?;
    /// let mut cache = re.create_cache();
    /// let input = Input::new("foo1 barfox bar");
    /// assert_eq!(
    ///     Some(HalfMatch::must(0, 11)),
    ///     re.try_search_fwd(&mut cache, &input)?,
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
    ///     hybrid::dfa::DFA,
    ///     util::prefilter::Prefilter,
    ///     Input, HalfMatch, MatchKind,
    /// };
    ///
    /// let pre = Prefilter::new(MatchKind::LeftmostFirst, &["foo", "car"]);
    /// let re = DFA::builder()
    ///     .configure(DFA::config().prefilter(pre))
    ///     .build(r"(foo|bar)[a-z]+")?;
    /// let mut cache = re.create_cache();
    /// let input = Input::new("foo1 barfox bar");
    /// assert_eq!(
    ///     // No match reported even though there clearly is one!
    ///     None,
    ///     re.try_search_fwd(&mut cache, &input)?,
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

    /// Whether to compile a separate start state for each pattern in the
    /// lazy DFA.
    ///
    /// When enabled, a separate **anchored** start state is added for each
    /// pattern in the lazy DFA. When this start state is used, then the DFA
    /// will only search for matches for the pattern specified, even if there
    /// are other patterns in the DFA.
    ///
    /// The main downside of this option is that it can potentially increase
    /// the size of the DFA and/or increase the time it takes to build the
    /// DFA at search time. However, since this is configuration for a lazy
    /// DFA, these states aren't actually built unless they're used. Enabling
    /// this isn't necessarily free, however, as it may result in higher cache
    /// usage.
    ///
    /// There are a few reasons one might want to enable this (it's disabled
    /// by default):
    ///
    /// 1. When looking for the start of an overlapping match (using a reverse
    /// DFA), doing it correctly requires starting the reverse search using the
    /// starting state of the pattern that matched in the forward direction.
    /// Indeed, when building a [`Regex`](crate::hybrid::regex::Regex), it
    /// will automatically enable this option when building the reverse DFA
    /// internally.
    /// 2. When you want to use a DFA with multiple patterns to both search
    /// for matches of any pattern or to search for anchored matches of one
    /// particular pattern while using the same DFA. (Otherwise, you would need
    /// to compile a new DFA for each pattern.)
    ///
    /// By default this is disabled.
    ///
    /// # Example
    ///
    /// This example shows how to use this option to permit the same lazy DFA
    /// to run both general searches for any pattern and anchored searches for
    /// a specific pattern.
    ///
    /// ```
    /// use regex_automata::{
    ///     hybrid::dfa::DFA,
    ///     Anchored, HalfMatch, Input, PatternID,
    /// };
    ///
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().starts_for_each_pattern(true))
    ///     .build_many(&[r"[a-z0-9]{6}", r"[a-z][a-z0-9]{5}"])?;
    /// let mut cache = dfa.create_cache();
    /// let haystack = "bar foo123";
    ///
    /// // Here's a normal unanchored search that looks for any pattern.
    /// let expected = HalfMatch::must(0, 10);
    /// let input = Input::new(haystack);
    /// assert_eq!(Some(expected), dfa.try_search_fwd(&mut cache, &input)?);
    /// // We can also do a normal anchored search for any pattern. Since it's
    /// // an anchored search, we position the start of the search where we
    /// // know the match will begin.
    /// let expected = HalfMatch::must(0, 10);
    /// let input = Input::new(haystack).range(4..);
    /// assert_eq!(Some(expected), dfa.try_search_fwd(&mut cache, &input)?);
    /// // Since we compiled anchored start states for each pattern, we can
    /// // also look for matches of other patterns explicitly, even if a
    /// // different pattern would have normally matched.
    /// let expected = HalfMatch::must(1, 10);
    /// let input = Input::new(haystack)
    ///     .range(4..)
    ///     .anchored(Anchored::Pattern(PatternID::must(1)));
    /// assert_eq!(Some(expected), dfa.try_search_fwd(&mut cache, &input)?);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn starts_for_each_pattern(mut self, yes: bool) -> Config {
        self.starts_for_each_pattern = Some(yes);
        self
    }

    /// Whether to attempt to shrink the size of the lazy DFA's alphabet or
    /// not.
    ///
    /// This option is enabled by default and should never be disabled unless
    /// one is debugging the lazy DFA.
    ///
    /// When enabled, the lazy DFA will use a map from all possible bytes
    /// to their corresponding equivalence class. Each equivalence class
    /// represents a set of bytes that does not discriminate between a match
    /// and a non-match in the DFA. For example, the pattern `[ab]+` has at
    /// least two equivalence classes: a set containing `a` and `b` and a set
    /// containing every byte except for `a` and `b`. `a` and `b` are in the
    /// same equivalence classes because they never discriminate between a
    /// match and a non-match.
    ///
    /// The advantage of this map is that the size of the transition table
    /// can be reduced drastically from `#states * 256 * sizeof(LazyStateID)`
    /// to `#states * k * sizeof(LazyStateID)` where `k` is the number of
    /// equivalence classes (rounded up to the nearest power of 2). As a
    /// result, total space usage can decrease substantially. Moreover, since a
    /// smaller alphabet is used, DFA compilation during search becomes faster
    /// as well since it will potentially be able to reuse a single transition
    /// for multiple bytes.
    ///
    /// **WARNING:** This is only useful for debugging lazy DFAs. Disabling
    /// this does not yield any speed advantages. Namely, even when this is
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
    /// [`MatchError::quit`] error is returned.
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
    /// When enabling this option, callers _must_ be prepared to
    /// handle a [`MatchError`] error during search. When using a
    /// [`Regex`](crate::hybrid::regex::Regex), this corresponds to using the
    /// `try_` suite of methods. Alternatively, if callers can guarantee that
    /// their input is ASCII only, then a [`MatchError::quit`] error will never
    /// be returned while searching.
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
    ///     hybrid::dfa::DFA,
    ///     HalfMatch, Input, MatchError,
    /// };
    ///
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().unicode_word_boundary(true))
    ///     .build(r"\b[0-9]+\b")?;
    /// let mut cache = dfa.create_cache();
    ///
    /// // The match occurs before the search ever observes the snowman
    /// // character, so no error occurs.
    /// let haystack = "foo 123  ☃";
    /// let expected = Some(HalfMatch::must(0, 7));
    /// let got = dfa.try_search_fwd(&mut cache, &Input::new(haystack))?;
    /// assert_eq!(expected, got);
    ///
    /// // Notice that this search fails, even though the snowman character
    /// // occurs after the ending match offset. This is because search
    /// // routines read one byte past the end of the search to account for
    /// // look-around, and indeed, this is required here to determine whether
    /// // the trailing \b matches.
    /// let haystack = "foo 123 ☃";
    /// let expected = MatchError::quit(0xE2, 8);
    /// let got = dfa.try_search_fwd(&mut cache, &Input::new(haystack));
    /// assert_eq!(Err(expected), got);
    ///
    /// // Another example is executing a search where the span of the haystack
    /// // we specify is all ASCII, but there is non-ASCII just before it. This
    /// // correctly also reports an error.
    /// let input = Input::new("β123").range(2..);
    /// let expected = MatchError::quit(0xB2, 1);
    /// let got = dfa.try_search_fwd(&mut cache, &input);
    /// assert_eq!(Err(expected), got);
    ///
    /// // And similarly for the trailing word boundary.
    /// let input = Input::new("123β").range(..3);
    /// let expected = MatchError::quit(0xCE, 3);
    /// let got = dfa.try_search_fwd(&mut cache, &input);
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

    /// Add a "quit" byte to the lazy DFA.
    ///
    /// When a quit byte is seen during search time, then search will return a
    /// [`MatchError::quit`] error indicating the offset at which the search
    /// stopped.
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
    /// building lazy DFAs. Of course, callers should enable
    /// [`Config::unicode_word_boundary`] if they want this behavior instead.
    /// (The advantage being that non-ASCII quit bytes will only be added if a
    /// Unicode word boundary is in the pattern.)
    ///
    /// When enabling this option, callers _must_ be prepared to
    /// handle a [`MatchError`] error during search. When using a
    /// [`Regex`](crate::hybrid::regex::Regex), this corresponds to using the
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
    /// use regex_automata::{hybrid::dfa::DFA, MatchError, Input};
    ///
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().quit(b'\n', true))
    ///     .build(r"foo\p{any}+bar")?;
    /// let mut cache = dfa.create_cache();
    ///
    /// let haystack = "foo\nbar";
    /// // Normally this would produce a match, since \p{any} contains '\n'.
    /// // But since we instructed the automaton to enter a quit state if a
    /// // '\n' is observed, this produces a match error instead.
    /// let expected = MatchError::quit(b'\n', 3);
    /// let got = dfa.try_search_fwd(
    ///     &mut cache,
    ///     &Input::new(haystack),
    /// ).unwrap_err();
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

    /// Enable specializing start states in the lazy DFA.
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
    /// use regex_automata::{hybrid::dfa::DFA, MatchError, Input};
    ///
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().specialize_start_states(true))
    ///     .build(r"[a-z]+")?;
    /// let mut cache = dfa.create_cache();
    ///
    /// let haystack = "123 foobar 4567".as_bytes();
    /// let sid = dfa.start_state_forward(&mut cache, &Input::new(haystack))?;
    /// // The ID returned by 'start_state_forward' will always be tagged as
    /// // a start state when start state specialization is enabled.
    /// assert!(sid.is_tagged());
    /// assert!(sid.is_start());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Compare the above with the default lazy DFA configuration where
    /// start states are _not_ specialized. In this case, the start state
    /// is not tagged and `sid.is_start()` returns false.
    ///
    /// ```
    /// use regex_automata::{hybrid::dfa::DFA, MatchError, Input};
    ///
    /// let dfa = DFA::new(r"[a-z]+")?;
    /// let mut cache = dfa.create_cache();
    ///
    /// let haystack = "123 foobar 4567".as_bytes();
    /// let sid = dfa.start_state_forward(&mut cache, &Input::new(haystack))?;
    /// // Start states are not tagged in the default configuration!
    /// assert!(!sid.is_tagged());
    /// assert!(!sid.is_start());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn specialize_start_states(mut self, yes: bool) -> Config {
        self.specialize_start_states = Some(yes);
        self
    }

    /// Sets the maximum amount of heap memory, in bytes, to allocate to the
    /// cache for use during a lazy DFA search. If the lazy DFA would otherwise
    /// use more heap memory, then, depending on other configuration knobs,
    /// either stop the search and return an error or clear the cache and
    /// continue the search.
    ///
    /// The default cache capacity is some "reasonable" number that will
    /// accommodate most regular expressions. You may find that if you need
    /// to build a large DFA then it may be necessary to increase the cache
    /// capacity.
    ///
    /// Note that while building a lazy DFA will do a "minimum" check to ensure
    /// the capacity is big enough, this is more or less about correctness.
    /// If the cache is bigger than the minimum but still "too small," then the
    /// lazy DFA could wind up spending a lot of time clearing the cache and
    /// recomputing transitions, thus negating the performance benefits of a
    /// lazy DFA. Thus, setting the cache capacity is mostly an experimental
    /// endeavor. For most common patterns, however, the default should be
    /// sufficient.
    ///
    /// For more details on how the lazy DFA's cache is used, see the
    /// documentation for [`Cache`].
    ///
    /// # Example
    ///
    /// This example shows what happens if the configured cache capacity is
    /// too small. In such cases, one can override the cache capacity to make
    /// it bigger. Alternatively, one might want to use less memory by setting
    /// a smaller cache capacity.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{hybrid::dfa::DFA, HalfMatch, Input};
    ///
    /// let pattern = r"\p{L}{1000}";
    ///
    /// // The default cache capacity is likely too small to deal with regexes
    /// // that are very large. Large repetitions of large Unicode character
    /// // classes are a common way to make very large regexes.
    /// let _ = DFA::new(pattern).unwrap_err();
    /// // Bump up the capacity to something bigger.
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().cache_capacity(100 * (1<<20))) // 100 MB
    ///     .build(pattern)?;
    /// let mut cache = dfa.create_cache();
    ///
    /// let haystack = "ͰͲͶͿΆΈΉΊΌΎΏΑΒΓΔΕΖΗΘΙ".repeat(50);
    /// let expected = Some(HalfMatch::must(0, 2000));
    /// let got = dfa.try_search_fwd(&mut cache, &Input::new(&haystack))?;
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn cache_capacity(mut self, bytes: usize) -> Config {
        self.cache_capacity = Some(bytes);
        self
    }

    /// Configures construction of a lazy DFA to use the minimum cache capacity
    /// if the configured capacity is otherwise too small for the provided NFA.
    ///
    /// This is useful if you never want lazy DFA construction to fail because
    /// of a capacity that is too small.
    ///
    /// In general, this option is typically not a good idea. In particular,
    /// while a minimum cache capacity does permit the lazy DFA to function
    /// where it otherwise couldn't, it's plausible that it may not function
    /// well if it's constantly running out of room. In that case, the speed
    /// advantages of the lazy DFA may be negated. On the other hand, the
    /// "minimum" cache capacity computed may not be completely accurate and
    /// could actually be bigger than what is really necessary. Therefore, it
    /// is plausible that using the minimum cache capacity could still result
    /// in very good performance.
    ///
    /// This is disabled by default.
    ///
    /// # Example
    ///
    /// This example shows what happens if the configured cache capacity is
    /// too small. In such cases, one could override the capacity explicitly.
    /// An alternative, demonstrated here, let's us force construction to use
    /// the minimum cache capacity if the configured capacity is otherwise
    /// too small.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{hybrid::dfa::DFA, HalfMatch, Input};
    ///
    /// let pattern = r"\p{L}{1000}";
    ///
    /// // The default cache capacity is likely too small to deal with regexes
    /// // that are very large. Large repetitions of large Unicode character
    /// // classes are a common way to make very large regexes.
    /// let _ = DFA::new(pattern).unwrap_err();
    /// // Configure construction such it automatically selects the minimum
    /// // cache capacity if it would otherwise be too small.
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().skip_cache_capacity_check(true))
    ///     .build(pattern)?;
    /// let mut cache = dfa.create_cache();
    ///
    /// let haystack = "ͰͲͶͿΆΈΉΊΌΎΏΑΒΓΔΕΖΗΘΙ".repeat(50);
    /// let expected = Some(HalfMatch::must(0, 2000));
    /// let got = dfa.try_search_fwd(&mut cache, &Input::new(&haystack))?;
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn skip_cache_capacity_check(mut self, yes: bool) -> Config {
        self.skip_cache_capacity_check = Some(yes);
        self
    }

    /// Configure a lazy DFA search to quit after a certain number of cache
    /// clearings.
    ///
    /// When a minimum is set, then a lazy DFA search will *possibly* "give
    /// up" after the minimum number of cache clearings has occurred. This is
    /// typically useful in scenarios where callers want to detect whether the
    /// lazy DFA search is "efficient" or not. If the cache is cleared too many
    /// times, this is a good indicator that it is not efficient, and thus, the
    /// caller may wish to use some other regex engine.
    ///
    /// Note that the number of times a cache is cleared is a property of
    /// the cache itself. Thus, if a cache is used in a subsequent search
    /// with a similarly configured lazy DFA, then it could cause the
    /// search to "give up" if the cache needed to be cleared, depending
    /// on its internal count and configured minimum. The cache clear
    /// count can only be reset to `0` via [`DFA::reset_cache`] (or
    /// [`Regex::reset_cache`](crate::hybrid::regex::Regex::reset_cache) if
    /// you're using the `Regex` API).
    ///
    /// By default, no minimum is configured. Thus, a lazy DFA search will
    /// never give up due to cache clearings. If you do set this option, you
    /// might consider also setting [`Config::minimum_bytes_per_state`] in
    /// order for the lazy DFA to take efficiency into account before giving
    /// up.
    ///
    /// # Example
    ///
    /// This example uses a somewhat pathological configuration to demonstrate
    /// the _possible_ behavior of cache clearing and how it might result
    /// in a search that returns an error.
    ///
    /// It is important to note that the precise mechanics of how and when
    /// a cache gets cleared is an implementation detail.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{hybrid::dfa::DFA, Input, MatchError, MatchErrorKind};
    ///
    /// // This is a carefully chosen regex. The idea is to pick one
    /// // that requires some decent number of states (hence the bounded
    /// // repetition). But we specifically choose to create a class with an
    /// // ASCII letter and a non-ASCII letter so that we can check that no new
    /// // states are created once the cache is full. Namely, if we fill up the
    /// // cache on a haystack of 'a's, then in order to match one 'β', a new
    /// // state will need to be created since a 'β' is encoded with multiple
    /// // bytes. Since there's no room for this state, the search should quit
    /// // at the very first position.
    /// let pattern = r"[aβ]{100}";
    /// let dfa = DFA::builder()
    ///     .configure(
    ///         // Configure it so that we have the minimum cache capacity
    ///         // possible. And that if any clearings occur, the search quits.
    ///         DFA::config()
    ///             .skip_cache_capacity_check(true)
    ///             .cache_capacity(0)
    ///             .minimum_cache_clear_count(Some(0)),
    ///     )
    ///     .build(pattern)?;
    /// let mut cache = dfa.create_cache();
    ///
    /// // Our search will give up before reaching the end!
    /// let haystack = "a".repeat(101).into_bytes();
    /// let result = dfa.try_search_fwd(&mut cache, &Input::new(&haystack));
    /// assert!(matches!(
    ///     *result.unwrap_err().kind(),
    ///     MatchErrorKind::GaveUp { .. },
    /// ));
    ///
    /// // Now that we know the cache is full, if we search a haystack that we
    /// // know will require creating at least one new state, it should not
    /// // be able to make much progress.
    /// let haystack = "β".repeat(101).into_bytes();
    /// let result = dfa.try_search_fwd(&mut cache, &Input::new(&haystack));
    /// assert!(matches!(
    ///     *result.unwrap_err().kind(),
    ///     MatchErrorKind::GaveUp { .. },
    /// ));
    ///
    /// // If we reset the cache, then we should be able to create more states
    /// // and make more progress with searching for betas.
    /// cache.reset(&dfa);
    /// let haystack = "β".repeat(101).into_bytes();
    /// let result = dfa.try_search_fwd(&mut cache, &Input::new(&haystack));
    /// assert!(matches!(
    ///     *result.unwrap_err().kind(),
    ///     MatchErrorKind::GaveUp { .. },
    /// ));
    ///
    /// // ... switching back to ASCII still makes progress since it just needs
    /// // to set transitions on existing states!
    /// let haystack = "a".repeat(101).into_bytes();
    /// let result = dfa.try_search_fwd(&mut cache, &Input::new(&haystack));
    /// assert!(matches!(
    ///     *result.unwrap_err().kind(),
    ///     MatchErrorKind::GaveUp { .. },
    /// ));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn minimum_cache_clear_count(mut self, min: Option<usize>) -> Config {
        self.minimum_cache_clear_count = Some(min);
        self
    }

    /// Configure a lazy DFA search to quit only when its efficiency drops
    /// below the given minimum.
    ///
    /// The efficiency of the cache is determined by the number of DFA states
    /// compiled per byte of haystack searched. For example, if the efficiency
    /// is 2, then it means the lazy DFA is creating a new DFA state after
    /// searching approximately 2 bytes in a haystack. Generally speaking, 2
    /// is quite bad and it's likely that even a slower regex engine like the
    /// [`PikeVM`](crate::nfa::thompson::pikevm::PikeVM) would be faster.
    ///
    /// This has no effect if [`Config::minimum_cache_clear_count`] is not set.
    /// Namely, this option only kicks in when the cache has been cleared more
    /// than the minimum number. If no minimum is set, then the cache is simply
    /// cleared whenever it fills up and it is impossible for the lazy DFA to
    /// quit due to ineffective use of the cache.
    ///
    /// In general, if one is setting [`Config::minimum_cache_clear_count`],
    /// then one should probably also set this knob as well. The reason is
    /// that the absolute number of times the cache is cleared is generally
    /// not a great predictor of efficiency. For example, if a new DFA state
    /// is created for every 1,000 bytes searched, then it wouldn't be hard
    /// for the cache to get cleared more than `N` times and then cause the
    /// lazy DFA to quit. But a new DFA state every 1,000 bytes is likely quite
    /// good from a performance perspective, and it's likely that the lazy
    /// DFA should continue searching, even if it requires clearing the cache
    /// occasionally.
    ///
    /// Finally, note that if you're implementing your own lazy DFA search
    /// routine and also want this efficiency check to work correctly, then
    /// you'll need to use the following routines to record search progress:
    ///
    /// * Call [`Cache::search_start`] at the beginning of every search.
    /// * Call [`Cache::search_update`] whenever [`DFA::next_state`] is
    /// called.
    /// * Call [`Cache::search_finish`] before completing a search. (It is
    /// not strictly necessary to call this when an error is returned, as
    /// `Cache::search_start` will automatically finish the previous search
    /// for you. But calling it where possible before returning helps improve
    /// the accuracy of how many bytes have actually been searched.)
    pub fn minimum_bytes_per_state(mut self, min: Option<usize>) -> Config {
        self.minimum_bytes_per_state = Some(min);
        self
    }

    /// Returns the match semantics set in this configuration.
    pub fn get_match_kind(&self) -> MatchKind {
        self.match_kind.unwrap_or(MatchKind::LeftmostFirst)
    }

    /// Returns the prefilter set in this configuration, if one at all.
    pub fn get_prefilter(&self) -> Option<&Prefilter> {
        self.pre.as_ref().unwrap_or(&None).as_ref()
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

    /// Returns whether this configuration will instruct the lazy DFA to enter
    /// a quit state whenever the given byte is seen during a search. When at
    /// least one byte has this enabled, it is possible for a search to return
    /// an error.
    pub fn get_quit(&self, byte: u8) -> bool {
        self.quitset.map_or(false, |q| q.contains(byte))
    }

    /// Returns whether this configuration will instruct the lazy DFA to
    /// "specialize" start states. When enabled, the lazy DFA will tag start
    /// states so that search routines using the lazy DFA can detect when
    /// it's in a start state and do some kind of optimization (like run a
    /// prefilter).
    pub fn get_specialize_start_states(&self) -> bool {
        self.specialize_start_states.unwrap_or(false)
    }

    /// Returns the cache capacity set on this configuration.
    pub fn get_cache_capacity(&self) -> usize {
        self.cache_capacity.unwrap_or(2 * (1 << 20))
    }

    /// Returns whether the cache capacity check should be skipped.
    pub fn get_skip_cache_capacity_check(&self) -> bool {
        self.skip_cache_capacity_check.unwrap_or(false)
    }

    /// Returns, if set, the minimum number of times the cache must be cleared
    /// before a lazy DFA search can give up. When no minimum is set, then a
    /// search will never quit and will always clear the cache whenever it
    /// fills up.
    pub fn get_minimum_cache_clear_count(&self) -> Option<usize> {
        self.minimum_cache_clear_count.unwrap_or(None)
    }

    /// Returns, if set, the minimum number of bytes per state that need to be
    /// processed in order for the lazy DFA to keep going. If the minimum falls
    /// below this number (and the cache has been cleared a minimum number of
    /// times), then the lazy DFA will return a "gave up" error.
    pub fn get_minimum_bytes_per_state(&self) -> Option<usize> {
        self.minimum_bytes_per_state.unwrap_or(None)
    }

    /// Returns the minimum lazy DFA cache capacity required for the given NFA.
    ///
    /// The cache capacity required for a particular NFA may change without
    /// notice. Callers should not rely on it being stable.
    ///
    /// This is useful for informational purposes, but can also be useful for
    /// other reasons. For example, if one wants to check the minimum cache
    /// capacity themselves or if one wants to set the capacity based on the
    /// minimum.
    ///
    /// This may return an error if this configuration does not support all of
    /// the instructions used in the given NFA. For example, if the NFA has a
    /// Unicode word boundary but this configuration does not enable heuristic
    /// support for Unicode word boundaries.
    pub fn get_minimum_cache_capacity(
        &self,
        nfa: &thompson::NFA,
    ) -> Result<usize, BuildError> {
        let quitset = self.quit_set_from_nfa(nfa)?;
        let classes = self.byte_classes_from_nfa(nfa, &quitset);
        let starts = self.get_starts_for_each_pattern();
        Ok(minimum_cache_capacity(nfa, &classes, starts))
    }

    /// Returns the byte class map used during search from the given NFA.
    ///
    /// If byte classes are disabled on this configuration, then a map is
    /// returned that puts each byte in its own equivalent class.
    fn byte_classes_from_nfa(
        &self,
        nfa: &thompson::NFA,
        quit: &ByteSet,
    ) -> ByteClasses {
        if !self.get_byte_classes() {
            // The lazy DFA will always use the equivalence class map, but
            // enabling this option is useful for debugging. Namely, this will
            // cause all transitions to be defined over their actual bytes
            // instead of an opaque equivalence class identifier. The former is
            // much easier to grok as a human.
            ByteClasses::singletons()
        } else {
            let mut set = nfa.byte_class_set().clone();
            // It is important to distinguish any "quit" bytes from all other
            // bytes. Otherwise, a non-quit byte may end up in the same class
            // as a quit byte, and thus cause the DFA stop when it shouldn't.
            //
            // Test case:
            //
            //   regex-cli find match hybrid --unicode-word-boundary \
            //     -p '^#' -p '\b10\.55\.182\.100\b' -y @conn.json.1000x.log
            if !quit.is_empty() {
                set.add_set(&quit);
            }
            set.byte_classes()
        }
    }

    /// Return the quit set for this configuration and the given NFA.
    ///
    /// This may return an error if the NFA is incompatible with this
    /// configuration's quit set. For example, if the NFA has a Unicode word
    /// boundary and the quit set doesn't include non-ASCII bytes.
    fn quit_set_from_nfa(
        &self,
        nfa: &thompson::NFA,
    ) -> Result<ByteSet, BuildError> {
        let mut quit = self.quitset.unwrap_or(ByteSet::empty());
        if nfa.look_set_any().contains_word_unicode() {
            if self.get_unicode_word_boundary() {
                for b in 0x80..=0xFF {
                    quit.add(b);
                }
            } else {
                // If heuristic support for Unicode word boundaries wasn't
                // enabled, then we can still check if our quit set is correct.
                // If the caller set their quit bytes in a way that causes the
                // DFA to quit on at least all non-ASCII bytes, then that's all
                // we need for heuristic support to work.
                if !quit.contains_range(0x80, 0xFF) {
                    return Err(
                        BuildError::unsupported_dfa_word_boundary_unicode(),
                    );
                }
            }
        }
        Ok(quit)
    }

    /// Overwrite the default configuration such that the options in `o` are
    /// always used. If an option in `o` is not set, then the corresponding
    /// option in `self` is used. If it's not set in `self` either, then it
    /// remains not set.
    fn overwrite(&self, o: Config) -> Config {
        Config {
            match_kind: o.match_kind.or(self.match_kind),
            pre: o.pre.or_else(|| self.pre.clone()),
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
            cache_capacity: o.cache_capacity.or(self.cache_capacity),
            skip_cache_capacity_check: o
                .skip_cache_capacity_check
                .or(self.skip_cache_capacity_check),
            minimum_cache_clear_count: o
                .minimum_cache_clear_count
                .or(self.minimum_cache_clear_count),
            minimum_bytes_per_state: o
                .minimum_bytes_per_state
                .or(self.minimum_bytes_per_state),
        }
    }
}

/// A builder for constructing a lazy deterministic finite automaton from
/// regular expressions.
///
/// As a convenience, [`DFA::builder`] is an alias for [`Builder::new`]. The
/// advantage of the former is that it often lets you avoid importing the
/// `Builder` type directly.
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
/// This builder always constructs a *single* lazy DFA. As such, this builder
/// can only be used to construct regexes that either detect the presence
/// of a match or find the end location of a match. A single DFA cannot
/// produce both the start and end of a match. For that information, use a
/// [`Regex`](crate::hybrid::regex::Regex), which can be similarly configured
/// using [`regex::Builder`](crate::hybrid::regex::Builder). The main reason
/// to use a DFA directly is if the end location of a match is enough for your
/// use case. Namely, a `Regex` will construct two lazy DFAs instead of one,
/// since a second reverse DFA is needed to find the start of a match.
///
/// # Example
///
/// This example shows how to build a lazy DFA that uses a tiny cache capacity
/// and completely disables Unicode. That is:
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
///     hybrid::dfa::DFA,
///     nfa::thompson,
///     util::syntax,
///     HalfMatch, Input,
/// };
///
/// let dfa = DFA::builder()
///     .configure(DFA::config().cache_capacity(5_000))
///     .thompson(thompson::Config::new().utf8(false))
///     .syntax(syntax::Config::new().unicode(false).utf8(false))
///     .build(r"foo[^b]ar.*")?;
/// let mut cache = dfa.create_cache();
///
/// let haystack = b"\xFEfoo\xFFar\xE2\x98\xFF\n";
/// let expected = Some(HalfMatch::must(0, 10));
/// let got = dfa.try_search_fwd(&mut cache, &Input::new(haystack))?;
/// assert_eq!(expected, got);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct Builder {
    config: Config,
    #[cfg(feature = "syntax")]
    thompson: thompson::Compiler,
}

impl Builder {
    /// Create a new lazy DFA builder with the default configuration.
    pub fn new() -> Builder {
        Builder {
            config: Config::default(),
            #[cfg(feature = "syntax")]
            thompson: thompson::Compiler::new(),
        }
    }

    /// Build a lazy DFA from the given pattern.
    ///
    /// If there was a problem parsing or compiling the pattern, then an error
    /// is returned.
    #[cfg(feature = "syntax")]
    pub fn build(&self, pattern: &str) -> Result<DFA, BuildError> {
        self.build_many(&[pattern])
    }

    /// Build a lazy DFA from the given patterns.
    ///
    /// When matches are returned, the pattern ID corresponds to the index of
    /// the pattern in the slice given.
    #[cfg(feature = "syntax")]
    pub fn build_many<P: AsRef<str>>(
        &self,
        patterns: &[P],
    ) -> Result<DFA, BuildError> {
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
        self.build_from_nfa(nfa)
    }

    /// Build a DFA from the given NFA.
    ///
    /// Note that this requires owning a `thompson::NFA`. While this may force
    /// you to clone the NFA, such a clone is not a deep clone. Namely, NFAs
    /// are defined internally to support shared ownership such that cloning is
    /// very cheap.
    ///
    /// # Example
    ///
    /// This example shows how to build a lazy DFA if you already have an NFA
    /// in hand.
    ///
    /// ```
    /// use regex_automata::{
    ///     hybrid::dfa::DFA,
    ///     nfa::thompson,
    ///     HalfMatch, Input,
    /// };
    ///
    /// let haystack = "foo123bar";
    ///
    /// // This shows how to set non-default options for building an NFA.
    /// let nfa = thompson::Compiler::new()
    ///     .configure(thompson::Config::new().shrink(true))
    ///     .build(r"[0-9]+")?;
    /// let dfa = DFA::builder().build_from_nfa(nfa)?;
    /// let mut cache = dfa.create_cache();
    /// let expected = Some(HalfMatch::must(0, 6));
    /// let got = dfa.try_search_fwd(&mut cache, &Input::new(haystack))?;
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn build_from_nfa(
        &self,
        nfa: thompson::NFA,
    ) -> Result<DFA, BuildError> {
        let quitset = self.config.quit_set_from_nfa(&nfa)?;
        let classes = self.config.byte_classes_from_nfa(&nfa, &quitset);
        // Check that we can fit at least a few states into our cache,
        // otherwise it's pretty senseless to use the lazy DFA. This does have
        // a possible failure mode though. This assumes the maximum size of a
        // state in powerset space (so, the total number of NFA states), which
        // may never actually materialize, and could be quite a bit larger
        // than the actual biggest state. If this turns out to be a problem,
        // we could expose a knob that disables this check. But if so, we have
        // to be careful not to panic in other areas of the code (the cache
        // clearing and init code) that tend to assume some minimum useful
        // cache capacity.
        let min_cache = minimum_cache_capacity(
            &nfa,
            &classes,
            self.config.get_starts_for_each_pattern(),
        );
        let mut cache_capacity = self.config.get_cache_capacity();
        if cache_capacity < min_cache {
            // When the caller has asked us to skip the cache capacity check,
            // then we simply force the cache capacity to its minimum amount
            // and mush on.
            if self.config.get_skip_cache_capacity_check() {
                debug!(
                    "given capacity ({}) is too small, \
                     since skip_cache_capacity_check is enabled, \
                     setting cache capacity to minimum ({})",
                    cache_capacity, min_cache,
                );
                cache_capacity = min_cache;
            } else {
                return Err(BuildError::insufficient_cache_capacity(
                    min_cache,
                    cache_capacity,
                ));
            }
        }
        // We also need to check that we can fit at least some small number
        // of states in our state ID space. This is unlikely to trigger in
        // >=32-bit systems, but 16-bit systems have a pretty small state ID
        // space since a number of bits are used up as sentinels.
        if let Err(err) = minimum_lazy_state_id(&classes) {
            return Err(BuildError::insufficient_state_id_capacity(err));
        }
        let stride2 = classes.stride2();
        let start_map = StartByteMap::new(nfa.look_matcher());
        Ok(DFA {
            config: self.config.clone(),
            nfa,
            stride2,
            start_map,
            classes,
            quitset,
            cache_capacity,
        })
    }

    /// Apply the given lazy DFA configuration options to this builder.
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
    /// These settings only apply when constructing a lazy DFA directly from a
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

/// Represents the current state of an overlapping search.
///
/// This is used for overlapping searches since they need to know something
/// about the previous search. For example, when multiple patterns match at the
/// same position, this state tracks the last reported pattern so that the next
/// search knows whether to report another matching pattern or continue with
/// the search at the next position. Additionally, it also tracks which state
/// the last search call terminated in.
///
/// This type provides little introspection capabilities. The only thing a
/// caller can do is construct it and pass it around to permit search routines
/// to use it to track state, and also ask whether a match has been found.
///
/// Callers should always provide a fresh state constructed via
/// [`OverlappingState::start`] when starting a new search. Reusing state from
/// a previous search may result in incorrect results.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlappingState {
    /// The match reported by the most recent overlapping search to use this
    /// state.
    ///
    /// If a search does not find any matches, then it is expected to clear
    /// this value.
    pub(crate) mat: Option<HalfMatch>,
    /// The state ID of the state at which the search was in when the call
    /// terminated. When this is a match state, `last_match` must be set to a
    /// non-None value.
    ///
    /// A `None` value indicates the start state of the corresponding
    /// automaton. We cannot use the actual ID, since any one automaton may
    /// have many start states, and which one is in use depends on several
    /// search-time factors.
    pub(crate) id: Option<LazyStateID>,
    /// The position of the search.
    ///
    /// When `id` is None (i.e., we are starting a search), this is set to
    /// the beginning of the search as given by the caller regardless of its
    /// current value. Subsequent calls to an overlapping search pick up at
    /// this offset.
    pub(crate) at: usize,
    /// The index into the matching patterns of the next match to report if the
    /// current state is a match state. Note that this may be 1 greater than
    /// the total number of matches to report for the current match state. (In
    /// which case, no more matches should be reported at the current position
    /// and the search should advance to the next position.)
    pub(crate) next_match_index: Option<usize>,
    /// This is set to true when a reverse overlapping search has entered its
    /// EOI transitions.
    ///
    /// This isn't used in a forward search because it knows to stop once the
    /// position exceeds the end of the search range. In a reverse search,
    /// since we use unsigned offsets, we don't "know" once we've gone past
    /// `0`. So the only way to detect it is with this extra flag. The reverse
    /// overlapping search knows to terminate specifically after it has
    /// reported all matches after following the EOI transition.
    pub(crate) rev_eoi: bool,
}

impl OverlappingState {
    /// Create a new overlapping state that begins at the start state of any
    /// automaton.
    pub fn start() -> OverlappingState {
        OverlappingState {
            mat: None,
            id: None,
            at: 0,
            next_match_index: None,
            rev_eoi: false,
        }
    }

    /// Return the match result of the most recent search to execute with this
    /// state.
    ///
    /// A searches will clear this result automatically, such that if no
    /// match is found, this will correctly report `None`.
    pub fn get_match(&self) -> Option<HalfMatch> {
        self.mat
    }
}

/// Runs the given overlapping `search` function (forwards or backwards) until
/// a match is found whose offset does not split a codepoint.
///
/// This is *not* always correct to call. It should only be called when the
/// underlying NFA has UTF-8 mode enabled *and* it can produce zero-width
/// matches. Calling this when both of those things aren't true might result
/// in legitimate matches getting skipped.
#[cold]
#[inline(never)]
fn skip_empty_utf8_splits_overlapping<F>(
    input: &Input<'_>,
    state: &mut OverlappingState,
    mut search: F,
) -> Result<(), MatchError>
where
    F: FnMut(&Input<'_>, &mut OverlappingState) -> Result<(), MatchError>,
{
    // Note that this routine works for forwards and reverse searches
    // even though there's no code here to handle those cases. That's
    // because overlapping searches drive themselves to completion via
    // `OverlappingState`. So all we have to do is push it until no matches are
    // found.

    let mut hm = match state.get_match() {
        None => return Ok(()),
        Some(hm) => hm,
    };
    if input.get_anchored().is_anchored() {
        if !input.is_char_boundary(hm.offset()) {
            state.mat = None;
        }
        return Ok(());
    }
    while !input.is_char_boundary(hm.offset()) {
        search(input, state)?;
        hm = match state.get_match() {
            None => return Ok(()),
            Some(hm) => hm,
        };
    }
    Ok(())
}

/// Based on the minimum number of states required for a useful lazy DFA cache,
/// this returns the minimum lazy state ID that must be representable.
///
/// It's not likely for this to have any impact 32-bit systems (or higher), but
/// on 16-bit systems, the lazy state ID space is quite constrained and thus
/// may be insufficient if our MIN_STATES value is (for some reason) too high.
fn minimum_lazy_state_id(
    classes: &ByteClasses,
) -> Result<LazyStateID, LazyStateIDError> {
    let stride = 1 << classes.stride2();
    let min_state_index = MIN_STATES.checked_sub(1).unwrap();
    LazyStateID::new(min_state_index * stride)
}

/// Based on the minimum number of states required for a useful lazy DFA cache,
/// this returns a heuristic minimum number of bytes of heap space required.
///
/// This is a "heuristic" because the minimum it returns is likely bigger than
/// the true minimum. Namely, it assumes that each powerset NFA/DFA state uses
/// the maximum number of NFA states (all of them). This is likely bigger
/// than what is required in practice. Computing the true minimum effectively
/// requires determinization, which is probably too much work to do for a
/// simple check like this.
///
/// One of the issues with this approach IMO is that it requires that this
/// be in sync with the calculation above for computing how much heap memory
/// the DFA cache uses. If we get it wrong, it's possible for example for the
/// minimum to be smaller than the computed heap memory, and thus, it may be
/// the case that we can't add the required minimum number of states. That in
/// turn will make lazy DFA panic because we assume that we can add at least a
/// minimum number of states.
///
/// Another approach would be to always allow the minimum number of states to
/// be added to the lazy DFA cache, even if it exceeds the configured cache
/// limit. This does mean that the limit isn't really a limit in all cases,
/// which is unfortunate. But it does at least guarantee that the lazy DFA can
/// always make progress, even if it is slow. (This approach is very similar to
/// enabling the 'skip_cache_capacity_check' config knob, except it wouldn't
/// rely on cache size calculation. Instead, it would just always permit a
/// minimum number of states to be added.)
fn minimum_cache_capacity(
    nfa: &thompson::NFA,
    classes: &ByteClasses,
    starts_for_each_pattern: bool,
) -> usize {
    const ID_SIZE: usize = size_of::<LazyStateID>();
    const STATE_SIZE: usize = size_of::<State>();

    let stride = 1 << classes.stride2();
    let states_len = nfa.states().len();
    let sparses = 2 * states_len * NFAStateID::SIZE;
    let trans = MIN_STATES * stride * ID_SIZE;

    let mut starts = Start::len() * ID_SIZE;
    if starts_for_each_pattern {
        starts += (Start::len() * nfa.pattern_len()) * ID_SIZE;
    }

    // The min number of states HAS to be at least 4: we have 3 sentinel states
    // and then we need space for one more when we save a state after clearing
    // the cache. We also need space for one more, otherwise we get stuck in a
    // loop where we try to add a 5th state, which gets rejected, which clears
    // the cache, which adds back a saved state (4th total state) which then
    // tries to add the 5th state again.
    assert!(MIN_STATES >= 5, "minimum number of states has to be at least 5");
    // The minimum number of non-sentinel states. We consider this separately
    // because sentinel states are much smaller in that they contain no NFA
    // states. Given our aggressive calculation here, it's worth being more
    // precise with the number of states we need.
    let non_sentinel = MIN_STATES.checked_sub(SENTINEL_STATES).unwrap();

    // Every `State` has 5 bytes for flags, 4 bytes (max) for the number of
    // patterns, followed by 32-bit encodings of patterns and then delta
    // varint encodings of NFA state IDs. We use the worst case (which isn't
    // technically possible) of 5 bytes for each NFA state ID.
    //
    // HOWEVER, three of the states needed by a lazy DFA are just the sentinel
    // unknown, dead and quit states. Those states have a known size and it is
    // small.
    let dead_state_size = State::dead().memory_usage();
    let max_state_size = 5 + 4 + (nfa.pattern_len() * 4) + (states_len * 5);
    let states = (SENTINEL_STATES * (STATE_SIZE + dead_state_size))
        + (non_sentinel * (STATE_SIZE + max_state_size));
    // NOTE: We don't double count heap memory used by State for this map since
    // we use reference counting to avoid doubling memory usage. (This tends to
    // be where most memory is allocated in the cache.)
    let states_to_sid = (MIN_STATES * STATE_SIZE) + (MIN_STATES * ID_SIZE);
    let stack = states_len * NFAStateID::SIZE;
    let scratch_state_builder = max_state_size;

    trans
        + starts
        + states
        + states_to_sid
        + sparses
        + stack
        + scratch_state_builder
}

#[cfg(all(test, feature = "syntax"))]
mod tests {
    use super::*;

    // Tests that we handle heuristic Unicode word boundary support in reverse
    // DFAs in the specific case of contextual searches.
    //
    // I wrote this test when I discovered a bug in how heuristic word
    // boundaries were handled. Namely, that the starting state selection
    // didn't consider the DFA's quit byte set when looking at the byte
    // immediately before the start of the search (or immediately after the
    // end of the search in the case of a reverse search). As a result, it was
    // possible for '\bfoo\b' to match 'β123' because the trailing \xB2 byte
    // in the 'β' codepoint would be treated as a non-word character. But of
    // course, this search should trigger the DFA to quit, since there is a
    // non-ASCII byte in consideration.
    //
    // Thus, I fixed 'start_state_{forward,reverse}' to check the quit byte set
    // if it wasn't empty. The forward case is tested in the doc test for the
    // Config::unicode_word_boundary API. We test the reverse case here, which
    // is sufficiently niche that it doesn't really belong in a doc test.
    #[test]
    fn heuristic_unicode_reverse() {
        let dfa = DFA::builder()
            .configure(DFA::config().unicode_word_boundary(true))
            .thompson(thompson::Config::new().reverse(true))
            .build(r"\b[0-9]+\b")
            .unwrap();
        let mut cache = dfa.create_cache();

        let input = Input::new("β123").range(2..);
        let expected = MatchError::quit(0xB2, 1);
        let got = dfa.try_search_rev(&mut cache, &input);
        assert_eq!(Err(expected), got);

        let input = Input::new("123β").range(..3);
        let expected = MatchError::quit(0xCE, 3);
        let got = dfa.try_search_rev(&mut cache, &input);
        assert_eq!(Err(expected), got);
    }
}
