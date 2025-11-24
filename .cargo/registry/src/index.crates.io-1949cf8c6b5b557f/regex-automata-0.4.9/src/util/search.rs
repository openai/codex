/*!
Types and routines that support the search APIs of most regex engines.

This sub-module isn't exposed directly, but rather, its contents are exported
at the crate root due to the universality of most of the types and routines in
this module.
*/

use core::ops::{Range, RangeBounds};

use crate::util::{escape::DebugByte, primitives::PatternID, utf8};

/// The parameters for a regex search including the haystack to search.
///
/// It turns out that regex searches have a few parameters, and in most cases,
/// those parameters have defaults that work in the vast majority of cases.
/// This `Input` type exists to make that common case seamnless while also
/// providing an avenue for changing the parameters of a search. In particular,
/// this type enables doing so without a combinatorial explosion of different
/// methods and/or superfluous parameters in the common cases.
///
/// An `Input` permits configuring the following things:
///
/// * Search only a substring of a haystack, while taking the broader context
/// into account for resolving look-around assertions.
/// * Indicating whether to search for all patterns in a regex, or to
/// only search for one pattern in particular.
/// * Whether to perform an anchored on unanchored search.
/// * Whether to report a match as early as possible.
///
/// All of these parameters, except for the haystack, have sensible default
/// values. This means that the minimal search configuration is simply a call
/// to [`Input::new`] with your haystack. Setting any other parameter is
/// optional.
///
/// Moreover, for any `H` that implements `AsRef<[u8]>`, there exists a
/// `From<H> for Input` implementation. This is useful because many of the
/// search APIs in this crate accept an `Into<Input>`. This means you can
/// provide string or byte strings to these routines directly, and they'll
/// automatically get converted into an `Input` for you.
///
/// The lifetime parameter `'h` refers to the lifetime of the haystack.
///
/// # Organization
///
/// The API of `Input` is split into a few different parts:
///
/// * A builder-like API that transforms a `Input` by value. Examples:
/// [`Input::span`] and [`Input::anchored`].
/// * A setter API that permits mutating parameters in place. Examples:
/// [`Input::set_span`] and [`Input::set_anchored`].
/// * A getter API that permits retrieving any of the search parameters.
/// Examples: [`Input::get_span`] and [`Input::get_anchored`].
/// * A few convenience getter routines that don't conform to the above naming
/// pattern due to how common they are. Examples: [`Input::haystack`],
/// [`Input::start`] and [`Input::end`].
/// * Miscellaneous predicates and other helper routines that are useful
/// in some contexts. Examples: [`Input::is_char_boundary`].
///
/// A `Input` exposes so much because it is meant to be used by both callers of
/// regex engines _and_ implementors of regex engines. A constraining factor is
/// that regex engines should accept a `&Input` as its lowest level API, which
/// means that implementors should only use the "getter" APIs of a `Input`.
///
/// # Valid bounds and search termination
///
/// An `Input` permits setting the bounds of a search via either
/// [`Input::span`] or [`Input::range`]. The bounds set must be valid, or
/// else a panic will occur. Bounds are valid if and only if:
///
/// * The bounds represent a valid range into the input's haystack.
/// * **or** the end bound is a valid ending bound for the haystack *and*
/// the start bound is exactly one greater than the start bound.
///
/// In the latter case, [`Input::is_done`] will return true and indicates any
/// search receiving such an input should immediately return with no match.
///
/// Note that while `Input` is used for reverse searches in this crate, the
/// `Input::is_done` predicate assumes a forward search. Because unsigned
/// offsets are used internally, there is no way to tell from only the offsets
/// whether a reverse search is done or not.
///
/// # Regex engine support
///
/// Any regex engine accepting an `Input` must support at least the following
/// things:
///
/// * Searching a `&[u8]` for matches.
/// * Searching a substring of `&[u8]` for a match, such that any match
/// reported must appear entirely within that substring.
/// * For a forwards search, a match should never be reported when
/// [`Input::is_done`] returns true. (For reverse searches, termination should
/// be handled outside of `Input`.)
///
/// Supporting other aspects of an `Input` are optional, but regex engines
/// should handle aspects they don't support gracefully. How this is done is
/// generally up to the regex engine. This crate generally treats unsupported
/// anchored modes as an error to report for example, but for simplicity, in
/// the meta regex engine, trying to search with an invalid pattern ID just
/// results in no match being reported.
#[derive(Clone)]
pub struct Input<'h> {
    haystack: &'h [u8],
    span: Span,
    anchored: Anchored,
    earliest: bool,
}

impl<'h> Input<'h> {
    /// Create a new search configuration for the given haystack.
    #[inline]
    pub fn new<H: ?Sized + AsRef<[u8]>>(haystack: &'h H) -> Input<'h> {
        // Perform only one call to `haystack.as_ref()` to protect from incorrect
        // implementations that return different values from multiple calls.
        // This is important because there's code that relies on `span` not being
        // out of bounds with respect to the stored `haystack`.
        let haystack = haystack.as_ref();
        Input {
            haystack,
            span: Span { start: 0, end: haystack.len() },
            anchored: Anchored::No,
            earliest: false,
        }
    }

    /// Set the span for this search.
    ///
    /// This routine does not panic if the span given is not a valid range for
    /// this search's haystack. If this search is run with an invalid range,
    /// then the most likely outcome is that the actual search execution will
    /// panic.
    ///
    /// This routine is generic over how a span is provided. While
    /// a [`Span`] may be given directly, one may also provide a
    /// `std::ops::Range<usize>`. To provide anything supported by range
    /// syntax, use the [`Input::range`] method.
    ///
    /// The default span is the entire haystack.
    ///
    /// Note that [`Input::range`] overrides this method and vice versa.
    ///
    /// # Panics
    ///
    /// This panics if the given span does not correspond to valid bounds in
    /// the haystack or the termination of a search.
    ///
    /// # Example
    ///
    /// This example shows how the span of the search can impact whether a
    /// match is reported or not. This is particularly relevant for look-around
    /// operators, which might take things outside of the span into account
    /// when determining whether they match.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{
    ///     nfa::thompson::pikevm::PikeVM,
    ///     Match, Input,
    /// };
    ///
    /// // Look for 'at', but as a distinct word.
    /// let re = PikeVM::new(r"\bat\b")?;
    /// let mut cache = re.create_cache();
    /// let mut caps = re.create_captures();
    ///
    /// // Our haystack contains 'at', but not as a distinct word.
    /// let haystack = "batter";
    ///
    /// // A standard search finds nothing, as expected.
    /// let input = Input::new(haystack);
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(None, caps.get_match());
    ///
    /// // But if we wanted to search starting at position '1', we might
    /// // slice the haystack. If we do this, it's impossible for the \b
    /// // anchors to take the surrounding context into account! And thus,
    /// // a match is produced.
    /// let input = Input::new(&haystack[1..3]);
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(Some(Match::must(0, 0..2)), caps.get_match());
    ///
    /// // But if we specify the span of the search instead of slicing the
    /// // haystack, then the regex engine can "see" outside of the span
    /// // and resolve the anchors correctly.
    /// let input = Input::new(haystack).span(1..3);
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(None, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// This may seem a little ham-fisted, but this scenario tends to come up
    /// if some other regex engine found the match span and now you need to
    /// re-process that span to look for capturing groups. (e.g., Run a faster
    /// DFA first, find a match, then run the PikeVM on just the match span to
    /// resolve capturing groups.) In order to implement that sort of logic
    /// correctly, you need to set the span on the search instead of slicing
    /// the haystack directly.
    ///
    /// The other advantage of using this routine to specify the bounds of the
    /// search is that the match offsets are still reported in terms of the
    /// original haystack. For example, the second search in the example above
    /// reported a match at position `0`, even though `at` starts at offset
    /// `1` because we sliced the haystack.
    #[inline]
    pub fn span<S: Into<Span>>(mut self, span: S) -> Input<'h> {
        self.set_span(span);
        self
    }

    /// Like `Input::span`, but accepts any range instead.
    ///
    /// This routine does not panic if the range given is not a valid range for
    /// this search's haystack. If this search is run with an invalid range,
    /// then the most likely outcome is that the actual search execution will
    /// panic.
    ///
    /// The default range is the entire haystack.
    ///
    /// Note that [`Input::span`] overrides this method and vice versa.
    ///
    /// # Panics
    ///
    /// This routine will panic if the given range could not be converted
    /// to a valid [`Range`]. For example, this would panic when given
    /// `0..=usize::MAX` since it cannot be represented using a half-open
    /// interval in terms of `usize`.
    ///
    /// This also panics if the given range does not correspond to valid bounds
    /// in the haystack or the termination of a search.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::Input;
    ///
    /// let input = Input::new("foobar");
    /// assert_eq!(0..6, input.get_range());
    ///
    /// let input = Input::new("foobar").range(2..=4);
    /// assert_eq!(2..5, input.get_range());
    /// ```
    #[inline]
    pub fn range<R: RangeBounds<usize>>(mut self, range: R) -> Input<'h> {
        self.set_range(range);
        self
    }

