use core::ops::{Range, RangeBounds};

use crate::util::primitives::PatternID;

/// The configuration and the haystack to use for an Aho-Corasick search.
///
/// When executing a search, there are a few parameters one might want to
/// configure:
///
/// * The haystack to search, provided to the [`Input::new`] constructor. This
/// is the only required parameter.
/// * The span _within_ the haystack to limit a search to. (The default
/// is the entire haystack.) This is configured via [`Input::span`] or
/// [`Input::range`].
/// * Whether to run an unanchored (matches can occur anywhere after the
/// start of the search) or anchored (matches can only occur beginning at
/// the start of the search) search. Unanchored search is the default. This is
/// configured via [`Input::anchored`].
/// * Whether to quit the search as soon as a match has been found, regardless
/// of the [`MatchKind`] that the searcher was built with. This is configured
/// via [`Input::earliest`].
///
/// For most cases, the defaults for all optional parameters are appropriate.
/// The utility of this type is that it keeps the default or common case simple
/// while permitting tweaking parameters in more niche use cases while reusing
/// the same search APIs.
///
/// # Valid bounds and search termination
///
/// An `Input` permits setting the bounds of a search via either
/// [`Input::span`] or [`Input::range`]. The bounds set must be valid, or
/// else a panic will occur. Bounds are valid if and only if:
///
/// * The bounds represent a valid range into the input's haystack.
/// * **or** the end bound is a valid ending bound for the haystack *and*
/// the start bound is exactly one greater than the end bound.
///
/// In the latter case, [`Input::is_done`] will return true and indicates any
/// search receiving such an input should immediately return with no match.
///
/// Other than representing "search is complete," the `Input::span` and
/// `Input::range` APIs are never necessary. Instead, callers can slice the
/// haystack instead, e.g., with `&haystack[start..end]`. With that said, they
/// can be more convenient than slicing because the match positions reported
/// when using `Input::span` or `Input::range` are in terms of the original
/// haystack. If you instead use `&haystack[start..end]`, then you'll need to
/// add `start` to any match position returned in order for it to be a correct
/// index into `haystack`.
///
/// # Example: `&str` and `&[u8]` automatically convert to an `Input`
///
/// There is a `From<&T> for Input` implementation for all `T: AsRef<[u8]>`.
/// Additionally, the [`AhoCorasick`](crate::AhoCorasick) search APIs accept
/// a `Into<Input>`. These two things combined together mean you can provide
/// things like `&str` and `&[u8]` to search APIs when the defaults are
/// suitable, but also an `Input` when they're not. For example:
///
/// ```
/// use aho_corasick::{AhoCorasick, Anchored, Input, Match, StartKind};
///
/// // Build a searcher that supports both unanchored and anchored modes.
/// let ac = AhoCorasick::builder()
///     .start_kind(StartKind::Both)
///     .build(&["abcd", "b"])
///     .unwrap();
/// let haystack = "abcd";
///
/// // A search using default parameters is unanchored. With standard
/// // semantics, this finds `b` first.
/// assert_eq!(
///     Some(Match::must(1, 1..2)),
///     ac.find(haystack),
/// );
/// // Using the same 'find' routine, we can provide an 'Input' explicitly
/// // that is configured to do an anchored search. Since 'b' doesn't start
/// // at the beginning of the search, it is not reported as a match.
/// assert_eq!(
///     Some(Match::must(0, 0..4)),
///     ac.find(Input::new(haystack).anchored(Anchored::Yes)),
/// );
/// ```
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
        Input {
            haystack: haystack.as_ref(),
            span: Span { start: 0, end: haystack.as_ref().len() },
            anchored: Anchored::No,
            earliest: false,
        }
    }

    /// Set the span for this search.
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
    /// match is reported or not.
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, Input, MatchKind};
    ///
    /// let patterns = &["b", "abcd", "abc"];
    /// let haystack = "abcd";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    /// let input = Input::new(haystack).span(0..3);
    /// let mat = ac.try_find(input)?.expect("should have a match");
    /// // Without the span stopping the search early, 'abcd' would be reported
    /// // because it is the correct leftmost-first match.
    /// assert_eq!("abc", &haystack[mat.span()]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn span<S: Into<Span>>(mut self, span: S) -> Input<'h> {
        self.set_span(span);
        self
    }

    /// Like `Input::span`, but accepts any range instead.
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
    /// This routine also panics if the given range does not correspond to
    /// valid bounds in the haystack or the termination of a search.
    ///
    /// # Example
    ///
    /// ```
    /// use aho_corasick::Input;
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
    /// When a search is anchored (via [`Anchored::Yes`]), a match must begin
    /// at the start of a search. When a search is not anchored (that's
    /// [`Anchored::No`]), searchers will look for a match anywhere in the
    /// haystack.
    ///
    /// By default, the anchored mode is [`Anchored::No`].
    ///
    /// # Support for anchored searches
    ///
    /// Anchored or unanchored searches might not always be available,
    /// depending on the type of searcher used and its configuration:
    ///
    /// * [`noncontiguous::NFA`](crate::nfa::noncontiguous::NFA) always
    /// supports both unanchored and anchored searches.
    /// * [`contiguous::NFA`](crate::nfa::contiguous::NFA) always supports both
    /// unanchored and anchored searches.
    /// * [`dfa::DFA`](crate::dfa::DFA) supports only unanchored
    /// searches by default.
    /// [`dfa::Builder::start_kind`](crate::dfa::Builder::start_kind) can
    /// be used to change the default to supporting both kinds of searches
    /// or even just anchored searches.
    /// * [`AhoCorasick`](crate::AhoCorasick) inherits the same setup as a
    /// `DFA`. Namely, it only supports unanchored searches by default, but
    /// [`AhoCorasickBuilder::start_kind`](crate::AhoCorasickBuilder::start_kind)
    /// can change this.
    ///
    /// If you try to execute a search using a `try_` ("fallible") method with
    /// an unsupported anchor mode, then an error will be returned. For calls
    /// to infallible search methods, a panic will result.
    ///
    /// # Example
    ///
    /// This demonstrates the differences between an anchored search and
    /// an unanchored search. Notice that we build our `AhoCorasick` searcher
    /// with [`StartKind::Both`] so that it supports both unanchored and
    /// anchored searches simultaneously.
    ///
    /// ```
    /// use aho_corasick::{
    ///     AhoCorasick, Anchored, Input, MatchKind, StartKind,
    /// };
    ///
    /// let patterns = &["bcd"];
    /// let haystack = "abcd";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .start_kind(StartKind::Both)
    ///     .build(patterns)
    ///     .unwrap();
    ///
    /// // Note that 'Anchored::No' is the default, so it doesn't need to
    /// // be explicitly specified here.
    /// let input = Input::new(haystack);
    /// let mat = ac.try_find(input)?.expect("should have a match");
    /// assert_eq!("bcd", &haystack[mat.span()]);
    ///
    /// // While 'bcd' occurs in the haystack, it does not begin where our
    /// // search begins, so no match is found.
    /// let input = Input::new(haystack).anchored(Anchored::Yes);
    /// assert_eq!(None, ac.try_find(input)?);
    ///
    /// // However, if we start our search where 'bcd' starts, then we will
    /// // find a match.
    /// let input = Input::new(haystack).range(1..).anchored(Anchored::Yes);
    /// let mat = ac.try_find(input)?.expect("should have a match");
    /// assert_eq!("bcd", &haystack[mat.span()]);
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
    /// When running a non-overlapping search, an "earliest" search will
    /// return the match location as early as possible. For example, given
    /// the patterns `abc` and `b`, and a haystack of `abc`, a normal
    /// leftmost-first search will return `abc` as a match. But an "earliest"
    /// search will return as soon as it is known that a match occurs, which
    /// happens once `b` is seen.
    ///
    /// Note that when using [`MatchKind::Standard`], the "earliest" option
    /// has no effect since standard semantics are already "earliest." Note
    /// also that this has no effect in overlapping searches, since overlapping
    /// searches also use standard semantics and report all possible matches.
    ///
    /// This is disabled by default.
    ///
    /// # Example
    ///
    /// This example shows the difference between "earliest" searching and
    /// normal leftmost searching.
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, Anchored, Input, MatchKind, StartKind};
    ///
    /// let patterns = &["abc", "b"];
    /// let haystack = "abc";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    ///
    /// // The normal leftmost-first match.
    /// let input = Input::new(haystack);
    /// let mat = ac.try_find(input)?.expect("should have a match");
    /// assert_eq!("abc", &haystack[mat.span()]);
    ///
    /// // The "earliest" possible match, even if it isn't leftmost-first.
    /// let input = Input::new(haystack).earliest(true);
    /// let mat = ac.try_find(input)?.expect("should have a match");
    /// assert_eq!("b", &haystack[mat.span()]);
    ///
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
    /// use aho_corasick::Input;
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
    /// # Panics
    ///
    /// This routine will panic if the given range could not be converted
    /// to a valid [`Range`]. For example, this would panic when given
    /// `0..=usize::MAX` since it cannot be represented using a half-open
    /// interval in terms of `usize`.
    ///
    /// This routine also panics if the given range does not correspond to
    /// valid bounds in the haystack or the termination of a search.
    ///
    /// # Example
    ///
    /// ```
    /// use aho_corasick::Input;
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
    /// This panics if the given span does not correspond to valid bounds in
    /// the haystack or the termination of a search.
    ///
    /// # Example
    ///
    /// ```
    /// use aho_corasick::Input;
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
    /// This panics if the given span does not correspond to valid bounds in
    /// the haystack or the termination of a search.
    ///
    /// # Example
    ///
    /// ```
    /// use aho_corasick::Input;
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
    /// use aho_corasick::{Anchored, Input};
    ///
    /// let mut input = Input::new("foobar");
    /// assert_eq!(Anchored::No, input.get_anchored());
    ///
    /// input.set_anchored(Anchored::Yes);
    /// assert_eq!(Anchored::Yes, input.get_anchored());
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
    /// use aho_corasick::Input;
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
    /// use aho_corasick::Input;
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
    /// # Example
    ///
    /// ```
    /// use aho_corasick::Input;
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
    /// # Example
    ///
    /// ```
    /// use aho_corasick::Input;
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
    /// # Example
    ///
    /// ```
    /// use aho_corasick::{Input, Span};
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
    /// # Example
    ///
    /// ```
    /// use aho_corasick::Input;
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
    /// use aho_corasick::{Anchored, Input};
    ///
    /// let mut input = Input::new("foobar");
    /// assert_eq!(Anchored::No, input.get_anchored());
    ///
    /// input.set_anchored(Anchored::Yes);
    /// assert_eq!(Anchored::Yes, input.get_anchored());
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
    /// use aho_corasick::Input;
    ///
    /// let input = Input::new("foobar");
    /// assert!(!input.get_earliest());
    /// ```
    #[inline]
    pub fn get_earliest(&self) -> bool {
        self.earliest
    }

    /// Return true if this input has been exhausted, which in turn means all
    /// subsequent searches will return no matches.
    ///
    /// This occurs precisely when the start position of this search is greater
    /// than the end position of the search.
    ///
    /// # Example
    ///
    /// ```
    /// use aho_corasick::Input;
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
}

