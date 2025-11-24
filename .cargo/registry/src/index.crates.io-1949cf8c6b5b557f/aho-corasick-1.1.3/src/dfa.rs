/*!
Provides direct access to a DFA implementation of Aho-Corasick.

This is a low-level API that generally only needs to be used in niche
circumstances. When possible, prefer using [`AhoCorasick`](crate::AhoCorasick)
instead of a DFA directly. Using an `DFA` directly is typically only necessary
when one needs access to the [`Automaton`] trait implementation.
*/

use alloc::{vec, vec::Vec};

use crate::{
    automaton::Automaton,
    nfa::noncontiguous,
    util::{
        alphabet::ByteClasses,
        error::{BuildError, MatchError},
        int::{Usize, U32},
        prefilter::Prefilter,
        primitives::{IteratorIndexExt, PatternID, SmallIndex, StateID},
        search::{Anchored, MatchKind, StartKind},
        special::Special,
    },
};

/// A DFA implementation of Aho-Corasick.
///
/// When possible, prefer using [`AhoCorasick`](crate::AhoCorasick) instead of
/// this type directly. Using a `DFA` directly is typically only necessary when
/// one needs access to the [`Automaton`] trait implementation.
///
/// This DFA can only be built by first constructing a [`noncontiguous::NFA`].
/// Both [`DFA::new`] and [`Builder::build`] do this for you automatically, but
/// [`Builder::build_from_noncontiguous`] permits doing it explicitly.
///
/// A DFA provides the best possible search performance (in this crate) via two
/// mechanisms:
///
/// * All states use a dense representation for their transitions.
/// * All failure transitions are pre-computed such that they are never
/// explicitly handled at search time.
///
/// These two facts combined mean that every state transition is performed
/// using a constant number of instructions. However, this comes at
/// great cost. The memory usage of a DFA can be quite exorbitant.
/// It is potentially multiple orders of magnitude greater than a
/// [`contiguous::NFA`](crate::nfa::contiguous::NFA) for example. In exchange,
/// a DFA will typically have better search speed than a `contiguous::NFA`, but
/// not by orders of magnitude.
///
/// Unless you have a small number of patterns or memory usage is not a concern
/// and search performance is critical, a DFA is usually not the best choice.
///
/// Moreover, unlike the NFAs in this crate, it is costly for a DFA to
/// support for anchored and unanchored search configurations. Namely,
/// since failure transitions are pre-computed, supporting both anchored
/// and unanchored searches requires a duplication of the transition table,
/// making the memory usage of such a DFA ever bigger. (The NFAs in this crate
/// unconditionally support both anchored and unanchored searches because there
/// is essentially no added cost for doing so.) It is for this reason that
/// a DFA's support for anchored and unanchored searches can be configured
/// via [`Builder::start_kind`]. By default, a DFA only supports unanchored
/// searches.
///
/// # Example
///
/// This example shows how to build an `DFA` directly and use it to execute
/// [`Automaton::try_find`]:
///
/// ```
/// use aho_corasick::{
///     automaton::Automaton,
///     dfa::DFA,
///     Input, Match,
/// };
///
/// let patterns = &["b", "abc", "abcd"];
/// let haystack = "abcd";
///
/// let nfa = DFA::new(patterns).unwrap();
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
pub struct DFA {
    /// The DFA transition table. IDs in this table are pre-multiplied. So
    /// instead of the IDs being 0, 1, 2, 3, ..., they are 0*stride, 1*stride,
    /// 2*stride, 3*stride, ...
    trans: Vec<StateID>,
    /// The matches for every match state in this DFA. This is first indexed by
    /// state index (so that's `sid >> stride2`) and then by order in which the
    /// matches are meant to occur.
    matches: Vec<Vec<PatternID>>,
    /// The amount of heap memory used, in bytes, by the inner Vecs of
    /// 'matches'.
    matches_memory_usage: usize,
    /// The length of each pattern. This is used to compute the start offset
    /// of a match.
    pattern_lens: Vec<SmallIndex>,
    /// A prefilter for accelerating searches, if one exists.
    prefilter: Option<Prefilter>,
    /// The match semantics built into this DFA.
    match_kind: MatchKind,
    /// The total number of states in this DFA.
    state_len: usize,
    /// The alphabet size, or total number of equivalence classes, for this
    /// DFA. Note that the actual number of transitions in each state is
    /// stride=2^stride2, where stride is the smallest power of 2 greater than
    /// or equal to alphabet_len. We do things this way so that we can use
    /// bitshifting to go from a state ID to an index into 'matches'.
    alphabet_len: usize,
    /// The exponent with a base 2, such that stride=2^stride2. Given a state
    /// index 'i', its state identifier is 'i << stride2'. Given a state
    /// identifier 'sid', its state index is 'sid >> stride2'.
    stride2: usize,
    /// The equivalence classes for this DFA. All transitions are defined on
    /// equivalence classes and not on the 256 distinct byte values.
    byte_classes: ByteClasses,
    /// The length of the shortest pattern in this automaton.
    min_pattern_len: usize,
    /// The length of the longest pattern in this automaton.
    max_pattern_len: usize,
    /// The information required to deduce which states are "special" in this
    /// DFA.
    special: Special,
}