    /// Sets the anchor mode of a search.
    ///
    /// When a search is anchored (so that's [`Anchored::Yes`] or
    /// [`Anchored::Pattern`]), a match must begin at the start of a search.
    /// When a search is not anchored (that's [`Anchored::No`]), regex engines
    /// will behave as if the pattern started with a `(?s-u:.)*?`. This prefix
    /// permits a match to appear anywhere.
    ///
    /// By default, the anchored mode is [`Anchored::No`].
    ///
    /// **WARNING:** this is subtly different than using a `^` at the start of
    /// your regex. A `^` forces a regex to match exclusively at the start of
    /// a haystack, regardless of where you begin your search. In contrast,
    /// anchoring a search will allow your regex to match anywhere in your
    /// haystack, but the match must start at the beginning of a search.
    ///
    /// For example, consider the haystack `aba` and the following searches:
    ///
    /// 1. The regex `^a` is compiled with `Anchored::No` and searches `aba`
    ///    starting at position `2`. Since `^` requires the match to start at
    ///    the beginning of the haystack and `2 > 0`, no match is found.
    /// 2. The regex `a` is compiled with `Anchored::Yes` and searches `aba`
    ///    starting at position `2`. This reports a match at `[2, 3]` since
    ///    the match starts where the search started. Since there is no `^`,
    ///    there is no requirement for the match to start at the beginning of
    ///    the haystack.
    /// 3. The regex `a` is compiled with `Anchored::Yes` and searches `aba`
    ///    starting at position `1`. Since `b` corresponds to position `1` and
    ///    since the search is anchored, it finds no match. While the regex
    ///    matches at other positions, configuring the search to be anchored
    ///    requires that it only report a match that begins at the same offset
    ///    as the beginning of the search.
    /// 4. The regex `a` is compiled with `Anchored::No` and searches `aba`
    ///    starting at position `1`. Since the search is not anchored and
    ///    the regex does not start with `^`, the search executes as if there
    ///    is a `(?s:.)*?` prefix that permits it to match anywhere. Thus, it
    ///    reports a match at `[2, 3]`.
    ///
    /// Note that the [`Anchored::Pattern`] mode is like `Anchored::Yes`,
    /// except it only reports matches for a particular pattern.
    ///
    /// # Example
    ///
    /// This demonstrates the differences between an anchored search and
    /// a pattern that begins with `^` (as described in the above warning
    /// message).
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson::pikevm::PikeVM,
    ///     Anchored, Match, Input,
    /// };
    ///
    /// let haystack = "aba";
    ///
    /// let re = PikeVM::new(r"^a")?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    /// let input = Input::new(haystack).span(2..3).anchored(Anchored::No);
    /// re.search(&mut cache, &input, &mut caps);
    /// // No match is found because 2 is not the beginning of the haystack,
    /// // which is what ^ requires.
    /// assert_eq!(None, caps.get_match());
    ///
    /// let re = PikeVM::new(r"a")?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    /// let input = Input::new(haystack).span(2..3).anchored(Anchored::Yes);
    /// re.search(&mut cache, &input, &mut caps);
    /// // An anchored search can still match anywhere in the haystack, it just
    /// // must begin at the start of the search which is '2' in this case.
    /// assert_eq!(Some(Match::must(0, 2..3)), caps.get_match());
    ///
    /// let re = PikeVM::new(r"a")?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    /// let input = Input::new(haystack).span(1..3).anchored(Anchored::Yes);
    /// re.search(&mut cache, &input, &mut caps);
    /// // No match is found since we start searching at offset 1 which
    /// // corresponds to 'b'. Since there is no '(?s:.)*?' prefix, no match
    /// // is found.
    /// assert_eq!(None, caps.get_match());
    ///
    /// let re = PikeVM::new(r"a")?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    /// let input = Input::new(haystack).span(1..3).anchored(Anchored::No);
    /// re.search(&mut cache, &input, &mut caps);
    /// // Since anchored=no, an implicit '(?s:.)*?' prefix was added to the
    /// // pattern. Even though the search starts at 'b', the 'match anything'
    /// // prefix allows the search to match 'a'.
    /// let expected = Some(Match::must(0, 2..3));
    /// assert_eq!(expected, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn anchored(mut self, mode: Anchored) -> Input<'h> {
        self.set_anchored(mode);
        self
    }

    /// Whether to execute an "earliest" search or not.
    ///
    /// When running a non-overlapping search, an "earliest" search will return
    /// the match location as early as possible. For example, given a pattern
    /// of `foo[0-9]+` and a haystack of `foo12345`, a normal leftmost search
    /// will return `foo12345` as a match. But an "earliest" search for regex
    /// engines that support "earliest" semantics will return `foo1` as a
    /// match, since as soon as the first digit following `foo` is seen, it is
    /// known to have found a match.
    ///
    /// Note that "earliest" semantics generally depend on the regex engine.
    /// Different regex engines may determine there is a match at different
    /// points. So there is no guarantee that "earliest" matches will always
    /// return the same offsets for all regex engines. The "earliest" notion
    /// is really about when the particular regex engine determines there is
    /// a match rather than a consistent semantic unto itself. This is often
    /// useful for implementing "did a match occur or not" predicates, but
    /// sometimes the offset is useful as well.
    ///
    /// This is disabled by default.
    ///
    /// # Example
    ///
    /// This example shows the difference between "earliest" searching and
    /// normal searching.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::pikevm::PikeVM, Match, Input};
    ///
    /// let re = PikeVM::new(r"foo[0-9]+")?;
    /// let mut cache = re.create_cache();
    /// let mut caps = re.create_captures();
    ///
    /// // A normal search implements greediness like you expect.
    /// let input = Input::new("foo12345");
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(Some(Match::must(0, 0..8)), caps.get_match());
    ///
    /// // When 'earliest' is enabled and the regex engine supports
    /// // it, the search will bail once it knows a match has been
    /// // found.
    /// let input = Input::new("foo12345").earliest(true);
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(Some(Match::must(0, 0..4)), caps.get_match());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn earliest(mut self, yes: bool) -> Input<'h> {
        self.set_earliest(yes);
        self
    }

    /// Set the span for this search configuration.
    ///
    /// This is like the [`Input::span`] method, except this mutates the
    /// span in place.
    ///
    /// This routine is generic over how a span is provided. While
    /// a [`Span`] may be given directly, one may also provide a
    /// `std::ops::Range<usize>`.
    ///
    /// # Panics
    ///
    /// This panics if the given span does not correspond to valid bounds in
    /// the haystack or the termination of a search.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::Input;
    ///
    /// let mut input = Input::new("foobar");
    /// assert_eq!(0..6, input.get_range());
    /// input.set_span(2..4);
    /// assert_eq!(2..4, input.get_range());
    /// ```
    #[inline]
    pub fn set_span<S: Into<Span>>(&mut self, span: S) {
        let span = span.into();
        assert!(
            span.end <= self.haystack.len()
                && span.start <= span.end.wrapping_add(1),
            "invalid span {:?} for haystack of length {}",
            span,
            self.haystack.len(),
        );
        self.span = span;
    }

    /// Set the span for this search configuration given any range.
    ///
    /// This is like the [`Input::range`] method, except this mutates the
    /// span in place.
    ///
    /// This routine does not panic if the range given is not a valid range for
    /// this search's haystack. If this search is run with an invalid range,
    /// then the most likely outcome is that the actual search execution will
    /// panic.
    ///
    /// # Panics
    ///
    /// This routine will panic if the given range could not be converted
    /// to a valid [`Range`]. For example, this would panic when given
    /// `0..=usize::MAX` since it cannot be represented using a half-open
    /// interval in terms of `usize`.
    ///
    /// This also panics if the given span does not correspond to valid bounds
    /// in the haystack or the termination of a search.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::Input;
    ///
    /// let mut input = Input::new("foobar");
    /// assert_eq!(0..6, input.get_range());
    /// input.set_range(2..=4);
    /// assert_eq!(2..5, input.get_range());
    /// ```
    #[inline]
    pub fn set_range<R: RangeBounds<usize>>(&mut self, range: R) {
        use core::ops::Bound;

        // It's a little weird to convert ranges into spans, and then spans
        // back into ranges when we actually slice the haystack. Because
        // of that process, we always represent everything as a half-open
        // internal. Therefore, handling things like m..=n is a little awkward.
        let start = match range.start_bound() {
            Bound::Included(&i) => i,
            // Can this case ever happen? Range syntax doesn't support it...
            Bound::Excluded(&i) => i.checked_add(1).unwrap(),
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&i) => i.checked_add(1).unwrap(),
            Bound::Excluded(&i) => i,
            Bound::Unbounded => self.haystack().len(),
        };
        self.set_span(Span { start, end });
    }

    /// Set the starting offset for the span for this search configuration.
    ///
    /// This is a convenience routine for only mutating the start of a span
    /// without having to set the entire span.
    ///
    /// # Panics
    ///
    /// This panics if the span resulting from the new start position does not
    /// correspond to valid bounds in the haystack or the termination of a
    /// search.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::Input;
    ///
    /// let mut input = Input::new("foobar");
    /// assert_eq!(0..6, input.get_range());
    /// input.set_start(5);
    /// assert_eq!(5..6, input.get_range());
    /// ```
    #[inline]
    pub fn set_start(&mut self, start: usize) {
        self.set_span(Span { start, ..self.get_span() });
    }

    /// Set the ending offset for the span for this search configuration.
    ///
    /// This is a convenience routine for only mutating the end of a span
    /// without having to set the entire span.
    ///
    /// # Panics
    ///
    /// This panics if the span resulting from the new end position does not
    /// correspond to valid bounds in the haystack or the termination of a
    /// search.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::Input;
    ///
    /// let mut input = Input::new("foobar");
    /// assert_eq!(0..6, input.get_range());
    /// input.set_end(5);
    /// assert_eq!(0..5, input.get_range());
    /// ```
    #[inline]
    pub fn set_end(&mut self, end: usize) {
        self.set_span(Span { end, ..self.get_span() });
    }

    /// Set the anchor mode of a search.
    ///
    /// This is like [`Input::anchored`], except it mutates the search
    /// configuration in place.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{Anchored, Input, PatternID};
    ///
    /// let mut input = Input::new("foobar");
    /// assert_eq!(Anchored::No, input.get_anchored());
    ///
    /// let pid = PatternID::must(5);
    /// input.set_anchored(Anchored::Pattern(pid));
    /// assert_eq!(Anchored::Pattern(pid), input.get_anchored());
    /// ```
    #[inline]
    pub fn set_anchored(&mut self, mode: Anchored) {
        self.anchored = mode;
    }

    /// Set whether the search should execute in "earliest" mode or not.
    ///
    /// This is like [`Input::earliest`], except it mutates the search
    /// configuration in place.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::Input;
    ///
    /// let mut input = Input::new("foobar");
    /// assert!(!input.get_earliest());
    /// input.set_earliest(true);
    /// assert!(input.get_earliest());
    /// ```
    #[inline]
    pub fn set_earliest(&mut self, yes: bool) {
        self.earliest = yes;
    }

    /// Return a borrow of the underlying haystack as a slice of bytes.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::Input;
    ///
    /// let input = Input::new("foobar");
    /// assert_eq!(b"foobar", input.haystack());
    /// ```
    #[inline]
    pub fn haystack(&self) -> &[u8] {
        self.haystack
    }

    /// Return the start position of this search.
    ///
    /// This is a convenience routine for `search.get_span().start()`.
    ///
    /// When [`Input::is_done`] is `false`, this is guaranteed to return
    /// an offset that is less than or equal to [`Input::end`]. Otherwise,
    /// the offset is one greater than [`Input::end`].
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::Input;
    ///
    /// let input = Input::new("foobar");
    /// assert_eq!(0, input.start());
    ///
    /// let input = Input::new("foobar").span(2..4);
    /// assert_eq!(2, input.start());
    /// ```
    #[inline]
    pub fn start(&self) -> usize {
        self.get_span().start
    }

    /// Return the end position of this search.
    ///
    /// This is a convenience routine for `search.get_span().end()`.
    ///
    /// This is guaranteed to return an offset that is a valid exclusive end
    /// bound for this input's haystack.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::Input;
    ///
    /// let input = Input::new("foobar");
    /// assert_eq!(6, input.end());
    ///
    /// let input = Input::new("foobar").span(2..4);
    /// assert_eq!(4, input.end());
    /// ```
    #[inline]
    pub fn end(&self) -> usize {
        self.get_span().end
    }

    /// Return the span for this search configuration.
    ///
    /// If one was not explicitly set, then the span corresponds to the entire
    /// range of the haystack.
    ///
    /// When [`Input::is_done`] is `false`, the span returned is guaranteed
    /// to correspond to valid bounds for this input's haystack.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{Input, Span};
    ///
    /// let input = Input::new("foobar");
    /// assert_eq!(Span { start: 0, end: 6 }, input.get_span());
    /// ```
    #[inline]
    pub fn get_span(&self) -> Span {
        self.span
    }

    /// Return the span as a range for this search configuration.
    ///
    /// If one was not explicitly set, then the span corresponds to the entire
    /// range of the haystack.
    ///
    /// When [`Input::is_done`] is `false`, the range returned is guaranteed
    /// to correspond to valid bounds for this input's haystack.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::Input;
    ///
    /// let input = Input::new("foobar");
    /// assert_eq!(0..6, input.get_range());
    /// ```
    #[inline]
    pub fn get_range(&self) -> Range<usize> {
        self.get_span().range()
    }

    /// Return the anchored mode for this search configuration.
    ///
    /// If no anchored mode was set, then it defaults to [`Anchored::No`].
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{Anchored, Input, PatternID};
    ///
    /// let mut input = Input::new("foobar");
    /// assert_eq!(Anchored::No, input.get_anchored());
    ///
    /// let pid = PatternID::must(5);
    /// input.set_anchored(Anchored::Pattern(pid));
    /// assert_eq!(Anchored::Pattern(pid), input.get_anchored());
    /// ```
    #[inline]
    pub fn get_anchored(&self) -> Anchored {
        self.anchored
    }

    /// Return whether this search should execute in "earliest" mode.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::Input;
    ///
    /// let input = Input::new("foobar");
    /// assert!(!input.get_earliest());
    /// ```
    #[inline]
    pub fn get_earliest(&self) -> bool {
        self.earliest
    }

    /// Return true if and only if this search can never return any other
    /// matches.
    ///
    /// This occurs when the start position of this search is greater than the
    /// end position of the search.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::Input;
    ///
    /// let mut input = Input::new("foobar");
    /// assert!(!input.is_done());
    /// input.set_start(6);
    /// assert!(!input.is_done());
    /// input.set_start(7);
    /// assert!(input.is_done());
    /// ```
    #[inline]
    pub fn is_done(&self) -> bool {
        self.get_span().start > self.get_span().end
    }

    /// Returns true if and only if the given offset in this search's haystack
    /// falls on a valid UTF-8 encoded codepoint boundary.
    ///
    /// If the haystack is not valid UTF-8, then the behavior of this routine
    /// is unspecified.
    ///
    /// # Example
    ///
    /// This shows where codepoint boundaries do and don't exist in valid
    /// UTF-8.
    ///
    /// ```
    /// use regex_automata::Input;
    ///
    /// let input = Input::new("☃");
    /// assert!(input.is_char_boundary(0));
    /// assert!(!input.is_char_boundary(1));
    /// assert!(!input.is_char_boundary(2));
    /// assert!(input.is_char_boundary(3));
    /// assert!(!input.is_char_boundary(4));
    /// ```
    #[inline]
    pub fn is_char_boundary(&self, offset: usize) -> bool {
        utf8::is_boundary(self.haystack(), offset)
    }
}

