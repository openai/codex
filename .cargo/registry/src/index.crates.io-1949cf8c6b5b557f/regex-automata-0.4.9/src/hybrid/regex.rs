/*!
A lazy DFA backed `Regex`.

This module provides a [`Regex`] backed by a lazy DFA. A `Regex` implements
convenience routines you might have come to expect, such as finding a match
and iterating over all non-overlapping matches. This `Regex` type is limited
in its capabilities to what a lazy DFA can provide. Therefore, APIs involving
capturing groups, for example, are not provided.

Internally, a `Regex` is composed of two DFAs. One is a "forward" DFA that
finds the end offset of a match, where as the other is a "reverse" DFA that
find the start offset of a match.

See the [parent module](crate::hybrid) for examples.
*/

use crate::{
    hybrid::{
        dfa::{self, DFA},
        error::BuildError,
    },
    nfa::thompson,
    util::{
        iter,
        search::{Anchored, Input, Match, MatchError, MatchKind},
    },
};

/// A regular expression that uses hybrid NFA/DFAs (also called "lazy DFAs")
/// for searching.
///
/// A regular expression is comprised of two lazy DFAs, a "forward" DFA and a
/// "reverse" DFA. The forward DFA is responsible for detecting the end of
/// a match while the reverse DFA is responsible for detecting the start
/// of a match. Thus, in order to find the bounds of any given match, a
/// forward search must first be run followed by a reverse search. A match
/// found by the forward DFA guarantees that the reverse DFA will also find
/// a match.
///
/// # Fallibility
///
/// Most of the search routines defined on this type will _panic_ when the
/// underlying search fails. This might be because the DFA gave up because it
/// saw a quit byte, whether configured explicitly or via heuristic Unicode
/// word boundary support, although neither are enabled by default. It might
/// also fail if the underlying DFA determines it isn't making effective use of
/// the cache (which also never happens by default). Or it might fail because
/// an invalid `Input` configuration is given, for example, with an unsupported
/// [`Anchored`] mode.
///
/// If you need to handle these error cases instead of allowing them to trigger
/// a panic, then the lower level [`Regex::try_search`] provides a fallible API
/// that never panics.
///
/// # Example
///
/// This example shows how to cause a search to terminate if it sees a
/// `\n` byte, and handle the error returned. This could be useful if, for
/// example, you wanted to prevent a user supplied pattern from matching
/// across a line boundary.
///
/// ```
/// # if cfg!(miri) { return Ok(()); } // miri takes too long
/// use regex_automata::{hybrid::{dfa, regex::Regex}, Input, MatchError};
///
/// let re = Regex::builder()
///     .dfa(dfa::Config::new().quit(b'\n', true))
///     .build(r"foo\p{any}+bar")?;
/// let mut cache = re.create_cache();
///
/// let input = Input::new("foo\nbar");
/// // Normally this would produce a match, since \p{any} contains '\n'.
/// // But since we instructed the automaton to enter a quit state if a
/// // '\n' is observed, this produces a match error instead.
/// let expected = MatchError::quit(b'\n', 3);
/// let got = re.try_search(&mut cache, &input).unwrap_err();
/// assert_eq!(expected, got);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct Regex {
    /// The forward lazy DFA. This can only find the end of a match.
    forward: DFA,
    /// The reverse lazy DFA. This can only find the start of a match.
    ///
    /// This is built with 'all' match semantics (instead of leftmost-first)
    /// so that it always finds the longest possible match (which corresponds
    /// to the leftmost starting position). It is also compiled as an anchored
    /// matcher and has 'starts_for_each_pattern' enabled. Including starting
    /// states for each pattern is necessary to ensure that we only look for
    /// matches of a pattern that matched in the forward direction. Otherwise,
    /// we might wind up finding the "leftmost" starting position of a totally
    /// different pattern!
    reverse: DFA,
}

