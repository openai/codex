use core::{
    fmt::Debug,
    panic::{RefUnwindSafe, UnwindSafe},
};

use alloc::{string::String, sync::Arc, vec::Vec};

use crate::{
    automaton::{self, Automaton, OverlappingState},
    dfa,
    nfa::{contiguous, noncontiguous},
    util::{
        error::{BuildError, MatchError},
        prefilter::Prefilter,
        primitives::{PatternID, StateID},
        search::{Anchored, Input, Match, MatchKind, StartKind},
    },
};

/// An automaton for searching multiple strings in linear time.
///
/// The `AhoCorasick` type supports a few basic ways of constructing an
/// automaton, with the default being [`AhoCorasick::new`]. However, there
/// are a fair number of configurable options that can be set by using
/// [`AhoCorasickBuilder`] instead. Such options include, but are not limited
/// to, how matches are determined, simple case insensitivity, whether to use a
/// DFA or not and various knobs for controlling the space-vs-time trade offs
/// taken when building the automaton.
///
/// # Resource usage
///
/// Aho-Corasick automatons are always constructed in `O(p)` time, where
/// `p` is the combined length of all patterns being searched. With that
/// said, building an automaton can be fairly costly because of high constant
/// factors, particularly when enabling the [DFA](AhoCorasickKind::DFA) option
/// with [`AhoCorasickBuilder::kind`]. For this reason, it's generally a good
/// idea to build an automaton once and reuse it as much as possible.
///
/// Aho-Corasick automatons can also use a fair bit of memory. To get
/// a concrete idea of how much memory is being used, try using the
/// [`AhoCorasick::memory_usage`] method.
///
/// To give a quick idea of the differences between Aho-Corasick
/// implementations and their resource usage, here's a sample of construction
/// times and heap memory used after building an automaton from 100,000
/// randomly selected titles from Wikipedia:
///
/// * 99MB for a [`noncontiguous::NFA`] in 240ms.
/// * 21MB for a [`contiguous::NFA`] in 275ms.
/// * 1.6GB for a [`dfa::DFA`] in 1.88s.
///
/// (Note that the memory usage above reflects the size of each automaton and
/// not peak memory usage. For example, building a contiguous NFA requires
/// first building a noncontiguous NFA. Once the contiguous NFA is built, the
/// noncontiguous NFA is freed.)
///
/// This experiment very strongly argues that a contiguous NFA is often the
/// best balance in terms of resource usage. It takes a little longer to build,
/// but its memory usage is quite small. Its search speed (not listed) is
/// also often faster than a noncontiguous NFA, but a little slower than a
/// DFA. Indeed, when no specific [`AhoCorasickKind`] is used (which is the
/// default), a contiguous NFA is used in most cases.
///
/// The only "catch" to using a contiguous NFA is that, because of its variety
/// of compression tricks, it may not be able to support automatons as large as
/// what the noncontiguous NFA supports. In which case, building a contiguous
/// NFA will fail and (by default) `AhoCorasick` will automatically fall
/// back to a noncontiguous NFA. (This typically only happens when building
/// automatons from millions of patterns.) Otherwise, the small additional time
/// for building a contiguous NFA is almost certainly worth it.
///
/// # Cloning
///
/// The `AhoCorasick` type uses thread safe reference counting internally. It
/// is guaranteed that it is cheap to clone.
///
/// # Search configuration
///
/// Most of the search routines accept anything that can be cheaply converted
/// to an [`Input`]. This includes `&[u8]`, `&str` and `Input` itself.
///
/// # Construction failure
///
/// It is generally possible for building an Aho-Corasick automaton to fail.
/// Construction can fail in generally one way: when the inputs provided are
/// too big. Whether that's a pattern that is too long, too many patterns
/// or some combination of both. A first approximation for the scale at which
/// construction can fail is somewhere around "millions of patterns."
///
/// For that reason, if you're building an Aho-Corasick automaton from
/// untrusted input (or input that doesn't have any reasonable bounds on its
/// size), then it is strongly recommended to handle the possibility of an
/// error.
///
/// If you're constructing an Aho-Corasick automaton from static or trusted
/// data, then it is likely acceptable to panic (by calling `unwrap()` or
/// `expect()`) if construction fails.
///
/// # Fallibility
///
/// The `AhoCorasick` type provides a number of methods for searching, as one
/// might expect. Depending on how the Aho-Corasick automaton was built and
/// depending on the search configuration, it is possible for a search to
/// return an error. Since an error is _never_ dependent on the actual contents
/// of the haystack, this type provides both infallible and fallible methods
/// for searching. The infallible methods panic if an error occurs, and can be
/// used for convenience and when you know the search will never return an
/// error.
///
/// For example, the [`AhoCorasick::find_iter`] method is the infallible
/// version of the [`AhoCorasick::try_find_iter`] method.
///
/// Examples of errors that can occur:
///
/// * Running a search that requires [`MatchKind::Standard`] semantics (such
/// as a stream or overlapping search) with an automaton that was built with
/// [`MatchKind::LeftmostFirst`] or [`MatchKind::LeftmostLongest`] semantics.
/// * Running an anchored search with an automaton that only supports
/// unanchored searches. (By default, `AhoCorasick` only supports unanchored
/// searches. But this can be toggled with [`AhoCorasickBuilder::start_kind`].)
/// * Running an unanchored search with an automaton that only supports
/// anchored searches.
///
/// The common thread between the different types of errors is that they are
/// all rooted in the automaton construction and search configurations. If
/// those configurations are a static property of your program, then it is
/// reasonable to call infallible routines since you know an error will never
/// occur. And if one _does_ occur, then it's a bug in your program.
///
/// To re-iterate, if the patterns, build or search configuration come from
/// user or untrusted data, then you should handle errors at build or search
/// time. If only the haystack comes from user or untrusted data, then there
/// should be no need to handle errors anywhere and it is generally encouraged
/// to `unwrap()` (or `expect()`) both build and search time calls.
///
/// # Examples
///
/// This example shows how to search for occurrences of multiple patterns
/// simultaneously in a case insensitive fashion. Each match includes the
/// pattern that matched along with the byte offsets of the match.
///
/// ```
/// use aho_corasick::{AhoCorasick, PatternID};
///
/// let patterns = &["apple", "maple", "snapple"];
/// let haystack = "Nobody likes maple in their apple flavored Snapple.";
///
/// let ac = AhoCorasick::builder()
///     .ascii_case_insensitive(true)
///     .build(patterns)
///     .unwrap();
/// let mut matches = vec![];
/// for mat in ac.find_iter(haystack) {
///     matches.push((mat.pattern(), mat.start(), mat.end()));
/// }
/// assert_eq!(matches, vec![
///     (PatternID::must(1), 13, 18),
///     (PatternID::must(0), 28, 33),
///     (PatternID::must(2), 43, 50),
/// ]);
/// ```
///
/// This example shows how to replace matches with some other string:
///
/// ```
/// use aho_corasick::AhoCorasick;
///
/// let patterns = &["fox", "brown", "quick"];
/// let haystack = "The quick brown fox.";
/// let replace_with = &["sloth", "grey", "slow"];
///
/// let ac = AhoCorasick::new(patterns).unwrap();
/// let result = ac.replace_all(haystack, replace_with);
/// assert_eq!(result, "The slow grey sloth.");
/// ```
#[derive(Clone)]
pub struct AhoCorasick {
    /// The underlying Aho-Corasick automaton. It's one of
    /// nfa::noncontiguous::NFA, nfa::contiguous::NFA or dfa::DFA.
    aut: Arc<dyn AcAutomaton>,
    /// The specific Aho-Corasick kind chosen. This makes it possible to
    /// inspect any `AhoCorasick` and know what kind of search strategy it
    /// uses.
    kind: AhoCorasickKind,
    /// The start kind of this automaton as configured by the caller.
    ///
    /// We don't really *need* to put this here, since the underlying automaton
    /// will correctly return errors if the caller requests an unsupported
    /// search type. But we do keep this here for API behavior consistency.
    /// Namely, the NFAs in this crate support both unanchored and anchored
    /// searches unconditionally. There's no way to disable one or the other.
    /// They always both work. But the DFA in this crate specifically only
    /// supports both unanchored and anchored searches if it's configured to
    /// do so. Why? Because for the DFA, supporting both essentially requires
    /// two copies of the transition table: one generated by following failure
    /// transitions from the original NFA and one generated by not following
    /// those failure transitions.
    ///
    /// So why record the start kind here? Well, consider what happens
    /// when no specific 'AhoCorasickKind' is selected by the caller and
    /// 'StartKind::Unanchored' is used (both are the default). It *might*
    /// result in using a DFA or it might pick an NFA. If it picks an NFA, the
    /// caller would then be able to run anchored searches, even though the
    /// caller only asked for support for unanchored searches. Maybe that's
    /// fine, but what if the DFA was chosen instead? Oops, the caller would
    /// get an error.
    ///
    /// Basically, it seems bad to return an error or not based on some
    /// internal implementation choice. So we smooth things out and ensure
    /// anchored searches *always* report an error when only unanchored support
    /// was asked for (and vice versa), even if the underlying automaton
    /// supports it.
    start_kind: StartKind,
}

/// Convenience constructors for an Aho-Corasick searcher. To configure the
/// searcher, use an [`AhoCorasickBuilder`] instead.
impl AhoCorasick {
    /// Create a new Aho-Corasick automaton using the default configuration.
    ///
    /// The default configuration optimizes for less space usage, but at the
    /// expense of longer search times. To change the configuration, use
    /// [`AhoCorasickBuilder`].
    ///
    /// This uses the default [`MatchKind::Standard`] match semantics, which
    /// reports a match as soon as it is found. This corresponds to the
    /// standard match semantics supported by textbook descriptions of the
    /// Aho-Corasick algorithm.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, PatternID};
    ///
    /// let ac = AhoCorasick::new(&["foo", "bar", "baz"]).unwrap();
    /// assert_eq!(
    ///     Some(PatternID::must(1)),
    ///     ac.find("xxx bar xxx").map(|m| m.pattern()),
    /// );
    /// ```
    pub fn new<I, P>(patterns: I) -> Result<AhoCorasick, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        AhoCorasickBuilder::new().build(patterns)
    }

    /// A convenience method for returning a new Aho-Corasick builder.
    ///
    /// This usually permits one to just import the `AhoCorasick` type.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, Match, MatchKind};
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(&["samwise", "sam"])
    ///     .unwrap();
    /// assert_eq!(Some(Match::must(0, 0..7)), ac.find("samwise"));
    /// ```
    pub fn builder() -> AhoCorasickBuilder {
        AhoCorasickBuilder::new()
    }
}

/// Infallible search routines. These APIs panic when the underlying search
/// would otherwise fail. Infallible routines are useful because the errors are
/// a result of both search-time configuration and what configuration is used
/// to build the Aho-Corasick searcher. Both of these things are not usually
/// the result of user input, and thus, an error is typically indicative of a
/// programmer error. In cases where callers want errors instead of panics, use
/// the corresponding `try` method in the section below.
impl AhoCorasick {
    /// Returns true if and only if this automaton matches the haystack at any
    /// position.
    ///
    /// `input` may be any type that is cheaply convertible to an `Input`. This
    /// includes, but is not limited to, `&str` and `&[u8]`.
    ///
    /// Aside from convenience, when `AhoCorasick` was built with
    /// leftmost-first or leftmost-longest semantics, this might result in a
    /// search that visits less of the haystack than [`AhoCorasick::find`]
    /// would otherwise. (For standard semantics, matches are always
    /// immediately returned once they are seen, so there is no way for this to
    /// do less work in that case.)
    ///
    /// Note that there is no corresponding fallible routine for this method.
    /// If you need a fallible version of this, then [`AhoCorasick::try_find`]
    /// can be used with [`Input::earliest`] enabled.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::AhoCorasick;
    ///
    /// let ac = AhoCorasick::new(&[
    ///     "foo", "bar", "quux", "baz",
    /// ]).unwrap();
    /// assert!(ac.is_match("xxx bar xxx"));
    /// assert!(!ac.is_match("xxx qux xxx"));
    /// ```
    pub fn is_match<'h, I: Into<Input<'h>>>(&self, input: I) -> bool {
        self.aut
            .try_find(&input.into().earliest(true))
            .expect("AhoCorasick::try_find is not expected to fail")
            .is_some()
    }

