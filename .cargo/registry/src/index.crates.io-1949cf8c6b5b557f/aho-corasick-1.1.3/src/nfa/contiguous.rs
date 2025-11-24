/*!
Provides a contiguous NFA implementation of Aho-Corasick.

This is a low-level API that generally only needs to be used in niche
circumstances. When possible, prefer using [`AhoCorasick`](crate::AhoCorasick)
instead of a contiguous NFA directly. Using an `NFA` directly is typically only
necessary when one needs access to the [`Automaton`] trait implementation.
*/

use alloc::{vec, vec::Vec};

use crate::{
    automaton::Automaton,
    nfa::noncontiguous,
    util::{
        alphabet::ByteClasses,
        error::{BuildError, MatchError},
        int::{Usize, U16, U32},
        prefilter::Prefilter,
        primitives::{IteratorIndexExt, PatternID, SmallIndex, StateID},
        search::{Anchored, MatchKind},
        special::Special,
    },
};

/// A contiguous NFA implementation of Aho-Corasick.
///
/// When possible, prefer using [`AhoCorasick`](crate::AhoCorasick) instead of
/// this type directly. Using an `NFA` directly is typically only necessary
/// when one needs access to the [`Automaton`] trait implementation.
///
/// This NFA can only be built by first constructing a [`noncontiguous::NFA`].
/// Both [`NFA::new`] and [`Builder::build`] do this for you automatically, but
/// [`Builder::build_from_noncontiguous`] permits doing it explicitly.
///
/// The main difference between a noncontiguous NFA and a contiguous NFA is
/// that the latter represents all of its states and transitions in a single
/// allocation, where as the former uses a separate allocation for each state.
/// Doing this at construction time while keeping a low memory footprint isn't
/// feasible, which is primarily why there are two different NFA types: one
/// that does the least amount of work possible to build itself, and another
/// that does a little extra work to compact itself and make state transitions
/// faster by making some states use a dense representation.
///
/// Because a contiguous NFA uses a single allocation, there is a lot more
/// opportunity for compression tricks to reduce the heap memory used. Indeed,
/// it is not uncommon for a contiguous NFA to use an order of magnitude less
/// heap memory than a noncontiguous NFA. Since building a contiguous NFA
/// usually only takes a fraction of the time it takes to build a noncontiguous
/// NFA, the overall build time is not much slower. Thus, in most cases, a
/// contiguous NFA is the best choice.
///
/// Since a contiguous NFA uses various tricks for compression and to achieve
/// faster state transitions, currently, its limit on the number of states
/// is somewhat smaller than what a noncontiguous NFA can achieve. Generally
/// speaking, you shouldn't expect to run into this limit if the number of
/// patterns is under 1 million. It is plausible that this limit will be
/// increased in the future. If the limit is reached, building a contiguous NFA
/// will return an error. Often, since building a contiguous NFA is relatively
/// cheap, it can make sense to always try it even if you aren't sure if it
/// will fail or not. If it does, you can always fall back to a noncontiguous
/// NFA. (Indeed, the main [`AhoCorasick`](crate::AhoCorasick) type employs a
/// strategy similar to this at construction time.)
///
/// # Example
///
/// This example shows how to build an `NFA` directly and use it to execute
/// [`Automaton::try_find`]:
///
/// ```
/// use aho_corasick::{
///     automaton::Automaton,
///     nfa::contiguous::NFA,
///     Input, Match,
/// };
///
/// let patterns = &["b", "abc", "abcd"];
/// let haystack = "abcd";
///
/// let nfa = NFA::new(patterns).unwrap();
/// assert_eq!(
///     Some(Match::must(0, 1..2)),
///     nfa.try_find(&Input::new(haystack))?,
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// It is also possible to implement your own version of `try_find`. See the
/// [`Automaton`] documentation for an example.
#[derive(Clone)]
pub struct NFA {
    /// The raw NFA representation. Each state is packed with a header
    /// (containing the format of the state, the failure transition and, for
    /// a sparse state, the number of transitions), its transitions and any
    /// matching pattern IDs for match states.
    repr: Vec<u32>,
    /// The length of each pattern. This is used to compute the start offset
    /// of a match.
    pattern_lens: Vec<SmallIndex>,
    /// The total number of states in this NFA.
    state_len: usize,
    /// A prefilter for accelerating searches, if one exists.
    prefilter: Option<Prefilter>,
    /// The match semantics built into this NFA.
    match_kind: MatchKind,
    /// The alphabet size, or total number of equivalence classes, for this
    /// NFA. Dense states always have this many transitions.
    alphabet_len: usize,
    /// The equivalence classes for this NFA. All transitions, dense and
    /// sparse, are defined on equivalence classes and not on the 256 distinct
    /// byte values.
    byte_classes: ByteClasses,
    /// The length of the shortest pattern in this automaton.
    min_pattern_len: usize,
    /// The length of the longest pattern in this automaton.
    max_pattern_len: usize,
    /// The information required to deduce which states are "special" in this
    /// NFA.
    special: Special,
}

