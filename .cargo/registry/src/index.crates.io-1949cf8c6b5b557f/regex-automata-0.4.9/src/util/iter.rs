/*!
Generic helpers for iteration of matches from a regex engine in a haystack.

The principle type in this module is a [`Searcher`]. A `Searcher` provides
its own lower level iterator-like API in addition to methods for constructing
types that implement `Iterator`. The documentation for `Searcher` explains a
bit more about why these different APIs exist.

Currently, this module supports iteration over any regex engine that works
with the [`HalfMatch`], [`Match`] or [`Captures`] types.
*/

#[cfg(feature = "alloc")]
use crate::util::captures::Captures;
use crate::util::search::{HalfMatch, Input, Match, MatchError};

/// A searcher for creating iterators and performing lower level iteration.
///
/// This searcher encapsulates the logic required for finding all successive
/// non-overlapping matches in a haystack. In theory, iteration would look
/// something like this:
///
/// 1. Setting the start position to `0`.
/// 2. Execute a regex search. If no match, end iteration.
/// 3. Report the match and set the start position to the end of the match.
/// 4. Go back to (2).
///
/// And if this were indeed the case, it's likely that `Searcher` wouldn't
/// exist. Unfortunately, because a regex may match the empty string, the above
/// logic won't work for all possible regexes. Namely, if an empty match is
/// found, then step (3) would set the start position of the search to the
/// position it was at. Thus, iteration would never end.
///
/// Instead, a `Searcher` knows how to detect these cases and forcefully
/// advance iteration in the case of an empty match that overlaps with a
/// previous match.
///
/// If you know that your regex cannot match any empty string, then the simple
/// algorithm described above will work correctly.
///
/// When possible, prefer the iterators defined on the regex engine you're
/// using. This tries to abstract over the regex engine and is thus a bit more
/// unwieldy to use.
///
/// In particular, a `Searcher` is not itself an iterator. Instead, it provides
/// `advance` routines that permit moving the search along explicitly. It also
/// provides various routines, like [`Searcher::into_matches_iter`], that
/// accept a closure (representing how a regex engine executes a search) and
/// returns a conventional iterator.
///
/// The lifetime parameters come from the [`Input`] type passed to
/// [`Searcher::new`]:
///
/// * `'h` is the lifetime of the underlying haystack.
///
/// # Searcher vs Iterator
///
/// Why does a search type with "advance" APIs exist at all when we also have
/// iterators? Unfortunately, the reasoning behind this split is a complex
/// combination of the following things:
///
/// 1. While many of the regex engines expose their own iterators, it is also
/// nice to expose this lower level iteration helper because it permits callers
/// to provide their own `Input` configuration. Moreover, a `Searcher` can work
/// with _any_ regex engine instead of only the ones defined in this crate.
/// This way, everyone benefits from a shared iteration implementation.
/// 2. There are many different regex engines that, while they have the same
/// match semantics, they have slightly different APIs. Iteration is just
/// complex enough to want to share code, and so we need a way of abstracting
/// over those different regex engines. While we could define a new trait that
/// describes any regex engine search API, it would wind up looking very close
/// to a closure. While there may still be reasons for the more generic trait
/// to exist, for now and for the purposes of iteration, we use a closure.
/// Closures also provide a lot of easy flexibility at the call site, in that
/// they permit the caller to borrow any kind of state they want for use during
/// each search call.
/// 3. As a result of using closures, and because closures are anonymous types
/// that cannot be named, it is difficult to encapsulate them without both
/// costs to speed and added complexity to the public API. For example, in
/// defining an iterator type like
/// [`dfa::regex::FindMatches`](crate::dfa::regex::FindMatches),
/// if we use a closure internally, it's not possible to name this type in the
/// return type of the iterator constructor. Thus, the only way around it is
/// to erase the type by boxing it and turning it into a `Box<dyn FnMut ...>`.
/// This boxed closure is unlikely to be inlined _and_ it infects the public
/// API in subtle ways. Namely, unless you declare the closure as implementing
/// `Send` and `Sync`, then the resulting iterator type won't implement it
/// either. But there are practical issues with requiring the closure to
/// implement `Send` and `Sync` that result in other API complexities that
/// are beyond the scope of this already long exposition.
/// 4. Some regex engines expose more complex match information than just
/// "which pattern matched" and "at what offsets." For example, the PikeVM
/// exposes match spans for each capturing group that participated in the
/// match. In such cases, it can be quite beneficial to reuse the capturing
/// group allocation on subsequent searches. A proper iterator doesn't permit
/// this API due to its interface, so it's useful to have something a bit lower
/// level that permits callers to amortize allocations while also reusing a
/// shared implementation of iteration. (See the documentation for
/// [`Searcher::advance`] for an example of using the "advance" API with the
/// PikeVM.)
///
/// What this boils down to is that there are "advance" APIs which require
/// handing a closure to it for every call, and there are also APIs to create
/// iterators from a closure. The former are useful for _implementing_
/// iterators or when you need more flexibility, while the latter are useful
/// for conveniently writing custom iterators on-the-fly.
///
/// # Example: iterating with captures
///
/// Several regex engines in this crate over convenient iterator APIs over
/// [`Captures`] values. To do so, this requires allocating a new `Captures`
/// value for each iteration step. This can perhaps be more costly than you
/// might want. Instead of implementing your own iterator to avoid that
/// cost (which can be a little subtle if you want to handle empty matches
/// correctly), you can use this `Searcher` to do it for you:
///
/// ```
/// use regex_automata::{
///     nfa::thompson::pikevm::PikeVM,
///     util::iter::Searcher,
///     Input, Span,
/// };
///
/// let re = PikeVM::new("foo(?P<numbers>[0-9]+)")?;
/// let haystack = "foo1 foo12 foo123";
///
/// let mut caps = re.create_captures();
/// let mut cache = re.create_cache();
/// let mut matches = vec![];
/// let mut searcher = Searcher::new(Input::new(haystack));
/// while let Some(_) = searcher.advance(|input| {
///     re.search(&mut cache, input, &mut caps);
///     Ok(caps.get_match())
/// }) {
///     // The unwrap is OK since 'numbers' matches if the pattern matches.
///     matches.push(caps.get_group_by_name("numbers").unwrap());
/// }
/// assert_eq!(matches, vec![
///     Span::from(3..4),
///     Span::from(8..10),
///     Span::from(14..17),
/// ]);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct Searcher<'h> {
    /// The input parameters to give to each regex engine call.
    ///
    /// The start position of the search is mutated during iteration.
    input: Input<'h>,
    /// Records the end offset of the most recent match. This is necessary to
    /// handle a corner case for preventing empty matches from overlapping with
    /// the ending bounds of a prior match.
    last_match_end: Option<usize>,
}