    /// Returns the location of the first match according to the match
    /// semantics that this automaton was constructed with.
    ///
    /// `input` may be any type that is cheaply convertible to an `Input`. This
    /// includes, but is not limited to, `&str` and `&[u8]`.
    ///
    /// This is the infallible version of [`AhoCorasick::try_find`].
    ///
    /// # Panics
    ///
    /// This panics when [`AhoCorasick::try_find`] would return an error.
    ///
    /// # Examples
    ///
    /// Basic usage, with standard semantics:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind};
    ///
    /// let patterns = &["b", "abc", "abcd"];
    /// let haystack = "abcd";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::Standard) // default, not necessary
    ///     .build(patterns)
    ///     .unwrap();
    /// let mat = ac.find(haystack).expect("should have a match");
    /// assert_eq!("b", &haystack[mat.start()..mat.end()]);
    /// ```
    ///
    /// Now with leftmost-first semantics:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind};
    ///
    /// let patterns = &["b", "abc", "abcd"];
    /// let haystack = "abcd";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    /// let mat = ac.find(haystack).expect("should have a match");
    /// assert_eq!("abc", &haystack[mat.start()..mat.end()]);
    /// ```
    ///
    /// And finally, leftmost-longest semantics:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind};
    ///
    /// let patterns = &["b", "abc", "abcd"];
    /// let haystack = "abcd";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostLongest)
    ///     .build(patterns)
    ///     .unwrap();
    /// let mat = ac.find(haystack).expect("should have a match");
    /// ```
    ///
    /// # Example: configuring a search
    ///
    /// Because this method accepts anything that can be turned into an
    /// [`Input`], it's possible to provide an `Input` directly in order to
    /// configure the search. In this example, we show how to use the
    /// `earliest` option to force the search to return as soon as it knows
    /// a match has occurred.
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, Input, MatchKind};
    ///
    /// let patterns = &["b", "abc", "abcd"];
    /// let haystack = "abcd";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostLongest)
    ///     .build(patterns)
    ///     .unwrap();
    /// let mat = ac.find(Input::new(haystack).earliest(true))
    ///     .expect("should have a match");
    /// // The correct leftmost-longest match here is 'abcd', but since we
    /// // told the search to quit as soon as it knows a match has occurred,
    /// // we get a different match back.
    /// assert_eq!("b", &haystack[mat.start()..mat.end()]);
    /// ```
    pub fn find<'h, I: Into<Input<'h>>>(&self, input: I) -> Option<Match> {
        self.try_find(input)
            .expect("AhoCorasick::try_find is not expected to fail")
    }

    /// Returns the location of the first overlapping match in the given
    /// input with respect to the current state of the underlying searcher.
    ///
    /// `input` may be any type that is cheaply convertible to an `Input`. This
    /// includes, but is not limited to, `&str` and `&[u8]`.
    ///
    /// Overlapping searches do not report matches in their return value.
    /// Instead, matches can be accessed via [`OverlappingState::get_match`]
    /// after a search call.
    ///
    /// This is the infallible version of
    /// [`AhoCorasick::try_find_overlapping`].
    ///
    /// # Panics
    ///
    /// This panics when [`AhoCorasick::try_find_overlapping`] would
    /// return an error. For example, when the Aho-Corasick searcher
    /// doesn't support overlapping searches. (Only searchers built with
    /// [`MatchKind::Standard`] semantics support overlapping searches.)
    ///
    /// # Example
    ///
    /// This shows how we can repeatedly call an overlapping search without
    /// ever needing to explicitly re-slice the haystack. Overlapping search
    /// works this way because searches depend on state saved during the
    /// previous search.
    ///
    /// ```
    /// use aho_corasick::{
    ///     automaton::OverlappingState,
    ///     AhoCorasick, Input, Match,
    /// };
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::new(patterns).unwrap();
    /// let mut state = OverlappingState::start();
    ///
    /// ac.find_overlapping(haystack, &mut state);
    /// assert_eq!(Some(Match::must(2, 0..3)), state.get_match());
    ///
    /// ac.find_overlapping(haystack, &mut state);
    /// assert_eq!(Some(Match::must(0, 0..6)), state.get_match());
    ///
    /// ac.find_overlapping(haystack, &mut state);
    /// assert_eq!(Some(Match::must(2, 11..14)), state.get_match());
    ///
    /// ac.find_overlapping(haystack, &mut state);
    /// assert_eq!(Some(Match::must(2, 22..25)), state.get_match());
    ///
    /// ac.find_overlapping(haystack, &mut state);
    /// assert_eq!(Some(Match::must(0, 22..28)), state.get_match());
    ///
    /// ac.find_overlapping(haystack, &mut state);
    /// assert_eq!(Some(Match::must(1, 22..31)), state.get_match());
    ///
    /// // No more match matches to be found.
    /// ac.find_overlapping(haystack, &mut state);
    /// assert_eq!(None, state.get_match());
    /// ```
    pub fn find_overlapping<'h, I: Into<Input<'h>>>(
        &self,
        input: I,
        state: &mut OverlappingState,
    ) {
        self.try_find_overlapping(input, state).expect(
            "AhoCorasick::try_find_overlapping is not expected to fail",
        )
    }

    /// Returns an iterator of non-overlapping matches, using the match
    /// semantics that this automaton was constructed with.
    ///
    /// `input` may be any type that is cheaply convertible to an `Input`. This
    /// includes, but is not limited to, `&str` and `&[u8]`.
    ///
    /// This is the infallible version of [`AhoCorasick::try_find_iter`].
    ///
    /// # Panics
    ///
    /// This panics when [`AhoCorasick::try_find_iter`] would return an error.
    ///
    /// # Examples
    ///
    /// Basic usage, with standard semantics:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind, PatternID};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::Standard) // default, not necessary
    ///     .build(patterns)
    ///     .unwrap();
    /// let matches: Vec<PatternID> = ac
    ///     .find_iter(haystack)
    ///     .map(|mat| mat.pattern())
    ///     .collect();
    /// assert_eq!(vec![
    ///     PatternID::must(2),
    ///     PatternID::must(2),
    ///     PatternID::must(2),
    /// ], matches);
    /// ```
    ///
    /// Now with leftmost-first semantics:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind, PatternID};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    /// let matches: Vec<PatternID> = ac
    ///     .find_iter(haystack)
    ///     .map(|mat| mat.pattern())
    ///     .collect();
    /// assert_eq!(vec![
    ///     PatternID::must(0),
    ///     PatternID::must(2),
    ///     PatternID::must(0),
    /// ], matches);
    /// ```
    ///
    /// And finally, leftmost-longest semantics:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind, PatternID};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostLongest)
    ///     .build(patterns)
    ///     .unwrap();
    /// let matches: Vec<PatternID> = ac
    ///     .find_iter(haystack)
    ///     .map(|mat| mat.pattern())
    ///     .collect();
    /// assert_eq!(vec![
    ///     PatternID::must(0),
    ///     PatternID::must(2),
    ///     PatternID::must(1),
    /// ], matches);
    /// ```
    pub fn find_iter<'a, 'h, I: Into<Input<'h>>>(
        &'a self,
        input: I,
    ) -> FindIter<'a, 'h> {
        self.try_find_iter(input)
            .expect("AhoCorasick::try_find_iter is not expected to fail")
    }

    /// Returns an iterator of overlapping matches. Stated differently, this
    /// returns an iterator of all possible matches at every position.
    ///
    /// `input` may be any type that is cheaply convertible to an `Input`. This
    /// includes, but is not limited to, `&str` and `&[u8]`.
    ///
    /// This is the infallible version of
    /// [`AhoCorasick::try_find_overlapping_iter`].
    ///
    /// # Panics
    ///
    /// This panics when `AhoCorasick::try_find_overlapping_iter` would return
    /// an error. For example, when the Aho-Corasick searcher is built with
    /// either leftmost-first or leftmost-longest match semantics. Stated
    /// differently, overlapping searches require one to build the searcher
    /// with [`MatchKind::Standard`] (it is the default).
    ///
    /// # Example: basic usage
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, PatternID};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::new(patterns).unwrap();
    /// let matches: Vec<PatternID> = ac
    ///     .find_overlapping_iter(haystack)
    ///     .map(|mat| mat.pattern())
    ///     .collect();
    /// assert_eq!(vec![
    ///     PatternID::must(2),
    ///     PatternID::must(0),
    ///     PatternID::must(2),
    ///     PatternID::must(2),
    ///     PatternID::must(0),
    ///     PatternID::must(1),
    /// ], matches);
    /// ```
    pub fn find_overlapping_iter<'a, 'h, I: Into<Input<'h>>>(
        &'a self,
        input: I,
    ) -> FindOverlappingIter<'a, 'h> {
        self.try_find_overlapping_iter(input).expect(
            "AhoCorasick::try_find_overlapping_iter is not expected to fail",
        )
    }

    /// Replace all matches with a corresponding value in the `replace_with`
    /// slice given. Matches correspond to the same matches as reported by
    /// [`AhoCorasick::find_iter`].
    ///
    /// Replacements are determined by the index of the matching pattern.
    /// For example, if the pattern with index `2` is found, then it is
    /// replaced by `replace_with[2]`.
    ///
    /// This is the infallible version of [`AhoCorasick::try_replace_all`].
    ///
    /// # Panics
    ///
    /// This panics when [`AhoCorasick::try_replace_all`] would return an
    /// error.
    ///
    /// This also panics when `replace_with.len()` does not equal
    /// [`AhoCorasick::patterns_len`].
    ///
    /// # Example: basic usage
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    /// let result = ac.replace_all(haystack, &["x", "y", "z"]);
    /// assert_eq!("x the z to the xage", result);
    /// ```
    pub fn replace_all<B>(&self, haystack: &str, replace_with: &[B]) -> String
    where
        B: AsRef<str>,
    {
        self.try_replace_all(haystack, replace_with)
            .expect("AhoCorasick::try_replace_all is not expected to fail")
    }

    /// Replace all matches using raw bytes with a corresponding value in the
    /// `replace_with` slice given. Matches correspond to the same matches as
    /// reported by [`AhoCorasick::find_iter`].
    ///
    /// Replacements are determined by the index of the matching pattern.
    /// For example, if the pattern with index `2` is found, then it is
    /// replaced by `replace_with[2]`.
    ///
    /// This is the infallible version of
    /// [`AhoCorasick::try_replace_all_bytes`].
    ///
    /// # Panics
    ///
    /// This panics when [`AhoCorasick::try_replace_all_bytes`] would return an
    /// error.
    ///
    /// This also panics when `replace_with.len()` does not equal
    /// [`AhoCorasick::patterns_len`].
    ///
    /// # Example: basic usage
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = b"append the app to the appendage";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    /// let result = ac.replace_all_bytes(haystack, &["x", "y", "z"]);
    /// assert_eq!(b"x the z to the xage".to_vec(), result);
    /// ```
    pub fn replace_all_bytes<B>(
        &self,
        haystack: &[u8],
        replace_with: &[B],
    ) -> Vec<u8>
    where
        B: AsRef<[u8]>,
    {
        self.try_replace_all_bytes(haystack, replace_with)
            .expect("AhoCorasick::try_replace_all_bytes should not fail")
    }

    /// Replace all matches using a closure called on each match.
    /// Matches correspond to the same matches as reported by
    /// [`AhoCorasick::find_iter`].
    ///
    /// The closure accepts three parameters: the match found, the text of
    /// the match and a string buffer with which to write the replaced text
    /// (if any). If the closure returns `true`, then it continues to the next
    /// match. If the closure returns `false`, then searching is stopped.
    ///
    /// Note that any matches with boundaries that don't fall on a valid UTF-8
    /// boundary are silently skipped.
    ///
    /// This is the infallible version of
    /// [`AhoCorasick::try_replace_all_with`].
    ///
    /// # Panics
    ///
    /// This panics when [`AhoCorasick::try_replace_all_with`] would return an
    /// error.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    /// let mut result = String::new();
    /// ac.replace_all_with(haystack, &mut result, |mat, _, dst| {
    ///     dst.push_str(&mat.pattern().as_usize().to_string());
    ///     true
    /// });
    /// assert_eq!("0 the 2 to the 0age", result);
    /// ```
    ///
    /// Stopping the replacement by returning `false` (continued from the
    /// example above):
    ///
    /// ```
    /// # use aho_corasick::{AhoCorasick, MatchKind, PatternID};
    /// # let patterns = &["append", "appendage", "app"];
    /// # let haystack = "append the app to the appendage";
    /// # let ac = AhoCorasick::builder()
    /// #    .match_kind(MatchKind::LeftmostFirst)
    /// #    .build(patterns)
    /// #    .unwrap();
    /// let mut result = String::new();
    /// ac.replace_all_with(haystack, &mut result, |mat, _, dst| {
    ///     dst.push_str(&mat.pattern().as_usize().to_string());
    ///     mat.pattern() != PatternID::must(2)
    /// });
    /// assert_eq!("0 the 2 to the appendage", result);
    /// ```
    pub fn replace_all_with<F>(
        &self,
        haystack: &str,
        dst: &mut String,
        replace_with: F,
    ) where
        F: FnMut(&Match, &str, &mut String) -> bool,
    {
        self.try_replace_all_with(haystack, dst, replace_with)
            .expect("AhoCorasick::try_replace_all_with should not fail")
    }

    /// Replace all matches using raw bytes with a closure called on each
    /// match. Matches correspond to the same matches as reported by
    /// [`AhoCorasick::find_iter`].
    ///
    /// The closure accepts three parameters: the match found, the text of
    /// the match and a byte buffer with which to write the replaced text
    /// (if any). If the closure returns `true`, then it continues to the next
    /// match. If the closure returns `false`, then searching is stopped.
    ///
    /// This is the infallible version of
    /// [`AhoCorasick::try_replace_all_with_bytes`].
    ///
    /// # Panics
    ///
    /// This panics when [`AhoCorasick::try_replace_all_with_bytes`] would
    /// return an error.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = b"append the app to the appendage";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    /// let mut result = vec![];
    /// ac.replace_all_with_bytes(haystack, &mut result, |mat, _, dst| {
    ///     dst.extend(mat.pattern().as_usize().to_string().bytes());
    ///     true
    /// });
    /// assert_eq!(b"0 the 2 to the 0age".to_vec(), result);
    /// ```
    ///
    /// Stopping the replacement by returning `false` (continued from the
    /// example above):
    ///
    /// ```
    /// # use aho_corasick::{AhoCorasick, MatchKind, PatternID};
    /// # let patterns = &["append", "appendage", "app"];
    /// # let haystack = b"append the app to the appendage";
    /// # let ac = AhoCorasick::builder()
    /// #    .match_kind(MatchKind::LeftmostFirst)
    /// #    .build(patterns)
    /// #    .unwrap();
    /// let mut result = vec![];
    /// ac.replace_all_with_bytes(haystack, &mut result, |mat, _, dst| {
    ///     dst.extend(mat.pattern().as_usize().to_string().bytes());
    ///     mat.pattern() != PatternID::must(2)
    /// });
    /// assert_eq!(b"0 the 2 to the appendage".to_vec(), result);
    /// ```
    pub fn replace_all_with_bytes<F>(
        &self,
        haystack: &[u8],
        dst: &mut Vec<u8>,
        replace_with: F,
    ) where
        F: FnMut(&Match, &[u8], &mut Vec<u8>) -> bool,
    {
        self.try_replace_all_with_bytes(haystack, dst, replace_with)
            .expect("AhoCorasick::try_replace_all_with_bytes should not fail")
    }

    /// Returns an iterator of non-overlapping matches in the given
    /// stream. Matches correspond to the same matches as reported by
    /// [`AhoCorasick::find_iter`].
    ///
    /// The matches yielded by this iterator use absolute position offsets in
    /// the stream given, where the first byte has index `0`. Matches are
    /// yieled until the stream is exhausted.
    ///
    /// Each item yielded by the iterator is an `Result<Match,
    /// std::io::Error>`, where an error is yielded if there was a problem
    /// reading from the reader given.
    ///
    /// When searching a stream, an internal buffer is used. Therefore, callers
    /// should avoiding providing a buffered reader, if possible.
    ///
    /// This is the infallible version of
    /// [`AhoCorasick::try_stream_find_iter`]. Note that both methods return
    /// iterators that produce `Result` values. The difference is that this
    /// routine panics if _construction_ of the iterator failed. The `Result`
    /// values yield by the iterator come from whether the given reader returns
    /// an error or not during the search.
    ///
    /// # Memory usage
    ///
    /// In general, searching streams will use a constant amount of memory for
    /// its internal buffer. The one requirement is that the internal buffer
    /// must be at least the size of the longest possible match. In most use
    /// cases, the default buffer size will be much larger than any individual
    /// match.
    ///
    /// # Panics
    ///
    /// This panics when [`AhoCorasick::try_stream_find_iter`] would return
    /// an error. For example, when the Aho-Corasick searcher doesn't support
    /// stream searches. (Only searchers built with [`MatchKind::Standard`]
    /// semantics support stream searches.)
    ///
    /// # Example: basic usage
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, PatternID};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::new(patterns).unwrap();
    /// let mut matches = vec![];
    /// for result in ac.stream_find_iter(haystack.as_bytes()) {
    ///     let mat = result?;
    ///     matches.push(mat.pattern());
    /// }
    /// assert_eq!(vec![
    ///     PatternID::must(2),
    ///     PatternID::must(2),
    ///     PatternID::must(2),
    /// ], matches);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "std")]
    pub fn stream_find_iter<'a, R: std::io::Read>(
        &'a self,
        rdr: R,
    ) -> StreamFindIter<'a, R> {
        self.try_stream_find_iter(rdr)
            .expect("AhoCorasick::try_stream_find_iter should not fail")
    }
}