impl<'h> core::fmt::Debug for Input<'h> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut fmter = f.debug_struct("Input");
        match core::str::from_utf8(self.haystack()) {
            Ok(nice) => fmter.field("haystack", &nice),
            Err(_) => fmter.field("haystack", &self.haystack()),
        }
        .field("span", &self.span)
        .field("anchored", &self.anchored)
        .field("earliest", &self.earliest)
        .finish()
    }
}

impl<'h, H: ?Sized + AsRef<[u8]>> From<&'h H> for Input<'h> {
    #[inline]
    fn from(haystack: &'h H) -> Input<'h> {
        Input::new(haystack)
    }
}

/// A representation of a range in a haystack.
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
/// of this crate. Indeed, `Span` exists only because `Range<usize>` does
/// not implement `Copy`. Like a range, this implements `Index` for `[u8]`
/// and `str`, and `IndexMut` for `[u8]`. For convenience, this also impls
/// `From<Range>`, which means things like `Span::from(5..10)` work.
///
/// There are no constraints on the values of a span. It is, for example, legal
/// to create a span where `start > end`.
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

/// The type of anchored search to perform.
///
/// If an Aho-Corasick searcher does not support the anchored mode selected,
/// then the search will return an error or panic, depending on whether a
/// fallible or an infallible routine was called.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Anchored {
    /// Run an unanchored search. This means a match may occur anywhere at or
    /// after the start position of the search up until the end position of the
    /// search.
    No,
    /// Run an anchored search. This means that a match must begin at the start
    /// position of the search and end before the end position of the search.
    Yes,
}

