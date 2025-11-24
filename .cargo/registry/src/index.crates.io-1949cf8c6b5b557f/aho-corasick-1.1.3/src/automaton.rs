/*!
Provides [`Automaton`] trait for abstracting over Aho-Corasick automata.

The `Automaton` trait provides a way to write generic code over any
Aho-Corasick automaton. It also provides access to lower level APIs that
permit walking the state transitions of an Aho-Corasick automaton manually.
*/

use alloc::{string::String, vec::Vec};

use crate::util::{
    error::MatchError,
    primitives::PatternID,
    search::{Anchored, Input, Match, MatchKind, Span},
};

pub use crate::util::{
    prefilter::{Candidate, Prefilter},
    primitives::{StateID, StateIDError},
};

/// We seal the `Automaton` trait for now. It's a big trait, and it's
/// conceivable that I might want to add new required methods, and sealing the
/// trait permits doing that in a backwards compatible fashion. On other the
/// hand, if you have a solid use case for implementing the trait yourself,
/// please file an issue and we can discuss it. This was *mostly* done as a
/// conservative step.
pub(crate) mod private {
    pub trait Sealed {}
}
impl private::Sealed for crate::nfa::noncontiguous::NFA {}
impl private::Sealed for crate::nfa::contiguous::NFA {}
impl private::Sealed for crate::dfa::DFA {}

impl<'a, T: private::Sealed + ?Sized> private::Sealed for &'a T {}