impl<'h> Searcher<'h> {
    /// Create a new fallible non-overlapping matches iterator.
    ///
    /// The given `input` provides the parameters (including the haystack),
    /// while the `finder` represents a closure that calls the underlying regex
    /// engine. The closure may borrow any additional state that is needed,
    /// such as a prefilter scanner.
    pub fn new(input: Input<'h>) -> Searcher<'h> {
        Searcher { input, last_match_end: None }
    }

    /// Returns the current `Input` used by this searcher.
    ///
    /// The `Input` returned is generally equivalent to the one given to
    /// [`Searcher::new`], but its start position may be different to reflect
    /// the start of the next search to be executed.
    pub fn input<'s>(&'s self) -> &'s Input<'h> {
        &self.input
    }

    /// Return the next half match for an infallible search if one exists, and
    /// advance to the next position.
    ///
    /// This is like `try_advance_half`, except errors are converted into
    /// panics.
    ///
    /// # Panics
    ///
    /// If the given closure returns an error, then this panics. This is useful
    /// when you know your underlying regex engine has been configured to not
    /// return an error.
    ///
    /// # Example
    ///
    /// This example shows how to use a `Searcher` to iterate over all matches
    /// when using a DFA, which only provides "half" matches.
    ///
    /// ```
    /// use regex_automata::{
    ///     hybrid::dfa::DFA,
    ///     util::iter::Searcher,
    ///     HalfMatch, Input,
    /// };
    ///
    /// let re = DFA::new(r"[0-9]{4}-[0-9]{2}-[0-9]{2}")?;
    /// let mut cache = re.create_cache();
    ///
    /// let input = Input::new("2010-03-14 2016-10-08 2020-10-22");
    /// let mut it = Searcher::new(input);
    ///
    /// let expected = Some(HalfMatch::must(0, 10));
    /// let got = it.advance_half(|input| re.try_search_fwd(&mut cache, input));
    /// assert_eq!(expected, got);
    ///
    /// let expected = Some(HalfMatch::must(0, 21));
    /// let got = it.advance_half(|input| re.try_search_fwd(&mut cache, input));
    /// assert_eq!(expected, got);
    ///
    /// let expected = Some(HalfMatch::must(0, 32));
    /// let got = it.advance_half(|input| re.try_search_fwd(&mut cache, input));
    /// assert_eq!(expected, got);
    ///
    /// let expected = None;
    /// let got = it.advance_half(|input| re.try_search_fwd(&mut cache, input));
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This correctly moves iteration forward even when an empty match occurs:
    ///
    /// ```
    /// use regex_automata::{
    ///     hybrid::dfa::DFA,
    ///     util::iter::Searcher,
    ///     HalfMatch, Input,
    /// };
    ///
    /// let re = DFA::new(r"a|")?;
    /// let mut cache = re.create_cache();
    ///
    /// let input = Input::new("abba");
    /// let mut it = Searcher::new(input);
    ///
    /// let expected = Some(HalfMatch::must(0, 1));
    /// let got = it.advance_half(|input| re.try_search_fwd(&mut cache, input));
    /// assert_eq!(expected, got);
    ///
    /// let expected = Some(HalfMatch::must(0, 2));
    /// let got = it.advance_half(|input| re.try_search_fwd(&mut cache, input));
    /// assert_eq!(expected, got);
    ///
    /// let expected = Some(HalfMatch::must(0, 4));
    /// let got = it.advance_half(|input| re.try_search_fwd(&mut cache, input));
    /// assert_eq!(expected, got);
    ///
    /// let expected = None;
    /// let got = it.advance_half(|input| re.try_search_fwd(&mut cache, input));
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn advance_half<F>(&mut self, finder: F) -> Option<HalfMatch>
    where
        F: FnMut(&Input<'_>) -> Result<Option<HalfMatch>, MatchError>,
    {
        match self.try_advance_half(finder) {
            Ok(m) => m,
            Err(err) => panic!(
                "unexpected regex half find error: {}\n\
                 to handle find errors, use 'try' or 'search' methods",
                err,
            ),
        }
    }

    /// Return the next match for an infallible search if one exists, and
    /// advance to the next position.
    ///
    /// The search is advanced even in the presence of empty matches by
    /// forbidding empty matches from overlapping with any other match.
    ///
    /// This is like `try_advance`, except errors are converted into panics.
    ///
    /// # Panics
    ///
    /// If the given closure returns an error, then this panics. This is useful
    /// when you know your underlying regex engine has been configured to not
    /// return an error.
    ///
    /// # Example
    ///
    /// This example shows how to use a `Searcher` to iterate over all matches
    /// when using a regex based on lazy DFAs:
    ///
    /// ```
    /// use regex_automata::{
    ///     hybrid::regex::Regex,
    ///     util::iter::Searcher,
    ///     Match, Input,
    /// };
    ///
    /// let re = Regex::new(r"[0-9]{4}-[0-9]{2}-[0-9]{2}")?;
    /// let mut cache = re.create_cache();
    ///
    /// let input = Input::new("2010-03-14 2016-10-08 2020-10-22");
    /// let mut it = Searcher::new(input);
    ///
    /// let expected = Some(Match::must(0, 0..10));
    /// let got = it.advance(|input| re.try_search(&mut cache, input));
    /// assert_eq!(expected, got);
    ///
    /// let expected = Some(Match::must(0, 11..21));
    /// let got = it.advance(|input| re.try_search(&mut cache, input));
    /// assert_eq!(expected, got);
    ///
    /// let expected = Some(Match::must(0, 22..32));
    /// let got = it.advance(|input| re.try_search(&mut cache, input));
    /// assert_eq!(expected, got);
    ///
    /// let expected = None;
    /// let got = it.advance(|input| re.try_search(&mut cache, input));
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This example shows the same as above, but with the PikeVM. This example
    /// is useful because it shows how to use this API even when the regex
    /// engine doesn't directly return a `Match`.
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson::pikevm::PikeVM,
    ///     util::iter::Searcher,
    ///     Match, Input,
    /// };
    ///
    /// let re = PikeVM::new(r"[0-9]{4}-[0-9]{2}-[0-9]{2}")?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// let input = Input::new("2010-03-14 2016-10-08 2020-10-22");
    /// let mut it = Searcher::new(input);
    ///
    /// let expected = Some(Match::must(0, 0..10));
    /// let got = it.advance(|input| {
    ///     re.search(&mut cache, input, &mut caps);
    ///     Ok(caps.get_match())
    /// });
    /// // Note that if we wanted to extract capturing group spans, we could
    /// // do that here with 'caps'.
    /// assert_eq!(expected, got);
    ///
    /// let expected = Some(Match::must(0, 11..21));
    /// let got = it.advance(|input| {
    ///     re.search(&mut cache, input, &mut caps);
    ///     Ok(caps.get_match())
    /// });
    /// assert_eq!(expected, got);
    ///
    /// let expected = Some(Match::must(0, 22..32));
    /// let got = it.advance(|input| {
    ///     re.search(&mut cache, input, &mut caps);
    ///     Ok(caps.get_match())
    /// });
    /// assert_eq!(expected, got);
    ///
    /// let expected = None;
    /// let got = it.advance(|input| {
    ///     re.search(&mut cache, input, &mut caps);
    ///     Ok(caps.get_match())
    /// });
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn advance<F>(&mut self, finder: F) -> Option<Match>
    where
        F: FnMut(&Input<'_>) -> Result<Option<Match>, MatchError>,
    {
        match self.try_advance(finder) {
            Ok(m) => m,
            Err(err) => panic!(
                "unexpected regex find error: {}\n\
                 to handle find errors, use 'try' or 'search' methods",
                err,
            ),
        }
    }

    /// Return the next half match for a fallible search if one exists, and
    /// advance to the next position.
    ///
    /// This is like `advance_half`, except it permits callers to handle errors
    /// during iteration.
    #[inline]
    pub fn try_advance_half<F>(
        &mut self,
        mut finder: F,
    ) -> Result<Option<HalfMatch>, MatchError>
    where
        F: FnMut(&Input<'_>) -> Result<Option<HalfMatch>, MatchError>,
    {
        let mut m = match finder(&self.input)? {
            None => return Ok(None),
            Some(m) => m,
        };
        if Some(m.offset()) == self.last_match_end {
            m = match self.handle_overlapping_empty_half_match(m, finder)? {
                None => return Ok(None),
                Some(m) => m,
            };
        }
        self.input.set_start(m.offset());
        self.last_match_end = Some(m.offset());
        Ok(Some(m))
    }

    /// Return the next match for a fallible search if one exists, and advance
    /// to the next position.
    ///
    /// This is like `advance`, except it permits callers to handle errors
    /// during iteration.
    #[inline]
    pub fn try_advance<F>(
        &mut self,
        mut finder: F,
    ) -> Result<Option<Match>, MatchError>
    where
        F: FnMut(&Input<'_>) -> Result<Option<Match>, MatchError>,
    {
        let mut m = match finder(&self.input)? {
            None => return Ok(None),
            Some(m) => m,
        };
        if m.is_empty() && Some(m.end()) == self.last_match_end {
            m = match self.handle_overlapping_empty_match(m, finder)? {
                None => return Ok(None),
                Some(m) => m,
            };
        }
        self.input.set_start(m.end());
        self.last_match_end = Some(m.end());
        Ok(Some(m))
    }

    /// Given a closure that executes a single search, return an iterator over
    /// all successive non-overlapping half matches.
    ///
    /// The iterator returned yields result values. If the underlying regex
    /// engine is configured to never return an error, consider calling
    /// [`TryHalfMatchesIter::infallible`] to convert errors into panics.
    ///
    /// # Example
    ///
    /// This example shows how to use a `Searcher` to create a proper
    /// iterator over half matches.
    ///
    /// ```
    /// use regex_automata::{
    ///     hybrid::dfa::DFA,
    ///     util::iter::Searcher,
    ///     HalfMatch, Input,
    /// };
    ///
    /// let re = DFA::new(r"[0-9]{4}-[0-9]{2}-[0-9]{2}")?;
    /// let mut cache = re.create_cache();
    ///
    /// let input = Input::new("2010-03-14 2016-10-08 2020-10-22");
    /// let mut it = Searcher::new(input).into_half_matches_iter(|input| {
    ///     re.try_search_fwd(&mut cache, input)
    /// });
    ///
    /// let expected = Some(Ok(HalfMatch::must(0, 10)));
    /// assert_eq!(expected, it.next());
    ///
    /// let expected = Some(Ok(HalfMatch::must(0, 21)));
    /// assert_eq!(expected, it.next());
    ///
    /// let expected = Some(Ok(HalfMatch::must(0, 32)));
    /// assert_eq!(expected, it.next());
    ///
    /// let expected = None;
    /// assert_eq!(expected, it.next());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn into_half_matches_iter<F>(
        self,
        finder: F,
    ) -> TryHalfMatchesIter<'h, F>
    where
        F: FnMut(&Input<'_>) -> Result<Option<HalfMatch>, MatchError>,
    {
        TryHalfMatchesIter { it: self, finder }
    }

    /// Given a closure that executes a single search, return an iterator over
    /// all successive non-overlapping matches.
    ///
    /// The iterator returned yields result values. If the underlying regex
    /// engine is configured to never return an error, consider calling
    /// [`TryMatchesIter::infallible`] to convert errors into panics.
    ///
    /// # Example
    ///
    /// This example shows how to use a `Searcher` to create a proper
    /// iterator over matches.
    ///
    /// ```
    /// use regex_automata::{
    ///     hybrid::regex::Regex,
    ///     util::iter::Searcher,
    ///     Match, Input,
    /// };
    ///
    /// let re = Regex::new(r"[0-9]{4}-[0-9]{2}-[0-9]{2}")?;
    /// let mut cache = re.create_cache();
    ///
    /// let input = Input::new("2010-03-14 2016-10-08 2020-10-22");
    /// let mut it = Searcher::new(input).into_matches_iter(|input| {
    ///     re.try_search(&mut cache, input)
    /// });
    ///
    /// let expected = Some(Ok(Match::must(0, 0..10)));
    /// assert_eq!(expected, it.next());
    ///
    /// let expected = Some(Ok(Match::must(0, 11..21)));
    /// assert_eq!(expected, it.next());
    ///
    /// let expected = Some(Ok(Match::must(0, 22..32)));
    /// assert_eq!(expected, it.next());
    ///
    /// let expected = None;
    /// assert_eq!(expected, it.next());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn into_matches_iter<F>(self, finder: F) -> TryMatchesIter<'h, F>
    where
        F: FnMut(&Input<'_>) -> Result<Option<Match>, MatchError>,
    {
        TryMatchesIter { it: self, finder }
    }

    /// Given a closure that executes a single search, return an iterator over
    /// all successive non-overlapping `Captures` values.
    ///
    /// The iterator returned yields result values. If the underlying regex
    /// engine is configured to never return an error, consider calling
    /// [`TryCapturesIter::infallible`] to convert errors into panics.
    ///
    /// Unlike the other iterator constructors, this accepts an initial
    /// `Captures` value. This `Captures` value is reused for each search, and
    /// the iterator implementation clones it before returning it. The caller
    /// must provide this value because the iterator is purposely ignorant
    /// of the underlying regex engine and thus doesn't know how to create
    /// one itself. More to the point, a `Captures` value itself has a few
    /// different constructors, which change which kind of information is
    /// available to query in exchange for search performance.
    ///
    /// # Example
    ///
    /// This example shows how to use a `Searcher` to create a proper iterator
    /// over `Captures` values, which provides access to all capturing group
    /// spans for each match.
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson::pikevm::PikeVM,
    ///     util::iter::Searcher,
    ///     Input,
    /// };
    ///
    /// let re = PikeVM::new(
    ///     r"(?P<y>[0-9]{4})-(?P<m>[0-9]{2})-(?P<d>[0-9]{2})",
    /// )?;
    /// let (mut cache, caps) = (re.create_cache(), re.create_captures());
    ///
    /// let haystack = "2010-03-14 2016-10-08 2020-10-22";
    /// let input = Input::new(haystack);
    /// let mut it = Searcher::new(input)
    ///     .into_captures_iter(caps, |input, caps| {
    ///         re.search(&mut cache, input, caps);
    ///         Ok(())
    ///     });
    ///
    /// let got = it.next().expect("first date")?;
    /// let year = got.get_group_by_name("y").expect("must match");
    /// assert_eq!("2010", &haystack[year]);
    ///
    /// let got = it.next().expect("second date")?;
    /// let month = got.get_group_by_name("m").expect("must match");
    /// assert_eq!("10", &haystack[month]);
    ///
    /// let got = it.next().expect("third date")?;
    /// let day = got.get_group_by_name("d").expect("must match");
    /// assert_eq!("22", &haystack[day]);
    ///
    /// assert!(it.next().is_none());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "alloc")]
    #[inline]
    pub fn into_captures_iter<F>(
        self,
        caps: Captures,
        finder: F,
    ) -> TryCapturesIter<'h, F>
    where
        F: FnMut(&Input<'_>, &mut Captures) -> Result<(), MatchError>,
    {
        TryCapturesIter { it: self, caps, finder }
    }

    /// Handles the special case of a match that begins where the previous
    /// match ended. Without this special handling, it'd be possible to get
    /// stuck where an empty match never results in forward progress. This
    /// also makes it more consistent with how presiding general purpose regex
    /// engines work.
    #[cold]
    #[inline(never)]
    fn handle_overlapping_empty_half_match<F>(
        &mut self,
        _: HalfMatch,
        mut finder: F,
    ) -> Result<Option<HalfMatch>, MatchError>
    where
        F: FnMut(&Input<'_>) -> Result<Option<HalfMatch>, MatchError>,
    {
        // Since we are only here when 'm.offset()' matches the offset of the
        // last match, it follows that this must have been an empty match.
        // Since we both need to make progress *and* prevent overlapping
        // matches, we discard this match and advance the search by 1.
        //
        // Note that this may start a search in the middle of a codepoint. The
        // regex engines themselves are expected to deal with that and not
        // report any matches within a codepoint if they are configured in
        // UTF-8 mode.
        self.input.set_start(self.input.start().checked_add(1).unwrap());
        finder(&self.input)
    }

    /// Handles the special case of an empty match by ensuring that 1) the
    /// iterator always advances and 2) empty matches never overlap with other
    /// matches.
    ///
    /// (1) is necessary because we principally make progress by setting the
    /// starting location of the next search to the ending location of the last
    /// match. But if a match is empty, then this results in a search that does
    /// not advance and thus does not terminate.
    ///
    /// (2) is not strictly necessary, but makes intuitive sense and matches
    /// the presiding behavior of most general purpose regex engines. The
    /// "intuitive sense" here is that we want to report NON-overlapping
    /// matches. So for example, given the regex 'a|(?:)' against the haystack
    /// 'a', without the special handling, you'd get the matches [0, 1) and [1,
    /// 1), where the latter overlaps with the end bounds of the former.
    ///
    /// Note that we mark this cold and forcefully prevent inlining because
    /// handling empty matches like this is extremely rare and does require
    /// quite a bit of code, comparatively. Keeping this code out of the main
    /// iterator function keeps it smaller and more amenable to inlining
    /// itself.
    #[cold]
    #[inline(never)]
    fn handle_overlapping_empty_match<F>(
        &mut self,
        m: Match,
        mut finder: F,
    ) -> Result<Option<Match>, MatchError>
    where
        F: FnMut(&Input<'_>) -> Result<Option<Match>, MatchError>,
    {
        assert!(m.is_empty());
        self.input.set_start(self.input.start().checked_add(1).unwrap());
        finder(&self.input)
    }
}

/// An iterator over all non-overlapping half matches for a fallible search.
///
/// The iterator yields a `Result<HalfMatch, MatchError>` value until no more
/// matches could be found.
///
/// The type parameters are as follows:
///
/// * `F` represents the type of a closure that executes the search.
///
/// The lifetime parameters come from the [`Input`] type:
///
/// * `'h` is the lifetime of the underlying haystack.
///
/// When possible, prefer the iterators defined on the regex engine you're
/// using. This tries to abstract over the regex engine and is thus a bit more
/// unwieldy to use.
///
/// This iterator is created by [`Searcher::into_half_matches_iter`].
pub struct TryHalfMatchesIter<'h, F> {
    it: Searcher<'h>,
    finder: F,
}

impl<'h, F> TryHalfMatchesIter<'h, F> {
    /// Return an infallible version of this iterator.
    ///
    /// Any item yielded that corresponds to an error results in a panic. This
    /// is useful if your underlying regex engine is configured in a way that
    /// it is guaranteed to never return an error.
    pub fn infallible(self) -> HalfMatchesIter<'h, F> {
        HalfMatchesIter(self)
    }

    /// Returns the current `Input` used by this iterator.
    ///
    /// The `Input` returned is generally equivalent to the one used to
    /// construct this iterator, but its start position may be different to
    /// reflect the start of the next search to be executed.
    pub fn input<'i>(&'i self) -> &'i Input<'h> {
        self.it.input()
    }
}

impl<'h, F> Iterator for TryHalfMatchesIter<'h, F>
where
    F: FnMut(&Input<'_>) -> Result<Option<HalfMatch>, MatchError>,
{
    type Item = Result<HalfMatch, MatchError>;

    #[inline]
    fn next(&mut self) -> Option<Result<HalfMatch, MatchError>> {
        self.it.try_advance_half(&mut self.finder).transpose()
    }
}

impl<'h, F> core::fmt::Debug for TryHalfMatchesIter<'h, F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TryHalfMatchesIter")
            .field("it", &self.it)
            .field("finder", &"<closure>")
            .finish()
    }
}

/// An iterator over all non-overlapping half matches for an infallible search.
///
/// The iterator yields a [`HalfMatch`] value until no more matches could be
/// found.
///
/// The type parameters are as follows:
///
/// * `F` represents the type of a closure that executes the search.
///
/// The lifetime parameters come from the [`Input`] type:
///
/// * `'h` is the lifetime of the underlying haystack.
///
/// When possible, prefer the iterators defined on the regex engine you're
/// using. This tries to abstract over the regex engine and is thus a bit more
/// unwieldy to use.
///
/// This iterator is created by [`Searcher::into_half_matches_iter`] and
/// then calling [`TryHalfMatchesIter::infallible`].
#[derive(Debug)]
pub struct HalfMatchesIter<'h, F>(TryHalfMatchesIter<'h, F>);

impl<'h, F> HalfMatchesIter<'h, F> {
    /// Returns the current `Input` used by this iterator.
    ///
    /// The `Input` returned is generally equivalent to the one used to
    /// construct this iterator, but its start position may be different to
    /// reflect the start of the next search to be executed.
    pub fn input<'i>(&'i self) -> &'i Input<'h> {
        self.0.it.input()
    }
}

impl<'h, F> Iterator for HalfMatchesIter<'h, F>
where
    F: FnMut(&Input<'_>) -> Result<Option<HalfMatch>, MatchError>,
{
    type Item = HalfMatch;

    #[inline]
    fn next(&mut self) -> Option<HalfMatch> {
        match self.0.next()? {
            Ok(m) => Some(m),
            Err(err) => panic!(
                "unexpected regex half find error: {}\n\
                 to handle find errors, use 'try' or 'search' methods",
                err,
            ),
        }
    }
}

/// An iterator over all non-overlapping matches for a fallible search.
///
/// The iterator yields a `Result<Match, MatchError>` value until no more
/// matches could be found.
///
/// The type parameters are as follows:
///
/// * `F` represents the type of a closure that executes the search.
///
/// The lifetime parameters come from the [`Input`] type:
///
/// * `'h` is the lifetime of the underlying haystack.
///
/// When possible, prefer the iterators defined on the regex engine you're
/// using. This tries to abstract over the regex engine and is thus a bit more
/// unwieldy to use.
///
/// This iterator is created by [`Searcher::into_matches_iter`].
pub struct TryMatchesIter<'h, F> {
    it: Searcher<'h>,
    finder: F,
}

impl<'h, F> TryMatchesIter<'h, F> {
    /// Return an infallible version of this iterator.
    ///
    /// Any item yielded that corresponds to an error results in a panic. This
    /// is useful if your underlying regex engine is configured in a way that
    /// it is guaranteed to never return an error.
    pub fn infallible(self) -> MatchesIter<'h, F> {
        MatchesIter(self)
    }

    /// Returns the current `Input` used by this iterator.
    ///
    /// The `Input` returned is generally equivalent to the one used to
    /// construct this iterator, but its start position may be different to
    /// reflect the start of the next search to be executed.
    pub fn input<'i>(&'i self) -> &'i Input<'h> {
        self.it.input()
    }
}