/// Convenience routines for regex and cache construction.
impl Regex {
    /// Parse the given regular expression using the default configuration and
    /// return the corresponding regex.
    ///
    /// If you want a non-default configuration, then use the [`Builder`] to
    /// set your own configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{hybrid::regex::Regex, Match};
    ///
    /// let re = Regex::new("foo[0-9]+bar")?;
    /// let mut cache = re.create_cache();
    /// assert_eq!(
    ///     Some(Match::must(0, 3..14)),
    ///     re.find(&mut cache, "zzzfoo12345barzzz"),
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "syntax")]
    pub fn new(pattern: &str) -> Result<Regex, BuildError> {
        Regex::builder().build(pattern)
    }

    /// Like `new`, but parses multiple patterns into a single "multi regex."
    /// This similarly uses the default regex configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{hybrid::regex::Regex, Match};
    ///
    /// let re = Regex::new_many(&["[a-z]+", "[0-9]+"])?;
    /// let mut cache = re.create_cache();
    ///
    /// let mut it = re.find_iter(&mut cache, "abc 1 foo 4567 0 quux");
    /// assert_eq!(Some(Match::must(0, 0..3)), it.next());
    /// assert_eq!(Some(Match::must(1, 4..5)), it.next());
    /// assert_eq!(Some(Match::must(0, 6..9)), it.next());
    /// assert_eq!(Some(Match::must(1, 10..14)), it.next());
    /// assert_eq!(Some(Match::must(1, 15..16)), it.next());
    /// assert_eq!(Some(Match::must(0, 17..21)), it.next());
    /// assert_eq!(None, it.next());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "syntax")]
    pub fn new_many<P: AsRef<str>>(
        patterns: &[P],
    ) -> Result<Regex, BuildError> {
        Regex::builder().build_many(patterns)
    }

    /// Return a builder for configuring the construction of a `Regex`.
    ///
    /// This is a convenience routine to avoid needing to import the
    /// [`Builder`] type in common cases.
    ///
    /// # Example
    ///
    /// This example shows how to use the builder to disable UTF-8 mode
    /// everywhere.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{
    ///     hybrid::regex::Regex, nfa::thompson, util::syntax, Match,
    /// };
    ///
    /// let re = Regex::builder()
    ///     .syntax(syntax::Config::new().utf8(false))
    ///     .thompson(thompson::Config::new().utf8(false))
    ///     .build(r"foo(?-u:[^b])ar.*")?;
    /// let mut cache = re.create_cache();
    ///
    /// let haystack = b"\xFEfoo\xFFarzz\xE2\x98\xFF\n";
    /// let expected = Some(Match::must(0, 1..9));
    /// let got = re.find(&mut cache, haystack);
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// Create a new cache for this `Regex`.
    ///
    /// The cache returned should only be used for searches for this
    /// `Regex`. If you want to reuse the cache for another `Regex`, then
    /// you must call [`Cache::reset`] with that `Regex` (or, equivalently,
    /// [`Regex::reset_cache`]).
    pub fn create_cache(&self) -> Cache {
        Cache::new(self)
    }

    /// Reset the given cache such that it can be used for searching with the
    /// this `Regex` (and only this `Regex`).
    ///
    /// A cache reset permits reusing memory already allocated in this cache
    /// with a different `Regex`.
    ///
    /// Resetting a cache sets its "clear count" to 0. This is relevant if the
    /// `Regex` has been configured to "give up" after it has cleared the cache
    /// a certain number of times.
    ///
    /// # Example
    ///
    /// This shows how to re-purpose a cache for use with a different `Regex`.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{hybrid::regex::Regex, Match};
    ///
    /// let re1 = Regex::new(r"\w")?;
    /// let re2 = Regex::new(r"\W")?;
    ///
    /// let mut cache = re1.create_cache();
    /// assert_eq!(
    ///     Some(Match::must(0, 0..2)),
    ///     re1.find(&mut cache, "Δ"),
    /// );
    ///
    /// // Using 'cache' with re2 is not allowed. It may result in panics or
    /// // incorrect results. In order to re-purpose the cache, we must reset
    /// // it with the Regex we'd like to use it with.
    /// //
    /// // Similarly, after this reset, using the cache with 're1' is also not
    /// // allowed.
    /// re2.reset_cache(&mut cache);
    /// assert_eq!(
    ///     Some(Match::must(0, 0..3)),
    ///     re2.find(&mut cache, "☃"),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn reset_cache(&self, cache: &mut Cache) {
        self.forward().reset_cache(&mut cache.forward);
        self.reverse().reset_cache(&mut cache.reverse);
    }
}