impl<'h> core::fmt::Debug for Input<'h> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::util::escape::DebugHaystack;

        f.debug_struct("Input")
            .field("haystack", &DebugHaystack(self.haystack()))
            .field("span", &self.span)
            .field("anchored", &self.anchored)
            .field("earliest", &self.earliest)
            .finish()
    }
}

impl<'h, H: ?Sized + AsRef<[u8]>> From<&'h H> for Input<'h> {
    fn from(haystack: &'h H) -> Input<'h> {
        Input::new(haystack)
    }
}

/// A representation of a span reported by a regex engine.
///
/// A span corresponds to the starting and ending _byte offsets_ of a
/// contiguous region of bytes. The starting offset is inclusive while the
/// ending offset is exclusive. That is, a span is a half-open interval.
///
/// A span is used to report the offsets of a match, but it is also used to
/// convey which region of a haystack should be searched via routines like
/// [`Input::span`].
///
/// This is basically equivalent to a `std::ops::Range<usize>`, except this
/// type implements `Copy` which makes it more ergonomic to use in the context
/// of this crate. Like a range, this implements `Index` for `[u8]` and `str`,
/// and `IndexMut` for `[u8]`. For convenience, this also impls `From<Range>`,
/// which means things like `Span::from(5..10)` work.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct Span {
    /// The start offset of the span, inclusive.
    pub start: usize,
    /// The end offset of the span, exclusive.
    pub end: usize,
}