/// Fallible search routines. These APIs return an error in cases where the
/// infallible routines would panic.
impl AhoCorasick {
    /// Returns the location of the first match according to the match
    /// semantics that this automaton was constructed with, and according
    /// to the given `Input` configuration.
    ///
    /// This is the fallible version of [`AhoCorasick::find`].
    ///
    /// # Errors
    ///
    /// This returns an error when this Aho-Corasick searcher does not support
    /// the given `Input` configuration.
    ///
    /// For example, if the Aho-Corasick searcher only supports anchored
    /// searches or only supports unanchored searches, then providing an
    /// `Input` that requests an anchored (or unanchored) search when it isn't
    /// supported would result in an error.
    ///
    /// # Example: leftmost-first searching
    ///
    /// Basic usage with leftmost-first semantics:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind, Input};
    ///
    /// let patterns = &["b", "abc", "abcd"];
    /// let haystack = "foo abcd";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    /// let mat = ac.try_find(haystack)?.expect("should have a match");
    /// assert_eq!("abc", &haystack[mat.span()]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: anchored leftmost-first searching
    ///
    /// This shows how to anchor the search, so that even if the haystack
    /// contains a match somewhere, a match won't be reported unless one can
    /// be found that starts at the beginning of the search:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, Anchored, Input, MatchKind, StartKind};
    ///
    /// let patterns = &["b", "abc", "abcd"];
    /// let haystack = "foo abcd";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .start_kind(StartKind::Anchored)
    ///     .build(patterns)
    ///     .unwrap();
    /// let input = Input::new(haystack).anchored(Anchored::Yes);
    /// assert_eq!(None, ac.try_find(input)?);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// If the beginning of the search is changed to where a match begins, then
    /// it will be found:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, Anchored, Input, MatchKind, StartKind};
    ///
    /// let patterns = &["b", "abc", "abcd"];
    /// let haystack = "foo abcd";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .start_kind(StartKind::Anchored)
    ///     .build(patterns)
    ///     .unwrap();
    /// let input = Input::new(haystack).range(4..).anchored(Anchored::Yes);
    /// let mat = ac.try_find(input)?.expect("should have a match");
    /// assert_eq!("abc", &haystack[mat.span()]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: earliest leftmost-first searching
    ///
    /// This shows how to run an "earliest" search even when the Aho-Corasick
    /// searcher was compiled with leftmost-first match semantics. In this
    /// case, the search is stopped as soon as it is known that a match has
    /// occurred, even if it doesn't correspond to the leftmost-first match.
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, Input, MatchKind};
    ///
    /// let patterns = &["b", "abc", "abcd"];
    /// let haystack = "foo abcd";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    /// let input = Input::new(haystack).earliest(true);
    /// let mat = ac.try_find(input)?.expect("should have a match");
    /// assert_eq!("b", &haystack[mat.span()]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn try_find<'h, I: Into<Input<'h>>>(
        &self,
        input: I,
    ) -> Result<Option<Match>, MatchError> {
        let input = input.into();
        enforce_anchored_consistency(self.start_kind, input.get_anchored())?;
        self.aut.try_find(&input)
    }

    /// Returns the location of the first overlapping match in the given
    /// input with respect to the current state of the underlying searcher.
    ///
    /// Overlapping searches do not report matches in their return value.
    /// Instead, matches can be accessed via [`OverlappingState::get_match`]
    /// after a search call.
    ///
    /// This is the fallible version of [`AhoCorasick::find_overlapping`].
    ///
    /// # Errors
    ///
    /// This returns an error when this Aho-Corasick searcher does not support
    /// the given `Input` configuration or if overlapping search is not
    /// supported.
    ///
    /// One example is that only Aho-Corasicker searchers built with
    /// [`MatchKind::Standard`] semantics support overlapping searches. Using
    /// any other match semantics will result in this returning an error.
    ///
    /// # Example: basic usage
    ///
    /// This shows how we can repeatedly call an overlapping search without
    /// ever needing to explicitly re-slice the haystack. Overlapping search
    /// works this way because searches depend on state saved during the
    /// previous search.
    ///
    /// ```
    /// use aho_corasick::{
    ///     automaton::OverlappingState,
    ///     AhoCorasick, Input, Match,
    /// };
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::new(patterns).unwrap();
    /// let mut state = OverlappingState::start();
    ///
    /// ac.try_find_overlapping(haystack, &mut state)?;
    /// assert_eq!(Some(Match::must(2, 0..3)), state.get_match());
    ///
    /// ac.try_find_overlapping(haystack, &mut state)?;
    /// assert_eq!(Some(Match::must(0, 0..6)), state.get_match());
    ///
    /// ac.try_find_overlapping(haystack, &mut state)?;
    /// assert_eq!(Some(Match::must(2, 11..14)), state.get_match());
    ///
    /// ac.try_find_overlapping(haystack, &mut state)?;
    /// assert_eq!(Some(Match::must(2, 22..25)), state.get_match());
    ///
    /// ac.try_find_overlapping(haystack, &mut state)?;
    /// assert_eq!(Some(Match::must(0, 22..28)), state.get_match());
    ///
    /// ac.try_find_overlapping(haystack, &mut state)?;
    /// assert_eq!(Some(Match::must(1, 22..31)), state.get_match());
    ///
    /// // No more match matches to be found.
    /// ac.try_find_overlapping(haystack, &mut state)?;
    /// assert_eq!(None, state.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: implementing your own overlapping iteration
    ///
    /// The previous example can be easily adapted to implement your own
    /// iteration by repeatedly calling `try_find_overlapping` until either
    /// an error occurs or no more matches are reported.
    ///
    /// This is effectively equivalent to the iterator returned by
    /// [`AhoCorasick::try_find_overlapping_iter`], with the only difference
    /// being that the iterator checks for errors before construction and
    /// absolves the caller of needing to check for errors on every search
    /// call. (Indeed, if the first `try_find_overlapping` call succeeds and
    /// the same `Input` is given to subsequent calls, then all subsequent
    /// calls are guaranteed to succeed.)
    ///
    /// ```
    /// use aho_corasick::{
    ///     automaton::OverlappingState,
    ///     AhoCorasick, Input, Match,
    /// };
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::new(patterns).unwrap();
    /// let mut state = OverlappingState::start();
    /// let mut matches = vec![];
    ///
    /// loop {
    ///     ac.try_find_overlapping(haystack, &mut state)?;
    ///     let mat = match state.get_match() {
    ///         None => break,
    ///         Some(mat) => mat,
    ///     };
    ///     matches.push(mat);
    /// }
    /// let expected = vec![
    ///     Match::must(2, 0..3),
    ///     Match::must(0, 0..6),
    ///     Match::must(2, 11..14),
    ///     Match::must(2, 22..25),
    ///     Match::must(0, 22..28),
    ///     Match::must(1, 22..31),
    /// ];
    /// assert_eq!(expected, matches);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: anchored iteration
    ///
    /// The previous example can also be adapted to implement
    /// iteration over all anchored matches. In particular,
    /// [`AhoCorasick::try_find_overlapping_iter`] does not support this
    /// because it isn't totally clear what the match semantics ought to be.
    ///
    /// In this example, we will find all overlapping matches that start at
    /// the beginning of our search.
    ///
    /// ```
    /// use aho_corasick::{
    ///     automaton::OverlappingState,
    ///     AhoCorasick, Anchored, Input, Match, StartKind,
    /// };
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .start_kind(StartKind::Anchored)
    ///     .build(patterns)
    ///     .unwrap();
    /// let input = Input::new(haystack).anchored(Anchored::Yes);
    /// let mut state = OverlappingState::start();
    /// let mut matches = vec![];
    ///
    /// loop {
    ///     ac.try_find_overlapping(input.clone(), &mut state)?;
    ///     let mat = match state.get_match() {
    ///         None => break,
    ///         Some(mat) => mat,
    ///     };
    ///     matches.push(mat);
    /// }
    /// let expected = vec![
    ///     Match::must(2, 0..3),
    ///     Match::must(0, 0..6),
    /// ];
    /// assert_eq!(expected, matches);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn try_find_overlapping<'h, I: Into<Input<'h>>>(
        &self,
        input: I,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        let input = input.into();
        enforce_anchored_consistency(self.start_kind, input.get_anchored())?;
        self.aut.try_find_overlapping(&input, state)
    }

    /// Returns an iterator of non-overlapping matches, using the match
    /// semantics that this automaton was constructed with.
    ///
    /// This is the fallible version of [`AhoCorasick::find_iter`].
    ///
    /// Note that the error returned by this method occurs during construction
    /// of the iterator. The iterator itself yields `Match` values. That is,
    /// once the iterator is constructed, the iteration itself will never
    /// report an error.
    ///
    /// # Errors
    ///
    /// This returns an error when this Aho-Corasick searcher does not support
    /// the given `Input` configuration.
    ///
    /// For example, if the Aho-Corasick searcher only supports anchored
    /// searches or only supports unanchored searches, then providing an
    /// `Input` that requests an anchored (or unanchored) search when it isn't
    /// supported would result in an error.
    ///
    /// # Example: leftmost-first searching
    ///
    /// Basic usage with leftmost-first semantics:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, Input, MatchKind, PatternID};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    /// let matches: Vec<PatternID> = ac
    ///     .try_find_iter(Input::new(haystack))?
    ///     .map(|mat| mat.pattern())
    ///     .collect();
    /// assert_eq!(vec![
    ///     PatternID::must(0),
    ///     PatternID::must(2),
    ///     PatternID::must(0),
    /// ], matches);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: anchored leftmost-first searching
    ///
    /// This shows how to anchor the search, such that all matches must begin
    /// at the starting location of the search. For an iterator, an anchored
    /// search implies that all matches are adjacent.
    ///
    /// ```
    /// use aho_corasick::{
    ///     AhoCorasick, Anchored, Input, MatchKind, PatternID, StartKind,
    /// };
    ///
    /// let patterns = &["foo", "bar", "quux"];
    /// let haystack = "fooquuxbar foo";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .start_kind(StartKind::Anchored)
    ///     .build(patterns)
    ///     .unwrap();
    /// let matches: Vec<PatternID> = ac
    ///     .try_find_iter(Input::new(haystack).anchored(Anchored::Yes))?
    ///     .map(|mat| mat.pattern())
    ///     .collect();
    /// assert_eq!(vec![
    ///     PatternID::must(0),
    ///     PatternID::must(2),
    ///     PatternID::must(1),
    ///     // The final 'foo' is not found because it is not adjacent to the
    ///     // 'bar' match. It needs to be adjacent because our search is
    ///     // anchored.
    /// ], matches);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn try_find_iter<'a, 'h, I: Into<Input<'h>>>(
        &'a self,
        input: I,
    ) -> Result<FindIter<'a, 'h>, MatchError> {
        let input = input.into();
        enforce_anchored_consistency(self.start_kind, input.get_anchored())?;
        Ok(FindIter(self.aut.try_find_iter(input)?))
    }

    /// Returns an iterator of overlapping matches.
    ///
    /// This is the fallible version of [`AhoCorasick::find_overlapping_iter`].
    ///
    /// Note that the error returned by this method occurs during construction
    /// of the iterator. The iterator itself yields `Match` values. That is,
    /// once the iterator is constructed, the iteration itself will never
    /// report an error.
    ///
    /// # Errors
    ///
    /// This returns an error when this Aho-Corasick searcher does not support
    /// the given `Input` configuration or does not support overlapping
    /// searches.
    ///
    /// One example is that only Aho-Corasicker searchers built with
    /// [`MatchKind::Standard`] semantics support overlapping searches. Using
    /// any other match semantics will result in this returning an error.
    ///
    /// # Example: basic usage
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, Input, PatternID};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::new(patterns).unwrap();
    /// let matches: Vec<PatternID> = ac
    ///     .try_find_overlapping_iter(Input::new(haystack))?
    ///     .map(|mat| mat.pattern())
    ///     .collect();
    /// assert_eq!(vec![
    ///     PatternID::must(2),
    ///     PatternID::must(0),
    ///     PatternID::must(2),
    ///     PatternID::must(2),
    ///     PatternID::must(0),
    ///     PatternID::must(1),
    /// ], matches);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: anchored overlapping search returns an error
    ///
    /// It isn't clear what the match semantics for anchored overlapping
    /// iterators *ought* to be, so currently an error is returned. Callers
    /// may use [`AhoCorasick::try_find_overlapping`] to implement their own
    /// semantics if desired.
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, Anchored, Input, StartKind};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "appendappendage app";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .start_kind(StartKind::Anchored)
    ///     .build(patterns)
    ///     .unwrap();
    /// let input = Input::new(haystack).anchored(Anchored::Yes);
    /// assert!(ac.try_find_overlapping_iter(input).is_err());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn try_find_overlapping_iter<'a, 'h, I: Into<Input<'h>>>(
        &'a self,
        input: I,
    ) -> Result<FindOverlappingIter<'a, 'h>, MatchError> {
        let input = input.into();
        enforce_anchored_consistency(self.start_kind, input.get_anchored())?;
        Ok(FindOverlappingIter(self.aut.try_find_overlapping_iter(input)?))
    }

    /// Replace all matches with a corresponding value in the `replace_with`
    /// slice given. Matches correspond to the same matches as reported by
    /// [`AhoCorasick::try_find_iter`].
    ///
    /// Replacements are determined by the index of the matching pattern.
    /// For example, if the pattern with index `2` is found, then it is
    /// replaced by `replace_with[2]`.
    ///
    /// # Panics
    ///
    /// This panics when `replace_with.len()` does not equal
    /// [`AhoCorasick::patterns_len`].
    ///
    /// # Errors
    ///
    /// This returns an error when this Aho-Corasick searcher does not support
    /// the default `Input` configuration. More specifically, this occurs only
    /// when the Aho-Corasick searcher does not support unanchored searches
    /// since this replacement routine always does an unanchored search.
    ///
    /// # Example: basic usage
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    /// let result = ac.try_replace_all(haystack, &["x", "y", "z"])?;
    /// assert_eq!("x the z to the xage", result);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn try_replace_all<B>(
        &self,
        haystack: &str,
        replace_with: &[B],
    ) -> Result<String, MatchError>
    where
        B: AsRef<str>,
    {
        enforce_anchored_consistency(self.start_kind, Anchored::No)?;
        self.aut.try_replace_all(haystack, replace_with)
    }

    /// Replace all matches using raw bytes with a corresponding value in the
    /// `replace_with` slice given. Matches correspond to the same matches as
    /// reported by [`AhoCorasick::try_find_iter`].
    ///
    /// Replacements are determined by the index of the matching pattern.
    /// For example, if the pattern with index `2` is found, then it is
    /// replaced by `replace_with[2]`.
    ///
    /// This is the fallible version of [`AhoCorasick::replace_all_bytes`].
    ///
    /// # Panics
    ///
    /// This panics when `replace_with.len()` does not equal
    /// [`AhoCorasick::patterns_len`].
    ///
    /// # Errors
    ///
    /// This returns an error when this Aho-Corasick searcher does not support
    /// the default `Input` configuration. More specifically, this occurs only
    /// when the Aho-Corasick searcher does not support unanchored searches
    /// since this replacement routine always does an unanchored search.
    ///
    /// # Example: basic usage
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = b"append the app to the appendage";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    /// let result = ac.try_replace_all_bytes(haystack, &["x", "y", "z"])?;
    /// assert_eq!(b"x the z to the xage".to_vec(), result);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn try_replace_all_bytes<B>(
        &self,
        haystack: &[u8],
        replace_with: &[B],
    ) -> Result<Vec<u8>, MatchError>
    where
        B: AsRef<[u8]>,
    {
        enforce_anchored_consistency(self.start_kind, Anchored::No)?;
        self.aut.try_replace_all_bytes(haystack, replace_with)
    }

    /// Replace all matches using a closure called on each match.
    /// Matches correspond to the same matches as reported by
    /// [`AhoCorasick::try_find_iter`].
    ///
    /// The closure accepts three parameters: the match found, the text of
    /// the match and a string buffer with which to write the replaced text
    /// (if any). If the closure returns `true`, then it continues to the next
    /// match. If the closure returns `false`, then searching is stopped.
    ///
    /// Note that any matches with boundaries that don't fall on a valid UTF-8
    /// boundary are silently skipped.
    ///
    /// This is the fallible version of [`AhoCorasick::replace_all_with`].
    ///
    /// # Errors
    ///
    /// This returns an error when this Aho-Corasick searcher does not support
    /// the default `Input` configuration. More specifically, this occurs only
    /// when the Aho-Corasick searcher does not support unanchored searches
    /// since this replacement routine always does an unanchored search.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    /// let mut result = String::new();
    /// ac.try_replace_all_with(haystack, &mut result, |mat, _, dst| {
    ///     dst.push_str(&mat.pattern().as_usize().to_string());
    ///     true
    /// })?;
    /// assert_eq!("0 the 2 to the 0age", result);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Stopping the replacement by returning `false` (continued from the
    /// example above):
    ///
    /// ```
    /// # use aho_corasick::{AhoCorasick, MatchKind, PatternID};
    /// # let patterns = &["append", "appendage", "app"];
    /// # let haystack = "append the app to the appendage";
    /// # let ac = AhoCorasick::builder()
    /// #    .match_kind(MatchKind::LeftmostFirst)
    /// #    .build(patterns)
    /// #    .unwrap();
    /// let mut result = String::new();
    /// ac.try_replace_all_with(haystack, &mut result, |mat, _, dst| {
    ///     dst.push_str(&mat.pattern().as_usize().to_string());
    ///     mat.pattern() != PatternID::must(2)
    /// })?;
    /// assert_eq!("0 the 2 to the appendage", result);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn try_replace_all_with<F>(
        &self,
        haystack: &str,
        dst: &mut String,
        replace_with: F,
    ) -> Result<(), MatchError>
    where
        F: FnMut(&Match, &str, &mut String) -> bool,
    {
        enforce_anchored_consistency(self.start_kind, Anchored::No)?;
        self.aut.try_replace_all_with(haystack, dst, replace_with)
    }

    /// Replace all matches using raw bytes with a closure called on each
    /// match. Matches correspond to the same matches as reported by
    /// [`AhoCorasick::try_find_iter`].
    ///
    /// The closure accepts three parameters: the match found, the text of
    /// the match and a byte buffer with which to write the replaced text
    /// (if any). If the closure returns `true`, then it continues to the next
    /// match. If the closure returns `false`, then searching is stopped.
    ///
    /// This is the fallible version of
    /// [`AhoCorasick::replace_all_with_bytes`].
    ///
    /// # Errors
    ///
    /// This returns an error when this Aho-Corasick searcher does not support
    /// the default `Input` configuration. More specifically, this occurs only
    /// when the Aho-Corasick searcher does not support unanchored searches
    /// since this replacement routine always does an unanchored search.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = b"append the app to the appendage";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    /// let mut result = vec![];
    /// ac.try_replace_all_with_bytes(haystack, &mut result, |mat, _, dst| {
    ///     dst.extend(mat.pattern().as_usize().to_string().bytes());
    ///     true
    /// })?;
    /// assert_eq!(b"0 the 2 to the 0age".to_vec(), result);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Stopping the replacement by returning `false` (continued from the
    /// example above):
    ///
    /// ```
    /// # use aho_corasick::{AhoCorasick, MatchKind, PatternID};
    /// # let patterns = &["append", "appendage", "app"];
    /// # let haystack = b"append the app to the appendage";
    /// # let ac = AhoCorasick::builder()
    /// #    .match_kind(MatchKind::LeftmostFirst)
    /// #    .build(patterns)
    /// #    .unwrap();
    /// let mut result = vec![];
    /// ac.try_replace_all_with_bytes(haystack, &mut result, |mat, _, dst| {
    ///     dst.extend(mat.pattern().as_usize().to_string().bytes());
    ///     mat.pattern() != PatternID::must(2)
    /// })?;
    /// assert_eq!(b"0 the 2 to the appendage".to_vec(), result);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn try_replace_all_with_bytes<F>(
        &self,
        haystack: &[u8],
        dst: &mut Vec<u8>,
        replace_with: F,
    ) -> Result<(), MatchError>
    where
        F: FnMut(&Match, &[u8], &mut Vec<u8>) -> bool,
    {
        enforce_anchored_consistency(self.start_kind, Anchored::No)?;
        self.aut.try_replace_all_with_bytes(haystack, dst, replace_with)
    }

    /// Returns an iterator of non-overlapping matches in the given
    /// stream. Matches correspond to the same matches as reported by
    /// [`AhoCorasick::try_find_iter`].
    ///
    /// The matches yielded by this iterator use absolute position offsets in
    /// the stream given, where the first byte has index `0`. Matches are
    /// yieled until the stream is exhausted.
    ///
    /// Each item yielded by the iterator is an `Result<Match,
    /// std::io::Error>`, where an error is yielded if there was a problem
    /// reading from the reader given.
    ///
    /// When searching a stream, an internal buffer is used. Therefore, callers
    /// should avoiding providing a buffered reader, if possible.
    ///
    /// This is the fallible version of [`AhoCorasick::stream_find_iter`].
    /// Note that both methods return iterators that produce `Result` values.
    /// The difference is that this routine returns an error if _construction_
    /// of the iterator failed. The `Result` values yield by the iterator
    /// come from whether the given reader returns an error or not during the
    /// search.
    ///
    /// # Memory usage
    ///
    /// In general, searching streams will use a constant amount of memory for
    /// its internal buffer. The one requirement is that the internal buffer
    /// must be at least the size of the longest possible match. In most use
    /// cases, the default buffer size will be much larger than any individual
    /// match.
    ///
    /// # Errors
    ///
    /// This returns an error when this Aho-Corasick searcher does not support
    /// the default `Input` configuration. More specifically, this occurs only
    /// when the Aho-Corasick searcher does not support unanchored searches
    /// since this stream searching routine always does an unanchored search.
    ///
    /// This also returns an error if the searcher does not support stream
    /// searches. Only searchers built with [`MatchKind::Standard`] semantics
    /// support stream searches.
    ///
    /// # Example: basic usage
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, PatternID};
    ///
    /// let patterns = &["append", "appendage", "app"];
    /// let haystack = "append the app to the appendage";
    ///
    /// let ac = AhoCorasick::new(patterns).unwrap();
    /// let mut matches = vec![];
    /// for result in ac.try_stream_find_iter(haystack.as_bytes())? {
    ///     let mat = result?;
    ///     matches.push(mat.pattern());
    /// }
    /// assert_eq!(vec![
    ///     PatternID::must(2),
    ///     PatternID::must(2),
    ///     PatternID::must(2),
    /// ], matches);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "std")]
    pub fn try_stream_find_iter<'a, R: std::io::Read>(
        &'a self,
        rdr: R,
    ) -> Result<StreamFindIter<'a, R>, MatchError> {
        enforce_anchored_consistency(self.start_kind, Anchored::No)?;
        self.aut.try_stream_find_iter(rdr).map(StreamFindIter)
    }

    /// Search for and replace all matches of this automaton in
    /// the given reader, and write the replacements to the given
    /// writer. Matches correspond to the same matches as reported by
    /// [`AhoCorasick::try_find_iter`].
    ///
    /// Replacements are determined by the index of the matching pattern. For
    /// example, if the pattern with index `2` is found, then it is replaced by
    /// `replace_with[2]`.
    ///
    /// After all matches are replaced, the writer is _not_ flushed.
    ///
    /// If there was a problem reading from the given reader or writing to the
    /// given writer, then the corresponding `io::Error` is returned and all
    /// replacement is stopped.
    ///
    /// When searching a stream, an internal buffer is used. Therefore, callers
    /// should avoiding providing a buffered reader, if possible. However,
    /// callers may want to provide a buffered writer.
    ///
    /// Note that there is currently no infallible version of this routine.
    ///
    /// # Memory usage
    ///
    /// In general, searching streams will use a constant amount of memory for
    /// its internal buffer. The one requirement is that the internal buffer
    /// must be at least the size of the longest possible match. In most use
    /// cases, the default buffer size will be much larger than any individual
    /// match.
    ///
    /// # Panics
    ///
    /// This panics when `replace_with.len()` does not equal
    /// [`AhoCorasick::patterns_len`].
    ///
    /// # Errors
    ///
    /// This returns an error when this Aho-Corasick searcher does not support
    /// the default `Input` configuration. More specifically, this occurs only
    /// when the Aho-Corasick searcher does not support unanchored searches
    /// since this stream searching routine always does an unanchored search.
    ///
    /// This also returns an error if the searcher does not support stream
    /// searches. Only searchers built with [`MatchKind::Standard`] semantics
    /// support stream searches.
    ///
    /// # Example: basic usage
    ///
    /// ```
    /// use aho_corasick::AhoCorasick;
    ///
    /// let patterns = &["fox", "brown", "quick"];
    /// let haystack = "The quick brown fox.";
    /// let replace_with = &["sloth", "grey", "slow"];
    ///
    /// let ac = AhoCorasick::new(patterns).unwrap();
    /// let mut result = vec![];
    /// ac.try_stream_replace_all(
    ///     haystack.as_bytes(),
    ///     &mut result,
    ///     replace_with,
    /// )?;
    /// assert_eq!(b"The slow grey sloth.".to_vec(), result);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "std")]
    pub fn try_stream_replace_all<R, W, B>(
        &self,
        rdr: R,
        wtr: W,
        replace_with: &[B],
    ) -> Result<(), std::io::Error>
    where
        R: std::io::Read,
        W: std::io::Write,
        B: AsRef<[u8]>,
    {
        enforce_anchored_consistency(self.start_kind, Anchored::No)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        self.aut.try_stream_replace_all(rdr, wtr, replace_with)
    }

    /// Search the given reader and replace all matches of this automaton
    /// using the given closure. The result is written to the given
    /// writer. Matches correspond to the same matches as reported by
    /// [`AhoCorasick::try_find_iter`].
    ///
    /// The closure accepts three parameters: the match found, the text of
    /// the match and the writer with which to write the replaced text (if any).
    ///
    /// After all matches are replaced, the writer is _not_ flushed.
    ///
    /// If there was a problem reading from the given reader or writing to the
    /// given writer, then the corresponding `io::Error` is returned and all
    /// replacement is stopped.
    ///
    /// When searching a stream, an internal buffer is used. Therefore, callers
    /// should avoiding providing a buffered reader, if possible. However,
    /// callers may want to provide a buffered writer.
    ///
    /// Note that there is currently no infallible version of this routine.
    ///
    /// # Memory usage
    ///
    /// In general, searching streams will use a constant amount of memory for
    /// its internal buffer. The one requirement is that the internal buffer
    /// must be at least the size of the longest possible match. In most use
    /// cases, the default buffer size will be much larger than any individual
    /// match.
    ///
    /// # Errors
    ///
    /// This returns an error when this Aho-Corasick searcher does not support
    /// the default `Input` configuration. More specifically, this occurs only
    /// when the Aho-Corasick searcher does not support unanchored searches
    /// since this stream searching routine always does an unanchored search.
    ///
    /// This also returns an error if the searcher does not support stream
    /// searches. Only searchers built with [`MatchKind::Standard`] semantics
    /// support stream searches.
    ///
    /// # Example: basic usage
    ///
    /// ```
    /// use std::io::Write;
    /// use aho_corasick::AhoCorasick;
    ///
    /// let patterns = &["fox", "brown", "quick"];
    /// let haystack = "The quick brown fox.";
    ///
    /// let ac = AhoCorasick::new(patterns).unwrap();
    /// let mut result = vec![];
    /// ac.try_stream_replace_all_with(
    ///     haystack.as_bytes(),
    ///     &mut result,
    ///     |mat, _, wtr| {
    ///         wtr.write_all(mat.pattern().as_usize().to_string().as_bytes())
    ///     },
    /// )?;
    /// assert_eq!(b"The 2 1 0.".to_vec(), result);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "std")]
    pub fn try_stream_replace_all_with<R, W, F>(
        &self,
        rdr: R,
        wtr: W,
        replace_with: F,
    ) -> Result<(), std::io::Error>
    where
        R: std::io::Read,
        W: std::io::Write,
        F: FnMut(&Match, &[u8], &mut W) -> Result<(), std::io::Error>,
    {
        enforce_anchored_consistency(self.start_kind, Anchored::No)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        self.aut.try_stream_replace_all_with(rdr, wtr, replace_with)
    }
}