impl Anchored {
    /// Returns true if and only if this anchor mode corresponds to an anchored
    /// search.
    ///
    /// # Example
    ///
    /// ```
    /// use aho_corasick::Anchored;
    ///
    /// assert!(!Anchored::No.is_anchored());
    /// assert!(Anchored::Yes.is_anchored());
    /// ```
    #[inline]
    pub fn is_anchored(&self) -> bool {
        matches!(*self, Anchored::Yes)
    }
}

/// A representation of a match reported by an Aho-Corasick searcher.
///
/// A match has two essential pieces of information: the [`PatternID`] that
/// matches, and the [`Span`] of the match in a haystack.
///
/// The pattern is identified by an ID, which corresponds to its position
/// (starting from `0`) relative to other patterns used to construct the
/// corresponding searcher. If only a single pattern is provided, then all
/// matches are guaranteed to have a pattern ID of `0`.
///
/// Every match reported by a searcher guarantees that its span has its start
/// offset as less than or equal to its end offset.
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
    /// This shows how to create a match for the first pattern in an
    /// Aho-Corasick searcher using convenient range syntax.
    ///
    /// ```
    /// use aho_corasick::{Match, PatternID};
    ///
    /// let m = Match::new(PatternID::ZERO, 5..10);
    /// assert_eq!(0, m.pattern().as_usize());
    /// assert_eq!(5, m.start());
    /// assert_eq!(10, m.end());
    /// ```
    #[inline]
    pub fn new<S: Into<Span>>(pattern: PatternID, span: S) -> Match {
        let span = span.into();
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
    /// This shows how to create a match for the third pattern in an
    /// Aho-Corasick searcher using convenient range syntax.
    ///
    /// ```
    /// use aho_corasick::Match;
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
    /// originally inserted into the corresponding searcher. The first pattern
    /// has identifier `0`, and each subsequent pattern is `1`, `2` and so on.
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
    /// An empty match can only be returned when empty pattern is in the
    /// Aho-Corasick searcher.
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

    /// Returns a new match with `offset` added to its span's `start` and `end`
    /// values.
    #[inline]
    pub fn offset(&self, offset: usize) -> Match {
        Match {
            pattern: self.pattern,
            span: Span {
                start: self.start() + offset,
                end: self.end() + offset,
            },
        }
    }
}