impl Span {
    /// Returns this span as a range.
    #[inline]
    pub fn range(&self) -> Range<usize> {
        Range::from(*self)
    }

    /// Returns true when this span is empty. That is, when `start >= end`.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// Returns the length of this span.
    ///
    /// This returns `0` in precisely the cases that `is_empty` returns `true`.
    #[inline]
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Returns true when the given offset is contained within this span.
    ///
    /// Note that an empty span contains no offsets and will always return
    /// false.
    #[inline]
    pub fn contains(&self, offset: usize) -> bool {
        !self.is_empty() && self.start <= offset && offset <= self.end
    }

    /// Returns a new span with `offset` added to this span's `start` and `end`
    /// values.
    #[inline]
    pub fn offset(&self, offset: usize) -> Span {
        Span { start: self.start + offset, end: self.end + offset }
    }
}

impl core::fmt::Debug for Span {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

impl core::ops::Index<Span> for [u8] {
    type Output = [u8];

    #[inline]
    fn index(&self, index: Span) -> &[u8] {
        &self[index.range()]
    }
}

impl core::ops::IndexMut<Span> for [u8] {
    #[inline]
    fn index_mut(&mut self, index: Span) -> &mut [u8] {
        &mut self[index.range()]
    }
}

impl core::ops::Index<Span> for str {
    type Output = str;

    #[inline]
    fn index(&self, index: Span) -> &str {
        &self[index.range()]
    }
}

impl From<Range<usize>> for Span {
    #[inline]
    fn from(range: Range<usize>) -> Span {
        Span { start: range.start, end: range.end }
    }
}

impl From<Span> for Range<usize> {
    #[inline]
    fn from(span: Span) -> Range<usize> {
        Range { start: span.start, end: span.end }
    }
}

impl PartialEq<Range<usize>> for Span {
    #[inline]
    fn eq(&self, range: &Range<usize>) -> bool {
        self.start == range.start && self.end == range.end
    }
}

impl PartialEq<Span> for Range<usize> {
    #[inline]
    fn eq(&self, span: &Span) -> bool {
        self.start == span.start && self.end == span.end
    }
}

/// A representation of "half" of a match reported by a DFA.
///
/// This is called a "half" match because it only includes the end location (or
/// start location for a reverse search) of a match. This corresponds to the
/// information that a single DFA scan can report. Getting the other half of
/// the match requires a second scan with a reversed DFA.
///
/// A half match also includes the pattern that matched. The pattern is
/// identified by an ID, which corresponds to its position (starting from `0`)
/// relative to other patterns used to construct the corresponding DFA. If only
/// a single pattern is provided to the DFA, then all matches are guaranteed to
/// have a pattern ID of `0`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HalfMatch {
    /// The pattern ID.
    pattern: PatternID,
    /// The offset of the match.
    ///
    /// For forward searches, the offset is exclusive. For reverse searches,
    /// the offset is inclusive.
    offset: usize,
}

impl HalfMatch {
    /// Create a new half match from a pattern ID and a byte offset.
    #[inline]
    pub fn new(pattern: PatternID, offset: usize) -> HalfMatch {
        HalfMatch { pattern, offset }
    }

    /// Create a new half match from a pattern ID and a byte offset.
    ///
    /// This is like [`HalfMatch::new`], but accepts a `usize` instead of a
    /// [`PatternID`]. This panics if the given `usize` is not representable
    /// as a `PatternID`.
    #[inline]
    pub fn must(pattern: usize, offset: usize) -> HalfMatch {
        HalfMatch::new(PatternID::new(pattern).unwrap(), offset)
    }

    /// Returns the ID of the pattern that matched.
    ///
    /// The ID of a pattern is derived from the position in which it was
    /// originally inserted into the corresponding DFA. The first pattern has
    /// identifier `0`, and each subsequent pattern is `1`, `2` and so on.
    #[inline]
    pub fn pattern(&self) -> PatternID {
        self.pattern
    }

    /// The position of the match.
    ///
    /// If this match was produced by a forward search, then the offset is
    /// exclusive. If this match was produced by a reverse search, then the
    /// offset is inclusive.
    #[inline]
    pub fn offset(&self) -> usize {
        self.offset
    }
}

/// A representation of a match reported by a regex engine.
///
/// A match has two essential pieces of information: the [`PatternID`] that
/// matches, and the [`Span`] of the match in a haystack.
///
/// The pattern is identified by an ID, which corresponds to its position
/// (starting from `0`) relative to other patterns used to construct the
/// corresponding regex engine. If only a single pattern is provided, then all
/// matches are guaranteed to have a pattern ID of `0`.
///
/// Every match reported by a regex engine guarantees that its span has its
/// start offset as less than or equal to its end offset.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Match {
    /// The pattern ID.
    pattern: PatternID,
    /// The underlying match span.
    span: Span,
}

impl Match {
    /// Create a new match from a pattern ID and a span.
    ///
    /// This constructor is generic over how a span is provided. While
    /// a [`Span`] may be given directly, one may also provide a
    /// `std::ops::Range<usize>`.
    ///
    /// # Panics
    ///
    /// This panics if `end < start`.
    ///
    /// # Example
    ///
    /// This shows how to create a match for the first pattern in a regex
    /// object using convenient range syntax.
    ///
    /// ```
    /// use regex_automata::{Match, PatternID};
    ///
    /// let m = Match::new(PatternID::ZERO, 5..10);
    /// assert_eq!(0, m.pattern().as_usize());
    /// assert_eq!(5, m.start());
    /// assert_eq!(10, m.end());
    /// ```
    #[inline]
    pub fn new<S: Into<Span>>(pattern: PatternID, span: S) -> Match {
        let span: Span = span.into();
        assert!(span.start <= span.end, "invalid match span");
        Match { pattern, span }
    }