/// Routines for querying information about the Aho-Corasick automaton.
impl AhoCorasick {
    /// Returns the kind of the Aho-Corasick automaton used by this searcher.
    ///
    /// Knowing the Aho-Corasick kind is principally useful for diagnostic
    /// purposes. In particular, if no specific kind was given to
    /// [`AhoCorasickBuilder::kind`], then one is automatically chosen and
    /// this routine will report which one.
    ///
    /// Note that the heuristics used for choosing which `AhoCorasickKind`
    /// may be changed in a semver compatible release.
    ///
    /// # Examples
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, AhoCorasickKind};
    ///
    /// let ac = AhoCorasick::new(&["foo", "bar", "quux", "baz"]).unwrap();
    /// // The specific Aho-Corasick kind chosen is not guaranteed!
    /// assert_eq!(AhoCorasickKind::DFA, ac.kind());
    /// ```
    pub fn kind(&self) -> AhoCorasickKind {
        self.kind
    }

    /// Returns the type of starting search configuration supported by this
    /// Aho-Corasick automaton.
    ///
    /// # Examples
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, StartKind};
    ///
    /// let ac = AhoCorasick::new(&["foo", "bar", "quux", "baz"]).unwrap();
    /// assert_eq!(StartKind::Unanchored, ac.start_kind());
    /// ```
    pub fn start_kind(&self) -> StartKind {
        self.start_kind
    }

    /// Returns the match kind used by this automaton.
    ///
    /// The match kind is important because it determines what kinds of
    /// matches are returned. Also, some operations (such as overlapping
    /// search and stream searching) are only supported when using the
    /// [`MatchKind::Standard`] match kind.
    ///
    /// # Examples
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind};
    ///
    /// let ac = AhoCorasick::new(&["foo", "bar", "quux", "baz"]).unwrap();
    /// assert_eq!(MatchKind::Standard, ac.match_kind());
    /// ```
    pub fn match_kind(&self) -> MatchKind {
        self.aut.match_kind()
    }

    /// Returns the length of the shortest pattern matched by this automaton.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::AhoCorasick;
    ///
    /// let ac = AhoCorasick::new(&["foo", "bar", "quux", "baz"]).unwrap();
    /// assert_eq!(3, ac.min_pattern_len());
    /// ```
    ///
    /// Note that an `AhoCorasick` automaton has a minimum length of `0` if
    /// and only if it can match the empty string:
    ///
    /// ```
    /// use aho_corasick::AhoCorasick;
    ///
    /// let ac = AhoCorasick::new(&["foo", "", "quux", "baz"]).unwrap();
    /// assert_eq!(0, ac.min_pattern_len());
    /// ```
    pub fn min_pattern_len(&self) -> usize {
        self.aut.min_pattern_len()
    }

    /// Returns the length of the longest pattern matched by this automaton.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::AhoCorasick;
    ///
    /// let ac = AhoCorasick::new(&["foo", "bar", "quux", "baz"]).unwrap();
    /// assert_eq!(4, ac.max_pattern_len());
    /// ```
    pub fn max_pattern_len(&self) -> usize {
        self.aut.max_pattern_len()
    }

    /// Return the total number of patterns matched by this automaton.
    ///
    /// This includes patterns that may never participate in a match. For
    /// example, if [`MatchKind::LeftmostFirst`] match semantics are used, and
    /// the patterns `Sam` and `Samwise` were used to build the automaton (in
    /// that order), then `Samwise` can never participate in a match because
    /// `Sam` will always take priority.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::AhoCorasick;
    ///
    /// let ac = AhoCorasick::new(&["foo", "bar", "baz"]).unwrap();
    /// assert_eq!(3, ac.patterns_len());
    /// ```
    pub fn patterns_len(&self) -> usize {
        self.aut.patterns_len()
    }

    /// Returns the approximate total amount of heap used by this automaton, in
    /// units of bytes.
    ///
    /// # Examples
    ///
    /// This example shows the difference in heap usage between a few
    /// configurations:
    ///
    /// ```
    /// # if !cfg!(target_pointer_width = "64") { return; }
    /// use aho_corasick::{AhoCorasick, AhoCorasickKind, MatchKind};
    ///
    /// let ac = AhoCorasick::builder()
    ///     .kind(None) // default
    ///     .build(&["foobar", "bruce", "triskaidekaphobia", "springsteen"])
    ///     .unwrap();
    /// assert_eq!(5_632, ac.memory_usage());
    ///
    /// let ac = AhoCorasick::builder()
    ///     .kind(None) // default
    ///     .ascii_case_insensitive(true)
    ///     .build(&["foobar", "bruce", "triskaidekaphobia", "springsteen"])
    ///     .unwrap();
    /// assert_eq!(11_136, ac.memory_usage());
    ///
    /// let ac = AhoCorasick::builder()
    ///     .kind(Some(AhoCorasickKind::NoncontiguousNFA))
    ///     .ascii_case_insensitive(true)
    ///     .build(&["foobar", "bruce", "triskaidekaphobia", "springsteen"])
    ///     .unwrap();
    /// assert_eq!(10_879, ac.memory_usage());
    ///
    /// let ac = AhoCorasick::builder()
    ///     .kind(Some(AhoCorasickKind::ContiguousNFA))
    ///     .ascii_case_insensitive(true)
    ///     .build(&["foobar", "bruce", "triskaidekaphobia", "springsteen"])
    ///     .unwrap();
    /// assert_eq!(2_584, ac.memory_usage());
    ///
    /// let ac = AhoCorasick::builder()
    ///     .kind(Some(AhoCorasickKind::DFA))
    ///     .ascii_case_insensitive(true)
    ///     .build(&["foobar", "bruce", "triskaidekaphobia", "springsteen"])
    ///     .unwrap();
    /// // While this shows the DFA being the biggest here by a small margin,
    /// // don't let the difference fool you. With such a small number of
    /// // patterns, the difference is small, but a bigger number of patterns
    /// // will reveal that the rate of growth of the DFA is far bigger than
    /// // the NFAs above. For a large number of patterns, it is easy for the
    /// // DFA to take an order of magnitude more heap space (or more!).
    /// assert_eq!(11_136, ac.memory_usage());
    /// ```
    pub fn memory_usage(&self) -> usize {
        self.aut.memory_usage()
    }
}