impl NFA {
    /// Create a new Aho-Corasick contiguous NFA using the default
    /// configuration.
    ///
    /// Use a [`Builder`] if you want to change the configuration.
    pub fn new<I, P>(patterns: I) -> Result<NFA, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        NFA::builder().build(patterns)
    }

    /// A convenience method for returning a new Aho-Corasick contiguous NFA
    /// builder.
    ///
    /// This usually permits one to just import the `NFA` type.
    pub fn builder() -> Builder {
        Builder::new()
    }
}

impl NFA {
    /// A sentinel state ID indicating that a search should stop once it has
    /// entered this state. When a search stops, it returns a match if one
    /// has been found, otherwise no match. A contiguous NFA always has an
    /// actual dead state at this ID.
    const DEAD: StateID = StateID::new_unchecked(0);
    /// Another sentinel state ID indicating that a search should move through
    /// current state's failure transition.
    ///
    /// Note that unlike DEAD, this does not actually point to a valid state
    /// in a contiguous NFA. (noncontiguous::NFA::FAIL does point to a valid
    /// state.) Instead, this points to the position that is guaranteed to
    /// never be a valid state ID (by making sure it points to a place in the
    /// middle of the encoding of the DEAD state). Since we never need to
    /// actually look at the FAIL state itself, this works out.
    ///
    /// By why do it this way? So that FAIL is a constant. I don't have any
    /// concrete evidence that this materially helps matters, but it's easy to
    /// do. The alternative would be making the FAIL ID point to the second
    /// state, which could be made a constant but is a little trickier to do.
    /// The easiest path is to just make the FAIL state a runtime value, but
    /// since comparisons with FAIL occur in perf critical parts of the search,
    /// we want it to be as tight as possible and not waste any registers.
    ///
    /// Very hand wavy... But the code complexity that results from this is
    /// very mild.
    const FAIL: StateID = StateID::new_unchecked(1);
}

// SAFETY: 'start_state' always returns a valid state ID, 'next_state' always
// returns a valid state ID given a valid state ID. We otherwise claim that
// all other methods are correct as well.
unsafe impl Automaton for NFA {
    #[inline(always)]
    fn start_state(&self, anchored: Anchored) -> Result<StateID, MatchError> {
        match anchored {
            Anchored::No => Ok(self.special.start_unanchored_id),
            Anchored::Yes => Ok(self.special.start_anchored_id),
        }
    }

    #[inline(always)]
    fn next_state(
        &self,
        anchored: Anchored,
        mut sid: StateID,
        byte: u8,
    ) -> StateID {
        let repr = &self.repr;
        let class = self.byte_classes.get(byte);
        let u32tosid = StateID::from_u32_unchecked;
        loop {
            let o = sid.as_usize();
            let kind = repr[o] & 0xFF;
            // I tried to encapsulate the "next transition" logic into its own
            // function, but it seemed to always result in sub-optimal codegen
            // that led to real and significant slowdowns. So we just inline
            // the logic here.
            //
            // I've also tried a lot of different ways to speed up this
            // routine, and most of them have failed.
            if kind == State::KIND_DENSE {
                let next = u32tosid(repr[o + 2 + usize::from(class)]);
                if next != NFA::FAIL {
                    return next;
                }
            } else if kind == State::KIND_ONE {
                if class == repr[o].low_u16().high_u8() {
                    return u32tosid(repr[o + 2]);
                }
            } else {
                // NOTE: I tried a SWAR technique in the loop below, but found
                // it slower. See the 'swar' test in the tests for this module.
                let trans_len = kind.as_usize();
                let classes_len = u32_len(trans_len);
                let trans_offset = o + 2 + classes_len;
                for (i, &chunk) in
                    repr[o + 2..][..classes_len].iter().enumerate()
                {
                    let classes = chunk.to_ne_bytes();
                    if classes[0] == class {
                        return u32tosid(repr[trans_offset + i * 4]);
                    }
                    if classes[1] == class {
                        return u32tosid(repr[trans_offset + i * 4 + 1]);
                    }
                    if classes[2] == class {
                        return u32tosid(repr[trans_offset + i * 4 + 2]);
                    }
                    if classes[3] == class {
                        return u32tosid(repr[trans_offset + i * 4 + 3]);
                    }
                }
            }
            // For an anchored search, we never follow failure transitions
            // because failure transitions lead us down a path to matching
            // a *proper* suffix of the path we were on. Thus, it can only
            // produce matches that appear after the beginning of the search.
            if anchored.is_anchored() {
                return NFA::DEAD;
            }
            sid = u32tosid(repr[o + 1]);
        }
    }

    #[inline(always)]
    fn is_special(&self, sid: StateID) -> bool {
        sid <= self.special.max_special_id
    }

    #[inline(always)]
    fn is_dead(&self, sid: StateID) -> bool {
        sid == NFA::DEAD
    }

    #[inline(always)]
    fn is_match(&self, sid: StateID) -> bool {
        !self.is_dead(sid) && sid <= self.special.max_match_id
    }

    #[inline(always)]
    fn is_start(&self, sid: StateID) -> bool {
        sid == self.special.start_unanchored_id
            || sid == self.special.start_anchored_id
    }

    #[inline(always)]
    fn match_kind(&self) -> MatchKind {
        self.match_kind
    }

    #[inline(always)]
    fn patterns_len(&self) -> usize {
        self.pattern_lens.len()
    }