/// A knob for controlling the match semantics of an Aho-Corasick automaton.
///
/// There are two generally different ways that Aho-Corasick automatons can
/// report matches. The first way is the "standard" approach that results from
/// implementing most textbook explanations of Aho-Corasick. The second way is
/// to report only the leftmost non-overlapping matches. The leftmost approach
/// is in turn split into two different ways of resolving ambiguous matches:
/// leftmost-first and leftmost-longest.
///
/// The `Standard` match kind is the default and is the only one that supports
/// overlapping matches and stream searching. (Trying to find overlapping or
/// streaming matches using leftmost match semantics will result in an error in
/// fallible APIs and a panic when using infallibe APIs.) The `Standard` match
/// kind will report matches as they are seen. When searching for overlapping
/// matches, then all possible matches are reported. When searching for
/// non-overlapping matches, the first match seen is reported. For example, for
/// non-overlapping matches, given the patterns `abcd` and `b` and the haystack
/// `abcdef`, only a match for `b` is reported since it is detected first. The
/// `abcd` match is never reported since it overlaps with the `b` match.
///
/// In contrast, the leftmost match kind always prefers the leftmost match
/// among all possible matches. Given the same example as above with `abcd` and
/// `b` as patterns and `abcdef` as the haystack, the leftmost match is `abcd`
/// since it begins before the `b` match, even though the `b` match is detected
/// before the `abcd` match. In this case, the `b` match is not reported at all
/// since it overlaps with the `abcd` match.
///
/// The difference between leftmost-first and leftmost-longest is in how they
/// resolve ambiguous matches when there are multiple leftmost matches to
/// choose from. Leftmost-first always chooses the pattern that was provided
/// earliest, where as leftmost-longest always chooses the longest matching
/// pattern. For example, given the patterns `a` and `ab` and the subject
/// string `ab`, the leftmost-first match is `a` but the leftmost-longest match
/// is `ab`. Conversely, if the patterns were given in reverse order, i.e.,
/// `ab` and `a`, then both the leftmost-first and leftmost-longest matches
/// would be `ab`. Stated differently, the leftmost-first match depends on the
/// order in which the patterns were given to the Aho-Corasick automaton.
/// Because of that, when leftmost-first matching is used, if a pattern `A`
/// that appears before a pattern `B` is a prefix of `B`, then it is impossible
/// to ever observe a match of `B`.
///
/// If you're not sure which match kind to pick, then stick with the standard
/// kind, which is the default. In particular, if you need overlapping or
/// streaming matches, then you _must_ use the standard kind. The leftmost
/// kinds are useful in specific circumstances. For example, leftmost-first can
/// be very useful as a way to implement match priority based on the order of
/// patterns given and leftmost-longest can be useful for dictionary searching
/// such that only the longest matching words are reported.
///
/// # Relationship with regular expression alternations
///
/// Understanding match semantics can be a little tricky, and one easy way
/// to conceptualize non-overlapping matches from an Aho-Corasick automaton
/// is to think about them as a simple alternation of literals in a regular
/// expression. For example, let's say we wanted to match the strings
/// `Sam` and `Samwise`, which would turn into the regex `Sam|Samwise`. It
/// turns out that regular expression engines have two different ways of
/// matching this alternation. The first way, leftmost-longest, is commonly
/// found in POSIX compatible implementations of regular expressions (such as
/// `grep`). The second way, leftmost-first, is commonly found in backtracking
/// implementations such as Perl. (Some regex engines, such as RE2 and Rust's
/// regex engine do not use backtracking, but still implement leftmost-first
/// semantics in an effort to match the behavior of dominant backtracking
/// regex engines such as those found in Perl, Ruby, Python, Javascript and
/// PHP.)
///
/// That is, when matching `Sam|Samwise` against `Samwise`, a POSIX regex
/// will match `Samwise` because it is the longest possible match, but a
/// Perl-like regex will match `Sam` since it appears earlier in the
/// alternation. Indeed, the regex `Sam|Samwise` in a Perl-like regex engine
/// will never match `Samwise` since `Sam` will always have higher priority.
/// Conversely, matching the regex `Samwise|Sam` against `Samwise` will lead to
/// a match of `Samwise` in both POSIX and Perl-like regexes since `Samwise` is
/// still longest match, but it also appears earlier than `Sam`.
///
/// The "standard" match semantics of Aho-Corasick generally don't correspond
/// to the match semantics of any large group of regex implementations, so
/// there's no direct analogy that can be made here. Standard match semantics
/// are generally useful for overlapping matches, or if you just want to see
/// matches as they are detected.
///
/// The main conclusion to draw from this section is that the match semantics
/// can be tweaked to precisely match either Perl-like regex alternations or
/// POSIX regex alternations.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatchKind {
    /// Use standard match semantics, which support overlapping matches. When
    /// used with non-overlapping matches, matches are reported as they are
    /// seen.
    Standard,
    /// Use leftmost-first match semantics, which reports leftmost matches.
    /// When there are multiple possible leftmost matches, the match
    /// corresponding to the pattern that appeared earlier when constructing
    /// the automaton is reported.
    ///
    /// This does **not** support overlapping matches or stream searching. If
    /// this match kind is used, attempting to find overlapping matches or
    /// stream matches will fail.
    LeftmostFirst,
    /// Use leftmost-longest match semantics, which reports leftmost matches.
    /// When there are multiple possible leftmost matches, the longest match
    /// is chosen.
    ///
    /// This does **not** support overlapping matches or stream searching. If
    /// this match kind is used, attempting to find overlapping matches or
    /// stream matches will fail.
    LeftmostLongest,
}