// We provide a manual debug impl so that we don't include the 'start_kind',
// principally because it's kind of weird to do so and because it screws with
// the carefully curated debug output for the underlying automaton.
impl core::fmt::Debug for AhoCorasick {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_tuple("AhoCorasick").field(&self.aut).finish()
    }
}

/// An iterator of non-overlapping matches in a particular haystack.
///
/// This iterator yields matches according to the [`MatchKind`] used by this
/// automaton.
///
/// This iterator is constructed via the [`AhoCorasick::find_iter`] and
/// [`AhoCorasick::try_find_iter`] methods.
///
/// The lifetime `'a` refers to the lifetime of the `AhoCorasick` automaton.
///
/// The lifetime `'h` refers to the lifetime of the haystack being searched.
#[derive(Debug)]
pub struct FindIter<'a, 'h>(automaton::FindIter<'a, 'h, Arc<dyn AcAutomaton>>);

impl<'a, 'h> Iterator for FindIter<'a, 'h> {
    type Item = Match;

    #[inline]
    fn next(&mut self) -> Option<Match> {
        self.0.next()
    }
}

/// An iterator of overlapping matches in a particular haystack.
///
/// This iterator will report all possible matches in a particular haystack,
/// even when the matches overlap.
///
/// This iterator is constructed via the [`AhoCorasick::find_overlapping_iter`]
/// and [`AhoCorasick::try_find_overlapping_iter`] methods.
///
/// The lifetime `'a` refers to the lifetime of the `AhoCorasick` automaton.
///
/// The lifetime `'h` refers to the lifetime of the haystack being searched.
#[derive(Debug)]
pub struct FindOverlappingIter<'a, 'h>(
    automaton::FindOverlappingIter<'a, 'h, Arc<dyn AcAutomaton>>,
);