impl<'h, F> Iterator for TryMatchesIter<'h, F>
where
    F: FnMut(&Input<'_>) -> Result<Option<Match>, MatchError>,
{
    type Item = Result<Match, MatchError>;

    #[inline]
    fn next(&mut self) -> Option<Result<Match, MatchError>> {
        self.it.try_advance(&mut self.finder).transpose()
    }
}

impl<'h, F> core::fmt::Debug for TryMatchesIter<'h, F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TryMatchesIter")
            .field("it", &self.it)
            .field("finder", &"<closure>")
            .finish()
    }
}

/// An iterator over all non-overlapping matches for an infallible search.
///
/// The iterator yields a [`Match`] value until no more matches could be found.
///
/// The type parameters are as follows:
///
/// * `F` represents the type of a closure that executes the search.
///
/// The lifetime parameters come from the [`Input`] type:
///
/// * `'h` is the lifetime of the underlying haystack.
///
/// When possible, prefer the iterators defined on the regex engine you're
/// using. This tries to abstract over the regex engine and is thus a bit more
/// unwieldy to use.
///
/// This iterator is created by [`Searcher::into_matches_iter`] and
/// then calling [`TryMatchesIter::infallible`].
#[derive(Debug)]
pub struct MatchesIter<'h, F>(TryMatchesIter<'h, F>);