/// A trait that abstracts over Aho-Corasick automata.
///
/// This trait primarily exists for niche use cases such as:
///
/// * Using an NFA or DFA directly, bypassing the top-level
/// [`AhoCorasick`](crate::AhoCorasick) searcher. Currently, these include
/// [`noncontiguous::NFA`](crate::nfa::noncontiguous::NFA),
/// [`contiguous::NFA`](crate::nfa::contiguous::NFA) and
/// [`dfa::DFA`](crate::dfa::DFA).
/// * Implementing your own custom search routine by walking the automaton
/// yourself. This might be useful for implementing search on non-contiguous
/// strings or streams.
///
/// For most use cases, it is not expected that users will need
/// to use or even know about this trait. Indeed, the top level
/// [`AhoCorasick`](crate::AhoCorasick) searcher does not expose any details
/// about this trait, nor does it implement it itself.
///
/// Note that this trait defines a number of default methods, such as
/// [`Automaton::try_find`] and [`Automaton::try_find_iter`], which implement
/// higher level search routines in terms of the lower level automata API.
///
/// # Sealed
///
/// Currently, this trait is sealed. That means users of this crate can write
/// generic routines over this trait but cannot implement it themselves. This
/// restriction may be lifted in the future, but sealing the trait permits
/// adding new required methods in a backwards compatible fashion.
///
/// # Special states
///
/// This trait encodes a notion of "special" states in an automaton. Namely,
/// a state is treated as special if it is a dead, match or start state:
///
/// * A dead state is a state that cannot be left once entered. All transitions
/// on a dead state lead back to itself. The dead state is meant to be treated
/// as a sentinel indicating that the search should stop and return a match if
/// one has been found, and nothing otherwise.
/// * A match state is a state that indicates one or more patterns have
/// matched. Depending on the [`MatchKind`] of the automaton, a search may
/// stop once a match is seen, or it may continue looking for matches until
/// it enters a dead state or sees the end of the haystack.
/// * A start state is a state that a search begins in. It is useful to know
/// when a search enters a start state because it may mean that a prefilter can
/// be used to skip ahead and quickly look for candidate matches. Unlike dead
/// and match states, it is never necessary to explicitly handle start states
/// for correctness. Indeed, in this crate, implementations of `Automaton`
/// will only treat start states as "special" when a prefilter is enabled and
/// active. Otherwise, treating it as special has no purpose and winds up
/// slowing down the overall search because it results in ping-ponging between
/// the main state transition and the "special" state logic.
///
/// Since checking whether a state is special by doing three different
/// checks would be too expensive inside a fast search loop, the
/// [`Automaton::is_special`] method is provided for quickly checking whether
/// the state is special. The `Automaton::is_dead`, `Automaton::is_match` and
/// `Automaton::is_start` predicates can then be used to determine which kind
/// of special state it is.
///
/// # Panics
///
/// Most of the APIs on this trait should panic or give incorrect results
/// if invalid inputs are given to it. For example, `Automaton::next_state`
/// has unspecified behavior if the state ID given to it is not a valid
/// state ID for the underlying automaton. Valid state IDs can only be
/// retrieved in one of two ways: calling `Automaton::start_state` or calling
/// `Automaton::next_state` with a valid state ID.
///
/// # Safety
///
/// This trait is not safe to implement so that code may rely on the
/// correctness of implementations of this trait to avoid undefined behavior.
/// The primary correctness guarantees are:
///
/// * `Automaton::start_state` always returns a valid state ID or an error or
/// panics.
/// * `Automaton::next_state`, when given a valid state ID, always returns
/// a valid state ID for all values of `anchored` and `byte`, or otherwise
/// panics.
///
/// In general, the rest of the methods on `Automaton` need to uphold their
/// contracts as well. For example, `Automaton::is_dead` should only returns
/// true if the given state ID is actually a dead state.
///
/// Note that currently this crate does not rely on the safety property defined
/// here to avoid undefined behavior. Instead, this was done to make it
/// _possible_ to do in the future.
///
/// # Example
///
/// This example shows how one might implement a basic but correct search
/// routine. We keep things simple by not using prefilters or worrying about
/// anchored searches, but do make sure our search is correct for all possible
/// [`MatchKind`] semantics. (The comments in the code below note the parts
/// that are needed to support certain `MatchKind` semantics.)
///
/// ```
/// use aho_corasick::{
///     automaton::Automaton,
///     nfa::noncontiguous::NFA,
///     Anchored, Match, MatchError, MatchKind,
/// };
///
/// // Run an unanchored search for 'aut' in 'haystack'. Return the first match
/// // seen according to the automaton's match semantics. This returns an error
/// // if the given automaton does not support unanchored searches.
/// fn find<A: Automaton>(
///     aut: A,
///     haystack: &[u8],
/// ) -> Result<Option<Match>, MatchError> {
///     let mut sid = aut.start_state(Anchored::No)?;
///     let mut at = 0;
///     let mut mat = None;
///     let get_match = |sid, at| {
///         let pid = aut.match_pattern(sid, 0);
///         let len = aut.pattern_len(pid);
///         Match::new(pid, (at - len)..at)
///     };
///     // Start states can be match states!
///     if aut.is_match(sid) {
///         mat = Some(get_match(sid, at));
///         // Standard semantics require matches to be reported as soon as
///         // they're seen. Otherwise, we continue until we see a dead state
///         // or the end of the haystack.
///         if matches!(aut.match_kind(), MatchKind::Standard) {
///             return Ok(mat);
///         }
///     }
///     while at < haystack.len() {
///         sid = aut.next_state(Anchored::No, sid, haystack[at]);
///         if aut.is_special(sid) {
///             if aut.is_dead(sid) {
///                 return Ok(mat);
///             } else if aut.is_match(sid) {
///                 mat = Some(get_match(sid, at + 1));
///                 // As above, standard semantics require that we return
///                 // immediately once a match is found.
///                 if matches!(aut.match_kind(), MatchKind::Standard) {
///                     return Ok(mat);
///                 }
///             }
///         }
///         at += 1;
///     }
///     Ok(mat)
/// }
///
/// // Show that it works for standard searches.
/// let nfa = NFA::new(&["samwise", "sam"]).unwrap();
/// assert_eq!(Some(Match::must(1, 0..3)), find(&nfa, b"samwise")?);
///
/// // But also works when using leftmost-first. Notice how the match result
/// // has changed!
/// let nfa = NFA::builder()
///     .match_kind(MatchKind::LeftmostFirst)
///     .build(&["samwise", "sam"])
///     .unwrap();
/// assert_eq!(Some(Match::must(0, 0..7)), find(&nfa, b"samwise")?);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub unsafe trait Automaton: private::Sealed {
    /// Returns the starting state for the given anchor mode.
    ///
    /// Upon success, the state ID returned is guaranteed to be valid for
    /// this automaton.
    ///
    /// # Errors
    ///
    /// This returns an error when the given search configuration is not
    /// supported by the underlying automaton. For example, if the underlying
    /// automaton only supports unanchored searches but the given configuration
    /// was set to an anchored search, then this must return an error.
    fn start_state(&self, anchored: Anchored) -> Result<StateID, MatchError>;

    /// Performs a state transition from `sid` for `byte` and returns the next
    /// state.
    ///
    /// `anchored` should be [`Anchored::Yes`] when executing an anchored
    /// search and [`Anchored::No`] otherwise. For some implementations of
    /// `Automaton`, it is required to know whether the search is anchored
    /// or not in order to avoid following failure transitions. Other
    /// implementations may ignore `anchored` altogether and depend on
    /// `Automaton::start_state` returning a state that walks a different path
    /// through the automaton depending on whether the search is anchored or
    /// not.
    ///
    /// # Panics
    ///
    /// This routine may panic or return incorrect results when the given state
    /// ID is invalid. A state ID is valid if and only if:
    ///
    /// 1. It came from a call to `Automaton::start_state`, or
    /// 2. It came from a previous call to `Automaton::next_state` with a
    /// valid state ID.
    ///
    /// Implementations must treat all possible values of `byte` as valid.
    ///
    /// Implementations may panic on unsupported values of `anchored`, but are
    /// not required to do so.
    fn next_state(
        &self,
        anchored: Anchored,
        sid: StateID,
        byte: u8,
    ) -> StateID;

    /// Returns true if the given ID represents a "special" state. A special
    /// state is a dead, match or start state.
    ///
    /// Note that implementations may choose to return false when the given ID
    /// corresponds to a start state. Namely, it always correct to treat start
    /// states as non-special. Implementations must return true for states that
    /// are dead or contain matches.
    ///
    /// This has unspecified behavior when given an invalid state ID.
    fn is_special(&self, sid: StateID) -> bool;

    /// Returns true if the given ID represents a dead state.
    ///
    /// A dead state is a type of "sink" in a finite state machine. It
    /// corresponds to a state whose transitions all loop back to itself. That
    /// is, once entered, it can never be left. In practice, it serves as a
    /// sentinel indicating that the search should terminate.
    ///
    /// This has unspecified behavior when given an invalid state ID.
    fn is_dead(&self, sid: StateID) -> bool;

    /// Returns true if the given ID represents a match state.
    ///
    /// A match state is always associated with one or more pattern IDs that
    /// matched at the position in the haystack when the match state was
    /// entered. When a match state is entered, the match semantics dictate
    /// whether it should be returned immediately (for `MatchKind::Standard`)
    /// or if the search should continue (for `MatchKind::LeftmostFirst` and
    /// `MatchKind::LeftmostLongest`) until a dead state is seen or the end of
    /// the haystack has been reached.
    ///
    /// This has unspecified behavior when given an invalid state ID.
    fn is_match(&self, sid: StateID) -> bool;

    /// Returns true if the given ID represents a start state.
    ///
    /// While it is never incorrect to ignore start states during a search
    /// (except for the start of the search of course), knowing whether one has
    /// entered a start state can be useful for certain classes of performance
    /// optimizations. For example, if one is in a start state, it may be legal
    /// to try to skip ahead and look for match candidates more quickly than
    /// would otherwise be accomplished by walking the automaton.
    ///
    /// Implementations of `Automaton` in this crate "unspecialize" start
    /// states when a prefilter is not active or enabled. In this case, it
    /// is possible for `Automaton::is_special(sid)` to return false while
    /// `Automaton::is_start(sid)` returns true.
    ///
    /// This has unspecified behavior when given an invalid state ID.
    fn is_start(&self, sid: StateID) -> bool;

    /// Returns the match semantics that this automaton was built with.
    fn match_kind(&self) -> MatchKind;

    /// Returns the total number of matches for the given state ID.
    ///
    /// This has unspecified behavior if the given ID does not refer to a match
    /// state.
    fn match_len(&self, sid: StateID) -> usize;

    /// Returns the pattern ID for the match state given by `sid` at the
    /// `index` given.
    ///
    /// Typically, `index` is only ever greater than `0` when implementing an
    /// overlapping search. Otherwise, it's likely that your search only cares
    /// about reporting the first pattern ID in a match state.
    ///
    /// This has unspecified behavior if the given ID does not refer to a match
    /// state, or if the index is greater than or equal to the total number of
    /// matches in this match state.
    fn match_pattern(&self, sid: StateID, index: usize) -> PatternID;

    /// Returns the total number of patterns compiled into this automaton.
    fn patterns_len(&self) -> usize;

    /// Returns the length of the pattern for the given ID.
    ///
    /// This has unspecified behavior when given an invalid pattern
    /// ID. A pattern ID is valid if and only if it is less than
    /// `Automaton::patterns_len`.
    fn pattern_len(&self, pid: PatternID) -> usize;

    /// Returns the length, in bytes, of the shortest pattern in this
    /// automaton.
    fn min_pattern_len(&self) -> usize;

    /// Returns the length, in bytes, of the longest pattern in this automaton.
    fn max_pattern_len(&self) -> usize;

    /// Returns the heap memory usage, in bytes, used by this automaton.
    fn memory_usage(&self) -> usize;

    /// Returns a prefilter, if available, that can be used to accelerate
    /// searches for this automaton.
    ///
    /// The typical way this is used is when the start state is entered during
    /// a search. When that happens, one can use a prefilter to skip ahead and
    /// look for candidate matches without having to walk the automaton on the
    /// bytes between candidates.
    ///
    /// Typically a prefilter is only available when there are a small (<100)
    /// number of patterns built into the automaton.
    fn prefilter(&self) -> Option<&Prefilter>;

    /// Executes a non-overlapping search with this automaton using the given
    /// configuration.
    ///
    /// See
    /// [`AhoCorasick::try_find`](crate::AhoCorasick::try_find)
    /// for more documentation and examples.
    fn try_find(
        &self,
        input: &Input<'_>,
    ) -> Result<Option<Match>, MatchError> {
        try_find_fwd(&self, input)
    }

    /// Executes a overlapping search with this automaton using the given
    /// configuration.
    ///
    /// See
    /// [`AhoCorasick::try_find_overlapping`](crate::AhoCorasick::try_find_overlapping)
    /// for more documentation and examples.
    fn try_find_overlapping(
        &self,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        try_find_overlapping_fwd(&self, input, state)
    }

    /// Returns an iterator of non-overlapping matches with this automaton
    /// using the given configuration.
    ///
    /// See
    /// [`AhoCorasick::try_find_iter`](crate::AhoCorasick::try_find_iter)
    /// for more documentation and examples.
    fn try_find_iter<'a, 'h>(
        &'a self,
        input: Input<'h>,
    ) -> Result<FindIter<'a, 'h, Self>, MatchError>
    where
        Self: Sized,
    {
        FindIter::new(self, input)
    }

    /// Returns an iterator of overlapping matches with this automaton
    /// using the given configuration.
    ///
    /// See
    /// [`AhoCorasick::try_find_overlapping_iter`](crate::AhoCorasick::try_find_overlapping_iter)
    /// for more documentation and examples.
    fn try_find_overlapping_iter<'a, 'h>(
        &'a self,
        input: Input<'h>,
    ) -> Result<FindOverlappingIter<'a, 'h, Self>, MatchError>
    where
        Self: Sized,
    {
        if !self.match_kind().is_standard() {
            return Err(MatchError::unsupported_overlapping(
                self.match_kind(),
            ));
        }
        //  We might consider lifting this restriction. The reason why I added
        // it was to ban the combination of "anchored search" and "overlapping
        // iteration." The match semantics aren't totally clear in that case.
        // Should we allow *any* matches that are adjacent to *any* previous
        // match? Or only following the most recent one? Or only matches
        // that start at the beginning of the search? We might also elect to
        // just keep this restriction in place, as callers should be able to
        // implement it themselves if they want to.
        if input.get_anchored().is_anchored() {
            return Err(MatchError::invalid_input_anchored());
        }
        let _ = self.start_state(input.get_anchored())?;
        let state = OverlappingState::start();
        Ok(FindOverlappingIter { aut: self, input, state })
    }

    /// Replaces all non-overlapping matches in `haystack` with
    /// strings from `replace_with` depending on the pattern that
    /// matched. The `replace_with` slice must have length equal to
    /// `Automaton::patterns_len`.
    ///
    /// See
    /// [`AhoCorasick::try_replace_all`](crate::AhoCorasick::try_replace_all)
    /// for more documentation and examples.
    fn try_replace_all<B>(
        &self,
        haystack: &str,
        replace_with: &[B],
    ) -> Result<String, MatchError>
    where
        Self: Sized,
        B: AsRef<str>,
    {
        assert_eq!(
            replace_with.len(),
            self.patterns_len(),
            "replace_all requires a replacement for every pattern \
             in the automaton"
        );
        let mut dst = String::with_capacity(haystack.len());
        self.try_replace_all_with(haystack, &mut dst, |mat, _, dst| {
            dst.push_str(replace_with[mat.pattern()].as_ref());
            true
        })?;
        Ok(dst)
    }

    /// Replaces all non-overlapping matches in `haystack` with
    /// strings from `replace_with` depending on the pattern that
    /// matched. The `replace_with` slice must have length equal to
    /// `Automaton::patterns_len`.
    ///
    /// See
    /// [`AhoCorasick::try_replace_all_bytes`](crate::AhoCorasick::try_replace_all_bytes)
    /// for more documentation and examples.
    fn try_replace_all_bytes<B>(
        &self,
        haystack: &[u8],
        replace_with: &[B],
    ) -> Result<Vec<u8>, MatchError>
    where
        Self: Sized,
        B: AsRef<[u8]>,
    {
        assert_eq!(
            replace_with.len(),
            self.patterns_len(),
            "replace_all requires a replacement for every pattern \
             in the automaton"
        );
        let mut dst = Vec::with_capacity(haystack.len());
        self.try_replace_all_with_bytes(haystack, &mut dst, |mat, _, dst| {
            dst.extend(replace_with[mat.pattern()].as_ref());
            true
        })?;
        Ok(dst)
    }

    /// Replaces all non-overlapping matches in `haystack` by calling the
    /// `replace_with` closure given.
    ///
    /// See
    /// [`AhoCorasick::try_replace_all_with`](crate::AhoCorasick::try_replace_all_with)
    /// for more documentation and examples.
    fn try_replace_all_with<F>(
        &self,
        haystack: &str,
        dst: &mut String,
        mut replace_with: F,
    ) -> Result<(), MatchError>
    where
        Self: Sized,
        F: FnMut(&Match, &str, &mut String) -> bool,
    {
        let mut last_match = 0;
        for m in self.try_find_iter(Input::new(haystack))? {
            // Since there are no restrictions on what kinds of patterns are
            // in an Aho-Corasick automaton, we might get matches that split
            // a codepoint, or even matches of a partial codepoint. When that
            // happens, we just skip the match.
            if !haystack.is_char_boundary(m.start())
                || !haystack.is_char_boundary(m.end())
            {
                continue;
            }
            dst.push_str(&haystack[last_match..m.start()]);
            last_match = m.end();
            if !replace_with(&m, &haystack[m.start()..m.end()], dst) {
                break;
            };
        }
        dst.push_str(&haystack[last_match..]);
        Ok(())
    }

    /// Replaces all non-overlapping matches in `haystack` by calling the
    /// `replace_with` closure given.
    ///
    /// See
    /// [`AhoCorasick::try_replace_all_with_bytes`](crate::AhoCorasick::try_replace_all_with_bytes)
    /// for more documentation and examples.
    fn try_replace_all_with_bytes<F>(
        &self,
        haystack: &[u8],
        dst: &mut Vec<u8>,
        mut replace_with: F,
    ) -> Result<(), MatchError>
    where
        Self: Sized,
        F: FnMut(&Match, &[u8], &mut Vec<u8>) -> bool,
    {
        let mut last_match = 0;
        for m in self.try_find_iter(Input::new(haystack))? {
            dst.extend(&haystack[last_match..m.start()]);
            last_match = m.end();
            if !replace_with(&m, &haystack[m.start()..m.end()], dst) {
                break;
            };
        }
        dst.extend(&haystack[last_match..]);
        Ok(())
    }

    /// Returns an iterator of non-overlapping matches with this automaton
    /// from the stream given.
    ///
    /// See
    /// [`AhoCorasick::try_stream_find_iter`](crate::AhoCorasick::try_stream_find_iter)
    /// for more documentation and examples.
    #[cfg(feature = "std")]
    fn try_stream_find_iter<'a, R: std::io::Read>(
        &'a self,
        rdr: R,
    ) -> Result<StreamFindIter<'a, Self, R>, MatchError>
    where
        Self: Sized,
    {
        Ok(StreamFindIter { it: StreamChunkIter::new(self, rdr)? })
    }

    /// Replaces all non-overlapping matches in `rdr` with strings from
    /// `replace_with` depending on the pattern that matched, and writes the
    /// result to `wtr`. The `replace_with` slice must have length equal to
    /// `Automaton::patterns_len`.
    ///
    /// See
    /// [`AhoCorasick::try_stream_replace_all`](crate::AhoCorasick::try_stream_replace_all)
    /// for more documentation and examples.
    #[cfg(feature = "std")]
    fn try_stream_replace_all<R, W, B>(
        &self,
        rdr: R,
        wtr: W,
        replace_with: &[B],
    ) -> std::io::Result<()>
    where
        Self: Sized,
        R: std::io::Read,
        W: std::io::Write,
        B: AsRef<[u8]>,
    {
        assert_eq!(
            replace_with.len(),
            self.patterns_len(),
            "streaming replace_all requires a replacement for every pattern \
             in the automaton",
        );
        self.try_stream_replace_all_with(rdr, wtr, |mat, _, wtr| {
            wtr.write_all(replace_with[mat.pattern()].as_ref())
        })
    }

    /// Replaces all non-overlapping matches in `rdr` by calling the
    /// `replace_with` closure given and writing the result to `wtr`.
    ///
    /// See
    /// [`AhoCorasick::try_stream_replace_all_with`](crate::AhoCorasick::try_stream_replace_all_with)
    /// for more documentation and examples.
    #[cfg(feature = "std")]
    fn try_stream_replace_all_with<R, W, F>(
        &self,
        rdr: R,
        mut wtr: W,
        mut replace_with: F,
    ) -> std::io::Result<()>
    where
        Self: Sized,
        R: std::io::Read,
        W: std::io::Write,
        F: FnMut(&Match, &[u8], &mut W) -> std::io::Result<()>,
    {
        let mut it = StreamChunkIter::new(self, rdr).map_err(|e| {
            let kind = std::io::ErrorKind::Other;
            std::io::Error::new(kind, e)
        })?;
        while let Some(result) = it.next() {
            let chunk = result?;
            match chunk {
                StreamChunk::NonMatch { bytes, .. } => {
                    wtr.write_all(bytes)?;
                }
                StreamChunk::Match { bytes, mat } => {
                    replace_with(&mat, bytes, &mut wtr)?;
                }
            }
        }
        Ok(())
    }
}

