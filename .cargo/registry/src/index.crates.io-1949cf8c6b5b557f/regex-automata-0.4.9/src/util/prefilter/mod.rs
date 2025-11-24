/*!
Defines a prefilter for accelerating regex searches.

A prefilter can be created by building a [`Prefilter`] value.

A prefilter represents one of the most important optimizations available for
accelerating regex searches. The idea of a prefilter is to very quickly find
candidate locations in a haystack where a regex _could_ match. Once a candidate
is found, it is then intended for the regex engine to run at that position to
determine whether the candidate is a match or a false positive.

In the aforementioned description of the prefilter optimization also lay its
demise. Namely, if a prefilter has a high false positive rate and it produces
lots of candidates, then a prefilter can overall make a regex search slower.
It can run more slowly because more time is spent ping-ponging between the
prefilter search and the regex engine attempting to confirm each candidate as
a match. This ping-ponging has overhead that adds up, and is exacerbated by
a high false positive rate.

Nevertheless, the optimization is still generally worth performing in most
cases. Particularly given just how much throughput can be improved. (It is not
uncommon for prefilter optimizations to improve throughput by one or two orders
of magnitude.)

Typically a prefilter is used to find occurrences of literal prefixes from a
regex pattern, but this isn't required. A prefilter can be used to look for
suffixes or even inner literals.

Note that as of now, prefilters throw away information about which pattern
each literal comes from. In other words, when a prefilter finds a match,
there's no way to know which pattern (or patterns) it came from. Therefore,
in order to confirm a match, you'll have to check all of the patterns by
running the full regex engine.
*/

mod aho_corasick;
mod byteset;
mod memchr;
mod memmem;
mod teddy;

use core::{
    borrow::Borrow,
    fmt::Debug,
    panic::{RefUnwindSafe, UnwindSafe},
};

#[cfg(feature = "alloc")]
use alloc::sync::Arc;

#[cfg(feature = "syntax")]
use regex_syntax::hir::{literal, Hir};

use crate::util::search::{MatchKind, Span};

pub(crate) use crate::util::prefilter::{
    aho_corasick::AhoCorasick,
    byteset::ByteSet,
    memchr::{Memchr, Memchr2, Memchr3},
    memmem::Memmem,
    teddy::Teddy,
};

/// A prefilter for accelerating regex searches.
///
/// If you already have your literals that you want to search with,
/// then the vanilla [`Prefilter::new`] constructor is for you. But
/// if you have an [`Hir`] value from the `regex-syntax` crate, then
/// [`Prefilter::from_hir_prefix`] might be more convenient. Namely, it uses
/// the [`regex-syntax::hir::literal`](regex_syntax::hir::literal) module to
/// extract literal prefixes for you, optimize them and then select and build a
/// prefilter matcher.
///
/// A prefilter must have **zero false negatives**. However, by its very
/// nature, it may produce false positives. That is, a prefilter will never
/// skip over a position in the haystack that corresponds to a match of the
/// original regex pattern, but it *may* produce a match for a position
/// in the haystack that does *not* correspond to a match of the original
/// regex pattern. If you use either the [`Prefilter::from_hir_prefix`] or
/// [`Prefilter::from_hirs_prefix`] constructors, then this guarantee is
/// upheld for you automatically. This guarantee is not preserved if you use
/// [`Prefilter::new`] though, since it is up to the caller to provide correct
/// literal strings with respect to the original regex pattern.
///
/// # Cloning
///
/// It is an API guarantee that cloning a prefilter is cheap. That is, cloning
/// it will not duplicate whatever heap memory is used to represent the
/// underlying matcher.
///
/// # Example
///
/// This example shows how to attach a `Prefilter` to the
/// [`PikeVM`](crate::nfa::thompson::pikevm::PikeVM) in order to accelerate
/// searches.
///
/// ```
/// use regex_automata::{
///     nfa::thompson::pikevm::PikeVM,
///     util::prefilter::Prefilter,
///     Match, MatchKind,
/// };
///
/// let pre = Prefilter::new(MatchKind::LeftmostFirst, &["Bruce "])
///     .expect("a prefilter");
/// let re = PikeVM::builder()
///     .configure(PikeVM::config().prefilter(Some(pre)))
///     .build(r"Bruce \w+")?;
/// let mut cache = re.create_cache();
/// assert_eq!(
///     Some(Match::must(0, 6..23)),
///     re.find(&mut cache, "Hello Bruce Springsteen!"),
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// But note that if you get your prefilter incorrect, it could lead to an
/// incorrect result!
///
/// ```
/// use regex_automata::{
///     nfa::thompson::pikevm::PikeVM,
///     util::prefilter::Prefilter,
///     Match, MatchKind,
/// };
///
/// // This prefilter is wrong!
/// let pre = Prefilter::new(MatchKind::LeftmostFirst, &["Patti "])
///     .expect("a prefilter");
/// let re = PikeVM::builder()
///     .configure(PikeVM::config().prefilter(Some(pre)))
///     .build(r"Bruce \w+")?;
/// let mut cache = re.create_cache();
/// // We find no match even though the regex does match.
/// assert_eq!(
///     None,
///     re.find(&mut cache, "Hello Bruce Springsteen!"),
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct Prefilter {
    #[cfg(not(feature = "alloc"))]
    _unused: (),
    #[cfg(feature = "alloc")]
    pre: Arc<dyn PrefilterI>,
    #[cfg(feature = "alloc")]
    is_fast: bool,
    #[cfg(feature = "alloc")]
    max_needle_len: usize,
}