impl<'h, F> MatchesIter<'h, F> {
    /// Returns the current `Input` used by this iterator.
    ///
    /// The `Input` returned is generally equivalent to the one used to
    /// construct this iterator, but its start position may be different to
    /// reflect the start of the next search to be executed.
    pub fn input<'i>(&'i self) -> &'i Input<'h> {
        self.0.it.input()
    }
}

impl<'h, F> Iterator for MatchesIter<'h, F>
where
    F: FnMut(&Input<'_>) -> Result<Option<Match>, MatchError>,
{
    type Item = Match;

    #[inline]
    fn next(&mut self) -> Option<Match> {
        match self.0.next()? {
            Ok(m) => Some(m),
            Err(err) => panic!(
                "unexpected regex find error: {}\n\
                 to handle find errors, use 'try' or 'search' methods",
                err,
            ),
        }
    }
}

/// An iterator over all non-overlapping captures for a fallible search.
///
/// The iterator yields a `Result<Captures, MatchError>` value until no more
/// matches could be found.
///
/// The type parameters are as follows:
///
/// * `F` represents the type of a closure that executes the search.
///
/// The lifetime parameters come from the [`Input`] type:
///
/// * `'h` is the lifetime of the underlying haystack.
///
/// When possible, prefer the iterators defined on the regex engine you're
/// using. This tries to abstract over the regex engine and is thus a bit more
/// unwieldy to use.
///
/// This iterator is created by [`Searcher::into_captures_iter`].
#[cfg(feature = "alloc")]
pub struct TryCapturesIter<'h, F> {
    it: Searcher<'h>,
    caps: Captures,
    finder: F,
}