/// Standard infallible search routines for finding and iterating over matches.
impl Regex {
    /// Returns true if and only if this regex matches the given haystack.
    ///
    /// This routine may short circuit if it knows that scanning future input
    /// will never lead to a different result. In particular, if the underlying
    /// DFA enters a match state or a dead state, then this routine will return
    /// `true` or `false`, respectively, without inspecting any future input.
    ///
    /// # Panics
    ///
    /// This routine panics if the search could not complete. This can occur
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
    /// When a search panics, callers cannot know whether a match exists or
    /// not.
    ///
    /// Use [`Regex::try_search`] if you want to handle these error conditions.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::hybrid::regex::Regex;
    ///
    /// let re = Regex::new("foo[0-9]+bar")?;
    /// let mut cache = re.create_cache();
    ///
    /// assert!(re.is_match(&mut cache, "foo12345bar"));
    /// assert!(!re.is_match(&mut cache, "foobar"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn is_match<'h, I: Into<Input<'h>>>(
        &self,
        cache: &mut Cache,
        input: I,
    ) -> bool {
        // Not only can we do an "earliest" search, but we can avoid doing a
        // reverse scan too.
        self.forward()
            .try_search_fwd(&mut cache.forward, &input.into().earliest(true))
            .unwrap()
            .is_some()
    }

    /// Returns the start and end offset of the leftmost match. If no match
    /// exists, then `None` is returned.
    ///
    /// # Panics
    ///
    /// This routine panics if the search could not complete. This can occur
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
    /// When a search panics, callers cannot know whether a match exists or
    /// not.
    ///
    /// Use [`Regex::try_search`] if you want to handle these error conditions.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{Match, hybrid::regex::Regex};
    ///
    /// let re = Regex::new("foo[0-9]+")?;
    /// let mut cache = re.create_cache();
    /// assert_eq!(
    ///     Some(Match::must(0, 3..11)),
    ///     re.find(&mut cache, "zzzfoo12345zzz"),
    /// );
    ///
    /// // Even though a match is found after reading the first byte (`a`),
    /// // the default leftmost-first match semantics demand that we find the
    /// // earliest match that prefers earlier parts of the pattern over latter
    /// // parts.
    /// let re = Regex::new("abc|a")?;
    /// let mut cache = re.create_cache();
    /// assert_eq!(Some(Match::must(0, 0..3)), re.find(&mut cache, "abc"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn find<'h, I: Into<Input<'h>>>(
        &self,
        cache: &mut Cache,
        input: I,
    ) -> Option<Match> {
        self.try_search(cache, &input.into()).unwrap()
    }

    /// Returns an iterator over all non-overlapping leftmost matches in the
    /// given bytes. If no match exists, then the iterator yields no elements.
    ///
    /// # Panics
    ///
    /// This routine panics if the search could not complete. This can occur
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
    /// When a search panics, callers cannot know whether a match exists or
    /// not.
    ///
    /// The above conditions also apply to the iterator returned as well. For
    /// example, if the lazy DFA gives up or quits during a search using this
    /// method, then a panic will occur during iteration.
    ///
    /// Use [`Regex::try_search`] with [`util::iter::Searcher`](iter::Searcher)
    /// if you want to handle these error conditions.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{hybrid::regex::Regex, Match};
    ///
    /// let re = Regex::new("foo[0-9]+")?;
    /// let mut cache = re.create_cache();
    ///
    /// let text = "foo1 foo12 foo123";
    /// let matches: Vec<Match> = re.find_iter(&mut cache, text).collect();
    /// assert_eq!(matches, vec![
    ///     Match::must(0, 0..4),
    ///     Match::must(0, 5..10),
    ///     Match::must(0, 11..17),
    /// ]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn find_iter<'r, 'c, 'h, I: Into<Input<'h>>>(
        &'r self,
        cache: &'c mut Cache,
        input: I,
    ) -> FindMatches<'r, 'c, 'h> {
        let it = iter::Searcher::new(input.into());
        FindMatches { re: self, cache, it }
    }
}