impl<'a, 'h> Iterator for FindOverlappingIter<'a, 'h> {
    type Item = Match;

    #[inline]
    fn next(&mut self) -> Option<Match> {
        self.0.next()
    }
}

/// An iterator that reports Aho-Corasick matches in a stream.
///
/// This iterator yields elements of type `Result<Match, std::io::Error>`,
/// where an error is reported if there was a problem reading from the
/// underlying stream. The iterator terminates only when the underlying stream
/// reaches `EOF`.
///
/// This iterator is constructed via the [`AhoCorasick::stream_find_iter`] and
/// [`AhoCorasick::try_stream_find_iter`] methods.
///
/// The type variable `R` refers to the `io::Read` stream that is being read
/// from.
///
/// The lifetime `'a` refers to the lifetime of the corresponding
/// [`AhoCorasick`] searcher.
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct StreamFindIter<'a, R>(
    automaton::StreamFindIter<'a, Arc<dyn AcAutomaton>, R>,
);

#[cfg(feature = "std")]
impl<'a, R: std::io::Read> Iterator for StreamFindIter<'a, R> {
    type Item = Result<Match, std::io::Error>;

    fn next(&mut self) -> Option<Result<Match, std::io::Error>> {
        self.0.next()
    }
}

/// A builder for configuring an Aho-Corasick automaton.
///
/// # Quick advice
///
/// * Use [`AhoCorasickBuilder::match_kind`] to configure your searcher
/// with [`MatchKind::LeftmostFirst`] if you want to match how backtracking
/// regex engines execute searches for `pat1|pat2|..|patN`. Use
/// [`MatchKind::LeftmostLongest`] if you want to match how POSIX regex engines
/// do it.
/// * If you need an anchored search, use [`AhoCorasickBuilder::start_kind`] to
/// set the [`StartKind::Anchored`] mode since [`StartKind::Unanchored`] is the
/// default. Or just use [`StartKind::Both`] to support both types of searches.
/// * You might want to use [`AhoCorasickBuilder::kind`] to set your searcher
/// to always use a [`AhoCorasickKind::DFA`] if search speed is critical and
/// memory usage isn't a concern. Otherwise, not setting a kind will probably
/// make the right choice for you. Beware that if you use [`StartKind::Both`]
/// to build a searcher that supports both unanchored and anchored searches
/// _and_ you set [`AhoCorasickKind::DFA`], then the DFA will essentially be
/// duplicated to support both simultaneously. This results in very high memory
/// usage.
/// * For all other options, their defaults are almost certainly what you want.
#[derive(Clone, Debug, Default)]
pub struct AhoCorasickBuilder {
    nfa_noncontiguous: noncontiguous::Builder,
    nfa_contiguous: contiguous::Builder,
    dfa: dfa::Builder,
    kind: Option<AhoCorasickKind>,
    start_kind: StartKind,
}

impl AhoCorasickBuilder {
    /// Create a new builder for configuring an Aho-Corasick automaton.
    ///
    /// The builder provides a way to configure a number of things, including
    /// ASCII case insensitivity and what kind of match semantics are used.
    pub fn new() -> AhoCorasickBuilder {
        AhoCorasickBuilder::default()
    }