impl DFA {
    /// Create a new Aho-Corasick DFA using the default configuration.
    ///
    /// Use a [`Builder`] if you want to change the configuration.
    pub fn new<I, P>(patterns: I) -> Result<DFA, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        DFA::builder().build(patterns)
    }

    /// A convenience method for returning a new Aho-Corasick DFA builder.
    ///
    /// This usually permits one to just import the `DFA` type.
    pub fn builder() -> Builder {
        Builder::new()
    }
}

impl DFA {
    /// A sentinel state ID indicating that a search should stop once it has
    /// entered this state. When a search stops, it returns a match if one has
    /// been found, otherwise no match. A DFA always has an actual dead state
    /// at this ID.
    ///
    /// N.B. DFAs, unlike NFAs, do not have any notion of a FAIL state.
    /// Namely, the whole point of a DFA is that the FAIL state is completely
    /// compiled away. That is, DFA construction involves pre-computing the
    /// failure transitions everywhere, such that failure transitions are no
    /// longer used at search time. This, combined with its uniformly dense
    /// representation, are the two most important factors in why it's faster
    /// than the NFAs in this crate.
    const DEAD: StateID = StateID::new_unchecked(0);

    /// Adds the given pattern IDs as matches to the given state and also
    /// records the added memory usage.
    fn set_matches(
        &mut self,
        sid: StateID,
        pids: impl Iterator<Item = PatternID>,
    ) {
        let index = (sid.as_usize() >> self.stride2).checked_sub(2).unwrap();
        let mut at_least_one = false;
        for pid in pids {
            self.matches[index].push(pid);
            self.matches_memory_usage += PatternID::SIZE;
            at_least_one = true;
        }
        assert!(at_least_one, "match state must have non-empty pids");
    }
}

// SAFETY: 'start_state' always returns a valid state ID, 'next_state' always
// returns a valid state ID given a valid state ID. We otherwise claim that
// all other methods are correct as well.
unsafe impl Automaton for DFA {
    #[inline(always)]
    fn start_state(&self, anchored: Anchored) -> Result<StateID, MatchError> {
        // Either of the start state IDs can be DEAD, in which case, support
        // for that type of search is not provided by this DFA. Which start
        // state IDs are inactive depends on the 'StartKind' configuration at
        // DFA construction time.
        match anchored {
            Anchored::No => {
                let start = self.special.start_unanchored_id;
                if start == DFA::DEAD {
                    Err(MatchError::invalid_input_unanchored())
                } else {
                    Ok(start)
                }
            }
            Anchored::Yes => {
                let start = self.special.start_anchored_id;
                if start == DFA::DEAD {
                    Err(MatchError::invalid_input_anchored())
                } else {
                    Ok(start)
                }
            }
        }
    }

    #[inline(always)]
    fn next_state(
        &self,
        _anchored: Anchored,
        sid: StateID,
        byte: u8,
    ) -> StateID {
        let class = self.byte_classes.get(byte);
        self.trans[(sid.as_u32() + u32::from(class)).as_usize()]
    }

    #[inline(always)]
    fn is_special(&self, sid: StateID) -> bool {
        sid <= self.special.max_special_id
    }

    #[inline(always)]
    fn is_dead(&self, sid: StateID) -> bool {
        sid == DFA::DEAD
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
        debug_assert!(self.is_match(sid));
        let offset = (sid.as_usize() >> self.stride2) - 2;
        self.matches[offset].len()
    }

    #[inline(always)]
    fn match_pattern(&self, sid: StateID, index: usize) -> PatternID {
        debug_assert!(self.is_match(sid));
        let offset = (sid.as_usize() >> self.stride2) - 2;
        self.matches[offset][index]
    }