// SAFETY: This just defers to the underlying 'AcAutomaton' and thus inherits
// its safety properties.
unsafe impl<'a, A: Automaton + ?Sized> Automaton for &'a A {
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
}

/// Represents the current state of an overlapping search.
///
/// This is used for overlapping searches since they need to know something
/// about the previous search. For example, when multiple patterns match at the
/// same position, this state tracks the last reported pattern so that the next
/// search knows whether to report another matching pattern or continue with
/// the search at the next position. Additionally, it also tracks which state
/// the last search call terminated in and the current offset of the search
/// in the haystack.
///
/// This type provides limited introspection capabilities. The only thing a
/// caller can do is construct it and pass it around to permit search routines
/// to use it to track state, and to ask whether a match has been found.
///
/// Callers should always provide a fresh state constructed via
/// [`OverlappingState::start`] when starting a new search. That same state
/// should be reused for subsequent searches on the same `Input`. The state
/// given will advance through the haystack itself. Callers can detect the end
/// of a search when neither an error nor a match is returned.
///
/// # Example
///
/// This example shows how to manually iterate over all overlapping matches. If
/// you need this, you might consider using
/// [`AhoCorasick::find_overlapping_iter`](crate::AhoCorasick::find_overlapping_iter)
/// instead, but this shows how to correctly use an `OverlappingState`.
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
///     ac.find_overlapping(haystack, &mut state);
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
/// ```
#[derive(Clone, Debug)]
pub struct OverlappingState {
    /// The match reported by the most recent overlapping search to use this
    /// state.
    ///
    /// If a search does not find any matches, then it is expected to clear
    /// this value.
    mat: Option<Match>,
    /// The state ID of the state at which the search was in when the call
    /// terminated. When this is a match state, `last_match` must be set to a
    /// non-None value.
    ///
    /// A `None` value indicates the start state of the corresponding
    /// automaton. We cannot use the actual ID, since any one automaton may
    /// have many start states, and which one is in use depends on search-time
    /// factors (such as whether the search is anchored or not).
    id: Option<StateID>,
    /// The position of the search.
    ///
    /// When `id` is None (i.e., we are starting a search), this is set to
    /// the beginning of the search as given by the caller regardless of its
    /// current value. Subsequent calls to an overlapping search pick up at
    /// this offset.
    at: usize,
    /// The index into the matching patterns of the next match to report if the
    /// current state is a match state. Note that this may be 1 greater than
    /// the total number of matches to report for the current match state. (In
    /// which case, no more matches should be reported at the current position
    /// and the search should advance to the next position.)
    next_match_index: Option<usize>,
}