    /// Build an Aho-Corasick automaton using the configuration set on this
    /// builder.
    ///
    /// A builder may be reused to create more automatons.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasickBuilder, PatternID};
    ///
    /// let patterns = &["foo", "bar", "baz"];
    /// let ac = AhoCorasickBuilder::new().build(patterns).unwrap();
    /// assert_eq!(
    ///     Some(PatternID::must(1)),
    ///     ac.find("xxx bar xxx").map(|m| m.pattern()),
    /// );
    /// ```
    pub fn build<I, P>(&self, patterns: I) -> Result<AhoCorasick, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        let nfa = self.nfa_noncontiguous.build(patterns)?;
        let (aut, kind): (Arc<dyn AcAutomaton>, AhoCorasickKind) =
            match self.kind {
                None => {
                    debug!(
                        "asked for automatic Aho-Corasick implementation, \
                     criteria: <patterns: {:?}, max pattern len: {:?}, \
                     start kind: {:?}>",
                        nfa.patterns_len(),
                        nfa.max_pattern_len(),
                        self.start_kind,
                    );
                    self.build_auto(nfa)
                }
                Some(AhoCorasickKind::NoncontiguousNFA) => {
                    debug!("forcefully chose noncontiguous NFA");
                    (Arc::new(nfa), AhoCorasickKind::NoncontiguousNFA)
                }
                Some(AhoCorasickKind::ContiguousNFA) => {
                    debug!("forcefully chose contiguous NFA");
                    let cnfa =
                        self.nfa_contiguous.build_from_noncontiguous(&nfa)?;
                    (Arc::new(cnfa), AhoCorasickKind::ContiguousNFA)
                }
                Some(AhoCorasickKind::DFA) => {
                    debug!("forcefully chose DFA");
                    let dfa = self.dfa.build_from_noncontiguous(&nfa)?;
                    (Arc::new(dfa), AhoCorasickKind::DFA)
                }
            };
        Ok(AhoCorasick { aut, kind, start_kind: self.start_kind })
    }

    /// Implements the automatic selection logic for the Aho-Corasick
    /// implementation to use. Since all Aho-Corasick automatons are built
    /// from a non-contiguous NFA, the caller is responsible for building
    /// that first.
    fn build_auto(
        &self,
        nfa: noncontiguous::NFA,
    ) -> (Arc<dyn AcAutomaton>, AhoCorasickKind) {
        // We try to build a DFA if we have a very small number of patterns,
        // otherwise the memory usage just gets too crazy. We also only do it
        // when the start kind is unanchored or anchored, but not both, because
        // both implies two full copies of the transition table.
        let try_dfa = !matches!(self.start_kind, StartKind::Both)
            && nfa.patterns_len() <= 100;
        if try_dfa {
            match self.dfa.build_from_noncontiguous(&nfa) {
                Ok(dfa) => {
                    debug!("chose a DFA");
                    return (Arc::new(dfa), AhoCorasickKind::DFA);
                }
                Err(_err) => {
                    debug!(
                        "failed to build DFA, trying something else: {}",
                        _err
                    );
                }
            }
        }
        // We basically always want a contiguous NFA if the limited
        // circumstances in which we use a DFA are not true. It is quite fast
        // and has excellent memory usage. The only way we don't use it is if
        // there are so many states that it can't fit in a contiguous NFA.
        // And the only way to know that is to try to build it. Building a
        // contiguous NFA is mostly just reshuffling data from a noncontiguous
        // NFA, so it isn't too expensive, especially relative to building a
        // noncontiguous NFA in the first place.
        match self.nfa_contiguous.build_from_noncontiguous(&nfa) {
            Ok(nfa) => {
                debug!("chose contiguous NFA");
                return (Arc::new(nfa), AhoCorasickKind::ContiguousNFA);
            }
            #[allow(unused_variables)] // unused when 'logging' is disabled
            Err(_err) => {
                debug!(
                    "failed to build contiguous NFA, \
                     trying something else: {}",
                    _err
                );
            }
        }
        debug!("chose non-contiguous NFA");
        (Arc::new(nfa), AhoCorasickKind::NoncontiguousNFA)
    }

    /// Set the desired match semantics.
    ///
    /// The default is [`MatchKind::Standard`], which corresponds to the match
    /// semantics supported by the standard textbook description of the
    /// Aho-Corasick algorithm. Namely, matches are reported as soon as they
    /// are found. Moreover, this is the only way to get overlapping matches
    /// or do stream searching.
    ///
    /// The other kinds of match semantics that are supported are
    /// [`MatchKind::LeftmostFirst`] and [`MatchKind::LeftmostLongest`]. The
    /// former corresponds to the match you would get if you were to try to
    /// match each pattern at each position in the haystack in the same order
    /// that you give to the automaton. That is, it returns the leftmost match
    /// corresponding to the earliest pattern given to the automaton. The
    /// latter corresponds to finding the longest possible match among all
    /// leftmost matches.
    ///
    /// For more details on match semantics, see the [documentation for
    /// `MatchKind`](MatchKind).
    ///
    /// Note that setting this to [`MatchKind::LeftmostFirst`] or
    /// [`MatchKind::LeftmostLongest`] will cause some search routines on
    /// [`AhoCorasick`] to return an error (or panic if you're using the
    /// infallible API). Notably, this includes stream and overlapping
    /// searches.
    ///
    /// # Examples
    ///
    /// In these examples, we demonstrate the differences between match
    /// semantics for a particular set of patterns in a specific order:
    /// `b`, `abc`, `abcd`.
    ///
    /// Standard semantics:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind};
    ///
    /// let patterns = &["b", "abc", "abcd"];
    /// let haystack = "abcd";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::Standard) // default, not necessary
    ///     .build(patterns)
    ///     .unwrap();
    /// let mat = ac.find(haystack).expect("should have a match");
    /// assert_eq!("b", &haystack[mat.start()..mat.end()]);
    /// ```
    ///
    /// Leftmost-first semantics:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind};
    ///
    /// let patterns = &["b", "abc", "abcd"];
    /// let haystack = "abcd";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .build(patterns)
    ///     .unwrap();
    /// let mat = ac.find(haystack).expect("should have a match");
    /// assert_eq!("abc", &haystack[mat.start()..mat.end()]);
    /// ```
    ///
    /// Leftmost-longest semantics:
    ///
    /// ```
    /// use aho_corasick::{AhoCorasick, MatchKind};
    ///
    /// let patterns = &["b", "abc", "abcd"];
    /// let haystack = "abcd";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostLongest)
    ///     .build(patterns)
    ///     .unwrap();
    /// let mat = ac.find(haystack).expect("should have a match");
    /// assert_eq!("abcd", &haystack[mat.start()..mat.end()]);
    /// ```
    pub fn match_kind(&mut self, kind: MatchKind) -> &mut AhoCorasickBuilder {
        self.nfa_noncontiguous.match_kind(kind);
        self.nfa_contiguous.match_kind(kind);
        self.dfa.match_kind(kind);
        self
    }

    /// Sets the starting state configuration for the automaton.
    ///
    /// Every Aho-Corasick automaton is capable of having two start states: one
    /// that is used for unanchored searches and one that is used for anchored
    /// searches. Some automatons, like the NFAs, support this with almost zero
    /// additional cost. Other automatons, like the DFA, require two copies of
    /// the underlying transition table to support both simultaneously.
    ///
    /// Because there may be an added non-trivial cost to supporting both, it
    /// is possible to configure which starting state configuration is needed.
    ///
    /// Indeed, since anchored searches tend to be somewhat more rare,
    /// _only_ unanchored searches are supported by default. Thus,
    /// [`StartKind::Unanchored`] is the default.
    ///
    /// Note that when this is set to [`StartKind::Unanchored`], then
    /// running an anchored search will result in an error (or a panic
    /// if using the infallible APIs). Similarly, when this is set to
    /// [`StartKind::Anchored`], then running an unanchored search will
    /// result in an error (or a panic if using the infallible APIs). When
    /// [`StartKind::Both`] is used, then both unanchored and anchored searches
    /// are always supported.
    ///
    /// Also note that even if an `AhoCorasick` searcher is using an NFA
    /// internally (which always supports both unanchored and anchored
    /// searches), an error will still be reported for a search that isn't
    /// supported by the configuration set via this method. This means,
    /// for example, that an error is never dependent on which internal
    /// implementation of Aho-Corasick is used.
    ///
    /// # Example: anchored search
    ///
    /// This shows how to build a searcher that only supports anchored
    /// searches:
    ///
    /// ```
    /// use aho_corasick::{
    ///     AhoCorasick, Anchored, Input, Match, MatchKind, StartKind,
    /// };
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .start_kind(StartKind::Anchored)
    ///     .build(&["b", "abc", "abcd"])
    ///     .unwrap();
    ///
    /// // An unanchored search is not supported! An error here is guaranteed
    /// // given the configuration above regardless of which kind of
    /// // Aho-Corasick implementation ends up being used internally.
    /// let input = Input::new("foo abcd").anchored(Anchored::No);
    /// assert!(ac.try_find(input).is_err());
    ///
    /// let input = Input::new("foo abcd").anchored(Anchored::Yes);
    /// assert_eq!(None, ac.try_find(input)?);
    ///
    /// let input = Input::new("abcd").anchored(Anchored::Yes);
    /// assert_eq!(Some(Match::must(1, 0..3)), ac.try_find(input)?);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: unanchored and anchored searches
    ///
    /// This shows how to build a searcher that supports both unanchored and
    /// anchored searches:
    ///
    /// ```
    /// use aho_corasick::{
    ///     AhoCorasick, Anchored, Input, Match, MatchKind, StartKind,
    /// };
    ///
    /// let ac = AhoCorasick::builder()
    ///     .match_kind(MatchKind::LeftmostFirst)
    ///     .start_kind(StartKind::Both)
    ///     .build(&["b", "abc", "abcd"])
    ///     .unwrap();
    ///
    /// let input = Input::new("foo abcd").anchored(Anchored::No);
    /// assert_eq!(Some(Match::must(1, 4..7)), ac.try_find(input)?);
    ///
    /// let input = Input::new("foo abcd").anchored(Anchored::Yes);
    /// assert_eq!(None, ac.try_find(input)?);
    ///
    /// let input = Input::new("abcd").anchored(Anchored::Yes);
    /// assert_eq!(Some(Match::must(1, 0..3)), ac.try_find(input)?);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn start_kind(&mut self, kind: StartKind) -> &mut AhoCorasickBuilder {
        self.dfa.start_kind(kind);
        self.start_kind = kind;
        self
    }

    /// Enable ASCII-aware case insensitive matching.
    ///
    /// When this option is enabled, searching will be performed without
    /// respect to case for ASCII letters (`a-z` and `A-Z`) only.
    ///
    /// Enabling this option does not change the search algorithm, but it may
    /// increase the size of the automaton.
    ///
    /// **NOTE:** It is unlikely that support for Unicode case folding will
    /// be added in the future. The ASCII case works via a simple hack to the
    /// underlying automaton, but full Unicode handling requires a fair bit of
    /// sophistication. If you do need Unicode handling, you might consider
    /// using the [`regex` crate](https://docs.rs/regex) or the lower level
    /// [`regex-automata` crate](https://docs.rs/regex-automata).
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use aho_corasick::AhoCorasick;
    ///
    /// let patterns = &["FOO", "bAr", "BaZ"];
    /// let haystack = "foo bar baz";
    ///
    /// let ac = AhoCorasick::builder()
    ///     .ascii_case_insensitive(true)
    ///     .build(patterns)
    ///     .unwrap();
    /// assert_eq!(3, ac.find_iter(haystack).count());
    /// ```
    pub fn ascii_case_insensitive(
        &mut self,
        yes: bool,
    ) -> &mut AhoCorasickBuilder {
        self.nfa_noncontiguous.ascii_case_insensitive(yes);
        self.nfa_contiguous.ascii_case_insensitive(yes);
        self.dfa.ascii_case_insensitive(yes);
        self
    }

    /// Choose the type of underlying automaton to use.
    ///
    /// Currently, there are four choices:
    ///
    /// * [`AhoCorasickKind::NoncontiguousNFA`] instructs the searcher to
    /// use a [`noncontiguous::NFA`]. A noncontiguous NFA is the fastest to
    /// be built, has moderate memory usage and is typically the slowest to
    /// execute a search.
    /// * [`AhoCorasickKind::ContiguousNFA`] instructs the searcher to use a
    /// [`contiguous::NFA`]. A contiguous NFA is a little slower to build than
    /// a noncontiguous NFA, has excellent memory usage and is typically a
    /// little slower than a DFA for a search.
    /// * [`AhoCorasickKind::DFA`] instructs the searcher to use a
    /// [`dfa::DFA`]. A DFA is very slow to build, uses exorbitant amounts of
    /// memory, but will typically execute searches the fastest.
    /// * `None` (the default) instructs the searcher to choose the "best"
    /// Aho-Corasick implementation. This choice is typically based primarily
    /// on the number of patterns.
    ///
    /// Setting this configuration does not change the time complexity for
    /// constructing the Aho-Corasick automaton (which is `O(p)` where `p`
    /// is the total number of patterns being compiled). Setting this to
    /// [`AhoCorasickKind::DFA`] does however reduce the time complexity of
    /// non-overlapping searches from `O(n + p)` to `O(n)`, where `n` is the
    /// length of the haystack.
    ///
    /// In general, you should probably stick to the default unless you have
    /// some kind of reason to use a specific Aho-Corasick implementation. For
    /// example, you might choose `AhoCorasickKind::DFA` if you don't care
    /// about memory usage and want the fastest possible search times.
    ///
    /// Setting this guarantees that the searcher returned uses the chosen
    /// implementation. If that implementation could not be constructed, then
    /// an error will be returned. In contrast, when `None` is used, it is
    /// possible for it to attempt to construct, for example, a contiguous
    /// NFA and have it fail. In which case, it will fall back to using a
    /// noncontiguous NFA.
    ///
    /// If `None` is given, then one may use [`AhoCorasick::kind`] to determine
    /// which Aho-Corasick implementation was chosen.
    ///
    /// Note that the heuristics used for choosing which `AhoCorasickKind`
    /// may be changed in a semver compatible release.
    pub fn kind(
        &mut self,
        kind: Option<AhoCorasickKind>,
    ) -> &mut AhoCorasickBuilder {
        self.kind = kind;
        self
    }

    /// Enable heuristic prefilter optimizations.
    ///
    /// When enabled, searching will attempt to quickly skip to match
    /// candidates using specialized literal search routines. A prefilter
    /// cannot always be used, and is generally treated as a heuristic. It
    /// can be useful to disable this if the prefilter is observed to be
    /// sub-optimal for a particular workload.
    ///
    /// Currently, prefilters are typically only active when building searchers
    /// with a small (less than 100) number of patterns.
    ///
    /// This is enabled by default.
    pub fn prefilter(&mut self, yes: bool) -> &mut AhoCorasickBuilder {
        self.nfa_noncontiguous.prefilter(yes);
        self.nfa_contiguous.prefilter(yes);
        self.dfa.prefilter(yes);
        self
    }

    /// Set the limit on how many states use a dense representation for their
    /// transitions. Other states will generally use a sparse representation.
    ///
    /// A dense representation uses more memory but is generally faster, since
    /// the next transition in a dense representation can be computed in a
    /// constant number of instructions. A sparse representation uses less
    /// memory but is generally slower, since the next transition in a sparse
    /// representation requires executing a variable number of instructions.
    ///
    /// This setting is only used when an Aho-Corasick implementation is used
    /// that supports the dense versus sparse representation trade off. Not all
    /// do.
    ///
    /// This limit is expressed in terms of the depth of a state, i.e., the
    /// number of transitions from the starting state of the automaton. The
    /// idea is that most of the time searching will be spent near the starting
    /// state of the automaton, so states near the start state should use a
    /// dense representation. States further away from the start state would
    /// then use a sparse representation.
    ///
    /// By default, this is set to a low but non-zero number. Setting this to
    /// `0` is almost never what you want, since it is likely to make searches
    /// very slow due to the start state itself being forced to use a sparse
    /// representation. However, it is unlikely that increasing this number
    /// will help things much, since the most active states have a small depth.
    /// More to the point, the memory usage increases superlinearly as this
    /// number increases.
    pub fn dense_depth(&mut self, depth: usize) -> &mut AhoCorasickBuilder {
        self.nfa_noncontiguous.dense_depth(depth);
        self.nfa_contiguous.dense_depth(depth);
        self
    }

    /// A debug settting for whether to attempt to shrink the size of the
    /// automaton's alphabet or not.
    ///
    /// This option is enabled by default and should never be disabled unless
    /// one is debugging the underlying automaton.
    ///
    /// When enabled, some (but not all) Aho-Corasick automatons will use a map
    /// from all possible bytes to their corresponding equivalence class. Each
    /// equivalence class represents a set of bytes that does not discriminate
    /// between a match and a non-match in the automaton.
    ///
    /// The advantage of this map is that the size of the transition table can
    /// be reduced drastically from `#states * 256 * sizeof(u32)` to
    /// `#states * k * sizeof(u32)` where `k` is the number of equivalence
    /// classes (rounded up to the nearest power of 2). As a result, total
    /// space usage can decrease substantially. Moreover, since a smaller
    /// alphabet is used, automaton compilation becomes faster as well.
    ///
    /// **WARNING:** This is only useful for debugging automatons. Disabling
    /// this does not yield any speed advantages. Namely, even when this is
    /// disabled, a byte class map is still used while searching. The only
    /// difference is that every byte will be forced into its own distinct
    /// equivalence class. This is useful for debugging the actual generated
    /// transitions because it lets one see the transitions defined on actual
    /// bytes instead of the equivalence classes.
    pub fn byte_classes(&mut self, yes: bool) -> &mut AhoCorasickBuilder {
        self.nfa_contiguous.byte_classes(yes);
        self.dfa.byte_classes(yes);
        self
    }
}