    /// Create a new match from a pattern ID and a byte offset span.
    ///
    /// This constructor is generic over how a span is provided. While
    /// a [`Span`] may be given directly, one may also provide a
    /// `std::ops::Range<usize>`.
    ///
    /// This is like [`Match::new`], but accepts a `usize` instead of a
    /// [`PatternID`]. This panics if the given `usize` is not representable
    /// as a `PatternID`.
    ///
    /// # Panics
    ///
    /// This panics if `end < start` or if `pattern > PatternID::MAX`.
    ///
    /// # Example
    ///
    /// This shows how to create a match for the third pattern in a regex
    /// object using convenient range syntax.
    ///
    /// ```
    /// use regex_automata::Match;
    ///
    /// let m = Match::must(3, 5..10);
    /// assert_eq!(3, m.pattern().as_usize());
    /// assert_eq!(5, m.start());
    /// assert_eq!(10, m.end());
    /// ```
    #[inline]
    pub fn must<S: Into<Span>>(pattern: usize, span: S) -> Match {
        Match::new(PatternID::must(pattern), span)
    }

    /// Returns the ID of the pattern that matched.
    ///
    /// The ID of a pattern is derived from the position in which it was
    /// originally inserted into the corresponding regex engine. The first
    /// pattern has identifier `0`, and each subsequent pattern is `1`, `2` and
    /// so on.
    #[inline]
    pub fn pattern(&self) -> PatternID {
        self.pattern
    }

    /// The starting position of the match.
    ///
    /// This is a convenience routine for `Match::span().start`.
    #[inline]
    pub fn start(&self) -> usize {
        self.span().start
    }

    /// The ending position of the match.
    ///
    /// This is a convenience routine for `Match::span().end`.
    #[inline]
    pub fn end(&self) -> usize {
        self.span().end
    }

    /// Returns the match span as a range.
    ///
    /// This is a convenience routine for `Match::span().range()`.
    #[inline]
    pub fn range(&self) -> core::ops::Range<usize> {
        self.span().range()
    }

    /// Returns the span for this match.
    #[inline]
    pub fn span(&self) -> Span {
        self.span
    }

    /// Returns true when the span in this match is empty.
    ///
    /// An empty match can only be returned when the regex itself can match
    /// the empty string.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.span().is_empty()
    }

    /// Returns the length of this match.
    ///
    /// This returns `0` in precisely the cases that `is_empty` returns `true`.
    #[inline]
    pub fn len(&self) -> usize {
        self.span().len()
    }
}

/// A set of `PatternID`s.
///
/// A set of pattern identifiers is useful for recording which patterns have
/// matched a particular haystack. A pattern set _only_ includes pattern
/// identifiers. It does not include offset information.
///
/// # Example
///
/// This shows basic usage of a set.
///
/// ```
/// use regex_automata::{PatternID, PatternSet};
///
/// let pid1 = PatternID::must(5);
/// let pid2 = PatternID::must(8);
/// // Create a new empty set.
/// let mut set = PatternSet::new(10);
/// // Insert pattern IDs.
/// set.insert(pid1);
/// set.insert(pid2);
/// // Test membership.
/// assert!(set.contains(pid1));
/// assert!(set.contains(pid2));
/// // Get all members.
/// assert_eq!(
///     vec![5, 8],
///     set.iter().map(|p| p.as_usize()).collect::<Vec<usize>>(),
/// );
/// // Clear the set.
/// set.clear();
/// // Test that it is indeed empty.
/// assert!(set.is_empty());
/// ```
#[cfg(feature = "alloc")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternSet {
    /// The number of patterns set to 'true' in this set.
    len: usize,
    /// A map from PatternID to boolean of whether a pattern matches or not.
    ///
    /// This should probably be a bitset, but it's probably unlikely to matter
    /// much in practice.
    ///
    /// The main downside of this representation (and similarly for a bitset)
    /// is that iteration scales with the capacity of the set instead of
    /// the length of the set. This doesn't seem likely to be a problem in
    /// practice.
    ///
    /// Another alternative is to just use a 'SparseSet' for this. It does use
    /// more memory (quite a bit more), but that seems fine I think compared
    /// to the memory being used by the regex engine. The real hiccup with
    /// it is that it yields pattern IDs in the order they were inserted.
    /// Which is actually kind of nice, but at the time of writing, pattern
    /// IDs are yielded in ascending order in the regex crate RegexSet API.
    /// If we did change to 'SparseSet', we could provide an additional
    /// 'iter_match_order' iterator, but keep the ascending order one for
    /// compatibility.
    which: alloc::boxed::Box<[bool]>,
}

#[cfg(feature = "alloc")]
impl PatternSet {
    /// Create a new set of pattern identifiers with the given capacity.
    ///
    /// The given capacity typically corresponds to (at least) the number of
    /// patterns in a compiled regex object.
    ///
    /// # Panics
    ///
    /// This panics if the given capacity exceeds [`PatternID::LIMIT`]. This is
    /// impossible if you use the `pattern_len()` method as defined on any of
    /// the regex engines in this crate. Namely, a regex will fail to build by
    /// returning an error if the number of patterns given to it exceeds the
    /// limit. Therefore, the number of patterns in a valid regex is always
    /// a correct capacity to provide here.
    pub fn new(capacity: usize) -> PatternSet {
        assert!(
            capacity <= PatternID::LIMIT,
            "pattern set capacity exceeds limit of {}",
            PatternID::LIMIT,
        );
        PatternSet {
            len: 0,
            which: alloc::vec![false; capacity].into_boxed_slice(),
        }
    }

    /// Clear this set such that it contains no pattern IDs.
    pub fn clear(&mut self) {
        self.len = 0;
        for matched in self.which.iter_mut() {
            *matched = false;
        }
    }

    /// Return true if and only if the given pattern identifier is in this set.
    pub fn contains(&self, pid: PatternID) -> bool {
        pid.as_usize() < self.capacity() && self.which[pid]
    }

    /// Insert the given pattern identifier into this set and return `true` if
    /// the given pattern ID was not previously in this set.
    ///
    /// If the pattern identifier is already in this set, then this is a no-op.
    ///
    /// Use [`PatternSet::try_insert`] for a fallible version of this routine.
    ///
    /// # Panics
    ///
    /// This panics if this pattern set has insufficient capacity to
    /// store the given pattern ID.
    pub fn insert(&mut self, pid: PatternID) -> bool {
        self.try_insert(pid)
            .expect("PatternSet should have sufficient capacity")
    }

    /// Insert the given pattern identifier into this set and return `true` if
    /// the given pattern ID was not previously in this set.
    ///
    /// If the pattern identifier is already in this set, then this is a no-op.
    ///
    /// # Errors
    ///
    /// This returns an error if this pattern set has insufficient capacity to
    /// store the given pattern ID.
    pub fn try_insert(
        &mut self,
        pid: PatternID,
    ) -> Result<bool, PatternSetInsertError> {
        if pid.as_usize() >= self.capacity() {
            return Err(PatternSetInsertError {
                attempted: pid,
                capacity: self.capacity(),
            });
        }
        if self.which[pid] {
            return Ok(false);
        }
        self.len += 1;
        self.which[pid] = true;
        Ok(true)
    }

    /*
    // This is currently commented out because it is unused and it is unclear
    // whether it's useful or not. What's the harm in having it? When, if
    // we ever wanted to change our representation to a 'SparseSet', then
    // supporting this method would be a bit tricky. So in order to keep some
    // API evolution flexibility, we leave it out for now.

    /// Remove the given pattern identifier from this set.
    ///
    /// If the pattern identifier was not previously in this set, then this
    /// does not change the set and returns `false`.
    ///
    /// # Panics
    ///
    /// This panics if `pid` exceeds the capacity of this set.
    pub fn remove(&mut self, pid: PatternID) -> bool {
        if !self.which[pid] {
            return false;
        }
        self.len -= 1;
        self.which[pid] = false;
        true
    }
    */

    /// Return true if and only if this set has no pattern identifiers in it.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return true if and only if this set has the maximum number of pattern
    /// identifiers in the set. This occurs precisely when `PatternSet::len()
    /// == PatternSet::capacity()`.
    ///
    /// This particular property is useful to test because it may allow one to
    /// stop a search earlier than you might otherwise. Namely, if a search is
    /// only reporting which patterns match a haystack and if you know all of
    /// the patterns match at a given point, then there's no new information
    /// that can be learned by continuing the search. (Because a pattern set
    /// does not keep track of offset information.)
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    /// Returns the total number of pattern identifiers in this set.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns the total number of pattern identifiers that may be stored
    /// in this set.
    ///
    /// This is guaranteed to be less than or equal to [`PatternID::LIMIT`].
    ///
    /// Typically, the capacity of a pattern set matches the number of patterns
    /// in a regex object with which you are searching.
    pub fn capacity(&self) -> usize {
        self.which.len()
    }