    #[inline(always)]
    fn pattern_len(&self, pid: PatternID) -> usize {
        self.pattern_lens[pid].as_usize()
    }

    #[inline(always)]
    fn min_pattern_len(&self) -> usize {
        self.min_pattern_len
    }

    #[inline(always)]
    fn max_pattern_len(&self) -> usize {
        self.max_pattern_len
    }

    #[inline(always)]
    fn match_len(&self, sid: StateID) -> usize {
        State::match_len(self.alphabet_len, &self.repr[sid.as_usize()..])
    }

    #[inline(always)]
    fn match_pattern(&self, sid: StateID, index: usize) -> PatternID {
        State::match_pattern(
            self.alphabet_len,
            &self.repr[sid.as_usize()..],
            index,
        )
    }

    #[inline(always)]
    fn memory_usage(&self) -> usize {
        use core::mem::size_of;

        (self.repr.len() * size_of::<u32>())
            + (self.pattern_lens.len() * size_of::<SmallIndex>())
            + self.prefilter.as_ref().map_or(0, |p| p.memory_usage())
    }

    #[inline(always)]
    fn prefilter(&self) -> Option<&Prefilter> {
        self.prefilter.as_ref()
    }
}

impl core::fmt::Debug for NFA {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::automaton::fmt_state_indicator;

        writeln!(f, "contiguous::NFA(")?;
        let mut sid = NFA::DEAD; // always the first state and always present
        loop {
            let raw = &self.repr[sid.as_usize()..];
            if raw.is_empty() {
                break;
            }
            let is_match = self.is_match(sid);
            let state = State::read(self.alphabet_len, is_match, raw);
            fmt_state_indicator(f, self, sid)?;
            write!(
                f,
                "{:06}({:06}): ",
                sid.as_usize(),
                state.fail.as_usize()
            )?;
            state.fmt(f)?;
            write!(f, "\n")?;
            if self.is_match(sid) {
                write!(f, "         matches: ")?;
                for i in 0..state.match_len {
                    let pid = State::match_pattern(self.alphabet_len, raw, i);
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", pid.as_usize())?;
                }
                write!(f, "\n")?;
            }
            // The FAIL state doesn't actually have space for a state allocated
            // for it, so we have to treat it as a special case. write below
            // the DEAD state.
            if sid == NFA::DEAD {
                writeln!(f, "F {:06}:", NFA::FAIL.as_usize())?;
            }
            let len = State::len(self.alphabet_len, is_match, raw);
            sid = StateID::new(sid.as_usize().checked_add(len).unwrap())
                .unwrap();
        }
        writeln!(f, "match kind: {:?}", self.match_kind)?;
        writeln!(f, "prefilter: {:?}", self.prefilter.is_some())?;
        writeln!(f, "state length: {:?}", self.state_len)?;
        writeln!(f, "pattern length: {:?}", self.patterns_len())?;
        writeln!(f, "shortest pattern length: {:?}", self.min_pattern_len)?;
        writeln!(f, "longest pattern length: {:?}", self.max_pattern_len)?;
        writeln!(f, "alphabet length: {:?}", self.alphabet_len)?;
        writeln!(f, "byte classes: {:?}", self.byte_classes)?;
        writeln!(f, "memory usage: {:?}", self.memory_usage())?;
        writeln!(f, ")")?;

        Ok(())
    }
}

/// The "in memory" representation a single dense or sparse state.
///
/// A `State`'s in memory representation is not ever actually materialized
/// during a search with a contiguous NFA. Doing so would be too slow. (Indeed,
/// the only time a `State` is actually constructed is in `Debug` impls.)
/// Instead, a `State` exposes a number of static methods for reading certain
/// things from the raw binary encoding of the state.
#[derive(Clone)]
struct State<'a> {
    /// The state to transition to when 'class_to_next' yields a transition
    /// to the FAIL state.
    fail: StateID,
    /// The number of pattern IDs in this state. For a non-match state, this is
    /// always zero. Otherwise it is always bigger than zero.
    match_len: usize,
    /// The sparse or dense representation of the transitions for this state.
    trans: StateTrans<'a>,
}

/// The underlying representation of sparse or dense transitions for a state.
///
/// Note that like `State`, we don't typically construct values of this type
/// during a search since we don't always need all values and thus would
/// represent a lot of wasteful work.
#[derive(Clone)]
enum StateTrans<'a> {
    /// A sparse representation of transitions for a state, where only non-FAIL
    /// transitions are explicitly represented.
    Sparse {
        classes: &'a [u32],
        /// The transitions for this state, where each transition is packed
        /// into a u32. The low 8 bits correspond to the byte class for the
        /// transition, and the high 24 bits correspond to the next state ID.
        ///
        /// This packing is why the max state ID allowed for a contiguous
        /// NFA is 2^24-1.
        nexts: &'a [u32],
    },
    /// A "one transition" state that is never a match state.
    ///
    /// These are by far the most common state, so we use a specialized and
    /// very compact representation for them.
    One {
        /// The element of this NFA's alphabet that this transition is
        /// defined for.
        class: u8,
        /// The state this should transition to if the current symbol is
        /// equal to 'class'.
        next: u32,
    },
    /// A dense representation of transitions for a state, where all
    /// transitions are explicitly represented, including transitions to the
    /// FAIL state.
    Dense {
        /// A dense set of transitions to other states. The transitions may
        /// point to a FAIL state, in which case, the search should try the
        /// same transition lookup at 'fail'.
        ///
        /// Note that this is indexed by byte equivalence classes and not
        /// byte values. That means 'class_to_next[byte]' is wrong and
        /// 'class_to_next[classes.get(byte)]' is correct. The number of
        /// transitions is always equivalent to 'classes.alphabet_len()'.
        class_to_next: &'a [u32],
    },
}