/// Lower level "search" primitives that accept a `&Input` for cheap reuse
/// and return an error if one occurs instead of panicking.
impl Regex {
    /// Returns the start and end offset of the leftmost match. If no match
    /// exists, then `None` is returned.
    ///
    /// This is like [`Regex::find`] but with two differences:
    ///
    /// 1. It is not generic over `Into<Input>` and instead accepts a
    /// `&Input`. This permits reusing the same `Input` for multiple searches
    /// without needing to create a new one. This _may_ help with latency.
    /// 2. It returns an error if the search could not complete where as
    /// [`Regex::find`] will panic.
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
    #[inline]
    pub fn try_search(
        &self,
        cache: &mut Cache,
        input: &Input<'_>,
    ) -> Result<Option<Match>, MatchError> {
        let (fcache, rcache) = (&mut cache.forward, &mut cache.reverse);
        let end = match self.forward().try_search_fwd(fcache, input)? {
            None => return Ok(None),
            Some(end) => end,
        };
        // This special cases an empty match at the beginning of the search. If
        // our end matches our start, then since a reverse DFA can't match past
        // the start, it must follow that our starting position is also our end
        // position. So short circuit and skip the reverse search.
        if input.start() == end.offset() {
            return Ok(Some(Match::new(
                end.pattern(),
                end.offset()..end.offset(),
            )));
        }
        // We can also skip the reverse search if we know our search was
        // anchored. This occurs either when the input config is anchored or
        // when we know the regex itself is anchored. In this case, we know the
        // start of the match, if one is found, must be the start of the
        // search.
        if self.is_anchored(input) {
            return Ok(Some(Match::new(
                end.pattern(),
                input.start()..end.offset(),
            )));
        }
        // N.B. I have tentatively convinced myself that it isn't necessary
        // to specify the specific pattern for the reverse search since the
        // reverse search will always find the same pattern to match as the
        // forward search. But I lack a rigorous proof. Why not just provide
        // the pattern anyway? Well, if it is needed, then leaving it out
        // gives us a chance to find a witness. (Also, if we don't need to
        // specify the pattern, then we don't need to build the reverse DFA
        // with 'starts_for_each_pattern' enabled. It doesn't matter too much
        // for the lazy DFA, but does make the overall DFA bigger.)
        //
        // We also need to be careful to disable 'earliest' for the reverse
        // search, since it could be enabled for the forward search. In the
        // reverse case, to satisfy "leftmost" criteria, we need to match as
        // much as we can. We also need to be careful to make the search
        // anchored. We don't want the reverse search to report any matches
        // other than the one beginning at the end of our forward search.
        let revsearch = input
            .clone()
            .span(input.start()..end.offset())
            .anchored(Anchored::Yes)
            .earliest(false);
        let start = self
            .reverse()
            .try_search_rev(rcache, &revsearch)?
            .expect("reverse search must match if forward search does");
        debug_assert_eq!(
            start.pattern(),
            end.pattern(),
            "forward and reverse search must match same pattern",
        );
        debug_assert!(start.offset() <= end.offset());
        Ok(Some(Match::new(end.pattern(), start.offset()..end.offset())))
    }

    /// Returns true if either the given input specifies an anchored search
    /// or if the underlying NFA is always anchored.
    fn is_anchored(&self, input: &Input<'_>) -> bool {
        match input.get_anchored() {
            Anchored::No => {
                self.forward().get_nfa().is_always_start_anchored()
            }
            Anchored::Yes | Anchored::Pattern(_) => true,
        }
    }
}

/// Non-search APIs for querying information about the regex and setting a
/// prefilter.
impl Regex {
    /// Return the underlying lazy DFA responsible for forward matching.
    ///
    /// This is useful for accessing the underlying lazy DFA and using it
    /// directly if the situation calls for it.
    pub fn forward(&self) -> &DFA {
        &self.forward
    }

    /// Return the underlying lazy DFA responsible for reverse matching.
    ///
    /// This is useful for accessing the underlying lazy DFA and using it
    /// directly if the situation calls for it.
    pub fn reverse(&self) -> &DFA {
        &self.reverse
    }