    /// Returns an iterator over all pattern identifiers in this set.
    ///
    /// The iterator yields pattern identifiers in ascending order, starting
    /// at zero.
    pub fn iter(&self) -> PatternSetIter<'_> {
        PatternSetIter { it: self.which.iter().enumerate() }
    }
}

/// An error that occurs when a `PatternID` failed to insert into a
/// `PatternSet`.
///
/// An insert fails when the given `PatternID` exceeds the configured capacity
/// of the `PatternSet`.
///
/// This error is created by the [`PatternSet::try_insert`] routine.
#[cfg(feature = "alloc")]
#[derive(Clone, Debug)]
pub struct PatternSetInsertError {
    attempted: PatternID,
    capacity: usize,
}

#[cfg(feature = "std")]
impl std::error::Error for PatternSetInsertError {}

#[cfg(feature = "alloc")]
impl core::fmt::Display for PatternSetInsertError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "failed to insert pattern ID {} into pattern set \
             with insufficiet capacity of {}",
            self.attempted.as_usize(),
            self.capacity,
        )
    }
}

/// An iterator over all pattern identifiers in a [`PatternSet`].
///
/// The lifetime parameter `'a` refers to the lifetime of the pattern set being
/// iterated over.
///
/// This iterator is created by the [`PatternSet::iter`] method.
#[cfg(feature = "alloc")]
#[derive(Clone, Debug)]
pub struct PatternSetIter<'a> {
    it: core::iter::Enumerate<core::slice::Iter<'a, bool>>,
}

#[cfg(feature = "alloc")]
impl<'a> Iterator for PatternSetIter<'a> {
    type Item = PatternID;