    #[inline(always)]
    fn memory_usage(&self) -> usize {
        use core::mem::size_of;

        (self.trans.len() * size_of::<u32>())
            + (self.matches.len() * size_of::<Vec<PatternID>>())
            + self.matches_memory_usage
            + (self.pattern_lens.len() * size_of::<SmallIndex>())
            + self.prefilter.as_ref().map_or(0, |p| p.memory_usage())
    }

    #[inline(always)]
    fn prefilter(&self) -> Option<&Prefilter> {
        self.prefilter.as_ref()
    }
}

impl core::fmt::Debug for DFA {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::{
            automaton::{fmt_state_indicator, sparse_transitions},
            util::debug::DebugByte,
        };

        writeln!(f, "dfa::DFA(")?;
        for index in 0..self.state_len {
            let sid = StateID::new_unchecked(index << self.stride2);
            // While we do currently include the FAIL state in the transition
            // table (to simplify construction), it is never actually used. It
            // poses problems with the code below because it gets treated as
            // a match state incidentally when it is, of course, not. So we
            // special case it. The fail state is always the first state after
            // the dead state.
            //
            // If the construction is changed to remove the fail state (it
            // probably should be), then this special case should be updated.
            if index == 1 {
                writeln!(f, "F {:06}:", sid.as_usize())?;
                continue;
            }
            fmt_state_indicator(f, self, sid)?;
            write!(f, "{:06}: ", sid.as_usize())?;

            let it = (0..self.byte_classes.alphabet_len()).map(|class| {
                (class.as_u8(), self.trans[sid.as_usize() + class])
            });
            for (i, (start, end, next)) in sparse_transitions(it).enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                if start == end {
                    write!(
                        f,
                        "{:?} => {:?}",
                        DebugByte(start),
                        next.as_usize()
                    )?;
                } else {
                    write!(
                        f,
                        "{:?}-{:?} => {:?}",
                        DebugByte(start),
                        DebugByte(end),
                        next.as_usize()
                    )?;
                }
            }
            write!(f, "\n")?;
            if self.is_match(sid) {
                write!(f, " matches: ")?;
                for i in 0..self.match_len(sid) {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    let pid = self.match_pattern(sid, i);
                    write!(f, "{}", pid.as_usize())?;
                }
                write!(f, "\n")?;
            }
        }
        writeln!(f, "match kind: {:?}", self.match_kind)?;
        writeln!(f, "prefilter: {:?}", self.prefilter.is_some())?;
        writeln!(f, "state length: {:?}", self.state_len)?;
        writeln!(f, "pattern length: {:?}", self.patterns_len())?;
        writeln!(f, "shortest pattern length: {:?}", self.min_pattern_len)?;
        writeln!(f, "longest pattern length: {:?}", self.max_pattern_len)?;
        writeln!(f, "alphabet length: {:?}", self.alphabet_len)?;
        writeln!(f, "stride: {:?}", 1 << self.stride2)?;
        writeln!(f, "byte classes: {:?}", self.byte_classes)?;
        writeln!(f, "memory usage: {:?}", self.memory_usage())?;
        writeln!(f, ")")?;
        Ok(())
    }
}

/// A builder for configuring an Aho-Corasick DFA.
///
/// This builder has a subset of the options available to a
/// [`AhoCorasickBuilder`](crate::AhoCorasickBuilder). Of the shared options,
/// their behavior is identical.
#[derive(Clone, Debug)]
pub struct Builder {
    noncontiguous: noncontiguous::Builder,
    start_kind: StartKind,
    byte_classes: bool,
}

impl Default for Builder {
    fn default() -> Builder {
        Builder {
            noncontiguous: noncontiguous::Builder::new(),
            start_kind: StartKind::Unanchored,
            byte_classes: true,
        }
    }
}

impl Builder {
    /// Create a new builder for configuring an Aho-Corasick DFA.
    pub fn new() -> Builder {
        Builder::default()
    }