impl OverlappingState {
    /// Create a new overlapping state that begins at the start state.
    pub fn start() -> OverlappingState {
        OverlappingState { mat: None, id: None, at: 0, next_match_index: None }
    }

    /// Return the match result of the most recent search to execute with this
    /// state.
    ///
    /// Every search will clear this result automatically, such that if no
    /// match is found, this will always correctly report `None`.
    pub fn get_match(&self) -> Option<Match> {
        self.mat
    }
}

/// An iterator of non-overlapping matches in a particular haystack.
///
/// This iterator yields matches according to the [`MatchKind`] used by this
/// automaton.
///
/// This iterator is constructed via the [`Automaton::try_find_iter`] method.
///
/// The type variable `A` refers to the implementation of the [`Automaton`]
/// trait used to execute the search.
///
/// The lifetime `'a` refers to the lifetime of the [`Automaton`]
/// implementation.
///
/// The lifetime `'h` refers to the lifetime of the haystack being searched.
#[derive(Debug)]
pub struct FindIter<'a, 'h, A> {
    /// The automaton used to drive the search.
    aut: &'a A,
    /// The input parameters to give to each search call.
    ///
    /// The start position of the search is mutated during iteration.
    input: Input<'h>,
    /// Records the end offset of the most recent match. This is necessary to
    /// handle a corner case for preventing empty matches from overlapping with
    /// the ending bounds of a prior match.
    last_match_end: Option<usize>,
}