    fn next(&mut self) -> Option<PatternID> {
        while let Some((index, &yes)) = self.it.next() {
            if yes {
                // Only valid 'PatternID' values can be inserted into the set
                // and construction of the set panics if the capacity would
                // permit storing invalid pattern IDs. Thus, 'yes' is only true
                // precisely when 'index' corresponds to a valid 'PatternID'.
                return Some(PatternID::new_unchecked(index));
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}

#[cfg(feature = "alloc")]
impl<'a> DoubleEndedIterator for PatternSetIter<'a> {
    fn next_back(&mut self) -> Option<PatternID> {
        while let Some((index, &yes)) = self.it.next_back() {
            if yes {
                // Only valid 'PatternID' values can be inserted into the set
                // and construction of the set panics if the capacity would
                // permit storing invalid pattern IDs. Thus, 'yes' is only true
                // precisely when 'index' corresponds to a valid 'PatternID'.
                return Some(PatternID::new_unchecked(index));
            }
        }
        None
    }
}

/// The type of anchored search to perform.
///
/// This is *almost* a boolean option. That is, you can either do an unanchored
/// search for any pattern in a regex, or you can do an anchored search for any
/// pattern in a regex.
///
/// A third option exists that, assuming the regex engine supports it, permits
/// you to do an anchored search for a specific pattern.
///
/// Note that there is no way to run an unanchored search for a specific
/// pattern. If you need that, you'll need to build separate regexes for each
/// pattern.
///
/// # Errors
///
/// If a regex engine does not support the anchored mode selected, then the
/// regex engine will return an error. While any non-trivial regex engine
/// should support at least one of the available anchored modes, there is no
/// singular mode that is guaranteed to be universally supported. Some regex
/// engines might only support unanchored searches (DFAs compiled without
/// anchored starting states) and some regex engines might only support
/// anchored searches (like the one-pass DFA).
///
/// The specific error returned is a [`MatchError`] with a
/// [`MatchErrorKind::UnsupportedAnchored`] kind. The kind includes the
/// `Anchored` value given that is unsupported.
///
/// Note that regex engines should report "no match" if, for example, an
/// `Anchored::Pattern` is provided with an invalid pattern ID _but_ where
/// anchored searches for a specific pattern are supported. This is smooths out
/// behavior such that it's possible to guarantee that an error never occurs
/// based on how the regex engine is configured. All regex engines in this
/// crate report "no match" when searching for an invalid pattern ID, but where
/// searching for a valid pattern ID is otherwise supported.
///
/// # Example
///
/// This example shows how to use the various `Anchored` modes to run a
/// search. We use the [`PikeVM`](crate::nfa::thompson::pikevm::PikeVM)
/// because it supports all modes unconditionally. Some regex engines, like
/// the [`onepass::DFA`](crate::dfa::onepass::DFA) cannot support unanchored
/// searches.
///
/// ```
/// # if cfg!(miri) { return Ok(()); } // miri takes too long
/// use regex_automata::{
///     nfa::thompson::pikevm::PikeVM,
///     Anchored, Input, Match, PatternID,
/// };
///
/// let re = PikeVM::new_many(&[
///     r"Mrs. \w+",
///     r"Miss \w+",
///     r"Mr. \w+",
///     r"Ms. \w+",
/// ])?;
/// let mut cache = re.create_cache();
/// let hay = "Hello Mr. Springsteen!";
///
/// // The default is to do an unanchored search.
/// assert_eq!(Some(Match::must(2, 6..21)), re.find(&mut cache, hay));
/// // Explicitly ask for an unanchored search. Same as above.
/// let input = Input::new(hay).anchored(Anchored::No);
/// assert_eq!(Some(Match::must(2, 6..21)), re.find(&mut cache, hay));
///
/// // Now try an anchored search. Since the match doesn't start at the
/// // beginning of the haystack, no match is found!
/// let input = Input::new(hay).anchored(Anchored::Yes);
/// assert_eq!(None, re.find(&mut cache, input));
///
/// // We can try an anchored search again, but move the location of where
/// // we start the search. Note that the offsets reported are still in
/// // terms of the overall haystack and not relative to where we started
/// // the search.
/// let input = Input::new(hay).anchored(Anchored::Yes).range(6..);
/// assert_eq!(Some(Match::must(2, 6..21)), re.find(&mut cache, input));
///
/// // Now try an anchored search for a specific pattern. We specifically
/// // choose a pattern that we know doesn't match to prove that the search
/// // only looks for the pattern we provide.
/// let input = Input::new(hay)
///     .anchored(Anchored::Pattern(PatternID::must(1)))
///     .range(6..);
/// assert_eq!(None, re.find(&mut cache, input));
///
/// // But if we switch it to the pattern that we know matches, then we find
/// // the match.
/// let input = Input::new(hay)
///     .anchored(Anchored::Pattern(PatternID::must(2)))
///     .range(6..);
/// assert_eq!(Some(Match::must(2, 6..21)), re.find(&mut cache, input));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Anchored {
    /// Run an unanchored search. This means a match may occur anywhere at or
    /// after the start position of the search.
    ///
    /// This search can return a match for any pattern in the regex.
    No,
    /// Run an anchored search. This means that a match must begin at the
    /// start position of the search.
    ///
    /// This search can return a match for any pattern in the regex.
    Yes,
    /// Run an anchored search for a specific pattern. This means that a match
    /// must be for the given pattern and must begin at the start position of
    /// the search.
    Pattern(PatternID),
}

impl Anchored {
    /// Returns true if and only if this anchor mode corresponds to any kind of
    /// anchored search.
    ///
    /// # Example
    ///
    /// This examples shows that both `Anchored::Yes` and `Anchored::Pattern`
    /// are considered anchored searches.
    ///
    /// ```
    /// use regex_automata::{Anchored, PatternID};
    ///
    /// assert!(!Anchored::No.is_anchored());
    /// assert!(Anchored::Yes.is_anchored());
    /// assert!(Anchored::Pattern(PatternID::ZERO).is_anchored());
    /// ```
    #[inline]
    pub fn is_anchored(&self) -> bool {
        matches!(*self, Anchored::Yes | Anchored::Pattern(_))
    }

    /// Returns the pattern ID associated with this configuration if it is an
    /// anchored search for a specific pattern. Otherwise `None` is returned.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{Anchored, PatternID};
    ///
    /// assert_eq!(None, Anchored::No.pattern());
    /// assert_eq!(None, Anchored::Yes.pattern());
    ///
    /// let pid = PatternID::must(5);
    /// assert_eq!(Some(pid), Anchored::Pattern(pid).pattern());
    /// ```
    #[inline]
    pub fn pattern(&self) -> Option<PatternID> {
        match *self {
            Anchored::Pattern(pid) => Some(pid),
            _ => None,
        }
    }
}

/// The kind of match semantics to use for a regex pattern.
///
/// The default match kind is `LeftmostFirst`, and this corresponds to the
/// match semantics used by most backtracking engines, such as Perl.
///
/// # Leftmost first or "preference order" match semantics
///
/// Leftmost-first semantics determine which match to report when there are
/// multiple paths through a regex that match at the same position. The tie is
/// essentially broken by how a backtracker would behave. For example, consider
/// running the regex `foofoofoo|foofoo|foo` on the haystack `foofoo`. In this
/// case, both the `foofoo` and `foo` branches match at position `0`. So should
/// the end of the match be `3` or `6`?
///
/// A backtracker will conceptually work by trying `foofoofoo` and failing.
/// Then it will try `foofoo`, find the match and stop there. Thus, the
/// leftmost-first match position is `6`. This is called "leftmost-first" or
/// "preference order" because the order of the branches as written in the
/// regex pattern is what determines how to break the tie.
///
/// (Note that leftmost-longest match semantics, which break ties by always
/// taking the longest matching string, are not currently supported by this
/// crate. These match semantics tend to be found in POSIX regex engines.)
///
/// This example shows how leftmost-first semantics work, and how it even
/// applies to multi-pattern regexes:
///
/// ```
/// use regex_automata::{
///     nfa::thompson::pikevm::PikeVM,
///     Match,
/// };
///
/// let re = PikeVM::new_many(&[
///     r"foofoofoo",
///     r"foofoo",
///     r"foo",
/// ])?;
/// let mut cache = re.create_cache();
/// let got: Vec<Match> = re.find_iter(&mut cache, "foofoo").collect();
/// let expected = vec![Match::must(1, 0..6)];
/// assert_eq!(expected, got);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # All matches
///
/// The `All` match semantics report any and all matches, and generally will
/// attempt to match as much as possible. It doesn't respect any sort of match
/// priority at all, so things like non-greedy matching don't work in this
/// mode.
///
/// The fact that non-greedy matching doesn't work generally makes most forms
/// of unanchored non-overlapping searches have unintuitive behavior. Namely,
/// unanchored searches behave as if there is a `(?s-u:.)*?` prefix at the
/// beginning of the pattern, which is specifically non-greedy. Since it will
/// be treated as greedy in `All` match semantics, this generally means that
/// it will first attempt to consume all of the haystack and is likely to wind
/// up skipping matches.
///
/// Generally speaking, `All` should only be used in two circumstances:
///
/// * When running an anchored search and there is a desire to match as much as
/// possible. For example, when building a reverse regex matcher to find the
/// start of a match after finding the end. In this case, the reverse search
/// is anchored to the end of the match found by the forward search.
/// * When running overlapping searches. Since `All` encodes all possible
/// matches, this is generally what you want for an overlapping search. If you
/// try to use leftmost-first in an overlapping search, it is likely to produce
/// counter-intuitive results since leftmost-first specifically excludes some
/// matches from its underlying finite state machine.
///
/// This example demonstrates the counter-intuitive behavior of `All` semantics
/// when using a standard leftmost unanchored search:
///
/// ```
/// use regex_automata::{
///     nfa::thompson::pikevm::PikeVM,
///     Match, MatchKind,
/// };
///
/// let re = PikeVM::builder()
///     .configure(PikeVM::config().match_kind(MatchKind::All))
///     .build("foo")?;
/// let hay = "first foo second foo wat";
/// let mut cache = re.create_cache();
/// let got: Vec<Match> = re.find_iter(&mut cache, hay).collect();
/// // Notice that it completely skips the first 'foo'!
/// let expected = vec![Match::must(0, 17..20)];
/// assert_eq!(expected, got);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// This second example shows how `All` semantics are useful for an overlapping
/// search. Note that we use lower level lazy DFA APIs here since the NFA
/// engines only currently support a very limited form of overlapping search.
///
/// ```
/// use regex_automata::{
///     hybrid::dfa::{DFA, OverlappingState},
///     HalfMatch, Input, MatchKind,
/// };
///
/// let re = DFA::builder()
///     // If we didn't set 'All' semantics here, then the regex would only
///     // match 'foo' at offset 3 and nothing else. Why? Because the state
///     // machine implements preference order and knows that the 'foofoo' and
///     // 'foofoofoo' branches can never match since 'foo' will always match
///     // when they match and take priority.
///     .configure(DFA::config().match_kind(MatchKind::All))
///     .build(r"foo|foofoo|foofoofoo")?;
/// let mut cache = re.create_cache();
/// let mut state = OverlappingState::start();
/// let input = Input::new("foofoofoo");
/// let mut got = vec![];
/// loop {
///     re.try_search_overlapping_fwd(&mut cache, &input, &mut state)?;
///     let m = match state.get_match() {
///         None => break,
///         Some(m) => m,
///     };
///     got.push(m);
/// }
/// let expected = vec![
///     HalfMatch::must(0, 3),
///     HalfMatch::must(0, 6),
///     HalfMatch::must(0, 9),
/// ];
/// assert_eq!(expected, got);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatchKind {
    /// Report all possible matches.
    All,
    /// Report only the leftmost matches. When multiple leftmost matches exist,
    /// report the match corresponding to the part of the regex that appears
    /// first in the syntax.
    LeftmostFirst,
    // There is prior art in RE2 that shows that we should be able to add
    // LeftmostLongest too. The tricky part of it is supporting ungreedy
    // repetitions. Instead of treating all NFA states as having equivalent
    // priority (as in 'All') or treating all NFA states as having distinct
    // priority based on order (as in 'LeftmostFirst'), we instead group NFA
    // states into sets, and treat members of each set as having equivalent
    // priority, but having greater priority than all following members
    // of different sets.
    //
    // However, it's not clear whether it's really worth adding this. After
    // all, leftmost-longest can be emulated when using literals by using
    // leftmost-first and sorting the literals by length in descending order.
    // However, this won't work for arbitrary regexes. e.g., `\w|\w\w` will
    // always match `a` in `ab` when using leftmost-first, but leftmost-longest
    // would match `ab`.
}

impl MatchKind {
    #[cfg(feature = "alloc")]
    pub(crate) fn continue_past_first_match(&self) -> bool {
        *self == MatchKind::All
    }
}

impl Default for MatchKind {
    fn default() -> MatchKind {
        MatchKind::LeftmostFirst
    }
}

/// An error indicating that a search stopped before reporting whether a
/// match exists or not.
///
/// To be very clear, this error type implies that one cannot assume that no
/// matches occur, since the search stopped before completing. That is, if
/// you're looking for information about where a search determined that no
/// match can occur, then this error type does *not* give you that. (Indeed, at
/// the time of writing, if you need such a thing, you have to write your own
/// search routine.)
///
/// Normally, when one searches for something, the response is either an
/// affirmative "it was found at this location" or a negative "not found at
/// all." However, in some cases, a regex engine can be configured to stop its
/// search before concluding whether a match exists or not. When this happens,
/// it may be important for the caller to know why the regex engine gave up and
/// where in the input it gave up at. This error type exposes the 'why' and the
/// 'where.'
///
/// For example, the DFAs provided by this library generally cannot correctly
/// implement Unicode word boundaries. Instead, they provide an option to
/// eagerly support them on ASCII text (since Unicode word boundaries are
/// equivalent to ASCII word boundaries when searching ASCII text), but will
/// "give up" if a non-ASCII byte is seen. In such cases, one is usually
/// required to either report the failure to the caller (unergonomic) or
/// otherwise fall back to some other regex engine (ergonomic, but potentially
/// costly).
///
/// More generally, some regex engines offer the ability for callers to specify
/// certain bytes that will trigger the regex engine to automatically quit if
/// they are seen.
///
/// Still yet, there may be other reasons for a failed match. For example,
/// the hybrid DFA provided by this crate can be configured to give up if it
/// believes that it is not efficient. This in turn permits callers to choose a
/// different regex engine.
///
/// (Note that DFAs are configured by default to never quit or give up in this
/// fashion. For example, by default, a DFA will fail to build if the regex
/// pattern contains a Unicode word boundary. One needs to opt into the "quit"
/// behavior via options, like
/// [`hybrid::dfa::Config::unicode_word_boundary`](crate::hybrid::dfa::Config::unicode_word_boundary).)
///
/// There are a couple other ways a search
/// can fail. For example, when using the
/// [`BoundedBacktracker`](crate::nfa::thompson::backtrack::BoundedBacktracker)
/// with a haystack that is too long, or trying to run an unanchored search
/// with a [one-pass DFA](crate::dfa::onepass).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchError(
    #[cfg(feature = "alloc")] alloc::boxed::Box<MatchErrorKind>,
    #[cfg(not(feature = "alloc"))] MatchErrorKind,
);