impl<'a> State<'a> {
    /// The offset of where the "kind" of a state is stored. If it isn't one
    /// of the sentinel values below, then it's a sparse state and the kind
    /// corresponds to the number of transitions in the state.
    const KIND: usize = 0;

    /// A sentinel value indicating that the state uses a dense representation.
    const KIND_DENSE: u32 = 0xFF;
    /// A sentinel value indicating that the state uses a special "one
    /// transition" encoding. In practice, non-match states with one transition
    /// make up the overwhelming majority of all states in any given
    /// Aho-Corasick automaton, so we can specialize them using a very compact
    /// representation.
    const KIND_ONE: u32 = 0xFE;

    /// The maximum number of transitions to encode as a sparse state. Usually
    /// states with a lot of transitions are either very rare, or occur near
    /// the start state. In the latter case, they are probably dense already
    /// anyway. In the former case, making them dense is fine because they're
    /// rare.
    ///
    /// This needs to be small enough to permit each of the sentinel values for
    /// 'KIND' above. Namely, a sparse state embeds the number of transitions
    /// into the 'KIND'. Basically, "sparse" is a state kind too, but it's the
    /// "else" branch.
    ///
    /// N.B. There isn't anything particularly magical about 127 here. I
    /// just picked it because I figured any sparse state with this many
    /// transitions is going to be exceptionally rare, and if it did have this
    /// many transitions, then it would be quite slow to do a linear scan on
    /// the transitions during a search anyway.
    const MAX_SPARSE_TRANSITIONS: usize = 127;

    /// Remap state IDs in-place.
    ///
    /// `state` should be the the raw binary encoding of a state. (The start
    /// of the slice must correspond to the start of the state, but the slice
    /// may extend past the end of the encoding of the state.)
    fn remap(
        alphabet_len: usize,
        old_to_new: &[StateID],
        state: &mut [u32],
    ) -> Result<(), BuildError> {
        let kind = State::kind(state);
        if kind == State::KIND_DENSE {
            state[1] = old_to_new[state[1].as_usize()].as_u32();
            for next in state[2..][..alphabet_len].iter_mut() {
                *next = old_to_new[next.as_usize()].as_u32();
            }
        } else if kind == State::KIND_ONE {
            state[1] = old_to_new[state[1].as_usize()].as_u32();
            state[2] = old_to_new[state[2].as_usize()].as_u32();
        } else {
            let trans_len = State::sparse_trans_len(state);
            let classes_len = u32_len(trans_len);
            state[1] = old_to_new[state[1].as_usize()].as_u32();
            for next in state[2 + classes_len..][..trans_len].iter_mut() {
                *next = old_to_new[next.as_usize()].as_u32();
            }
        }
        Ok(())
    }

    /// Returns the length, in number of u32s, of this state.
    ///
    /// This is useful for reading states consecutively, e.g., in the Debug
    /// impl without needing to store a separate map from state index to state
    /// identifier.
    ///
    /// `state` should be the the raw binary encoding of a state. (The start
    /// of the slice must correspond to the start of the state, but the slice
    /// may extend past the end of the encoding of the state.)
    fn len(alphabet_len: usize, is_match: bool, state: &[u32]) -> usize {
        let kind_len = 1;
        let fail_len = 1;
        let kind = State::kind(state);
        let (classes_len, trans_len) = if kind == State::KIND_DENSE {
            (0, alphabet_len)
        } else if kind == State::KIND_ONE {
            (0, 1)
        } else {
            let trans_len = State::sparse_trans_len(state);
            let classes_len = u32_len(trans_len);
            (classes_len, trans_len)
        };
        let match_len = if !is_match {
            0
        } else if State::match_len(alphabet_len, state) == 1 {
            // This is a special case because when there is one pattern ID for
            // a match state, it is represented by a single u32 with its high
            // bit set (which is impossible for a valid pattern ID).
            1
        } else {
            // We add 1 to include the u32 that indicates the number of
            // pattern IDs that follow.
            1 + State::match_len(alphabet_len, state)
        };
        kind_len + fail_len + classes_len + trans_len + match_len
    }

    /// Returns the kind of this state.
    ///
    /// This only includes the low byte.
    #[inline(always)]
    fn kind(state: &[u32]) -> u32 {
        state[State::KIND] & 0xFF
    }

    /// Get the number of sparse transitions in this state. This can never
    /// be more than State::MAX_SPARSE_TRANSITIONS, as all states with more
    /// transitions are encoded as dense states.
    ///
    /// `state` should be the the raw binary encoding of a sparse state. (The
    /// start of the slice must correspond to the start of the state, but the
    /// slice may extend past the end of the encoding of the state.) If this
    /// isn't a sparse state, then the return value is unspecified.
    ///
    /// Do note that this is only legal to call on a sparse state. So for
    /// example, "one transition" state is not a sparse state, so it would not
    /// be legal to call this method on such a state.
    #[inline(always)]
    fn sparse_trans_len(state: &[u32]) -> usize {
        (state[State::KIND] & 0xFF).as_usize()
    }

