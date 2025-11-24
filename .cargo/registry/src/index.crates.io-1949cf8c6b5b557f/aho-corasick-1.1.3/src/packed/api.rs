use alloc::sync::Arc;

use crate::{
    packed::{pattern::Patterns, rabinkarp::RabinKarp, teddy},
    util::search::{Match, Span},
};

/// This is a limit placed on the total number of patterns we're willing to try
/// and match at once. As more sophisticated algorithms are added, this number
/// may be increased.
const PATTERN_LIMIT: usize = 128;

/// A knob for controlling the match semantics of a packed multiple string
/// searcher.
///
/// This differs from the [`MatchKind`](crate::MatchKind) type in the top-level
/// crate module in that it doesn't support "standard" match semantics,
/// and instead only supports leftmost-first or leftmost-longest. Namely,
/// "standard" semantics cannot be easily supported by packed searchers.
///
/// For more information on the distinction between leftmost-first and
/// leftmost-longest, see the docs on the top-level `MatchKind` type.
///
/// Unlike the top-level `MatchKind` type, the default match semantics for this
/// type are leftmost-first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MatchKind {
    /// Use leftmost-first match semantics, which reports leftmost matches.
    /// When there are multiple possible leftmost matches, the match
    /// corresponding to the pattern that appeared earlier when constructing
    /// the automaton is reported.
    ///
    /// This is the default.
    LeftmostFirst,
    /// Use leftmost-longest match semantics, which reports leftmost matches.
    /// When there are multiple possible leftmost matches, the longest match
    /// is chosen.
    LeftmostLongest,
}

impl Default for MatchKind {
    fn default() -> MatchKind {
        MatchKind::LeftmostFirst
    }
}

/// The configuration for a packed multiple pattern searcher.
///
/// The configuration is currently limited only to being able to select the
/// match semantics (leftmost-first or leftmost-longest) of a searcher. In the
/// future, more knobs may be made available.
///
/// A configuration produces a [`packed::Builder`](Builder), which in turn can
/// be used to construct a [`packed::Searcher`](Searcher) for searching.
///
/// # Example
///
/// This example shows how to use leftmost-longest semantics instead of the
/// default (leftmost-first).
///
/// ```
/// use aho_corasick::{packed::{Config, MatchKind}, PatternID};
///
/// # fn example() -> Option<()> {
/// let searcher = Config::new()
///     .match_kind(MatchKind::LeftmostLongest)
///     .builder()
///     .add("foo")
///     .add("foobar")
///     .build()?;
/// let matches: Vec<PatternID> = searcher
///     .find_iter("foobar")
///     .map(|mat| mat.pattern())
///     .collect();
/// assert_eq!(vec![PatternID::must(1)], matches);
/// # Some(()) }
/// # if cfg!(all(feature = "std", any(
/// #     target_arch = "x86_64", target_arch = "aarch64",
/// # ))) {
/// #     example().unwrap()
/// # } else {
/// #     assert!(example().is_none());
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Config {
    kind: MatchKind,
    force: Option<ForceAlgorithm>,
    only_teddy_fat: Option<bool>,
    only_teddy_256bit: Option<bool>,
    heuristic_pattern_limits: bool,
}

/// An internal option for forcing the use of a particular packed algorithm.
///
/// When an algorithm is forced, if a searcher could not be constructed for it,
/// then no searcher will be returned even if an alternative algorithm would
/// work.
#[derive(Clone, Debug)]
enum ForceAlgorithm {
    Teddy,
    RabinKarp,
}

impl Default for Config {
    fn default() -> Config {
        Config::new()
    }
}

impl Config {
    /// Create a new default configuration. A default configuration uses
    /// leftmost-first match semantics.
    pub fn new() -> Config {
        Config {
            kind: MatchKind::LeftmostFirst,
            force: None,
            only_teddy_fat: None,
            only_teddy_256bit: None,
            heuristic_pattern_limits: true,
        }
    }

    /// Create a packed builder from this configuration. The builder can be
    /// used to accumulate patterns and create a [`Searcher`] from them.
    pub fn builder(&self) -> Builder {
        Builder::from_config(self.clone())
    }

    /// Set the match semantics for this configuration.
    pub fn match_kind(&mut self, kind: MatchKind) -> &mut Config {
        self.kind = kind;
        self
    }