impl Prefilter {
    /// Create a new prefilter from a sequence of needles and a corresponding
    /// match semantics.
    ///
    /// This may return `None` for a variety of reasons, for example, if
    /// a suitable prefilter could not be constructed. That might occur
    /// if they are unavailable (e.g., the `perf-literal-substring` and
    /// `perf-literal-multisubstring` features aren't enabled), or it might
    /// occur because of heuristics or other artifacts of how the prefilter
    /// works.
    ///
    /// Note that if you have an [`Hir`] expression, it may be more convenient
    /// to use [`Prefilter::from_hir_prefix`]. It will automatically handle the
    /// task of extracting prefix literals for you.
    ///
    /// # Example
    ///
    /// This example shows how match semantics can impact the matching
    /// algorithm used by the prefilter. For this reason, it is important to
    /// ensure that the match semantics given here are consistent with the
    /// match semantics intended for the regular expression that the literals
    /// were extracted from.
    ///
    /// ```
    /// use regex_automata::{
    ///     util::{prefilter::Prefilter, syntax},
    ///     MatchKind, Span,
    /// };
    ///
    /// let hay = "Hello samwise";
    ///
    /// // With leftmost-first, we find 'samwise' here because it comes
    /// // before 'sam' in the sequence we give it..
    /// let pre = Prefilter::new(MatchKind::LeftmostFirst, &["samwise", "sam"])
    ///     .expect("a prefilter");
    /// assert_eq!(
    ///     Some(Span::from(6..13)),
    ///     pre.find(hay.as_bytes(), Span::from(0..hay.len())),
    /// );
    /// // Still with leftmost-first but with the literals reverse, now 'sam'
    /// // will match instead!
    /// let pre = Prefilter::new(MatchKind::LeftmostFirst, &["sam", "samwise"])
    ///     .expect("a prefilter");
    /// assert_eq!(
    ///     Some(Span::from(6..9)),
    ///     pre.find(hay.as_bytes(), Span::from(0..hay.len())),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new<B: AsRef<[u8]>>(
        kind: MatchKind,
        needles: &[B],
    ) -> Option<Prefilter> {
        Choice::new(kind, needles).and_then(|choice| {
            let max_needle_len =
                needles.iter().map(|b| b.as_ref().len()).max().unwrap_or(0);
            Prefilter::from_choice(choice, max_needle_len)
        })
    }

    /// This turns a prefilter selection into a `Prefilter`. That is, in turns
    /// the enum given into a trait object.
    fn from_choice(
        choice: Choice,
        max_needle_len: usize,
    ) -> Option<Prefilter> {
        #[cfg(not(feature = "alloc"))]
        {
            None
        }
        #[cfg(feature = "alloc")]
        {
            let pre: Arc<dyn PrefilterI> = match choice {
                Choice::Memchr(p) => Arc::new(p),
                Choice::Memchr2(p) => Arc::new(p),
                Choice::Memchr3(p) => Arc::new(p),
                Choice::Memmem(p) => Arc::new(p),
                Choice::Teddy(p) => Arc::new(p),
                Choice::ByteSet(p) => Arc::new(p),
                Choice::AhoCorasick(p) => Arc::new(p),
            };
            let is_fast = pre.is_fast();
            Some(Prefilter { pre, is_fast, max_needle_len })
        }
    }

    /// This attempts to extract prefixes from the given `Hir` expression for
    /// the given match semantics, and if possible, builds a prefilter for
    /// them.
    ///
    /// # Example
    ///
    /// This example shows how to build a prefilter directly from an [`Hir`]
    /// expression, and use to find an occurrence of a prefix from the regex
    /// pattern.
    ///
    /// ```
    /// use regex_automata::{
    ///     util::{prefilter::Prefilter, syntax},
    ///     MatchKind, Span,
    /// };
    ///
    /// let hir = syntax::parse(r"(Bruce|Patti) \w+")?;
    /// let pre = Prefilter::from_hir_prefix(MatchKind::LeftmostFirst, &hir)
    ///     .expect("a prefilter");
    /// let hay = "Hello Patti Scialfa!";
    /// assert_eq!(
    ///     Some(Span::from(6..12)),
    ///     pre.find(hay.as_bytes(), Span::from(0..hay.len())),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "syntax")]
    pub fn from_hir_prefix(kind: MatchKind, hir: &Hir) -> Option<Prefilter> {
        Prefilter::from_hirs_prefix(kind, &[hir])
    }

    /// This attempts to extract prefixes from the given `Hir` expressions for
    /// the given match semantics, and if possible, builds a prefilter for
    /// them.
    ///
    /// Note that as of now, prefilters throw away information about which
    /// pattern each literal comes from. In other words, when a prefilter finds
    /// a match, there's no way to know which pattern (or patterns) it came
    /// from. Therefore, in order to confirm a match, you'll have to check all
    /// of the patterns by running the full regex engine.
    ///
    /// # Example
    ///
    /// This example shows how to build a prefilter directly from multiple
    /// `Hir` expressions expression, and use it to find an occurrence of a
    /// prefix from the regex patterns.
    ///
    /// ```
    /// use regex_automata::{
    ///     util::{prefilter::Prefilter, syntax},
    ///     MatchKind, Span,
    /// };
    ///
    /// let hirs = syntax::parse_many(&[
    ///     r"(Bruce|Patti) \w+",
    ///     r"Mrs?\. Doubtfire",
    /// ])?;
    /// let pre = Prefilter::from_hirs_prefix(MatchKind::LeftmostFirst, &hirs)
    ///     .expect("a prefilter");
    /// let hay = "Hello Mrs. Doubtfire";
    /// assert_eq!(
    ///     Some(Span::from(6..20)),
    ///     pre.find(hay.as_bytes(), Span::from(0..hay.len())),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "syntax")]
    pub fn from_hirs_prefix<H: Borrow<Hir>>(
        kind: MatchKind,
        hirs: &[H],
    ) -> Option<Prefilter> {
        prefixes(kind, hirs)
            .literals()
            .and_then(|lits| Prefilter::new(kind, lits))
    }

    /// Run this prefilter on `haystack[span.start..end]` and return a matching
    /// span if one exists.
    ///
    /// The span returned is guaranteed to have a start position greater than
    /// or equal to the one given, and an end position less than or equal to
    /// the one given.
    ///
    /// # Example
    ///
    /// This example shows how to build a prefilter directly from an [`Hir`]
    /// expression, and use it to find an occurrence of a prefix from the regex
    /// pattern.
    ///
    /// ```
    /// use regex_automata::{
    ///     util::{prefilter::Prefilter, syntax},
    ///     MatchKind, Span,
    /// };
    ///
    /// let hir = syntax::parse(r"Bruce \w+")?;
    /// let pre = Prefilter::from_hir_prefix(MatchKind::LeftmostFirst, &hir)
    ///     .expect("a prefilter");
    /// let hay = "Hello Bruce Springsteen!";
    /// assert_eq!(
    ///     Some(Span::from(6..12)),
    ///     pre.find(hay.as_bytes(), Span::from(0..hay.len())),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn find(&self, haystack: &[u8], span: Span) -> Option<Span> {
        #[cfg(not(feature = "alloc"))]
        {
            unreachable!()
        }
        #[cfg(feature = "alloc")]
        {
            self.pre.find(haystack, span)
        }
    }

    /// Returns the span of a prefix of `haystack[span.start..span.end]` if
    /// the prefilter matches.
    ///
    /// The span returned is guaranteed to have a start position equivalent to
    /// the one given, and an end position less than or equal to the one given.
    ///
    /// # Example
    ///
    /// This example shows how to build a prefilter directly from an [`Hir`]
    /// expression, and use it to find an occurrence of a prefix from the regex
    /// pattern that begins at the start of a haystack only.
    ///
    /// ```
    /// use regex_automata::{
    ///     util::{prefilter::Prefilter, syntax},
    ///     MatchKind, Span,
    /// };
    ///
    /// let hir = syntax::parse(r"Bruce \w+")?;
    /// let pre = Prefilter::from_hir_prefix(MatchKind::LeftmostFirst, &hir)
    ///     .expect("a prefilter");
    /// let hay = "Hello Bruce Springsteen!";
    /// // Nothing is found here because 'Bruce' does
    /// // not occur at the beginning of our search.
    /// assert_eq!(
    ///     None,
    ///     pre.prefix(hay.as_bytes(), Span::from(0..hay.len())),
    /// );
    /// // But if we change where we start the search
    /// // to begin where 'Bruce ' begins, then a
    /// // match will be found.
    /// assert_eq!(
    ///     Some(Span::from(6..12)),
    ///     pre.prefix(hay.as_bytes(), Span::from(6..hay.len())),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn prefix(&self, haystack: &[u8], span: Span) -> Option<Span> {
        #[cfg(not(feature = "alloc"))]
        {
            unreachable!()
        }
        #[cfg(feature = "alloc")]
        {
            self.pre.prefix(haystack, span)
        }
    }

    /// Returns the heap memory, in bytes, used by the underlying prefilter.
    #[inline]
    pub fn memory_usage(&self) -> usize {
        #[cfg(not(feature = "alloc"))]
        {
            unreachable!()
        }
        #[cfg(feature = "alloc")]
        {
            self.pre.memory_usage()
        }
    }

    /// Return the length of the longest needle
    /// in this Prefilter
    #[inline]
    pub fn max_needle_len(&self) -> usize {
        #[cfg(not(feature = "alloc"))]
        {
            unreachable!()
        }
        #[cfg(feature = "alloc")]
        {
            self.max_needle_len
        }
    }

    /// Implementations might return true here if they believe themselves to
    /// be "fast." The concept of "fast" is deliberately left vague, but in
    /// practice this usually corresponds to whether it's believed that SIMD
    /// will be used.
    ///
    /// Why do we care about this? Well, some prefilter tricks tend to come
    /// with their own bits of overhead, and so might only make sense if we
    /// know that a scan will be *much* faster than the regex engine itself.
    /// Otherwise, the trick may not be worth doing. Whether something is
    /// "much" faster than the regex engine generally boils down to whether
    /// SIMD is used. (But not always. Even a SIMD matcher with a high false
    /// positive rate can become quite slow.)
    ///
    /// Even if this returns true, it is still possible for the prefilter to
    /// be "slow." Remember, prefilters are just heuristics. We can't really
    /// *know* a prefilter will be fast without actually trying the prefilter.
    /// (Which of course we cannot afford to do.)
    #[inline]
    pub fn is_fast(&self) -> bool {
        #[cfg(not(feature = "alloc"))]
        {
            unreachable!()
        }
        #[cfg(feature = "alloc")]
        {
            self.is_fast
        }
    }
}