    /// Returns the total number of matching pattern IDs in this state. Calling
    /// this on a state that isn't a match results in unspecified behavior.
    /// Thus, the returned number is never 0 for all correct calls.
    ///
    /// `state` should be the the raw binary encoding of a state. (The start
    /// of the slice must correspond to the start of the state, but the slice
    /// may extend past the end of the encoding of the state.)
    #[inline(always)]
    fn match_len(alphabet_len: usize, state: &[u32]) -> usize {
        // We don't need to handle KIND_ONE here because it can never be a
        // match state.
        let packed = if State::kind(state) == State::KIND_DENSE {
            let start = 2 + alphabet_len;
            state[start].as_usize()
        } else {
            let trans_len = State::sparse_trans_len(state);
            let classes_len = u32_len(trans_len);
            let start = 2 + classes_len + trans_len;
            state[start].as_usize()
        };
        if packed & (1 << 31) == 0 {
            packed
        } else {
            1
        }
    }

    /// Returns the pattern ID corresponding to the given index for the state
    /// given. The `index` provided must be less than the number of pattern IDs
    /// in this state.
    ///
    /// `state` should be the the raw binary encoding of a state. (The start of
    /// the slice must correspond to the start of the state, but the slice may
    /// extend past the end of the encoding of the state.)
    ///
    /// If the given state is not a match state or if the index is out of
    /// bounds, then this has unspecified behavior.
    #[inline(always)]
    fn match_pattern(
        alphabet_len: usize,
        state: &[u32],
        index: usize,
    ) -> PatternID {
        // We don't need to handle KIND_ONE here because it can never be a
        // match state.
        let start = if State::kind(state) == State::KIND_DENSE {
            2 + alphabet_len
        } else {
            let trans_len = State::sparse_trans_len(state);
            let classes_len = u32_len(trans_len);
            2 + classes_len + trans_len
        };
        let packed = state[start];
        let pid = if packed & (1 << 31) == 0 {
            state[start + 1 + index]
        } else {
            assert_eq!(0, index);
            packed & !(1 << 31)
        };
        PatternID::from_u32_unchecked(pid)
    }