#[cfg(feature = "alloc")]
impl<'h, F> TryCapturesIter<'h, F> {
    /// Return an infallible version of this iterator.
    ///
    /// Any item yielded that corresponds to an error results in a panic. This
    /// is useful if your underlying regex engine is configured in a way that
    /// it is guaranteed to never return an error.
    pub fn infallible(self) -> CapturesIter<'h, F> {
        CapturesIter(self)
    }
}

#[cfg(feature = "alloc")]
impl<'h, F> Iterator for TryCapturesIter<'h, F>
where
    F: FnMut(&Input<'_>, &mut Captures) -> Result<(), MatchError>,
{
    type Item = Result<Captures, MatchError>;

    #[inline]
    fn next(&mut self) -> Option<Result<Captures, MatchError>> {
        let TryCapturesIter { ref mut it, ref mut caps, ref mut finder } =
            *self;
        let result = it
            .try_advance(|input| {
                (finder)(input, caps)?;
                Ok(caps.get_match())
            })
            .transpose()?;
        match result {
            Ok(_) => Some(Ok(caps.clone())),
            Err(err) => Some(Err(err)),
        }
    }
}

#[cfg(feature = "alloc")]
impl<'h, F> core::fmt::Debug for TryCapturesIter<'h, F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TryCapturesIter")
            .field("it", &self.it)
            .field("caps", &self.caps)
            .field("finder", &"<closure>")
            .finish()
    }
}