    /// An undocumented method for forcing the use of the Teddy algorithm.
    ///
    /// This is only exposed for more precise testing and benchmarks. Callers
    /// should not use it as it is not part of the API stability guarantees of
    /// this crate.
    #[doc(hidden)]
    pub fn only_teddy(&mut self, yes: bool) -> &mut Config {
        if yes {
            self.force = Some(ForceAlgorithm::Teddy);
        } else {
            self.force = None;
        }
        self
    }

    /// An undocumented method for forcing the use of the Fat Teddy algorithm.
    ///
    /// This is only exposed for more precise testing and benchmarks. Callers
    /// should not use it as it is not part of the API stability guarantees of
    /// this crate.
    #[doc(hidden)]
    pub fn only_teddy_fat(&mut self, yes: Option<bool>) -> &mut Config {
        self.only_teddy_fat = yes;
        self
    }

    /// An undocumented method for forcing the use of SSE (`Some(false)`) or
    /// AVX (`Some(true)`) algorithms.
    ///
    /// This is only exposed for more precise testing and benchmarks. Callers
    /// should not use it as it is not part of the API stability guarantees of
    /// this crate.
    #[doc(hidden)]
    pub fn only_teddy_256bit(&mut self, yes: Option<bool>) -> &mut Config {
        self.only_teddy_256bit = yes;
        self
    }

    /// An undocumented method for forcing the use of the Rabin-Karp algorithm.
    ///
    /// This is only exposed for more precise testing and benchmarks. Callers
    /// should not use it as it is not part of the API stability guarantees of
    /// this crate.
    #[doc(hidden)]
    pub fn only_rabin_karp(&mut self, yes: bool) -> &mut Config {
        if yes {
            self.force = Some(ForceAlgorithm::RabinKarp);
        } else {
            self.force = None;
        }
        self
    }

    /// Request that heuristic limitations on the number of patterns be
    /// employed. This useful to disable for benchmarking where one wants to
    /// explore how Teddy performs on large number of patterns even if the
    /// heuristics would otherwise refuse construction.
    ///
    /// This is enabled by default.
    pub fn heuristic_pattern_limits(&mut self, yes: bool) -> &mut Config {
        self.heuristic_pattern_limits = yes;
        self
    }
}

/// A builder for constructing a packed searcher from a collection of patterns.
///
/// # Example
///
/// This example shows how to use a builder to construct a searcher. By
/// default, leftmost-first match semantics are used.
///
/// ```
/// use aho_corasick::{packed::{Builder, MatchKind}, PatternID};
///
/// # fn example() -> Option<()> {
/// let searcher = Builder::new()
///     .add("foobar")
///     .add("foo")
///     .build()?;
/// let matches: Vec<PatternID> = searcher
///     .find_iter("foobar")
///     .map(|mat| mat.pattern())
///     .collect();
/// assert_eq!(vec![PatternID::ZERO], matches);
/// # Some(()) }
/// # if cfg!(all(feature = "std", any(
/// #     target_arch = "x86_64", target_arch = "aarch64",
/// # ))) {
/// #     example().unwrap()
/// # } else {
/// #     assert!(example().is_none());
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Builder {
    /// The configuration of this builder and subsequent matcher.
    config: Config,
    /// Set to true if the builder detects that a matcher cannot be built.
    inert: bool,
    /// The patterns provided by the caller.
    patterns: Patterns,
}

impl Builder {
    /// Create a new builder for constructing a multi-pattern searcher. This
    /// constructor uses the default configuration.
    pub fn new() -> Builder {
        Builder::from_config(Config::new())
    }

    fn from_config(config: Config) -> Builder {
        Builder { config, inert: false, patterns: Patterns::new() }
    }

    /// Build a searcher from the patterns added to this builder so far.
    pub fn build(&self) -> Option<Searcher> {
        if self.inert || self.patterns.is_empty() {
            return None;
        }
        let mut patterns = self.patterns.clone();
        patterns.set_match_kind(self.config.kind);
        let patterns = Arc::new(patterns);
        let rabinkarp = RabinKarp::new(&patterns);
        // Effectively, we only want to return a searcher if we can use Teddy,
        // since Teddy is our only fast packed searcher at the moment.
        // Rabin-Karp is only used when searching haystacks smaller than what
        // Teddy can support. Thus, the only way to get a Rabin-Karp searcher
        // is to force it using undocumented APIs (for tests/benchmarks).
        let (search_kind, minimum_len) = match self.config.force {
            None | Some(ForceAlgorithm::Teddy) => {
                debug!("trying to build Teddy packed matcher");
                let teddy = match self.build_teddy(Arc::clone(&patterns)) {
                    None => return None,
                    Some(teddy) => teddy,
                };
                let minimum_len = teddy.minimum_len();
                (SearchKind::Teddy(teddy), minimum_len)
            }
            Some(ForceAlgorithm::RabinKarp) => {
                debug!("using Rabin-Karp packed matcher");
                (SearchKind::RabinKarp, 0)
            }
        };
        Some(Searcher { patterns, rabinkarp, search_kind, minimum_len })
    }