    /// Returns the total number of patterns matched by this regex.
    ///
    /// # Example
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::hybrid::regex::Regex;
    ///
    /// let re = Regex::new_many(&[r"[a-z]+", r"[0-9]+", r"\w+"])?;
    /// assert_eq!(3, re.pattern_len());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn pattern_len(&self) -> usize {
        assert_eq!(self.forward().pattern_len(), self.reverse().pattern_len());
        self.forward().pattern_len()
    }
}

/// An iterator over all non-overlapping matches for an infallible search.
///
/// The iterator yields a [`Match`] value until no more matches could be found.
/// If the underlying regex engine returns an error, then a panic occurs.
///
/// The lifetime parameters are as follows:
///
/// * `'r` represents the lifetime of the regex object.
/// * `'h` represents the lifetime of the haystack being searched.
/// * `'c` represents the lifetime of the regex cache.
///
/// This iterator can be created with the [`Regex::find_iter`] method.
#[derive(Debug)]
pub struct FindMatches<'r, 'c, 'h> {
    re: &'r Regex,
    cache: &'c mut Cache,
    it: iter::Searcher<'h>,
}

impl<'r, 'c, 'h> Iterator for FindMatches<'r, 'c, 'h> {
    type Item = Match;

    #[inline]
    fn next(&mut self) -> Option<Match> {
        let FindMatches { re, ref mut cache, ref mut it } = *self;
        it.advance(|input| re.try_search(cache, input))
    }
}

/// A cache represents a partially computed forward and reverse DFA.
///
/// A cache is the key component that differentiates a classical DFA and a
/// hybrid NFA/DFA (also called a "lazy DFA"). Where a classical DFA builds a
/// complete transition table that can handle all possible inputs, a hybrid
/// NFA/DFA starts with an empty transition table and builds only the parts
/// required during search. The parts that are built are stored in a cache. For
/// this reason, a cache is a required parameter for nearly every operation on
/// a [`Regex`].
///
/// Caches can be created from their corresponding `Regex` via
/// [`Regex::create_cache`]. A cache can only be used with either the `Regex`
/// that created it, or the `Regex` that was most recently used to reset it
/// with [`Cache::reset`]. Using a cache with any other `Regex` may result in
/// panics or incorrect results.
#[derive(Debug, Clone)]
pub struct Cache {
    forward: dfa::Cache,
    reverse: dfa::Cache,
}

impl Cache {
    /// Create a new cache for the given `Regex`.
    ///
    /// The cache returned should only be used for searches for the given
    /// `Regex`. If you want to reuse the cache for another `Regex`, then you
    /// must call [`Cache::reset`] with that `Regex`.
    pub fn new(re: &Regex) -> Cache {
        let forward = dfa::Cache::new(re.forward());
        let reverse = dfa::Cache::new(re.reverse());
        Cache { forward, reverse }
    }