    /// Read a state's binary encoding to its in-memory representation.
    ///
    /// `alphabet_len` should be the total number of transitions defined for
    /// dense states.
    ///
    /// `is_match` should be true if this state is a match state and false
    /// otherwise.
    ///
    /// `state` should be the the raw binary encoding of a state. (The start
    /// of the slice must correspond to the start of the state, but the slice
    /// may extend past the end of the encoding of the state.)
    fn read(
        alphabet_len: usize,
        is_match: bool,
        state: &'a [u32],
    ) -> State<'a> {
        let kind = State::kind(state);
        let match_len =
            if !is_match { 0 } else { State::match_len(alphabet_len, state) };
        let (trans, fail) = if kind == State::KIND_DENSE {
            let fail = StateID::from_u32_unchecked(state[1]);
            let class_to_next = &state[2..][..alphabet_len];
            (StateTrans::Dense { class_to_next }, fail)
        } else if kind == State::KIND_ONE {
            let fail = StateID::from_u32_unchecked(state[1]);
            let class = state[State::KIND].low_u16().high_u8();
            let next = state[2];
            (StateTrans::One { class, next }, fail)
        } else {
            let fail = StateID::from_u32_unchecked(state[1]);
            let trans_len = State::sparse_trans_len(state);
            let classes_len = u32_len(trans_len);
            let classes = &state[2..][..classes_len];
            let nexts = &state[2 + classes_len..][..trans_len];
            (StateTrans::Sparse { classes, nexts }, fail)
        };
        State { fail, match_len, trans }
    }

    /// Encode the "old" state from a noncontiguous NFA to its binary
    /// representation to the given `dst` slice. `classes` should be the byte
    /// classes computed for the noncontiguous NFA that the given state came
    /// from.
    ///
    /// This returns an error if `dst` became so big that `StateID`s can no
    /// longer be created for new states. Otherwise, it returns the state ID of
    /// the new state created.
    ///
    /// When `force_dense` is true, then the encoded state will always use a
    /// dense format. Otherwise, the choice between dense and sparse will be
    /// automatically chosen based on the old state.
    fn write(
        nnfa: &noncontiguous::NFA,
        oldsid: StateID,
        old: &noncontiguous::State,
        classes: &ByteClasses,
        dst: &mut Vec<u32>,
        force_dense: bool,
    ) -> Result<StateID, BuildError> {
        let sid = StateID::new(dst.len()).map_err(|e| {
            BuildError::state_id_overflow(StateID::MAX.as_u64(), e.attempted())
        })?;
        let old_len = nnfa.iter_trans(oldsid).count();
        // For states with a lot of transitions, we might as well just make
        // them dense. These kinds of hot states tend to be very rare, so we're
        // okay with it. This also gives us more sentinels in the state's
        // 'kind', which lets us create different state kinds to save on
        // space.
        let kind = if force_dense || old_len > State::MAX_SPARSE_TRANSITIONS {
            State::KIND_DENSE
        } else if old_len == 1 && !old.is_match() {
            State::KIND_ONE
        } else {
            // For a sparse state, the kind is just the number of transitions.
            u32::try_from(old_len).unwrap()
        };
        if kind == State::KIND_DENSE {
            dst.push(kind);
            dst.push(old.fail().as_u32());
            State::write_dense_trans(nnfa, oldsid, classes, dst)?;
        } else if kind == State::KIND_ONE {
            let t = nnfa.iter_trans(oldsid).next().unwrap();
            let class = u32::from(classes.get(t.byte()));
            dst.push(kind | (class << 8));
            dst.push(old.fail().as_u32());
            dst.push(t.next().as_u32());
        } else {
            dst.push(kind);
            dst.push(old.fail().as_u32());
            State::write_sparse_trans(nnfa, oldsid, classes, dst)?;
        }
        // Now finally write the number of matches and the matches themselves.
        if old.is_match() {
            let matches_len = nnfa.iter_matches(oldsid).count();
            if matches_len == 1 {
                let pid = nnfa.iter_matches(oldsid).next().unwrap().as_u32();
                assert_eq!(0, pid & (1 << 31));
                dst.push((1 << 31) | pid);
            } else {
                assert_eq!(0, matches_len & (1 << 31));
                dst.push(matches_len.as_u32());
                dst.extend(nnfa.iter_matches(oldsid).map(|pid| pid.as_u32()));
            }
        }
        Ok(sid)
    }

    /// Encode the "old" state transitions from a noncontiguous NFA to its
    /// binary sparse representation to the given `dst` slice. `classes` should
    /// be the byte classes computed for the noncontiguous NFA that the given
    /// state came from.
    ///
    /// This returns an error if `dst` became so big that `StateID`s can no
    /// longer be created for new states.
    fn write_sparse_trans(
        nnfa: &noncontiguous::NFA,
        oldsid: StateID,
        classes: &ByteClasses,
        dst: &mut Vec<u32>,
    ) -> Result<(), BuildError> {
        let (mut chunk, mut len) = ([0; 4], 0);
        for t in nnfa.iter_trans(oldsid) {
            chunk[len] = classes.get(t.byte());
            len += 1;
            if len == 4 {
                dst.push(u32::from_ne_bytes(chunk));
                chunk = [0; 4];
                len = 0;
            }
        }
        if len > 0 {
            // In the case where the number of transitions isn't divisible
            // by 4, the last u32 chunk will have some left over room. In
            // this case, we "just" repeat the last equivalence class. By
            // doing this, we know the leftover faux transitions will never
            // be followed because if they were, it would have been followed
            // prior to it in the last equivalence class. This saves us some
            // branching in the search time state transition code.
            let repeat = chunk[len - 1];
            while len < 4 {
                chunk[len] = repeat;
                len += 1;
            }
            dst.push(u32::from_ne_bytes(chunk));
        }
        for t in nnfa.iter_trans(oldsid) {
            dst.push(t.next().as_u32());
        }
        Ok(())
    }

    /// Encode the "old" state transitions from a noncontiguous NFA to its
    /// binary dense representation to the given `dst` slice. `classes` should
    /// be the byte classes computed for the noncontiguous NFA that the given
    /// state came from.
    ///
    /// This returns an error if `dst` became so big that `StateID`s can no
    /// longer be created for new states.
    fn write_dense_trans(
        nnfa: &noncontiguous::NFA,
        oldsid: StateID,
        classes: &ByteClasses,
        dst: &mut Vec<u32>,
    ) -> Result<(), BuildError> {
        // Our byte classes let us shrink the size of our dense states to the
        // number of equivalence classes instead of just fixing it to 256.
        // Any non-explicitly defined transition is just a transition to the
        // FAIL state, so we fill that in first and then overwrite them with
        // explicitly defined transitions. (Most states probably only have one
        // or two explicitly defined transitions.)
        //
        // N.B. Remember that while building the contiguous NFA, we use state
        // IDs from the noncontiguous NFA. It isn't until we've added all
        // states that we go back and map noncontiguous IDs to contiguous IDs.
        let start = dst.len();
        dst.extend(
            core::iter::repeat(noncontiguous::NFA::FAIL.as_u32())
                .take(classes.alphabet_len()),
        );
        assert!(start < dst.len(), "equivalence classes are never empty");
        for t in nnfa.iter_trans(oldsid) {
            dst[start + usize::from(classes.get(t.byte()))] =
                t.next().as_u32();
        }
        Ok(())
    }

    /// Return an iterator over every explicitly defined transition in this
    /// state.
    fn transitions<'b>(&'b self) -> impl Iterator<Item = (u8, StateID)> + 'b {
        let mut i = 0;
        core::iter::from_fn(move || match self.trans {
            StateTrans::Sparse { classes, nexts } => {
                if i >= nexts.len() {
                    return None;
                }
                let chunk = classes[i / 4];
                let class = chunk.to_ne_bytes()[i % 4];
                let next = StateID::from_u32_unchecked(nexts[i]);
                i += 1;
                Some((class, next))
            }
            StateTrans::One { class, next } => {
                if i == 0 {
                    i += 1;
                    Some((class, StateID::from_u32_unchecked(next)))
                } else {
                    None
                }
            }
            StateTrans::Dense { class_to_next } => {
                if i >= class_to_next.len() {
                    return None;
                }
                let class = i.as_u8();
                let next = StateID::from_u32_unchecked(class_to_next[i]);
                i += 1;
                Some((class, next))
            }
        })
    }
}