    fn build_teddy(&self, patterns: Arc<Patterns>) -> Option<teddy::Searcher> {
        teddy::Builder::new()
            .only_256bit(self.config.only_teddy_256bit)
            .only_fat(self.config.only_teddy_fat)
            .heuristic_pattern_limits(self.config.heuristic_pattern_limits)
            .build(patterns)
    }

    /// Add the given pattern to this set to match.
    ///
    /// The order in which patterns are added is significant. Namely, when
    /// using leftmost-first match semantics, then when multiple patterns can
    /// match at a particular location, the pattern that was added first is
    /// used as the match.
    ///
    /// If the number of patterns added exceeds the amount supported by packed
    /// searchers, then the builder will stop accumulating patterns and render
    /// itself inert. At this point, constructing a searcher will always return
    /// `None`.
    pub fn add<P: AsRef<[u8]>>(&mut self, pattern: P) -> &mut Builder {
        if self.inert {
            return self;
        } else if self.patterns.len() >= PATTERN_LIMIT {
            self.inert = true;
            self.patterns.reset();
            return self;
        }
        // Just in case PATTERN_LIMIT increases beyond u16::MAX.
        assert!(self.patterns.len() <= core::u16::MAX as usize);

        let pattern = pattern.as_ref();
        if pattern.is_empty() {
            self.inert = true;
            self.patterns.reset();
            return self;
        }
        self.patterns.add(pattern);
        self
    }

    /// Add the given iterator of patterns to this set to match.
    ///
    /// The iterator must yield elements that can be converted into a `&[u8]`.
    ///
    /// The order in which patterns are added is significant. Namely, when
    /// using leftmost-first match semantics, then when multiple patterns can
    /// match at a particular location, the pattern that was added first is
    /// used as the match.
    ///
    /// If the number of patterns added exceeds the amount supported by packed
    /// searchers, then the builder will stop accumulating patterns and render
    /// itself inert. At this point, constructing a searcher will always return
    /// `None`.
    pub fn extend<I, P>(&mut self, patterns: I) -> &mut Builder
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        for p in patterns {
            self.add(p);
        }
        self
    }

    /// Returns the number of patterns added to this builder.
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Returns the length, in bytes, of the shortest pattern added.
    pub fn minimum_len(&self) -> usize {
        self.patterns.minimum_len()
    }
}

impl Default for Builder {
    fn default() -> Builder {
        Builder::new()
    }
}

/// A packed searcher for quickly finding occurrences of multiple patterns.
///
/// If callers need more flexible construction, or if one wants to change the
/// match semantics (either leftmost-first or leftmost-longest), then one can
/// use the [`Config`] and/or [`Builder`] types for more fine grained control.
///
/// # Example
///
/// This example shows how to create a searcher from an iterator of patterns.
/// By default, leftmost-first match semantics are used.
///
/// ```
/// use aho_corasick::{packed::{MatchKind, Searcher}, PatternID};
///
/// # fn example() -> Option<()> {
/// let searcher = Searcher::new(["foobar", "foo"].iter().cloned())?;
/// let matches: Vec<PatternID> = searcher
///     .find_iter("foobar")
///     .map(|mat| mat.pattern())
///     .collect();
/// assert_eq!(vec![PatternID::ZERO], matches);
/// # Some(()) }
/// # if cfg!(all(feature = "std", any(
/// #     target_arch = "x86_64", target_arch = "aarch64",
/// # ))) {
/// #     example().unwrap()
/// # } else {
/// #     assert!(example().is_none());
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Searcher {
    patterns: Arc<Patterns>,
    rabinkarp: RabinKarp,
    search_kind: SearchKind,
    minimum_len: usize,
}

#[derive(Clone, Debug)]
enum SearchKind {
    Teddy(teddy::Searcher),
    RabinKarp,
}