/// A trait for abstracting over prefilters. Basically, a prefilter is
/// something that do an unanchored *and* an anchored search in a haystack
/// within a given span.
///
/// This exists pretty much only so that we can use prefilters as a trait
/// object (which is what `Prefilter` is). If we ever move off of trait objects
/// and to an enum, then it's likely this trait could be removed.
pub(crate) trait PrefilterI:
    Debug + Send + Sync + RefUnwindSafe + UnwindSafe + 'static
{
    /// Run this prefilter on `haystack[span.start..end]` and return a matching
    /// span if one exists.
    ///
    /// The span returned is guaranteed to have a start position greater than
    /// or equal to the one given, and an end position less than or equal to
    /// the one given.
    fn find(&self, haystack: &[u8], span: Span) -> Option<Span>;

    /// Returns the span of a prefix of `haystack[span.start..span.end]` if
    /// the prefilter matches.
    ///
    /// The span returned is guaranteed to have a start position equivalent to
    /// the one given, and an end position less than or equal to the one given.
    fn prefix(&self, haystack: &[u8], span: Span) -> Option<Span>;

    /// Returns the heap memory, in bytes, used by the underlying prefilter.
    fn memory_usage(&self) -> usize;

    /// Implementations might return true here if they believe themselves to
    /// be "fast." See [`Prefilter::is_fast`] for more details.
    fn is_fast(&self) -> bool;
}