impl MatchError {
    /// Create a new error value with the given kind.
    ///
    /// This is a more verbose version of the kind-specific constructors,
    /// e.g., `MatchError::quit`.
    pub fn new(kind: MatchErrorKind) -> MatchError {
        #[cfg(feature = "alloc")]
        {
            MatchError(alloc::boxed::Box::new(kind))
        }
        #[cfg(not(feature = "alloc"))]
        {
            MatchError(kind)
        }
    }

    /// Returns a reference to the underlying error kind.
    pub fn kind(&self) -> &MatchErrorKind {
        &self.0
    }

    /// Create a new "quit" error. The given `byte` corresponds to the value
    /// that tripped a search's quit condition, and `offset` corresponds to the
    /// location in the haystack at which the search quit.
    ///
    /// This is the same as calling `MatchError::new` with a
    /// [`MatchErrorKind::Quit`] kind.
    pub fn quit(byte: u8, offset: usize) -> MatchError {
        MatchError::new(MatchErrorKind::Quit { byte, offset })
    }

    /// Create a new "gave up" error. The given `offset` corresponds to the
    /// location in the haystack at which the search gave up.
    ///
    /// This is the same as calling `MatchError::new` with a
    /// [`MatchErrorKind::GaveUp`] kind.
    pub fn gave_up(offset: usize) -> MatchError {
        MatchError::new(MatchErrorKind::GaveUp { offset })
    }

    /// Create a new "haystack too long" error. The given `len` corresponds to
    /// the length of the haystack that was problematic.
    ///
    /// This is the same as calling `MatchError::new` with a
    /// [`MatchErrorKind::HaystackTooLong`] kind.
    pub fn haystack_too_long(len: usize) -> MatchError {
        MatchError::new(MatchErrorKind::HaystackTooLong { len })
    }

    /// Create a new "unsupported anchored" error. This occurs when the caller
    /// requests a search with an anchor mode that is not supported by the
    /// regex engine.
    ///
    /// This is the same as calling `MatchError::new` with a
    /// [`MatchErrorKind::UnsupportedAnchored`] kind.
    pub fn unsupported_anchored(mode: Anchored) -> MatchError {
        MatchError::new(MatchErrorKind::UnsupportedAnchored { mode })
    }
}

/// The underlying kind of a [`MatchError`].
///
/// This is a **non-exhaustive** enum. That means new variants may be added in
/// a semver-compatible release.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatchErrorKind {
    /// The search saw a "quit" byte at which it was instructed to stop
    /// searching.
    Quit {
        /// The "quit" byte that was observed that caused the search to stop.
        byte: u8,
        /// The offset at which the quit byte was observed.
        offset: usize,
    },
    /// The search, based on heuristics, determined that it would be better
    /// to stop, typically to provide the caller an opportunity to use an
    /// alternative regex engine.
    ///
    /// Currently, the only way for this to occur is via the lazy DFA and
    /// only when it is configured to do so (it will not return this error by
    /// default).
    GaveUp {
        /// The offset at which the search stopped. This corresponds to the
        /// position immediately following the last byte scanned.
        offset: usize,
    },
    /// This error occurs if the haystack given to the regex engine was too
    /// long to be searched. This occurs, for example, with regex engines
    /// like the bounded backtracker that have a configurable fixed amount of
    /// capacity that is tied to the length of the haystack. Anything beyond
    /// that configured limit will result in an error at search time.
    HaystackTooLong {
        /// The length of the haystack that exceeded the limit.
        len: usize,
    },
    /// An error indicating that a particular type of anchored search was
    /// requested, but that the regex engine does not support it.
    ///
    /// Note that this error should not be returned by a regex engine simply
    /// because the pattern ID is invalid (i.e., equal to or exceeds the number
    /// of patterns in the regex). In that case, the regex engine should report
    /// a non-match.
    UnsupportedAnchored {
        /// The anchored mode given that is unsupported.
        mode: Anchored,
    },
}

#[cfg(feature = "std")]
impl std::error::Error for MatchError {}

impl core::fmt::Display for MatchError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match *self.kind() {
            MatchErrorKind::Quit { byte, offset } => write!(
                f,
                "quit search after observing byte {:?} at offset {}",
                DebugByte(byte),
                offset,
            ),
            MatchErrorKind::GaveUp { offset } => {
                write!(f, "gave up searching at offset {}", offset)
            }
            MatchErrorKind::HaystackTooLong { len } => {
                write!(f, "haystack of length {} is too long", len)
            }
            MatchErrorKind::UnsupportedAnchored { mode: Anchored::Yes } => {
                write!(f, "anchored searches are not supported or enabled")
            }
            MatchErrorKind::UnsupportedAnchored { mode: Anchored::No } => {
                write!(f, "unanchored searches are not supported or enabled")
            }
            MatchErrorKind::UnsupportedAnchored {
                mode: Anchored::Pattern(pid),
            } => {
                write!(
                    f,
                    "anchored searches for a specific pattern ({}) are \
                     not supported or enabled",
                    pid.as_usize(),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // We test that our 'MatchError' type is the size we expect. This isn't an
    // API guarantee, but if the size increases, we really want to make sure we
    // decide to do that intentionally. So this should be a speed bump. And in
    // general, we should not increase the size without a very good reason.
    //
    // Why? Because low level search APIs return Result<.., MatchError>. When
    // MatchError gets bigger, so to does the Result type.
    //
    // Now, when 'alloc' is enabled, we do box the error, which de-emphasizes
    // the importance of keeping a small error type. But without 'alloc', we
    // still want things to be small.
    #[test]
    fn match_error_size() {
        let expected_size = if cfg!(feature = "alloc") {
            core::mem::size_of::<usize>()
        } else {
            2 * core::mem::size_of::<usize>()
        };
        assert_eq!(expected_size, core::mem::size_of::<MatchError>());
    }

    // Same as above, but for the underlying match error kind.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn match_error_kind_size() {
        let expected_size = 2 * core::mem::size_of::<usize>();
        assert_eq!(expected_size, core::mem::size_of::<MatchErrorKind>());
    }

    #[cfg(target_pointer_width = "32")]
    #[test]
    fn match_error_kind_size() {
        let expected_size = 3 * core::mem::size_of::<usize>();
        assert_eq!(expected_size, core::mem::size_of::<MatchErrorKind>());
    }

    #[test]
    fn incorrect_asref_guard() {
        struct Bad(std::cell::Cell<bool>);

        impl AsRef<[u8]> for Bad {
            fn as_ref(&self) -> &[u8] {
                if self.0.replace(false) {
                    &[]
                } else {
                    &[0; 1000]
                }
            }
        }

        let bad = Bad(std::cell::Cell::new(true));
        let input = Input::new(&bad);
        assert!(input.end() <= input.haystack().len());
    }
}