impl<'a, 'h, A: Automaton> FindIter<'a, 'h, A> {
    /// Creates a new non-overlapping iterator. If the given automaton would
    /// return an error on a search with the given input configuration, then
    /// that error is returned here.
    fn new(
        aut: &'a A,
        input: Input<'h>,
    ) -> Result<FindIter<'a, 'h, A>, MatchError> {
        // The only way this search can fail is if we cannot retrieve the start
        // state. e.g., Asking for an anchored search when only unanchored
        // searches are supported.
        let _ = aut.start_state(input.get_anchored())?;
        Ok(FindIter { aut, input, last_match_end: None })
    }

    /// Executes a search and returns a match if one is found.
    ///
    /// This does not advance the input forward. It just executes a search
    /// based on the current configuration/offsets.
    fn search(&self) -> Option<Match> {
        // The unwrap is OK here because we check at iterator construction time
        // that no subsequent search call (using the same configuration) will
        // ever return an error.
        self.aut
            .try_find(&self.input)
            .expect("already checked that no match error can occur")
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
    /// the presiding behavior of most general purpose regex engines.
    /// (Obviously this crate isn't a regex engine, but we choose to match
    /// their semantics.) The "intuitive sense" here is that we want to report
    /// NON-overlapping matches. So for example, given the patterns 'a' and
    /// '' (an empty string) against the haystack 'a', without the special
    /// handling, you'd get the matches [0, 1) and [1, 1), where the latter
    /// overlaps with the end bounds of the former.
    ///
    /// Note that we mark this cold and forcefully prevent inlining because
    /// handling empty matches like this is extremely rare and does require
    /// quite a bit of code, comparatively. Keeping this code out of the main
    /// iterator function keeps it smaller and more amenable to inlining
    /// itself.
    #[cold]
    #[inline(never)]
    fn handle_overlapping_empty_match(
        &mut self,
        mut m: Match,
    ) -> Option<Match> {
        assert!(m.is_empty());
        if Some(m.end()) == self.last_match_end {
            self.input.set_start(self.input.start().checked_add(1).unwrap());
            m = self.search()?;
        }
        Some(m)
    }
}

impl<'a, 'h, A: Automaton> Iterator for FindIter<'a, 'h, A> {
    type Item = Match;

    #[inline(always)]
    fn next(&mut self) -> Option<Match> {
        let mut m = self.search()?;
        if m.is_empty() {
            m = self.handle_overlapping_empty_match(m)?;
        }
        self.input.set_start(m.end());
        self.last_match_end = Some(m.end());
        Some(m)
    }
}

/// An iterator of overlapping matches in a particular haystack.
///
/// This iterator will report all possible matches in a particular haystack,
/// even when the matches overlap.
///
/// This iterator is constructed via the
/// [`Automaton::try_find_overlapping_iter`] method.
///
/// The type variable `A` refers to the implementation of the [`Automaton`]
/// trait used to execute the search.
///
/// The lifetime `'a` refers to the lifetime of the [`Automaton`]
/// implementation.
///
/// The lifetime `'h` refers to the lifetime of the haystack being searched.
#[derive(Debug)]
pub struct FindOverlappingIter<'a, 'h, A> {
    aut: &'a A,
    input: Input<'h>,
    state: OverlappingState,
}

impl<'a, 'h, A: Automaton> Iterator for FindOverlappingIter<'a, 'h, A> {
    type Item = Match;

    #[inline(always)]
    fn next(&mut self) -> Option<Match> {
        self.aut
            .try_find_overlapping(&self.input, &mut self.state)
            .expect("already checked that no match error can occur here");
        self.state.get_match()
    }
}

/// An iterator that reports matches in a stream.
///
/// This iterator yields elements of type `io::Result<Match>`, where an error
/// is reported if there was a problem reading from the underlying stream.
/// The iterator terminates only when the underlying stream reaches `EOF`.
///
/// This iterator is constructed via the [`Automaton::try_stream_find_iter`]
/// method.
///
/// The type variable `A` refers to the implementation of the [`Automaton`]
/// trait used to execute the search.
///
/// The type variable `R` refers to the `io::Read` stream that is being read
/// from.
///
/// The lifetime `'a` refers to the lifetime of the [`Automaton`]
/// implementation.
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct StreamFindIter<'a, A, R> {
    it: StreamChunkIter<'a, A, R>,
}

#[cfg(feature = "std")]
impl<'a, A: Automaton, R: std::io::Read> Iterator
    for StreamFindIter<'a, A, R>
{
    type Item = std::io::Result<Match>;

    fn next(&mut self) -> Option<std::io::Result<Match>> {
        loop {
            match self.it.next() {
                None => return None,
                Some(Err(err)) => return Some(Err(err)),
                Some(Ok(StreamChunk::NonMatch { .. })) => {}
                Some(Ok(StreamChunk::Match { mat, .. })) => {
                    return Some(Ok(mat));
                }
            }
        }
    }
}

/// An iterator that reports matches in a stream.
///
/// (This doesn't actually implement the `Iterator` trait because it returns
/// something with a lifetime attached to a buffer it owns, but that's OK. It
/// still has a `next` method and is iterator-like enough to be fine.)
///
/// This iterator yields elements of type `io::Result<StreamChunk>`, where
/// an error is reported if there was a problem reading from the underlying
/// stream. The iterator terminates only when the underlying stream reaches
/// `EOF`.
///
/// The idea here is that each chunk represents either a match or a non-match,
/// and if you concatenated all of the chunks together, you'd reproduce the
/// entire contents of the stream, byte-for-byte.
///
/// This chunk machinery is a bit complicated and it isn't strictly required
/// for a stream searcher that just reports matches. But we do need something
/// like this to deal with the "replacement" API, which needs to know which
/// chunks it can copy and which it needs to replace.
#[cfg(feature = "std")]
#[derive(Debug)]
struct StreamChunkIter<'a, A, R> {
    /// The underlying automaton to do the search.
    aut: &'a A,
    /// The source of bytes we read from.
    rdr: R,
    /// A roll buffer for managing bytes from `rdr`. Basically, this is used
    /// to handle the case of a match that is split by two different
    /// calls to `rdr.read()`. This isn't strictly needed if all we needed to
    /// do was report matches, but here we are reporting chunks of non-matches
    /// and matches and in order to do that, we really just cannot treat our
    /// stream as non-overlapping blocks of bytes. We need to permit some
    /// overlap while we retain bytes from a previous `read` call in memory.
    buf: crate::util::buffer::Buffer,
    /// The unanchored starting state of this automaton.
    start: StateID,
    /// The state of the automaton.
    sid: StateID,
    /// The absolute position over the entire stream.
    absolute_pos: usize,
    /// The position we're currently at within `buf`.
    buffer_pos: usize,
    /// The buffer position of the end of the bytes that we last returned
    /// to the caller. Basically, whenever we find a match, we look to see if
    /// there is a difference between where the match started and the position
    /// of the last byte we returned to the caller. If there's a difference,
    /// then we need to return a 'NonMatch' chunk.
    buffer_reported_pos: usize,
}