    /// Reset this cache such that it can be used for searching with the given
    /// `Regex` (and only that `Regex`).
    ///
    /// A cache reset permits reusing memory already allocated in this cache
    /// with a different `Regex`.
    ///
    /// Resetting a cache sets its "clear count" to 0. This is relevant if the
    /// `Regex` has been configured to "give up" after it has cleared the cache
    /// a certain number of times.
    ///
    /// # Example
    ///
    /// This shows how to re-purpose a cache for use with a different `Regex`.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{hybrid::regex::Regex, Match};
    ///
    /// let re1 = Regex::new(r"\w")?;
    /// let re2 = Regex::new(r"\W")?;
    ///
    /// let mut cache = re1.create_cache();
    /// assert_eq!(
    ///     Some(Match::must(0, 0..2)),
    ///     re1.find(&mut cache, "Δ"),
    /// );
    ///
    /// // Using 'cache' with re2 is not allowed. It may result in panics or
    /// // incorrect results. In order to re-purpose the cache, we must reset
    /// // it with the Regex we'd like to use it with.
    /// //
    /// // Similarly, after this reset, using the cache with 're1' is also not
    /// // allowed.
    /// cache.reset(&re2);
    /// assert_eq!(
    ///     Some(Match::must(0, 0..3)),
    ///     re2.find(&mut cache, "☃"),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn reset(&mut self, re: &Regex) {
        self.forward.reset(re.forward());
        self.reverse.reset(re.reverse());
    }

    /// Return a reference to the forward cache.
    pub fn forward(&mut self) -> &dfa::Cache {
        &self.forward
    }

    /// Return a reference to the reverse cache.
    pub fn reverse(&mut self) -> &dfa::Cache {
        &self.reverse
    }

    /// Return a mutable reference to the forward cache.
    ///
    /// If you need mutable references to both the forward and reverse caches,
    /// then use [`Cache::as_parts_mut`].
    pub fn forward_mut(&mut self) -> &mut dfa::Cache {
        &mut self.forward
    }

    /// Return a mutable reference to the reverse cache.
    ///
    /// If you need mutable references to both the forward and reverse caches,
    /// then use [`Cache::as_parts_mut`].
    pub fn reverse_mut(&mut self) -> &mut dfa::Cache {
        &mut self.reverse
    }

    /// Return references to the forward and reverse caches, respectively.
    pub fn as_parts(&self) -> (&dfa::Cache, &dfa::Cache) {
        (&self.forward, &self.reverse)
    }

    /// Return mutable references to the forward and reverse caches,
    /// respectively.
    pub fn as_parts_mut(&mut self) -> (&mut dfa::Cache, &mut dfa::Cache) {
        (&mut self.forward, &mut self.reverse)
    }

    /// Returns the heap memory usage, in bytes, as a sum of the forward and
    /// reverse lazy DFA caches.
    ///
    /// This does **not** include the stack size used up by this cache. To
    /// compute that, use `std::mem::size_of::<Cache>()`.
    pub fn memory_usage(&self) -> usize {
        self.forward.memory_usage() + self.reverse.memory_usage()
    }
}

/// A builder for a regex based on a hybrid NFA/DFA.
///
/// This builder permits configuring options for the syntax of a pattern, the
/// NFA construction, the lazy DFA construction and finally the regex searching
/// itself. This builder is different from a general purpose regex builder
/// in that it permits fine grain configuration of the construction process.
/// The trade off for this is complexity, and the possibility of setting a
/// configuration that might not make sense. For example, there are two
/// different UTF-8 modes:
///
/// * [`syntax::Config::utf8`](crate::util::syntax::Config::utf8) controls
/// whether the pattern itself can contain sub-expressions that match invalid
/// UTF-8.
/// * [`thompson::Config::utf8`] controls how the regex iterators themselves
/// advance the starting position of the next search when a match with zero
/// length is found.
///
/// Generally speaking, callers will want to either enable all of these or
/// disable all of these.
///
/// Internally, building a regex requires building two hybrid NFA/DFAs,
/// where one is responsible for finding the end of a match and the other is
/// responsible for finding the start of a match. If you only need to detect
/// whether something matched, or only the end of a match, then you should use
/// a [`dfa::Builder`] to construct a single hybrid NFA/DFA, which is cheaper
/// than building two of them.
///
/// # Example
///
/// This example shows how to disable UTF-8 mode in the syntax and the regex
/// itself. This is generally what you want for matching on arbitrary bytes.
///
/// ```
/// # if cfg!(miri) { return Ok(()); } // miri takes too long
/// use regex_automata::{
///     hybrid::regex::Regex, nfa::thompson, util::syntax, Match,
/// };
///
/// let re = Regex::builder()
///     .syntax(syntax::Config::new().utf8(false))
///     .thompson(thompson::Config::new().utf8(false))
///     .build(r"foo(?-u:[^b])ar.*")?;
/// let mut cache = re.create_cache();
///
/// let haystack = b"\xFEfoo\xFFarzz\xE2\x98\xFF\n";
/// let expected = Some(Match::must(0, 1..9));
/// let got = re.find(&mut cache, haystack);
/// assert_eq!(expected, got);
/// // Notice that `(?-u:[^b])` matches invalid UTF-8,
/// // but the subsequent `.*` does not! Disabling UTF-8
/// // on the syntax permits this.
/// assert_eq!(b"foo\xFFarzz", &haystack[got.unwrap().range()]);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct Builder {
    dfa: dfa::Builder,
}