/// An iterator over all non-overlapping captures for an infallible search.
///
/// The iterator yields a [`Captures`] value until no more matches could be
/// found.
///
/// The type parameters are as follows:
///
/// * `F` represents the type of a closure that executes the search.
///
/// The lifetime parameters come from the [`Input`] type:
///
/// * `'h` is the lifetime of the underlying haystack.
///
/// When possible, prefer the iterators defined on the regex engine you're
/// using. This tries to abstract over the regex engine and is thus a bit more
/// unwieldy to use.
///
/// This iterator is created by [`Searcher::into_captures_iter`] and then
/// calling [`TryCapturesIter::infallible`].
#[cfg(feature = "alloc")]
#[derive(Debug)]
pub struct CapturesIter<'h, F>(TryCapturesIter<'h, F>);

#[cfg(feature = "alloc")]
impl<'h, F> Iterator for CapturesIter<'h, F>
where
    F: FnMut(&Input<'_>, &mut Captures) -> Result<(), MatchError>,
{
    type Item = Captures;

    #[inline]
    fn next(&mut self) -> Option<Captures> {
        match self.0.next()? {
            Ok(m) => Some(m),
            Err(err) => panic!(
                "unexpected regex captures error: {}\n\
                 to handle find errors, use 'try' or 'search' methods",
                err,
            ),
        }
    }
}