#[cfg(feature = "std")]
impl<'a, A: Automaton, R: std::io::Read> StreamChunkIter<'a, A, R> {
    fn new(
        aut: &'a A,
        rdr: R,
    ) -> Result<StreamChunkIter<'a, A, R>, MatchError> {
        // This restriction is a carry-over from older versions of this crate.
        // I didn't have the bandwidth to think through how to handle, say,
        // leftmost-first or leftmost-longest matching, but... it should be
        // possible? The main problem is that once you see a match state in
        // leftmost-first semantics, you can't just stop at that point and
        // report a match. You have to keep going until you either hit a dead
        // state or EOF. So how do you know when you'll hit a dead state? Well,
        // you don't. With Aho-Corasick, I believe you can put a bound on it
        // and say, "once a match has been seen, you'll need to scan forward at
        // most N bytes" where N=aut.max_pattern_len().
        //
        // Which is fine, but it does mean that state about whether we're still
        // looking for a dead state or not needs to persist across buffer
        // refills. Which this code doesn't really handle. It does preserve
        // *some* state across buffer refills, basically ensuring that a match
        // span is always in memory.
        if !aut.match_kind().is_standard() {
            return Err(MatchError::unsupported_stream(aut.match_kind()));
        }
        // This is kind of a cop-out, but empty matches are SUPER annoying.
        // If we know they can't happen (which is what we enforce here), then
        // it makes a lot of logic much simpler. With that said, I'm open to
        // supporting this case, but we need to define proper semantics for it
        // first. It wasn't totally clear to me what it should do at the time
        // of writing, so I decided to just be conservative.
        //
        // It also seems like a very weird case to support anyway. Why search a
        // stream if you're just going to get a match at every position?
        //
        // ¯\_(ツ)_/¯
        if aut.min_pattern_len() == 0 {
            return Err(MatchError::unsupported_empty());
        }
        let start = aut.start_state(Anchored::No)?;
        Ok(StreamChunkIter {
            aut,
            rdr,
            buf: crate::util::buffer::Buffer::new(aut.max_pattern_len()),
            start,
            sid: start,
            absolute_pos: 0,
            buffer_pos: 0,
            buffer_reported_pos: 0,
        })
    }

    fn next(&mut self) -> Option<std::io::Result<StreamChunk>> {
        // This code is pretty gnarly. It IS simpler than the equivalent code
        // in the previous aho-corasick release, in part because we inline
        // automaton traversal here and also in part because we have abdicated
        // support for automatons that contain an empty pattern.
        //
        // I suspect this code could be made a bit simpler by designing a
        // better buffer abstraction.
        //
        // But in general, this code is basically write-only. So you'll need
        // to go through it step-by-step to grok it. One of the key bits of
        // complexity is tracking a few different offsets. 'buffer_pos' is
        // where we are in the buffer for search. 'buffer_reported_pos' is the
        // position immediately following the last byte in the buffer that
        // we've returned to the caller. And 'absolute_pos' is the overall
        // current absolute position of the search in the entire stream, and
        // this is what match spans are reported in terms of.
        loop {
            if self.aut.is_match(self.sid) {
                let mat = self.get_match();
                if let Some(r) = self.get_non_match_chunk(mat) {
                    self.buffer_reported_pos += r.len();
                    let bytes = &self.buf.buffer()[r];
                    return Some(Ok(StreamChunk::NonMatch { bytes }));
                }
                self.sid = self.start;
                let r = self.get_match_chunk(mat);
                self.buffer_reported_pos += r.len();
                let bytes = &self.buf.buffer()[r];
                return Some(Ok(StreamChunk::Match { bytes, mat }));
            }
            if self.buffer_pos >= self.buf.buffer().len() {
                if let Some(r) = self.get_pre_roll_non_match_chunk() {
                    self.buffer_reported_pos += r.len();
                    let bytes = &self.buf.buffer()[r];
                    return Some(Ok(StreamChunk::NonMatch { bytes }));
                }
                if self.buf.buffer().len() >= self.buf.min_buffer_len() {
                    self.buffer_pos = self.buf.min_buffer_len();
                    self.buffer_reported_pos -=
                        self.buf.buffer().len() - self.buf.min_buffer_len();
                    self.buf.roll();
                }
                match self.buf.fill(&mut self.rdr) {
                    Err(err) => return Some(Err(err)),
                    Ok(true) => {}
                    Ok(false) => {
                        // We've hit EOF, but if there are still some
                        // unreported bytes remaining, return them now.
                        if let Some(r) = self.get_eof_non_match_chunk() {
                            self.buffer_reported_pos += r.len();
                            let bytes = &self.buf.buffer()[r];
                            return Some(Ok(StreamChunk::NonMatch { bytes }));
                        }
                        // We've reported everything!
                        return None;
                    }
                }
            }
            let start = self.absolute_pos;
            for &byte in self.buf.buffer()[self.buffer_pos..].iter() {
                self.sid = self.aut.next_state(Anchored::No, self.sid, byte);
                self.absolute_pos += 1;
                if self.aut.is_match(self.sid) {
                    break;
                }
            }
            self.buffer_pos += self.absolute_pos - start;
        }
    }

    /// Return a match chunk for the given match. It is assumed that the match
    /// ends at the current `buffer_pos`.
    fn get_match_chunk(&self, mat: Match) -> core::ops::Range<usize> {
        let start = self.buffer_pos - mat.len();
        let end = self.buffer_pos;
        start..end
    }

    /// Return a non-match chunk, if necessary, just before reporting a match.
    /// This returns `None` if there is nothing to report. Otherwise, this
    /// assumes that the given match ends at the current `buffer_pos`.
    fn get_non_match_chunk(
        &self,
        mat: Match,
    ) -> Option<core::ops::Range<usize>> {
        let buffer_mat_start = self.buffer_pos - mat.len();
        if buffer_mat_start > self.buffer_reported_pos {
            let start = self.buffer_reported_pos;
            let end = buffer_mat_start;
            return Some(start..end);
        }
        None
    }

    /// Look for any bytes that should be reported as a non-match just before
    /// rolling the buffer.
    ///
    /// Note that this only reports bytes up to `buffer.len() -
    /// min_buffer_len`, as it's not possible to know whether the bytes
    /// following that will participate in a match or not.
    fn get_pre_roll_non_match_chunk(&self) -> Option<core::ops::Range<usize>> {
        let end =
            self.buf.buffer().len().saturating_sub(self.buf.min_buffer_len());
        if self.buffer_reported_pos < end {
            return Some(self.buffer_reported_pos..end);
        }
        None
    }

    /// Return any unreported bytes as a non-match up to the end of the buffer.
    ///
    /// This should only be called when the entire contents of the buffer have
    /// been searched and EOF has been hit when trying to fill the buffer.
    fn get_eof_non_match_chunk(&self) -> Option<core::ops::Range<usize>> {
        if self.buffer_reported_pos < self.buf.buffer().len() {
            return Some(self.buffer_reported_pos..self.buf.buffer().len());
        }
        None
    }

    /// Return the match at the current position for the current state.
    ///
    /// This panics if `self.aut.is_match(self.sid)` isn't true.
    fn get_match(&self) -> Match {
        get_match(self.aut, self.sid, 0, self.absolute_pos)
    }
}