    /// Build an Aho-Corasick DFA from the given iterator of patterns.
    ///
    /// A builder may be reused to create more DFAs.
    pub fn build<I, P>(&self, patterns: I) -> Result<DFA, BuildError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        let nnfa = self.noncontiguous.build(patterns)?;
        self.build_from_noncontiguous(&nnfa)
    }

    /// Build an Aho-Corasick DFA from the given noncontiguous NFA.
    ///
    /// Note that when this method is used, only the `start_kind` and
    /// `byte_classes` settings on this builder are respected. The other
    /// settings only apply to the initial construction of the Aho-Corasick
    /// automaton. Since using this method requires that initial construction
    /// has already completed, all settings impacting only initial construction
    /// are no longer relevant.
    pub fn build_from_noncontiguous(
        &self,
        nnfa: &noncontiguous::NFA,
    ) -> Result<DFA, BuildError> {
        debug!("building DFA");
        let byte_classes = if self.byte_classes {
            nnfa.byte_classes().clone()
        } else {
            ByteClasses::singletons()
        };
        let state_len = match self.start_kind {
            StartKind::Unanchored | StartKind::Anchored => nnfa.states().len(),
            StartKind::Both => {
                // These unwraps are OK because we know that the number of
                // NFA states is < StateID::LIMIT which is in turn less than
                // i32::MAX. Thus, there is always room to multiply by 2.
                // Finally, the number of states is always at least 4 in the
                // NFA (DEAD, FAIL, START-UNANCHORED, START-ANCHORED), so the
                // subtraction of 4 is okay.
                //
                // Note that we subtract 4 because the "anchored" part of
                // the DFA duplicates the unanchored part (without failure
                // transitions), but reuses the DEAD, FAIL and START states.
                nnfa.states()
                    .len()
                    .checked_mul(2)
                    .unwrap()
                    .checked_sub(4)
                    .unwrap()
            }
        };
        let trans_len =
            match state_len.checked_shl(byte_classes.stride2().as_u32()) {
                Some(trans_len) => trans_len,
                None => {
                    return Err(BuildError::state_id_overflow(
                        StateID::MAX.as_u64(),
                        usize::MAX.as_u64(),
                    ))
                }
            };
        StateID::new(trans_len.checked_sub(byte_classes.stride()).unwrap())
            .map_err(|e| {
                BuildError::state_id_overflow(
                    StateID::MAX.as_u64(),
                    e.attempted(),
                )
            })?;
        let num_match_states = match self.start_kind {
            StartKind::Unanchored | StartKind::Anchored => {
                nnfa.special().max_match_id.as_usize().checked_sub(1).unwrap()
            }
            StartKind::Both => nnfa
                .special()
                .max_match_id
                .as_usize()
                .checked_sub(1)
                .unwrap()
                .checked_mul(2)
                .unwrap(),
        };
        let mut dfa = DFA {
            trans: vec![DFA::DEAD; trans_len],
            matches: vec![vec![]; num_match_states],
            matches_memory_usage: 0,
            pattern_lens: nnfa.pattern_lens_raw().to_vec(),
            prefilter: nnfa.prefilter().map(|p| p.clone()),
            match_kind: nnfa.match_kind(),
            state_len,
            alphabet_len: byte_classes.alphabet_len(),
            stride2: byte_classes.stride2(),
            byte_classes,
            min_pattern_len: nnfa.min_pattern_len(),
            max_pattern_len: nnfa.max_pattern_len(),
            // The special state IDs are set later.
            special: Special::zero(),
        };
        match self.start_kind {
            StartKind::Both => {
                self.finish_build_both_starts(nnfa, &mut dfa);
            }
            StartKind::Unanchored => {
                self.finish_build_one_start(Anchored::No, nnfa, &mut dfa);
            }
            StartKind::Anchored => {
                self.finish_build_one_start(Anchored::Yes, nnfa, &mut dfa)
            }
        }
        debug!(
            "DFA built, <states: {:?}, size: {:?}, \
             alphabet len: {:?}, stride: {:?}>",
            dfa.state_len,
            dfa.memory_usage(),
            dfa.byte_classes.alphabet_len(),
            dfa.byte_classes.stride(),
        );
        // The vectors can grow ~twice as big during construction because a
        // Vec amortizes growth. But here, let's shrink things back down to
        // what we actually need since we're never going to add more to it.
        dfa.trans.shrink_to_fit();
        dfa.pattern_lens.shrink_to_fit();
        dfa.matches.shrink_to_fit();
        // TODO: We might also want to shrink each Vec inside of `dfa.matches`,
        // or even better, convert it to one contiguous allocation. But I think
        // I went with nested allocs for good reason (can't remember), so this
        // may be tricky to do. I decided not to shrink them here because it
        // might require a fair bit of work to do. It's unclear whether it's
        // worth it.
        Ok(dfa)
    }

    /// Finishes building a DFA for either unanchored or anchored searches,
    /// but NOT both.
    fn finish_build_one_start(
        &self,
        anchored: Anchored,
        nnfa: &noncontiguous::NFA,
        dfa: &mut DFA,
    ) {
        // This function always succeeds because we check above that all of the
        // states in the NFA can be mapped to DFA state IDs.
        let stride2 = dfa.stride2;
        let old2new = |oldsid: StateID| {
            StateID::new_unchecked(oldsid.as_usize() << stride2)
        };
        for (oldsid, state) in nnfa.states().iter().with_state_ids() {
            let newsid = old2new(oldsid);
            if state.is_match() {
                dfa.set_matches(newsid, nnfa.iter_matches(oldsid));
            }
            sparse_iter(
                nnfa,
                oldsid,
                &dfa.byte_classes,
                |byte, class, mut oldnextsid| {
                    if oldnextsid == noncontiguous::NFA::FAIL {
                        if anchored.is_anchored() {
                            oldnextsid = noncontiguous::NFA::DEAD;
                        } else if state.fail() == noncontiguous::NFA::DEAD {
                            // This is a special case that avoids following
                            // DEAD transitions in a non-contiguous NFA.
                            // Following these transitions is pretty slow
                            // because the non-contiguous NFA will always use
                            // a sparse representation for it (because the
                            // DEAD state is usually treated as a sentinel).
                            // The *vast* majority of failure states are DEAD
                            // states, so this winds up being pretty slow if
                            // we go through the non-contiguous NFA state
                            // transition logic. Instead, just do it ourselves.
                            oldnextsid = noncontiguous::NFA::DEAD;
                        } else {
                            oldnextsid = nnfa.next_state(
                                Anchored::No,
                                state.fail(),
                                byte,
                            );
                        }
                    }
                    dfa.trans[newsid.as_usize() + usize::from(class)] =
                        old2new(oldnextsid);
                },
            );
        }
        // Now that we've remapped all the IDs in our states, all that's left
        // is remapping the special state IDs.
        let old = nnfa.special();
        let new = &mut dfa.special;
        new.max_special_id = old2new(old.max_special_id);
        new.max_match_id = old2new(old.max_match_id);
        if anchored.is_anchored() {
            new.start_unanchored_id = DFA::DEAD;
            new.start_anchored_id = old2new(old.start_anchored_id);
        } else {
            new.start_unanchored_id = old2new(old.start_unanchored_id);
            new.start_anchored_id = DFA::DEAD;
        }
    }

    /// Finishes building a DFA that supports BOTH unanchored and anchored
    /// searches. It works by inter-leaving unanchored states with anchored
    /// states in the same transition table. This way, we avoid needing to
    /// re-shuffle states afterward to ensure that our states still look like
    /// DEAD, MATCH, ..., START-UNANCHORED, START-ANCHORED, NON-MATCH, ...
    ///
    /// Honestly this is pretty inscrutable... Simplifications are most
    /// welcome.
    fn finish_build_both_starts(
        &self,
        nnfa: &noncontiguous::NFA,
        dfa: &mut DFA,
    ) {
        let stride2 = dfa.stride2;
        let stride = 1 << stride2;
        let mut remap_unanchored = vec![DFA::DEAD; nnfa.states().len()];
        let mut remap_anchored = vec![DFA::DEAD; nnfa.states().len()];
        let mut is_anchored = vec![false; dfa.state_len];
        let mut newsid = DFA::DEAD;
        let next_dfa_id =
            |sid: StateID| StateID::new_unchecked(sid.as_usize() + stride);
        for (oldsid, state) in nnfa.states().iter().with_state_ids() {
            if oldsid == noncontiguous::NFA::DEAD
                || oldsid == noncontiguous::NFA::FAIL
            {
                remap_unanchored[oldsid] = newsid;
                remap_anchored[oldsid] = newsid;
                newsid = next_dfa_id(newsid);
            } else if oldsid == nnfa.special().start_unanchored_id
                || oldsid == nnfa.special().start_anchored_id
            {
                if oldsid == nnfa.special().start_unanchored_id {
                    remap_unanchored[oldsid] = newsid;
                    remap_anchored[oldsid] = DFA::DEAD;
                } else {
                    remap_unanchored[oldsid] = DFA::DEAD;
                    remap_anchored[oldsid] = newsid;
                    is_anchored[newsid.as_usize() >> stride2] = true;
                }
                if state.is_match() {
                    dfa.set_matches(newsid, nnfa.iter_matches(oldsid));
                }
                sparse_iter(
                    nnfa,
                    oldsid,
                    &dfa.byte_classes,
                    |_, class, oldnextsid| {
                        let class = usize::from(class);
                        if oldnextsid == noncontiguous::NFA::FAIL {
                            dfa.trans[newsid.as_usize() + class] = DFA::DEAD;
                        } else {
                            dfa.trans[newsid.as_usize() + class] = oldnextsid;
                        }
                    },
                );
                newsid = next_dfa_id(newsid);
            } else {
                let unewsid = newsid;
                newsid = next_dfa_id(newsid);
                let anewsid = newsid;
                newsid = next_dfa_id(newsid);

                remap_unanchored[oldsid] = unewsid;
                remap_anchored[oldsid] = anewsid;
                is_anchored[anewsid.as_usize() >> stride2] = true;
                if state.is_match() {
                    dfa.set_matches(unewsid, nnfa.iter_matches(oldsid));
                    dfa.set_matches(anewsid, nnfa.iter_matches(oldsid));
                }
                sparse_iter(
                    nnfa,
                    oldsid,
                    &dfa.byte_classes,
                    |byte, class, oldnextsid| {
                        let class = usize::from(class);
                        if oldnextsid == noncontiguous::NFA::FAIL {
                            let oldnextsid =
                                if state.fail() == noncontiguous::NFA::DEAD {
                                    noncontiguous::NFA::DEAD
                                } else {
                                    nnfa.next_state(
                                        Anchored::No,
                                        state.fail(),
                                        byte,
                                    )
                                };
                            dfa.trans[unewsid.as_usize() + class] = oldnextsid;
                        } else {
                            dfa.trans[unewsid.as_usize() + class] = oldnextsid;
                            dfa.trans[anewsid.as_usize() + class] = oldnextsid;
                        }
                    },
                );
            }
        }
        for i in 0..dfa.state_len {
            let sid = i << stride2;
            if is_anchored[i] {
                for next in dfa.trans[sid..][..stride].iter_mut() {
                    *next = remap_anchored[*next];
                }
            } else {
                for next in dfa.trans[sid..][..stride].iter_mut() {
                    *next = remap_unanchored[*next];
                }
            }
        }
        // Now that we've remapped all the IDs in our states, all that's left
        // is remapping the special state IDs.
        let old = nnfa.special();
        let new = &mut dfa.special;
        new.max_special_id = remap_anchored[old.max_special_id];
        new.max_match_id = remap_anchored[old.max_match_id];
        new.start_unanchored_id = remap_unanchored[old.start_unanchored_id];
        new.start_anchored_id = remap_anchored[old.start_anchored_id];
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

    /// Sets the starting state configuration for the automaton.
    ///
    /// See
    /// [`AhoCorasickBuilder::start_kind`](crate::AhoCorasickBuilder::start_kind)
    /// for more documentation and examples.
    pub fn start_kind(&mut self, kind: StartKind) -> &mut Builder {
        self.start_kind = kind;
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

/// Iterate over all possible equivalence class transitions in this state.
/// The closure is called for all transitions with a distinct equivalence
/// class, even those not explicitly represented in this sparse state. For
/// any implicitly defined transitions, the given closure is called with
/// the fail state ID.
///
/// The closure is guaranteed to be called precisely
/// `byte_classes.alphabet_len()` times, once for every possible class in
/// ascending order.
fn sparse_iter<F: FnMut(u8, u8, StateID)>(
    nnfa: &noncontiguous::NFA,
    oldsid: StateID,
    classes: &ByteClasses,
    mut f: F,
) {
    let mut prev_class = None;
    let mut byte = 0usize;
    for t in nnfa.iter_trans(oldsid) {
        while byte < usize::from(t.byte()) {
            let rep = byte.as_u8();
            let class = classes.get(rep);
            byte += 1;
            if prev_class != Some(class) {
                f(rep, class, noncontiguous::NFA::FAIL);
                prev_class = Some(class);
            }
        }
        let rep = t.byte();
        let class = classes.get(rep);
        byte += 1;
        if prev_class != Some(class) {
            f(rep, class, t.next());
            prev_class = Some(class);
        }
    }
    for b in byte..=255 {
        let rep = b.as_u8();
        let class = classes.get(rep);
        if prev_class != Some(class) {
            f(rep, class, noncontiguous::NFA::FAIL);
            prev_class = Some(class);
        }
    }
}