/// The default match kind is `MatchKind::Standard`.
impl Default for MatchKind {
    fn default() -> MatchKind {
        MatchKind::Standard
    }
}

impl MatchKind {
    #[inline]
    pub(crate) fn is_standard(&self) -> bool {
        matches!(*self, MatchKind::Standard)
    }

    #[inline]
    pub(crate) fn is_leftmost(&self) -> bool {
        matches!(*self, MatchKind::LeftmostFirst | MatchKind::LeftmostLongest)
    }

    #[inline]
    pub(crate) fn is_leftmost_first(&self) -> bool {
        matches!(*self, MatchKind::LeftmostFirst)
    }

    /// Convert this match kind into a packed match kind. If this match kind
    /// corresponds to standard semantics, then this returns None, since
    /// packed searching does not support standard semantics.
    #[inline]
    pub(crate) fn as_packed(&self) -> Option<crate::packed::MatchKind> {
        match *self {
            MatchKind::Standard => None,
            MatchKind::LeftmostFirst => {
                Some(crate::packed::MatchKind::LeftmostFirst)
            }
            MatchKind::LeftmostLongest => {
                Some(crate::packed::MatchKind::LeftmostLongest)
            }
        }
    }
}

/// The kind of anchored starting configurations to support in an Aho-Corasick
/// searcher.
///
/// Depending on which searcher is used internally by
/// [`AhoCorasick`](crate::AhoCorasick), supporting both unanchored
/// and anchored searches can be quite costly. For this reason,
/// [`AhoCorasickBuilder::start_kind`](crate::AhoCorasickBuilder::start_kind)
/// can be used to configure whether your searcher supports unanchored,
/// anchored or both kinds of searches.
///
/// This searcher configuration knob works in concert with the search time
/// configuration [`Input::anchored`]. Namely, if one requests an unsupported
/// anchored mode, then the search will either panic or return an error,
/// depending on whether you're using infallible or fallibe APIs, respectively.
///
/// `AhoCorasick` by default only supports unanchored searches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartKind {
    /// Support both anchored and unanchored searches.
    Both,
    /// Support only unanchored searches. Requesting an anchored search will
    /// return an error in fallible APIs and panic in infallible APIs.
    Unanchored,
    /// Support only anchored searches. Requesting an unanchored search will
    /// return an error in fallible APIs and panic in infallible APIs.
    Anchored,
}

impl Default for StartKind {
    fn default() -> StartKind {
        StartKind::Unanchored
    }
}