impl Builder {
    /// Create a new regex builder with the default configuration.
    pub fn new() -> Builder {
        Builder { dfa: DFA::builder() }
    }

    /// Build a regex from the given pattern.
    ///
    /// If there was a problem parsing or compiling the pattern, then an error
    /// is returned.
    #[cfg(feature = "syntax")]
    pub fn build(&self, pattern: &str) -> Result<Regex, BuildError> {
        self.build_many(&[pattern])
    }

    /// Build a regex from the given patterns.
    #[cfg(feature = "syntax")]
    pub fn build_many<P: AsRef<str>>(
        &self,
        patterns: &[P],
    ) -> Result<Regex, BuildError> {
        let forward = self.dfa.build_many(patterns)?;
        let reverse = self
            .dfa
            .clone()
            .configure(
                DFA::config()
                    .prefilter(None)
                    .specialize_start_states(false)
                    .match_kind(MatchKind::All),
            )
            .thompson(thompson::Config::new().reverse(true))
            .build_many(patterns)?;
        Ok(self.build_from_dfas(forward, reverse))
    }

    /// Build a regex from its component forward and reverse hybrid NFA/DFAs.
    ///
    /// This is useful when you've built a forward and reverse lazy DFA
    /// separately, and want to combine them into a single regex. Once build,
    /// the individual DFAs given can still be accessed via [`Regex::forward`]
    /// and [`Regex::reverse`].
    ///
    /// It is important that the reverse lazy DFA be compiled under the
    /// following conditions:
    ///
    /// * It should use [`MatchKind::All`] semantics.
    /// * It should match in reverse.
    /// * Otherwise, its configuration should match the forward DFA.
    ///
    /// If these conditions aren't satisfied, then the behavior of searches is
    /// unspecified.
    ///
    /// Note that when using this constructor, no configuration is applied.
    /// Since this routine provides the DFAs to the builder, there is no
    /// opportunity to apply other configuration options.
    ///
    /// # Example
    ///
    /// This shows how to build individual lazy forward and reverse DFAs, and
    /// then combine them into a single `Regex`.
    ///
    /// ```
    /// use regex_automata::{
    ///     hybrid::{dfa::DFA, regex::Regex},
    ///     nfa::thompson,
    ///     MatchKind,
    /// };
    ///
    /// let fwd = DFA::new(r"foo[0-9]+")?;
    /// let rev = DFA::builder()
    ///     .configure(DFA::config().match_kind(MatchKind::All))
    ///     .thompson(thompson::Config::new().reverse(true))
    ///     .build(r"foo[0-9]+")?;
    ///
    /// let re = Regex::builder().build_from_dfas(fwd, rev);
    /// let mut cache = re.create_cache();
    /// assert_eq!(true, re.is_match(&mut cache, "foo123"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn build_from_dfas(&self, forward: DFA, reverse: DFA) -> Regex {
        Regex { forward, reverse }
    }

    /// Set the syntax configuration for this builder using
    /// [`syntax::Config`](crate::util::syntax::Config).
    ///
    /// This permits setting things like case insensitivity, Unicode and multi
    /// line mode.
    #[cfg(feature = "syntax")]
    pub fn syntax(
        &mut self,
        config: crate::util::syntax::Config,
    ) -> &mut Builder {
        self.dfa.syntax(config);
        self
    }

    /// Set the Thompson NFA configuration for this builder using
    /// [`nfa::thompson::Config`](thompson::Config).
    ///
    /// This permits setting things like whether additional time should be
    /// spent shrinking the size of the NFA.
    #[cfg(feature = "syntax")]
    pub fn thompson(&mut self, config: thompson::Config) -> &mut Builder {
        self.dfa.thompson(config);
        self
    }

    /// Set the lazy DFA compilation configuration for this builder using
    /// [`dfa::Config`].
    ///
    /// This permits setting things like whether Unicode word boundaries should
    /// be heuristically supported or settings how the behavior of the cache.
    pub fn dfa(&mut self, config: dfa::Config) -> &mut Builder {
        self.dfa.configure(config);
        self
    }
}

impl Default for Builder {
    fn default() -> Builder {
        Builder::new()
    }
}