#[cfg(feature = "alloc")]
impl<P: PrefilterI + ?Sized> PrefilterI for Arc<P> {
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn find(&self, haystack: &[u8], span: Span) -> Option<Span> {
        (&**self).find(haystack, span)
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn prefix(&self, haystack: &[u8], span: Span) -> Option<Span> {
        (&**self).prefix(haystack, span)
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn memory_usage(&self) -> usize {
        (&**self).memory_usage()
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn is_fast(&self) -> bool {
        (&**self).is_fast()
    }
}

/// A type that encapsulates the selection of a prefilter algorithm from a
/// sequence of needles.
///
/// The existence of this type is a little tricky, because we don't (currently)
/// use it for performing a search. Instead, we really only consume it by
/// converting the underlying prefilter into a trait object, whether that be
/// `dyn PrefilterI` or `dyn Strategy` (for the meta regex engine). In order
/// to avoid re-copying the prefilter selection logic, we isolate it here, and
/// then force anything downstream that wants to convert it to a trait object
/// to do trivial case analysis on it.
///
/// One wonders whether we *should* use an enum instead of a trait object.
/// At time of writing, I chose trait objects based on instinct because 1) I
/// knew I wasn't going to inline anything and 2) there would potentially be
/// many different choices. However, as of time of writing, I haven't actually
/// compared the trait object approach to the enum approach. That probably
/// should be litigated, but I ran out of steam.
///
/// Note that if the `alloc` feature is disabled, then values of this type
/// are (and should) never be constructed. Also, in practice, for any of the
/// prefilters to be selected, you'll need at least one of the `perf-literal-*`
/// features enabled.
#[derive(Clone, Debug)]
pub(crate) enum Choice {
    Memchr(Memchr),
    Memchr2(Memchr2),
    Memchr3(Memchr3),
    Memmem(Memmem),
    Teddy(Teddy),
    ByteSet(ByteSet),
    AhoCorasick(AhoCorasick),
}

impl Choice {
    /// Select what is believed to be the best prefilter algorithm for the
    /// match semantics and sequence of needles given.
    ///
    /// This selection algorithm uses the needles as given without any
    /// modification. For example, if `[bar]` is given, then this doesn't
    /// try to select `memchr` for `b`. Instead, it would select `memmem`
    /// for `bar`. If callers would want `memchr` selected for `[bar]`, then
    /// callers should massages the literals themselves. That is, callers are
    /// responsible for heuristics surrounding which sequence of literals is
    /// best.
    ///
    /// What this selection algorithm does is attempt to use the fastest
    /// prefilter that works for the literals given. So if `[a, b]`, is given,
    /// then `memchr2` is selected.
    ///
    /// Of course, which prefilter is selected is also subject to what
    /// is available. For example, if `alloc` isn't enabled, then
    /// that limits which prefilters can be selected. Similarly, if
    /// `perf-literal-substring` isn't enabled, then nothing from the `memchr`
    /// crate can be returned.
    pub(crate) fn new<B: AsRef<[u8]>>(
        kind: MatchKind,
        needles: &[B],
    ) -> Option<Choice> {
        // An empty set means the regex matches nothing, so no sense in
        // building a prefilter.
        if needles.len() == 0 {
            debug!("prefilter building failed: found empty set of literals");
            return None;
        }
        // If the regex can match the empty string, then the prefilter
        // will by definition match at every position. This is obviously
        // completely ineffective.
        if needles.iter().any(|n| n.as_ref().is_empty()) {
            debug!("prefilter building failed: literals match empty string");
            return None;
        }
        // BREADCRUMBS: Perhaps the literal optimizer should special case
        // sequences of length two or three if the leading bytes of each are
        // "rare"? Or perhaps, if there are two or three total possible leading
        // bytes, regardless of the number of literals, and all are rare...
        // Then well, perhaps we should use memchr2 or memchr3 in those cases?
        if let Some(pre) = Memchr::new(kind, needles) {
            debug!("prefilter built: memchr");
            return Some(Choice::Memchr(pre));
        }
        if let Some(pre) = Memchr2::new(kind, needles) {
            debug!("prefilter built: memchr2");
            return Some(Choice::Memchr2(pre));
        }
        if let Some(pre) = Memchr3::new(kind, needles) {
            debug!("prefilter built: memchr3");
            return Some(Choice::Memchr3(pre));
        }
        if let Some(pre) = Memmem::new(kind, needles) {
            debug!("prefilter built: memmem");
            return Some(Choice::Memmem(pre));
        }
        if let Some(pre) = Teddy::new(kind, needles) {
            debug!("prefilter built: teddy");
            return Some(Choice::Teddy(pre));
        }
        if let Some(pre) = ByteSet::new(kind, needles) {
            debug!("prefilter built: byteset");
            return Some(Choice::ByteSet(pre));
        }
        if let Some(pre) = AhoCorasick::new(kind, needles) {
            debug!("prefilter built: aho-corasick");
            return Some(Choice::AhoCorasick(pre));
        }
        debug!("prefilter building failed: no strategy could be found");
        None
    }
}

/// Extracts all of the prefix literals from the given HIR expressions into a
/// single `Seq`. The literals in the sequence are ordered with respect to the
/// order of the given HIR expressions and consistent with the match semantics
/// given.
///
/// The sequence returned is "optimized." That is, they may be shrunk or even
/// truncated according to heuristics with the intent of making them more
/// useful as a prefilter. (Which translates to both using faster algorithms
/// and minimizing the false positive rate.)
///
/// Note that this erases any connection between the literals and which pattern
/// (or patterns) they came from.
///
/// The match kind given must correspond to the match semantics of the regex
/// that is represented by the HIRs given. The match semantics may change the
/// literal sequence returned.
#[cfg(feature = "syntax")]
pub(crate) fn prefixes<H>(kind: MatchKind, hirs: &[H]) -> literal::Seq
where
    H: core::borrow::Borrow<Hir>,
{
    let mut extractor = literal::Extractor::new();
    extractor.kind(literal::ExtractKind::Prefix);

    let mut prefixes = literal::Seq::empty();
    for hir in hirs {
        prefixes.union(&mut extractor.extract(hir.borrow()));
    }
    debug!(
        "prefixes (len={:?}, exact={:?}) extracted before optimization: {:?}",
        prefixes.len(),
        prefixes.is_exact(),
        prefixes
    );
    match kind {
        MatchKind::All => {
            prefixes.sort();
            prefixes.dedup();
        }
        MatchKind::LeftmostFirst => {
            prefixes.optimize_for_prefix_by_preference();
        }
    }
    debug!(
        "prefixes (len={:?}, exact={:?}) extracted after optimization: {:?}",
        prefixes.len(),
        prefixes.is_exact(),
        prefixes
    );
    prefixes
}

/// Like `prefixes`, but for all suffixes of all matches for the given HIRs.
#[cfg(feature = "syntax")]
pub(crate) fn suffixes<H>(kind: MatchKind, hirs: &[H]) -> literal::Seq
where
    H: core::borrow::Borrow<Hir>,
{
    let mut extractor = literal::Extractor::new();
    extractor.kind(literal::ExtractKind::Suffix);

    let mut suffixes = literal::Seq::empty();
    for hir in hirs {
        suffixes.union(&mut extractor.extract(hir.borrow()));
    }
    debug!(
        "suffixes (len={:?}, exact={:?}) extracted before optimization: {:?}",
        suffixes.len(),
        suffixes.is_exact(),
        suffixes
    );
    match kind {
        MatchKind::All => {
            suffixes.sort();
            suffixes.dedup();
        }
        MatchKind::LeftmostFirst => {
            suffixes.optimize_for_suffix_by_preference();
        }
    }
    debug!(
        "suffixes (len={:?}, exact={:?}) extracted after optimization: {:?}",
        suffixes.len(),
        suffixes.is_exact(),
        suffixes
    );
    suffixes
}