/// A single chunk yielded by the stream chunk iterator.
///
/// The `'r` lifetime refers to the lifetime of the stream chunk iterator.
#[cfg(feature = "std")]
#[derive(Debug)]
enum StreamChunk<'r> {
    /// A chunk that does not contain any matches.
    NonMatch { bytes: &'r [u8] },
    /// A chunk that precisely contains a match.
    Match { bytes: &'r [u8], mat: Match },
}

#[inline(never)]
pub(crate) fn try_find_fwd<A: Automaton + ?Sized>(
    aut: &A,
    input: &Input<'_>,
) -> Result<Option<Match>, MatchError> {
    if input.is_done() {
        return Ok(None);
    }
    let earliest = aut.match_kind().is_standard() || input.get_earliest();
    if input.get_anchored().is_anchored() {
        try_find_fwd_imp(aut, input, None, Anchored::Yes, earliest)
    } else if let Some(pre) = aut.prefilter() {
        if earliest {
            try_find_fwd_imp(aut, input, Some(pre), Anchored::No, true)
        } else {
            try_find_fwd_imp(aut, input, Some(pre), Anchored::No, false)
        }
    } else {
        if earliest {
            try_find_fwd_imp(aut, input, None, Anchored::No, true)
        } else {
            try_find_fwd_imp(aut, input, None, Anchored::No, false)
        }
    }
}

#[inline(always)]
fn try_find_fwd_imp<A: Automaton + ?Sized>(
    aut: &A,
    input: &Input<'_>,
    pre: Option<&Prefilter>,
    anchored: Anchored,
    earliest: bool,
) -> Result<Option<Match>, MatchError> {
    let mut sid = aut.start_state(input.get_anchored())?;
    let mut at = input.start();
    let mut mat = None;
    if aut.is_match(sid) {
        mat = Some(get_match(aut, sid, 0, at));
        if earliest {
            return Ok(mat);
        }
    }
    if let Some(pre) = pre {
        match pre.find_in(input.haystack(), input.get_span()) {
            Candidate::None => return Ok(None),
            Candidate::Match(m) => return Ok(Some(m)),
            Candidate::PossibleStartOfMatch(i) => {
                at = i;
            }
        }
    }
    while at < input.end() {
        // I've tried unrolling this loop and eliding bounds checks, but no
        // matter what I did, I could not observe a consistent improvement on
        // any benchmark I could devise. (If someone wants to re-litigate this,
        // the way to do it is to add an 'next_state_unchecked' method to the
        // 'Automaton' trait with a default impl that uses 'next_state'. Then
        // use 'aut.next_state_unchecked' here and implement it on DFA using
        // unchecked slice index acces.)
        sid = aut.next_state(anchored, sid, input.haystack()[at]);
        if aut.is_special(sid) {
            if aut.is_dead(sid) {
                return Ok(mat);
            } else if aut.is_match(sid) {
                // We use 'at + 1' here because the match state is entered
                // at the last byte of the pattern. Since we use half-open
                // intervals, the end of the range of the match is one past the
                // last byte.
                let m = get_match(aut, sid, 0, at + 1);
                // For the automata in this crate, we make a size trade off
                // where we reuse the same automaton for both anchored and
                // unanchored searches. We achieve this, principally, by simply
                // not following failure transitions while computing the next
                // state. Instead, if we fail to find the next state, we return
                // a dead state, which instructs the search to stop. (This
                // is why 'next_state' needs to know whether the search is
                // anchored or not.) In addition, we have different start
                // states for anchored and unanchored searches. The latter has
                // a self-loop where as the former does not.
                //
                // In this way, we can use the same trie to execute both
                // anchored and unanchored searches. There is a catch though.
                // When building an Aho-Corasick automaton for unanchored
                // searches, we copy matches from match states to other states
                // (which would otherwise not be match states) if they are
                // reachable via a failure transition. In the case of an
                // anchored search, we *specifically* do not want to report
                // these matches because they represent matches that start past
                // the beginning of the search.
                //
                // Now we could tweak the automaton somehow to differentiate
                // anchored from unanchored match states, but this would make
                // 'aut.is_match' and potentially 'aut.is_special' slower. And
                // also make the automaton itself more complex.
                //
                // Instead, we insert a special hack: if the search is
                // anchored, we simply ignore matches that don't begin at
                // the start of the search. This is not quite ideal, but we
                // do specialize this function in such a way that unanchored
                // searches don't pay for this additional branch. While this
                // might cause a search to continue on for more than it
                // otherwise optimally would, it will be no more than the
                // longest pattern in the automaton. The reason for this is
                // that we ensure we don't follow failure transitions during
                // an anchored search. Combined with using a different anchored
                // starting state with no self-loop, we guarantee that we'll
                // at worst move through a number of transitions equal to the
                // longest pattern.
                //
                // Now for DFAs, the whole point of them is to eliminate
                // failure transitions entirely. So there is no way to say "if
                // it's an anchored search don't follow failure transitions."
                // Instead, we actually have to build two entirely separate
                // automatons into the transition table. One with failure
                // transitions built into it and another that is effectively
                // just an encoding of the base trie into a transition table.
                // DFAs still need this check though, because the match states
                // still carry matches only reachable via a failure transition.
                // Why? Because removing them seems difficult, although I
                // haven't given it a lot of thought.
                if !(anchored.is_anchored() && m.start() > input.start()) {
                    mat = Some(m);
                    if earliest {
                        return Ok(mat);
                    }
                }
            } else if let Some(pre) = pre {
                // If we're here, we know it's a special state that is not a
                // dead or a match state AND that a prefilter is active. Thus,
                // it must be a start state.
                debug_assert!(aut.is_start(sid));
                // We don't care about 'Candidate::Match' here because if such
                // a match were possible, it would have been returned above
                // when we run the prefilter before walking the automaton.
                let span = Span::from(at..input.end());
                match pre.find_in(input.haystack(), span).into_option() {
                    None => return Ok(None),
                    Some(i) => {
                        if i > at {
                            at = i;
                            continue;
                        }
                    }
                }
            } else {
                // When pre.is_none(), then starting states should not be
                // treated as special. That is, without a prefilter, is_special
                // should only return true when the state is a dead or a match
                // state.
                //
                // It is possible to execute a search without a prefilter even
                // when the underlying searcher has one: an anchored search.
                // But in this case, the automaton makes it impossible to move
                // back to the start state by construction, and thus, we should
                // never reach this branch.
                debug_assert!(false, "unreachable");
            }
        }
        at += 1;
    }
    Ok(mat)
}

#[inline(never)]
fn try_find_overlapping_fwd<A: Automaton + ?Sized>(
    aut: &A,
    input: &Input<'_>,
    state: &mut OverlappingState,
) -> Result<(), MatchError> {
    state.mat = None;
    if input.is_done() {
        return Ok(());
    }
    // Searching with a pattern ID is always anchored, so we should only ever
    // use a prefilter when no pattern ID is given.
    if aut.prefilter().is_some() && !input.get_anchored().is_anchored() {
        let pre = aut.prefilter().unwrap();
        try_find_overlapping_fwd_imp(aut, input, Some(pre), state)
    } else {
        try_find_overlapping_fwd_imp(aut, input, None, state)
    }
}

#[inline(always)]
fn try_find_overlapping_fwd_imp<A: Automaton + ?Sized>(
    aut: &A,
    input: &Input<'_>,
    pre: Option<&Prefilter>,
    state: &mut OverlappingState,
) -> Result<(), MatchError> {
    let mut sid = match state.id {
        None => {
            let sid = aut.start_state(input.get_anchored())?;
            // Handle the case where the start state is a match state. That is,
            // the empty string is in our automaton. We report every match we
            // can here before moving on and updating 'state.at' and 'state.id'
            // to find more matches in other parts of the haystack.
            if aut.is_match(sid) {
                let i = state.next_match_index.unwrap_or(0);
                let len = aut.match_len(sid);
                if i < len {
                    state.next_match_index = Some(i + 1);
                    state.mat = Some(get_match(aut, sid, i, input.start()));
                    return Ok(());
                }
            }
            state.at = input.start();
            state.id = Some(sid);
            state.next_match_index = None;
            state.mat = None;
            sid
        }
        Some(sid) => {
            // If we still have matches left to report in this state then
            // report them until we've exhausted them. Only after that do we
            // advance to the next offset in the haystack.
            if let Some(i) = state.next_match_index {
                let len = aut.match_len(sid);
                if i < len {
                    state.next_match_index = Some(i + 1);
                    state.mat = Some(get_match(aut, sid, i, state.at + 1));
                    return Ok(());
                }
                // Once we've reported all matches at a given position, we need
                // to advance the search to the next position.
                state.at += 1;
                state.next_match_index = None;
                state.mat = None;
            }
            sid
        }
    };
    while state.at < input.end() {
        sid = aut.next_state(
            input.get_anchored(),
            sid,
            input.haystack()[state.at],
        );
        if aut.is_special(sid) {
            state.id = Some(sid);
            if aut.is_dead(sid) {
                return Ok(());
            } else if aut.is_match(sid) {
                state.next_match_index = Some(1);
                state.mat = Some(get_match(aut, sid, 0, state.at + 1));
                return Ok(());
            } else if let Some(pre) = pre {
                // If we're here, we know it's a special state that is not a
                // dead or a match state AND that a prefilter is active. Thus,
                // it must be a start state.
                debug_assert!(aut.is_start(sid));
                let span = Span::from(state.at..input.end());
                match pre.find_in(input.haystack(), span).into_option() {
                    None => return Ok(()),
                    Some(i) => {
                        if i > state.at {
                            state.at = i;
                            continue;
                        }
                    }
                }
            } else {
                // When pre.is_none(), then starting states should not be
                // treated as special. That is, without a prefilter, is_special
                // should only return true when the state is a dead or a match
                // state.
                //
                // ... except for one special case: in stream searching, we
                // currently call overlapping search with a 'None' prefilter,
                // regardless of whether one exists or not, because stream
                // searching can't currently deal with prefilters correctly in
                // all cases.
            }
        }
        state.at += 1;
    }
    state.id = Some(sid);
    Ok(())
}

#[inline(always)]
fn get_match<A: Automaton + ?Sized>(
    aut: &A,
    sid: StateID,
    index: usize,
    at: usize,
) -> Match {
    let pid = aut.match_pattern(sid, index);
    let len = aut.pattern_len(pid);
    Match::new(pid, (at - len)..at)
}

/// Write a prefix "state" indicator for fmt::Debug impls. It always writes
/// exactly two printable bytes to the given formatter.
///
/// Specifically, this tries to succinctly distinguish the different types of
/// states: dead states, start states and match states. It even accounts for
/// the possible overlappings of different state types. (The only possible
/// overlapping is that of match and start states.)
pub(crate) fn fmt_state_indicator<A: Automaton>(
    f: &mut core::fmt::Formatter<'_>,
    aut: A,
    id: StateID,
) -> core::fmt::Result {
    if aut.is_dead(id) {
        write!(f, "D ")?;
    } else if aut.is_match(id) {
        if aut.is_start(id) {
            write!(f, "*>")?;
        } else {
            write!(f, "* ")?;
        }
    } else if aut.is_start(id) {
        write!(f, " >")?;
    } else {
        write!(f, "  ")?;
    }
    Ok(())
}

/// Return an iterator of transitions in a sparse format given an iterator
/// of all explicitly defined transitions. The iterator yields ranges of
/// transitions, such that any adjacent transitions mapped to the same
/// state are combined into a single range.
pub(crate) fn sparse_transitions<'a>(
    mut it: impl Iterator<Item = (u8, StateID)> + 'a,
) -> impl Iterator<Item = (u8, u8, StateID)> + 'a {
    let mut cur: Option<(u8, u8, StateID)> = None;
    core::iter::from_fn(move || {
        while let Some((class, next)) = it.next() {
            let (prev_start, prev_end, prev_next) = match cur {
                Some(x) => x,
                None => {
                    cur = Some((class, class, next));
                    continue;
                }
            };
            if prev_next == next {
                cur = Some((prev_start, class, prev_next));
            } else {
                cur = Some((class, class, next));
                return Some((prev_start, prev_end, prev_next));
            }
        }
        if let Some((start, end, next)) = cur.take() {
            return Some((start, end, next));
        }
        None
    })
}