/// The type of Aho-Corasick implementation to use in an [`AhoCorasick`]
/// searcher.
///
/// This is principally used as an input to the
/// [`AhoCorasickBuilder::start_kind`] method. Its documentation goes into more
/// detail about each choice.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AhoCorasickKind {
    /// Use a noncontiguous NFA.
    NoncontiguousNFA,
    /// Use a contiguous NFA.
    ContiguousNFA,
    /// Use a DFA. Warning: DFAs typically use a large amount of memory.
    DFA,
}

/// A trait that effectively gives us practical dynamic dispatch over anything
/// that impls `Automaton`, but without needing to add a bunch of bounds to
/// the core `Automaton` trait. Basically, we provide all of the marker traits
/// that our automatons have, in addition to `Debug` impls and requiring that
/// there is no borrowed data. Without these, the main `AhoCorasick` type would
/// not be able to meaningfully impl `Debug` or the marker traits without also
/// requiring that all impls of `Automaton` do so, which would be not great.
trait AcAutomaton:
    Automaton + Debug + Send + Sync + UnwindSafe + RefUnwindSafe + 'static
{
}

impl<A> AcAutomaton for A where
    A: Automaton + Debug + Send + Sync + UnwindSafe + RefUnwindSafe + 'static
{
}

impl crate::automaton::private::Sealed for Arc<dyn AcAutomaton> {}

// I'm not sure why this trait impl shows up in the docs, as the AcAutomaton
// trait is not exported. So we forcefully hide it.
//
// SAFETY: This just defers to the underlying 'AcAutomaton' and thus inherits
// its safety properties.
#[doc(hidden)]
unsafe impl Automaton for Arc<dyn AcAutomaton> {
    #[inline(always)]
    fn start_state(&self, anchored: Anchored) -> Result<StateID, MatchError> {
        (**self).start_state(anchored)
    }

    #[inline(always)]
    fn next_state(
        &self,
        anchored: Anchored,
        sid: StateID,
        byte: u8,
    ) -> StateID {
        (**self).next_state(anchored, sid, byte)
    }

    #[inline(always)]
    fn is_special(&self, sid: StateID) -> bool {
        (**self).is_special(sid)
    }

    #[inline(always)]
    fn is_dead(&self, sid: StateID) -> bool {
        (**self).is_dead(sid)
    }

    #[inline(always)]
    fn is_match(&self, sid: StateID) -> bool {
        (**self).is_match(sid)
    }

    #[inline(always)]
    fn is_start(&self, sid: StateID) -> bool {
        (**self).is_start(sid)
    }

    #[inline(always)]
    fn match_kind(&self) -> MatchKind {
        (**self).match_kind()
    }

    #[inline(always)]
    fn match_len(&self, sid: StateID) -> usize {
        (**self).match_len(sid)
    }

    #[inline(always)]
    fn match_pattern(&self, sid: StateID, index: usize) -> PatternID {
        (**self).match_pattern(sid, index)
    }

    #[inline(always)]
    fn patterns_len(&self) -> usize {
        (**self).patterns_len()
    }

    #[inline(always)]
    fn pattern_len(&self, pid: PatternID) -> usize {
        (**self).pattern_len(pid)
    }

    #[inline(always)]
    fn min_pattern_len(&self) -> usize {
        (**self).min_pattern_len()
    }

    #[inline(always)]
    fn max_pattern_len(&self) -> usize {
        (**self).max_pattern_len()
    }

    #[inline(always)]
    fn memory_usage(&self) -> usize {
        (**self).memory_usage()
    }

    #[inline(always)]
    fn prefilter(&self) -> Option<&Prefilter> {
        (**self).prefilter()
    }

    // Even though 'try_find' and 'try_find_overlapping' each have their
    // own default impls, we explicitly define them here to fix a perf bug.
    // Without these explicit definitions, the default impl will wind up using
    // dynamic dispatch for all 'Automaton' method calls, including things like
    // 'next_state' that absolutely must get inlined or else perf is trashed.
    // Defining them explicitly here like this still requires dynamic dispatch
    // to call 'try_find' itself, but all uses of 'Automaton' within 'try_find'
    // are monomorphized.
    //
    // We don't need to explicitly impl any other methods, I think, because
    // they are all implemented themselves in terms of 'try_find' and
    // 'try_find_overlapping'. We still might wind up with an extra virtual
    // call here or there, but that's okay since it's outside of any perf
    // critical areas.

    #[inline(always)]
    fn try_find(
        &self,
        input: &Input<'_>,
    ) -> Result<Option<Match>, MatchError> {
        (**self).try_find(input)
    }

    #[inline(always)]
    fn try_find_overlapping(
        &self,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        (**self).try_find_overlapping(input, state)
    }
}

/// Returns an error if the start state configuration does not support the
/// desired search configuration. See the internal 'AhoCorasick::start_kind'
/// field docs for more details.
fn enforce_anchored_consistency(
    have: StartKind,
    want: Anchored,
) -> Result<(), MatchError> {
    match have {
        StartKind::Both => Ok(()),
        StartKind::Unanchored if !want.is_anchored() => Ok(()),
        StartKind::Unanchored => Err(MatchError::invalid_input_anchored()),
        StartKind::Anchored if want.is_anchored() => Ok(()),
        StartKind::Anchored => Err(MatchError::invalid_input_unanchored()),
    }
}