impl Searcher {
    /// A convenience function for constructing a searcher from an iterator
    /// of things that can be converted to a `&[u8]`.
    ///
    /// If a searcher could not be constructed (either because of an
    /// unsupported CPU or because there are too many patterns), then `None`
    /// is returned.
    ///
    /// # Example
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::{packed::{MatchKind, Searcher}, PatternID};
    ///
    /// # fn example() -> Option<()> {
    /// let searcher = Searcher::new(["foobar", "foo"].iter().cloned())?;
    /// let matches: Vec<PatternID> = searcher
    ///     .find_iter("foobar")
    ///     .map(|mat| mat.pattern())
    ///     .collect();
    /// assert_eq!(vec![PatternID::ZERO], matches);
    /// # Some(()) }
    /// # if cfg!(all(feature = "std", any(
    /// #     target_arch = "x86_64", target_arch = "aarch64",
    /// # ))) {
    /// #     example().unwrap()
    /// # } else {
    /// #     assert!(example().is_none());
    /// # }
    /// ```
    pub fn new<I, P>(patterns: I) -> Option<Searcher>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        Builder::new().extend(patterns).build()
    }

    /// A convenience function for calling `Config::new()`.
    ///
    /// This is useful for avoiding an additional import.
    pub fn config() -> Config {
        Config::new()
    }

    /// A convenience function for calling `Builder::new()`.
    ///
    /// This is useful for avoiding an additional import.
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// Return the first occurrence of any of the patterns in this searcher,
    /// according to its match semantics, in the given haystack. The `Match`
    /// returned will include the identifier of the pattern that matched, which
    /// corresponds to the index of the pattern (starting from `0`) in which it
    /// was added.
    ///
    /// # Example
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::{packed::{MatchKind, Searcher}, PatternID};
    ///
    /// # fn example() -> Option<()> {
    /// let searcher = Searcher::new(["foobar", "foo"].iter().cloned())?;
    /// let mat = searcher.find("foobar")?;
    /// assert_eq!(PatternID::ZERO, mat.pattern());
    /// assert_eq!(0, mat.start());
    /// assert_eq!(6, mat.end());
    /// # Some(()) }
    /// # if cfg!(all(feature = "std", any(
    /// #     target_arch = "x86_64", target_arch = "aarch64",
    /// # ))) {
    /// #     example().unwrap()
    /// # } else {
    /// #     assert!(example().is_none());
    /// # }
    /// ```
    #[inline]
    pub fn find<B: AsRef<[u8]>>(&self, haystack: B) -> Option<Match> {
        let haystack = haystack.as_ref();
        self.find_in(haystack, Span::from(0..haystack.len()))
    }

    /// Return the first occurrence of any of the patterns in this searcher,
    /// according to its match semantics, in the given haystack starting from
    /// the given position.
    ///
    /// The `Match` returned will include the identifier of the pattern that
    /// matched, which corresponds to the index of the pattern (starting from
    /// `0`) in which it was added. The offsets in the `Match` will be relative
    /// to the start of `haystack` (and not `at`).
    ///
    /// # Example
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::{packed::{MatchKind, Searcher}, PatternID, Span};
    ///
    /// # fn example() -> Option<()> {
    /// let haystack = "foofoobar";
    /// let searcher = Searcher::new(["foobar", "foo"].iter().cloned())?;
    /// let mat = searcher.find_in(haystack, Span::from(3..haystack.len()))?;
    /// assert_eq!(PatternID::ZERO, mat.pattern());
    /// assert_eq!(3, mat.start());
    /// assert_eq!(9, mat.end());
    /// # Some(()) }
    /// # if cfg!(all(feature = "std", any(
    /// #     target_arch = "x86_64", target_arch = "aarch64",
    /// # ))) {
    /// #     example().unwrap()
    /// # } else {
    /// #     assert!(example().is_none());
    /// # }
    /// ```
    #[inline]
    pub fn find_in<B: AsRef<[u8]>>(
        &self,
        haystack: B,
        span: Span,
    ) -> Option<Match> {
        let haystack = haystack.as_ref();
        match self.search_kind {
            SearchKind::Teddy(ref teddy) => {
                if haystack[span].len() < teddy.minimum_len() {
                    return self.find_in_slow(haystack, span);
                }
                teddy.find(&haystack[..span.end], span.start)
            }
            SearchKind::RabinKarp => {
                self.rabinkarp.find_at(&haystack[..span.end], span.start)
            }
        }
    }

    /// Return an iterator of non-overlapping occurrences of the patterns in
    /// this searcher, according to its match semantics, in the given haystack.
    ///
    /// # Example
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::{packed::{MatchKind, Searcher}, PatternID};
    ///
    /// # fn example() -> Option<()> {
    /// let searcher = Searcher::new(["foobar", "foo"].iter().cloned())?;
    /// let matches: Vec<PatternID> = searcher
    ///     .find_iter("foobar fooba foofoo")
    ///     .map(|mat| mat.pattern())
    ///     .collect();
    /// assert_eq!(vec![
    ///     PatternID::must(0),
    ///     PatternID::must(1),
    ///     PatternID::must(1),
    ///     PatternID::must(1),
    /// ], matches);
    /// # Some(()) }
    /// # if cfg!(all(feature = "std", any(
    /// #     target_arch = "x86_64", target_arch = "aarch64",
    /// # ))) {
    /// #     example().unwrap()
    /// # } else {
    /// #     assert!(example().is_none());
    /// # }
    /// ```
    #[inline]
    pub fn find_iter<'a, 'b, B: ?Sized + AsRef<[u8]>>(
        &'a self,
        haystack: &'b B,
    ) -> FindIter<'a, 'b> {
        let haystack = haystack.as_ref();
        let span = Span::from(0..haystack.len());
        FindIter { searcher: self, haystack, span }
    }

    /// Returns the match kind used by this packed searcher.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::packed::{MatchKind, Searcher};
    ///
    /// # fn example() -> Option<()> {
    /// let searcher = Searcher::new(["foobar", "foo"].iter().cloned())?;
    /// // leftmost-first is the default.
    /// assert_eq!(&MatchKind::LeftmostFirst, searcher.match_kind());
    /// # Some(()) }
    /// # if cfg!(all(feature = "std", any(
    /// #     target_arch = "x86_64", target_arch = "aarch64",
    /// # ))) {
    /// #     example().unwrap()
    /// # } else {
    /// #     assert!(example().is_none());
    /// # }
    /// ```
    #[inline]
    pub fn match_kind(&self) -> &MatchKind {
        self.patterns.match_kind()
    }

    /// Returns the minimum length of a haystack that is required in order for
    /// packed searching to be effective.
    ///
    /// In some cases, the underlying packed searcher may not be able to search
    /// very short haystacks. When that occurs, the implementation will defer
    /// to a slower non-packed searcher (which is still generally faster than
    /// Aho-Corasick for a small number of patterns). However, callers may
    /// want to avoid ever using the slower variant, which one can do by
    /// never passing a haystack shorter than the minimum length returned by
    /// this method.
    #[inline]
    pub fn minimum_len(&self) -> usize {
        self.minimum_len
    }

    /// Returns the approximate total amount of heap used by this searcher, in
    /// units of bytes.
    #[inline]
    pub fn memory_usage(&self) -> usize {
        self.patterns.memory_usage()
            + self.rabinkarp.memory_usage()
            + self.search_kind.memory_usage()
    }

    /// Use a slow (non-packed) searcher.
    ///
    /// This is useful when a packed searcher could be constructed, but could
    /// not be used to search a specific haystack. For example, if Teddy was
    /// built but the haystack is smaller than ~34 bytes, then Teddy might not
    /// be able to run.
    fn find_in_slow(&self, haystack: &[u8], span: Span) -> Option<Match> {
        self.rabinkarp.find_at(&haystack[..span.end], span.start)
    }
}

impl SearchKind {
    fn memory_usage(&self) -> usize {
        match *self {
            SearchKind::Teddy(ref ted) => ted.memory_usage(),
            SearchKind::RabinKarp => 0,
        }
    }
}

/// An iterator over non-overlapping matches from a packed searcher.
///
/// The lifetime `'s` refers to the lifetime of the underlying [`Searcher`],
/// while the lifetime `'h` refers to the lifetime of the haystack being
/// searched.
#[derive(Debug)]
pub struct FindIter<'s, 'h> {
    searcher: &'s Searcher,
    haystack: &'h [u8],
    span: Span,
}

impl<'s, 'h> Iterator for FindIter<'s, 'h> {
    type Item = Match;

    fn next(&mut self) -> Option<Match> {
        if self.span.start > self.span.end {
            return None;
        }
        match self.searcher.find_in(&self.haystack, self.span) {
            None => None,
            Some(m) => {
                self.span.start = m.end();
                Some(m)
            }
        }
    }
}