impl<'a> core::fmt::Debug for State<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use crate::{automaton::sparse_transitions, util::debug::DebugByte};

        let it = sparse_transitions(self.transitions())
            // Writing out all FAIL transitions is quite noisy. Instead, we
            // just require readers of the output to assume anything absent
            // maps to the FAIL transition.
            .filter(|&(_, _, sid)| sid != NFA::FAIL)
            .enumerate();
        for (i, (start, end, sid)) in it {
            if i > 0 {
                write!(f, ", ")?;
            }
            if start == end {
                write!(f, "{:?} => {:?}", DebugByte(start), sid.as_usize())?;
            } else {
                write!(
                    f,
                    "{:?}-{:?} => {:?}",
                    DebugByte(start),
                    DebugByte(end),
                    sid.as_usize()
                )?;
            }
        }
        Ok(())
    }
}

/// A builder for configuring an Aho-Corasick contiguous NFA.
///
/// This builder has a subset of the options available to a
/// [`AhoCorasickBuilder`](crate::AhoCorasickBuilder). Of the shared options,
/// their behavior is identical.
#[derive(Clone, Debug)]
pub struct Builder {
    noncontiguous: noncontiguous::Builder,
    dense_depth: usize,
    byte_classes: bool,
}

impl Default for Builder {
    fn default() -> Builder {
        Builder {
            noncontiguous: noncontiguous::Builder::new(),
            dense_depth: 2,
            byte_classes: true,
        }
    }
}

impl Builder {
    /// Create a new builder for configuring an Aho-Corasick contiguous NFA.
    pub fn new() -> Builder {
        Builder::default()
    }

    /// Build an Aho-Corasick contiguous NFA from the given iterator of
    /// patterns.
    ///
    /// A builder may be reused to create more NFAs.
    pub fn build<I, P>(&self, patterns: I) -> Result<NFA, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        let nnfa = self.noncontiguous.build(patterns)?;
        self.build_from_noncontiguous(&nnfa)
    }

    /// Build an Aho-Corasick contiguous NFA from the given noncontiguous NFA.
    ///
    /// Note that when this method is used, only the `dense_depth` and
    /// `byte_classes` settings on this builder are respected. The other
    /// settings only apply to the initial construction of the Aho-Corasick
    /// automaton. Since using this method requires that initial construction
    /// has already completed, all settings impacting only initial construction
    /// are no longer relevant.
    pub fn build_from_noncontiguous(
        &self,
        nnfa: &noncontiguous::NFA,
    ) -> Result<NFA, BuildError> {
        debug!("building contiguous NFA");
        let byte_classes = if self.byte_classes {
            nnfa.byte_classes().clone()
        } else {
            ByteClasses::singletons()
        };
        let mut index_to_state_id = vec![NFA::DEAD; nnfa.states().len()];
        let mut nfa = NFA {
            repr: vec![],
            pattern_lens: nnfa.pattern_lens_raw().to_vec(),
            state_len: nnfa.states().len(),
            prefilter: nnfa.prefilter().map(|p| p.clone()),
            match_kind: nnfa.match_kind(),
            alphabet_len: byte_classes.alphabet_len(),
            byte_classes,
            min_pattern_len: nnfa.min_pattern_len(),
            max_pattern_len: nnfa.max_pattern_len(),
            // The special state IDs are set later.
            special: Special::zero(),
        };
        for (oldsid, state) in nnfa.states().iter().with_state_ids() {
            // We don't actually encode a fail state since it isn't necessary.
            // But we still want to make sure any FAIL ids are mapped
            // correctly.
            if oldsid == noncontiguous::NFA::FAIL {
                index_to_state_id[oldsid] = NFA::FAIL;
                continue;
            }
            let force_dense = state.depth().as_usize() < self.dense_depth;
            let newsid = State::write(
                nnfa,
                oldsid,
                state,
                &nfa.byte_classes,
                &mut nfa.repr,
                force_dense,
            )?;
            index_to_state_id[oldsid] = newsid;
        }
        for &newsid in index_to_state_id.iter() {
            if newsid == NFA::FAIL {
                continue;
            }
            let state = &mut nfa.repr[newsid.as_usize()..];
            State::remap(nfa.alphabet_len, &index_to_state_id, state)?;
        }
        // Now that we've remapped all the IDs in our states, all that's left
        // is remapping the special state IDs.
        let remap = &index_to_state_id;
        let old = nnfa.special();
        let new = &mut nfa.special;
        new.max_special_id = remap[old.max_special_id];
        new.max_match_id = remap[old.max_match_id];
        new.start_unanchored_id = remap[old.start_unanchored_id];
        new.start_anchored_id = remap[old.start_anchored_id];
        debug!(
            "contiguous NFA built, <states: {:?}, size: {:?}, \
             alphabet len: {:?}>",
            nfa.state_len,
            nfa.memory_usage(),
            nfa.byte_classes.alphabet_len(),
        );
        // The vectors can grow ~twice as big during construction because a
        // Vec amortizes growth. But here, let's shrink things back down to
        // what we actually need since we're never going to add more to it.
        nfa.repr.shrink_to_fit();
        nfa.pattern_lens.shrink_to_fit();
        Ok(nfa)
    }

    /// Set the desired match semantics.
    ///
    /// This only applies when using [`Builder::build`] and not
    /// [`Builder::build_from_noncontiguous`].
    ///
    /// See
    /// [`AhoCorasickBuilder::match_kind`](crate::AhoCorasickBuilder::match_kind)
    /// for more documentation and examples.
    pub fn match_kind(&mut self, kind: MatchKind) -> &mut Builder {
        self.noncontiguous.match_kind(kind);
        self
    }

    /// Enable ASCII-aware case insensitive matching.
    ///
    /// This only applies when using [`Builder::build`] and not
    /// [`Builder::build_from_noncontiguous`].
    ///
    /// See
    /// [`AhoCorasickBuilder::ascii_case_insensitive`](crate::AhoCorasickBuilder::ascii_case_insensitive)
    /// for more documentation and examples.
    pub fn ascii_case_insensitive(&mut self, yes: bool) -> &mut Builder {
        self.noncontiguous.ascii_case_insensitive(yes);
        self
    }

    /// Enable heuristic prefilter optimizations.
    ///
    /// This only applies when using [`Builder::build`] and not
    /// [`Builder::build_from_noncontiguous`].
    ///
    /// See
    /// [`AhoCorasickBuilder::prefilter`](crate::AhoCorasickBuilder::prefilter)
    /// for more documentation and examples.
    pub fn prefilter(&mut self, yes: bool) -> &mut Builder {
        self.noncontiguous.prefilter(yes);
        self
    }

    /// Set the limit on how many states use a dense representation for their
    /// transitions. Other states will generally use a sparse representation.
    ///
    /// See
    /// [`AhoCorasickBuilder::dense_depth`](crate::AhoCorasickBuilder::dense_depth)
    /// for more documentation and examples.
    pub fn dense_depth(&mut self, depth: usize) -> &mut Builder {
        self.dense_depth = depth;
        self
    }

    /// A debug setting for whether to attempt to shrink the size of the
    /// automaton's alphabet or not.
    ///
    /// This should never be enabled unless you're debugging an automaton.
    /// Namely, disabling byte classes makes transitions easier to reason
    /// about, since they use the actual bytes instead of equivalence classes.
    /// Disabling this confers no performance benefit at search time.
    ///
    /// See
    /// [`AhoCorasickBuilder::byte_classes`](crate::AhoCorasickBuilder::byte_classes)
    /// for more documentation and examples.
    pub fn byte_classes(&mut self, yes: bool) -> &mut Builder {
        self.byte_classes = yes;
        self
    }
}

/// Computes the number of u32 values needed to represent one byte per the
/// number of transitions given.
fn u32_len(ntrans: usize) -> usize {
    if ntrans % 4 == 0 {
        ntrans >> 2
    } else {
        (ntrans >> 2) + 1
    }
}

#[cfg(test)]
mod tests {
    // This test demonstrates a SWAR technique I tried in the sparse transition
    // code inside of 'next_state'. Namely, sparse transitions work by
    // iterating over u32 chunks, with each chunk containing up to 4 classes
    // corresponding to 4 transitions. This SWAR technique lets us find a
    // matching transition without converting the u32 to a [u8; 4].
    //
    // It turned out to be a little slower unfortunately, which isn't too
    // surprising, since this is likely a throughput oriented optimization.
    // Loop unrolling doesn't really help us because the vast majority of
    // states have very few transitions.
    //
    // Anyway, this code was a little tricky to write, so I converted it to a
    // test in case someone figures out how to use it more effectively than
    // I could.
    //
    // (This also only works on little endian. So big endian would need to be
    // accounted for if we ever decided to use this I think.)
    #[cfg(target_endian = "little")]
    #[test]
    fn swar() {
        use super::*;

        fn has_zero_byte(x: u32) -> u32 {
            const LO_U32: u32 = 0x01010101;
            const HI_U32: u32 = 0x80808080;

            x.wrapping_sub(LO_U32) & !x & HI_U32
        }

        fn broadcast(b: u8) -> u32 {
            (u32::from(b)) * (u32::MAX / 255)
        }

        fn index_of(x: u32) -> usize {
            let o =
                (((x - 1) & 0x01010101).wrapping_mul(0x01010101) >> 24) - 1;
            o.as_usize()
        }

        let bytes: [u8; 4] = [b'1', b'A', b'a', b'z'];
        let chunk = u32::from_ne_bytes(bytes);

        let needle = broadcast(b'1');
        assert_eq!(0, index_of(has_zero_byte(needle ^ chunk)));
        let needle = broadcast(b'A');
        assert_eq!(1, index_of(has_zero_byte(needle ^ chunk)));
        let needle = broadcast(b'a');
        assert_eq!(2, index_of(has_zero_byte(needle ^ chunk)));
        let needle = broadcast(b'z');
        assert_eq!(3, index_of(has_zero_byte(needle ^ chunk)));
    }
}
