use alloc::{vec, vec::Vec};

use crate::{
    int::{NonMaxUsize, U32},
    nfa::{State, StateID, NFA},
    pool::CachePoolGuard,
    utf8,
};

/// A PikeVM searcher.
///
/// A PikeVM uses the standard Thompson NFA linear time search algorithm, but
/// augmented to support tracking the offsets of matching capture groups.
#[derive(Clone, Debug)]
pub(crate) struct PikeVM {
    nfa: NFA,
}

impl PikeVM {
    /// Create a new PikeVM searcher that uses the given NFA.
    pub(crate) fn new(nfa: NFA) -> PikeVM {
        PikeVM { nfa }
    }

    /// Return the underlying NFA used by this PikeVM.
    pub(crate) fn nfa(&self) -> &NFA {
        &self.nfa
    }

    /// Returns an iterator of non-overlapping matches in the given haystack.
    pub(crate) fn find_iter<'r, 'h>(
        &'r self,
        cache: CachePoolGuard<'r>,
        haystack: &'h [u8],
    ) -> FindMatches<'r, 'h> {
        FindMatches {
            pikevm: self,
            cache,
            haystack,
            at: 0,
            slots: vec![None, None],
            last_match_end: None,
        }
    }

    /// Returns an iterator of non-overlapping capture matches in the given
    /// haystack.
    pub(crate) fn captures_iter<'r, 'h>(
        &'r self,
        cache: CachePoolGuard<'r>,
        haystack: &'h [u8],
    ) -> CapturesMatches<'r, 'h> {
        // OK because the NFA wouldn't have compiled if this could overflow.
        let len = self.nfa().group_len().checked_mul(2).unwrap();
        CapturesMatches {
            it: FindMatches {
                pikevm: self,
                cache,
                haystack,
                at: 0,
                slots: vec![None; len],
                last_match_end: None,
            },
        }
    }

    /// The implementation of standard leftmost search.
    ///
    /// Capturing group spans are written to `slots`, but only if requested.
    /// `slots` can be any length. Any slot in the NFA that is activated but
    /// which is out of bounds for the given `slots` is ignored.
    pub(crate) fn search(
        &self,
        cache: &mut Cache,
        haystack: &[u8],
        start: usize,
        end: usize,
        earliest: bool,
        slots: &mut [Option<NonMaxUsize>],
    ) -> bool {
        cache.setup_search(slots.len());
        if start > end {
            return false;
        }
        // Why do we even care about this? Well, in our `slots` representation,
        // we use usize::MAX as a sentinel to indicate "no match." This isn't
        // problematic so long as our haystack doesn't have a maximal length.
        // Byte slices are guaranteed by Rust to have a length that fits into
        // isize, and so this assert should always pass. But we put it here to
        // make our assumption explicit.
        assert!(
            haystack.len() < core::usize::MAX,
            "byte slice lengths must be less than usize MAX",
        );

        let Cache { ref mut stack, ref mut curr, ref mut next } = cache;
        let start_id = self.nfa().start();
        let anchored = self.nfa().is_start_anchored();
        let mut matched = false;
        // Yes, our search doesn't end at `end`, but includes it. This is
        // necessary because matches are delayed by one byte. The delay is used
        // to handle look-behind assertions. In the case of the PikeVM, the
        // delay is implemented by not considering a match to exist until it
        // is visited in `nexts`. Technically, we know a match exists in the
        // previous iteration via `epsilon_closure`.
        let mut at = start;
        while at <= end {
            // If we have no states left to visit, then there are some cases
            // where we know we can quit early or even skip ahead.
            if curr.set.is_empty() {
                // We have a match so we can quit.
                if matched {
                    break;
                }
                // If we're running an anchored search and we've advanced
                // beyond the start position with no other states to try, then
                // we will never observe a match and thus can stop.
                if anchored && at > start {
                    break;
                }
            }
            // Instead of using a hypothetical unanchored start state in the
            // NFA (which doesn't exist, but we could add it), we actually
            // always use its anchored starting state. As a result, when doing
            // an unanchored search, we need to simulate our own '(?s:.)*?'
            // prefix, to permit a match to appear anywhere.
            //
            // Now, we don't *have* to do things this way. We could create
            // a proper unanchored start state in the NFA and do one
            // `epsilon_closure` call from that starting state before the main
            // loop here. And that is just as correct. However, it turns out to
            // be slower than our approach here because it slightly increases
            // the cost of processing each byte by requiring us to visit
            // more NFA states to deal with the additional NFA states in the
            // unanchored prefix. By simulating it explicitly here, we lower
            // those costs substantially. The cost is itself small, but it adds
            // up for large haystacks.
            //
            // In order to simulate the '(?s:.)*?' prefix---which is not
            // greedy---we are careful not to perform an epsilon closure on
            // the start state if we already have a match. Namely, if we
            // did otherwise, we would never reach a terminating condition
            // because there would always be additional states to process.
            if !matched {
                // Since we are adding to the 'curr' active states and since
                // this is for the start ID, we use a slots slice that is
                // guaranteed to have the right length but where every element
                // is absent. This is exactly what we want, because this
                // epsilon closure is responsible for simulating an unanchored
                // '(?s:.)*?' prefix. It is specifically outside of any
                // capturing groups, and thus, using slots that are always
                // absent is correct.
                //
                // Note though that we can't just use `&mut []` here, since
                // this epsilon closure may traverse through `Capture` states
                // transitions, and thus must be able to write offsets to the
                // slots given which are later copied to slot values in `curr`.
                let slots = next.slot_table.all_absent();
                self.epsilon_closure(
                    stack, slots, curr, haystack, at, start_id,
                );
            }
            let (ch, len) = utf8::decode_lossy(&haystack[at..]);
            if self.nexts(stack, curr, next, haystack, at, ch, len, slots) {
                matched = true;
            }
            // Unless the caller asked us to return early, we need to mush
            // on to see if we can extend our match. (But note that 'nexts'
            // will quit right after seeing a match, as is consistent with
            // leftmost-first match priority.)
            if (earliest && matched) || len == 0 {
                break;
            }
            core::mem::swap(curr, next);
            next.set.clear();
            at += len;
        }
        matched
    }

    /// Process the active states in 'curr' to find the states (written to
    /// 'next') we should process for the next byte in the haystack.
    ///
    /// 'stack' is used to perform a depth first traversal of the NFA when
    /// computing an epsilon closure.
    ///
    /// When a match is found, the slots for that match state (in 'curr') are
    /// copied to 'caps'. Moreover, once a match is seen, processing for 'curr'
    /// stops (unless the PikeVM was configured with MatchKind::All semantics).
    ///
    /// `at_ch` is the Unicode scalar value whose UTF-8 encoding begins at `at`
    /// in `haystack`.
    ///
    /// `at_len` is the number of bytes consumed by `at_ch`. This is usually
    /// equal to `at_ch.len_utf8()`, but not always. For example, in the case
    /// where `at_ch` is the replacement codepoint that results from decoding
    /// invalid UTF-8. In that case, `at_len` can be 1, 2 or 3.
    fn nexts(
        &self,
        stack: &mut Vec<FollowEpsilon>,
        curr: &mut ActiveStates,
        next: &mut ActiveStates,
        haystack: &[u8],
        at: usize,
        at_ch: char,
        at_len: usize,
        slots: &mut [Option<NonMaxUsize>],
    ) -> bool {
        let ActiveStates { ref set, ref mut slot_table } = *curr;
        for sid in set.iter() {
            if self.next(
                stack, slot_table, next, haystack, at, at_ch, at_len, sid,
            ) {
                slots.copy_from_slice(slot_table.for_state(sid));
                return true;
            }
        }
        false
    }

    /// Starting from `sid`, if the position `at` in the `haystack` has a
    /// transition defined out of `sid`, then add the state transitioned to and
    /// its epsilon closure to the `next` set of states to explore.
    ///
    /// `stack` is used by the epsilon closure computation to perform a depth
    /// first traversal of the NFA.
    ///
    /// `curr_slot_table` should be the table of slots for the current set of
    /// states being explored. If there is a transition out of `sid`, then
    /// sid's row in the slot table is used to perform the epsilon closure.
    ///
    /// `at_ch` is the Unicode scalar value whose UTF-8 encoding begins at `at`
    /// in `haystack`. The caller provides it so that this routine doesn't
    /// need to re-decode it. (Since it's expected that this routine is called
    /// multiple times for each position.)
    ///
    /// `at_len` is the number of bytes consumed by `at_ch`. This is usually
    /// equal to `at_ch.len_utf8()`, but not always. For example, in the case
    /// where `at_ch` is the replacement codepoint that results from decoding
    /// invalid UTF-8. In that case, `at_len` can be 1, 2 or 3.
    fn next(
        &self,
        stack: &mut Vec<FollowEpsilon>,
        curr_slot_table: &mut SlotTable,
        next: &mut ActiveStates,
        haystack: &[u8],
        at: usize,
        at_ch: char,
        at_len: usize,
        sid: StateID,
    ) -> bool {
        match *self.nfa.state(sid) {
            State::Fail
            | State::Goto { .. }
            | State::Splits { .. }
            | State::Capture { .. } => false,
            State::Char { target, ch } => {
                if at_ch == ch && at_len > 0 {
                    let slots = curr_slot_table.for_state(sid);
                    // OK because `at_len` is always derived from the number
                    // of bytes read from `at` that make up `at_ch`. So this
                    // will never wrap.
                    let at = at.wrapping_add(at_len);
                    self.epsilon_closure(
                        stack, slots, next, haystack, at, target,
                    );
                }
                false
            }
            State::Ranges { target, ref ranges } => {
                for (start, end) in ranges.iter().copied() {
                    if start > at_ch {
                        break;
                    } else if start <= at_ch && at_ch <= end {
                        if at_len == 0 {
                            return false;
                        }
                        let slots = curr_slot_table.for_state(sid);
                        // OK because `at_len` is always derived from the
                        // number of bytes read from `at` that make up `at_ch`.
                        // So this will never wrap.
                        let at = at.wrapping_add(at_len);
                        self.epsilon_closure(
                            stack, slots, next, haystack, at, target,
                        );
                    }
                }
                false
            }
            State::Match => true,
        }
    }

    /// Compute the epsilon closure of `sid`, writing the closure into `next`
    /// while copying slot values from `curr_slots` into corresponding states
    /// in `next`. `curr_slots` should be the slot values corresponding to
    /// `sid`.
    ///
    /// The given `stack` is used to perform a depth first traversal of the
    /// NFA by recursively following all epsilon transitions out of `sid`.
    /// Conditional epsilon transitions are followed if and only if they are
    /// satisfied for the position `at` in the `input` haystack.
    ///
    /// While this routine may write to `curr_slots`, once it returns, any
    /// writes are undone and the original values (even if absent) are
    /// restored.
    fn epsilon_closure(
        &self,
        stack: &mut Vec<FollowEpsilon>,
        curr_slots: &mut [Option<NonMaxUsize>],
        next: &mut ActiveStates,
        haystack: &[u8],
        at: usize,
        sid: StateID,
    ) {
        stack.push(FollowEpsilon::Explore(sid));
        while let Some(frame) = stack.pop() {
            match frame {
                FollowEpsilon::RestoreCapture { slot, offset } => {
                    curr_slots[slot.as_usize()] = offset;
                }
                FollowEpsilon::Explore(sid) => {
                    self.epsilon_closure_explore(
                        stack, curr_slots, next, haystack, at, sid,
                    );
                }
            }
        }
    }

    /// Explore all of the epsilon transitions out of `sid`. This is mostly
    /// split out from `epsilon_closure` in order to clearly delineate
    /// the actual work of computing an epsilon closure from the stack
    /// book-keeping.
    ///
    /// This will push any additional explorations needed on to `stack`.
    ///
    /// `curr_slots` should refer to the slots for the currently active NFA
    /// state. That is, the current state we are stepping through. These
    /// slots are mutated in place as new `Captures` states are traversed
    /// during epsilon closure, but the slots are restored to their original
    /// values once the full epsilon closure is completed. The ultimate use of
    /// `curr_slots` is to copy them to the corresponding `next_slots`, so that
    /// the capturing group spans are forwarded from the currently active state
    /// to the next.
    ///
    /// `next` refers to the next set of active states. Computing an epsilon
    /// closure may increase the next set of active states.
    ///
    /// `haystack` refers to the what we're searching and `at` refers to the
    /// current position in the haystack. These are used to check whether
    /// conditional epsilon transitions (like look-around) are satisfied at
    /// the current position. If they aren't, then the epsilon closure won't
    /// include them.
    fn epsilon_closure_explore(
        &self,
        stack: &mut Vec<FollowEpsilon>,
        curr_slots: &mut [Option<NonMaxUsize>],
        next: &mut ActiveStates,
        haystack: &[u8],
        at: usize,
        mut sid: StateID,
    ) {
        // We can avoid pushing some state IDs on to our stack in precisely
        // the cases where a 'push(x)' would be immediately followed by a 'x
        // = pop()'. This is achieved by this outer-loop. We simply set 'sid'
        // to be the next state ID we want to explore once we're done with
        // our initial exploration. In practice, this avoids a lot of stack
        // thrashing.
        loop {
            // Record this state as part of our next set of active states. If
            // we've already explored it, then no need to do it again.
            if !next.set.insert(sid) {
                return;
            }
            match *self.nfa.state(sid) {
                State::Fail
                | State::Match { .. }
                | State::Char { .. }
                | State::Ranges { .. } => {
                    next.slot_table.for_state(sid).copy_from_slice(curr_slots);
                    return;
                }
                State::Goto { target, look: None } => {
                    sid = target;
                }
                State::Goto { target, look: Some(look) } => {
                    if !look.is_match(haystack, at) {
                        return;
                    }
                    sid = target;
                }
                State::Splits { ref targets, reverse: false } => {
                    sid = match targets.get(0) {
                        None => return,
                        Some(&sid) => sid,
                    };
                    stack.extend(
                        targets[1..]
                            .iter()
                            .copied()
                            .rev()
                            .map(FollowEpsilon::Explore),
                    );
                }
                State::Splits { ref targets, reverse: true } => {
                    sid = match targets.last() {
                        None => return,
                        Some(&sid) => sid,
                    };
                    stack.extend(
                        targets[..targets.len() - 1]
                            .iter()
                            .copied()
                            .map(FollowEpsilon::Explore),
                    );
                }
                State::Capture { target, slot } => {
                    // There's no need to do anything with slots that
                    // ultimately won't be copied into the caller-provided
                    // 'Captures' value. So we just skip dealing with them at
                    // all.
                    if slot.as_usize() < curr_slots.len() {
                        stack.push(FollowEpsilon::RestoreCapture {
                            slot,
                            offset: curr_slots[slot.as_usize()],
                        });
                        // OK because length of a slice must fit into an isize.
                        curr_slots[slot.as_usize()] =
                            Some(NonMaxUsize::new(at).unwrap());
                    }
                    sid = target;
                }
            }
        }
    }
}

/// An iterator over all successive non-overlapping matches in a particular
/// haystack. `'r` represents the lifetime of the regex, `'c` is the lifetime
/// of the cache and `'h` represents the lifetime of the haystack.
#[derive(Debug)]
pub(crate) struct FindMatches<'r, 'h> {
    pikevm: &'r PikeVM,
    cache: CachePoolGuard<'r>,
    haystack: &'h [u8],
    at: usize,
    slots: Vec<Option<NonMaxUsize>>,
    last_match_end: Option<usize>,
}

impl<'r, 'h> Iterator for FindMatches<'r, 'h> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<(usize, usize)> {
        if !self.pikevm.search(
            &mut self.cache,
            self.haystack,
            self.at,
            self.haystack.len(),
            false,
            &mut self.slots,
        ) {
            return None;
        }
        let mut m =
            (self.slots[0].unwrap().get(), self.slots[1].unwrap().get());
        if m.0 >= m.1 {
            m = self.handle_overlapping_empty_match(m)?;
        }
        self.at = m.1;
        self.last_match_end = Some(m.1);
        Some(m)
    }
}

impl<'r, 'h> FindMatches<'r, 'h> {
    /// Handles the special case of an empty match by ensuring that 1) the
    /// iterator always advances and 2) empty matches never overlap with other
    /// matches.
    ///
    /// Note that we mark this cold and forcefully prevent inlining because
    /// handling empty matches like this is extremely rare and does require a
    /// bit of code, comparatively. Keeping this code out of the main iterator
    /// function keeps it smaller and more amenable to inlining itself.
    #[cold]
    #[inline(never)]
    fn handle_overlapping_empty_match(
        &mut self,
        mut m: (usize, usize),
    ) -> Option<(usize, usize)> {
        assert!(m.0 >= m.1);
        if Some(m.1) == self.last_match_end {
            let len =
                core::cmp::max(1, utf8::decode(&self.haystack[self.at..]).1);
            self.at = self.at.checked_add(len).unwrap();
            if !self.pikevm.search(
                &mut self.cache,
                self.haystack,
                self.at,
                self.haystack.len(),
                false,
                &mut self.slots,
            ) {
                return None;
            }
            m = (self.slots[0].unwrap().get(), self.slots[1].unwrap().get());
        }
        Some(m)
    }
}

/// An iterator over all successive non-overlapping capture matches in a particular
/// haystack. `'r` represents the lifetime of the regex, `'c` is the lifetime
/// of the cache and `'h` represents the lifetime of the haystack.
#[derive(Debug)]
pub(crate) struct CapturesMatches<'r, 'h> {
    it: FindMatches<'r, 'h>,
}

impl<'r, 'h> Iterator for CapturesMatches<'r, 'h> {
    type Item = Vec<Option<NonMaxUsize>>;

    fn next(&mut self) -> Option<Vec<Option<NonMaxUsize>>> {
        self.it.next()?;
        Some(self.it.slots.clone())
    }
}

/// A cache represents mutable state that a `PikeVM` requires during a search.
///
/// For a given `PikeVM`, its corresponding cache may be created either via
/// `PikeVM::create_cache`, or via `Cache::new`. They are equivalent in every
/// way, except the former does not require explicitly importing `Cache`.
///
/// A particular `Cache` is coupled with the `PikeVM` from which it was
/// created. It may only be used with that `PikeVM`. A cache and its
/// allocations may be re-purposed via `Cache::reset`, in which case, it can
/// only be used with the new `PikeVM` (and not the old one).
#[derive(Clone, Debug)]
pub(crate) struct Cache {
    /// Stack used while computing epsilon closure. This effectively lets us
    /// move what is more naturally expressed through recursion to a stack
    /// on the heap.
    stack: Vec<FollowEpsilon>,
    /// The current active states being explored for the current byte in the
    /// haystack.
    curr: ActiveStates,
    /// The next set of states we're building that will be explored for the
    /// next byte in the haystack.
    next: ActiveStates,
}

impl Cache {
    /// Create a new `PikeVM` cache.
    ///
    /// A potentially more convenient routine to create a cache is
    /// `PikeVM::create_cache`, as it does not require also importing the
    /// `Cache` type.
    ///
    /// If you want to reuse the returned `Cache` with some other `PikeVM`,
    /// then you must call `Cache::reset` with the desired `PikeVM`.
    pub(crate) fn new(re: &PikeVM) -> Cache {
        Cache {
            stack: vec![],
            curr: ActiveStates::new(re),
            next: ActiveStates::new(re),
        }
    }

    /// Clears this cache. This should be called at the start of every search
    /// to ensure we start with a clean slate.
    ///
    /// This also sets the length of the capturing groups used in the current
    /// search. This permits an optimization where by 'SlotTable::for_state'
    /// only returns the number of slots equivalent to the number of slots
    /// given in the 'Captures' value. This may be less than the total number
    /// of possible slots, e.g., when one only wants to track overall match
    /// offsets. This in turn permits less copying of capturing group spans
    /// in the PikeVM.
    fn setup_search(&mut self, captures_slot_len: usize) {
        self.stack.clear();
        self.curr.setup_search(captures_slot_len);
        self.next.setup_search(captures_slot_len);
    }
}

/// A set of active states used to "simulate" the execution of an NFA via the
/// PikeVM.
///
/// There are two sets of these used during NFA simulation. One set corresponds
/// to the "current" set of states being traversed for the current position
/// in a haystack. The other set corresponds to the "next" set of states being
/// built, which will become the new "current" set for the next position in the
/// haystack. These two sets correspond to CLIST and NLIST in Thompson's
/// original paper regexes: https://dl.acm.org/doi/pdf/10.1145/363347.363387
///
/// In addition to representing a set of NFA states, this also maintains slot
/// values for each state. These slot values are what turn the NFA simulation
/// into the "Pike VM." Namely, they track capturing group values for each
/// state. During the computation of epsilon closure, we copy slot values from
/// states in the "current" set to the "next" set. Eventually, once a match
/// is found, the slot values for that match state are what we write to the
/// caller provided slots.
#[derive(Clone, Debug)]
struct ActiveStates {
    /// The set of active NFA states. This set preserves insertion order, which
    /// is critical for simulating the match semantics of backtracking regex
    /// engines.
    set: SparseSet,
    /// The slots for every NFA state, where each slot stores a (possibly
    /// absent) offset. Every capturing group has two slots. One for a start
    /// offset and one for an end offset.
    slot_table: SlotTable,
}

impl ActiveStates {
    /// Create a new set of active states for the given PikeVM. The active
    /// states returned may only be used with the given PikeVM. (Use 'reset'
    /// to re-purpose the allocation for a different PikeVM.)
    fn new(re: &PikeVM) -> ActiveStates {
        let mut active = ActiveStates {
            set: SparseSet::new(0),
            slot_table: SlotTable::new(),
        };
        active.reset(re);
        active
    }

    /// Reset this set of active states such that it can be used with the given
    /// PikeVM (and only that PikeVM).
    fn reset(&mut self, re: &PikeVM) {
        self.set.resize(re.nfa().len());
        self.slot_table.reset(re);
    }

    /// Setup this set of active states for a new search. The given slot
    /// length should be the number of slots in a caller provided 'Captures'
    /// (and may be zero).
    fn setup_search(&mut self, captures_slot_len: usize) {
        self.set.clear();
        self.slot_table.setup_search(captures_slot_len);
    }
}

/// A table of slots, where each row represent a state in an NFA. Thus, the
/// table has room for storing slots for every single state in an NFA.
///
/// This table is represented with a single contiguous allocation. In general,
/// the notion of "capturing group" doesn't really exist at this level of
/// abstraction, hence the name "slot" instead. (Indeed, every capturing group
/// maps to a pair of slots, one for the start offset and one for the end
/// offset.) Slots are indexed by the `Captures` NFA state.
#[derive(Clone, Debug)]
struct SlotTable {
    /// The actual table of offsets.
    table: Vec<Option<NonMaxUsize>>,
    /// The number of slots per state, i.e., the table's stride or the length
    /// of each row.
    slots_per_state: usize,
    /// The number of slots in the caller-provided `Captures` value for the
    /// current search. Setting this to `slots_per_state` is always correct,
    /// but may be wasteful.
    slots_for_captures: usize,
}

impl SlotTable {
    /// Create a new slot table.
    ///
    /// One should call 'reset' with the corresponding PikeVM before use.
    fn new() -> SlotTable {
        SlotTable { table: vec![], slots_for_captures: 0, slots_per_state: 0 }
    }

    /// Reset this slot table such that it can be used with the given PikeVM
    /// (and only that PikeVM).
    fn reset(&mut self, re: &PikeVM) {
        let nfa = re.nfa();
        // OK because NFA construction would have failed if this overflowed.
        self.slots_per_state = nfa.group_len().checked_mul(2).unwrap();
        // This is always correct, but may be reduced for a particular search
        // if fewer slots were given by the caller, e.g., none at all or only
        // slots for tracking the overall match instead of all slots for every
        // group.
        self.slots_for_captures = self.slots_per_state;
        let len = nfa
            .len()
            // We add 1 so that our last row is always empty. We use it as
            // "scratch" space for computing the epsilon closure off of the
            // starting state.
            .checked_add(1)
            .and_then(|x| x.checked_mul(self.slots_per_state))
            // It seems like this could actually panic on legitimate inputs
            // on 32-bit targets. Should we somehow convert this to an error?
            // What about something similar for the lazy DFA cache? If you're
            // tripping this assert, please file a bug.
            .expect("slot table length doesn't overflow");
        self.table.resize(len, None);
    }

    /// Perform any per-search setup for this slot table.
    ///
    /// In particular, this sets the length of the number of slots used in the
    /// slots given by the caller (if any at all). This number may be smaller
    /// than the total number of slots available, e.g., when the caller is only
    /// interested in tracking the overall match and not the spans of every
    /// matching capturing group. Only tracking the overall match can save a
    /// substantial amount of time copying capturing spans during a search.
    fn setup_search(&mut self, captures_slot_len: usize) {
        self.slots_for_captures = captures_slot_len;
    }

    /// Return a mutable slice of the slots for the given state.
    ///
    /// Note that the length of the slice returned may be less than the total
    /// number of slots available for this state. In particular, the length
    /// always matches the number of slots indicated via `setup_search`.
    fn for_state(&mut self, sid: StateID) -> &mut [Option<NonMaxUsize>] {
        let i = sid.as_usize() * self.slots_per_state;
        &mut self.table[i..i + self.slots_for_captures]
    }

    /// Return a slice of slots of appropriate length where every slot offset
    /// is guaranteed to be absent. This is useful in cases where you need to
    /// compute an epsilon closure outside of the user supplied regex, and thus
    /// never want it to have any capturing slots set.
    fn all_absent(&mut self) -> &mut [Option<NonMaxUsize>] {
        let i = self.table.len() - self.slots_per_state;
        &mut self.table[i..i + self.slots_for_captures]
    }
}

/// Represents a stack frame for use while computing an epsilon closure.
///
/// (An "epsilon closure" refers to the set of reachable NFA states from a
/// single state without consuming any input. That is, the set of all epsilon
/// transitions not only from that single state, but from every other state
/// reachable by an epsilon transition as well. This is why it's called a
/// "closure.")
///
/// Computing the epsilon closure in a Thompson NFA proceeds via a depth
/// first traversal over all epsilon transitions from a particular state.
/// (A depth first traversal is important because it emulates the same priority
/// of matches that is typically found in backtracking regex engines.) This
/// depth first traversal is naturally expressed using recursion, but to avoid
/// a call stack size proportional to the size of a regex, we put our stack on
/// the heap instead.
///
/// This stack thus consists of call frames. The typical call frame is
/// `Explore`, which instructs epsilon closure to explore the epsilon
/// transitions from that state. (Subsequent epsilon transitions are then
/// pushed on to the stack as more `Explore` frames.) If the state ID being
/// explored has no epsilon transitions, then the capturing group slots are
/// copied from the original state that sparked the epsilon closure (from the
/// 'step' routine) to the state ID being explored. This way, capturing group
/// slots are forwarded from the previous state to the next.
///
/// The other stack frame, `RestoreCaptures`, instructs the epsilon closure to
/// set the position for a particular slot back to some particular offset. This
/// frame is pushed when `Explore` sees a `Capture` transition. `Explore` will
/// set the offset of the slot indicated in `Capture` to the current offset,
/// and then push the old offset on to the stack as a `RestoreCapture` frame.
/// Thus, the new offset is only used until the epsilon closure reverts back to
/// the `RestoreCapture` frame. In effect, this gives the `Capture` epsilon
/// transition its "scope" to only states that come "after" it during depth
/// first traversal.
#[derive(Clone, Debug)]
enum FollowEpsilon {
    /// Explore the epsilon transitions from a state ID.
    Explore(StateID),
    /// Reset the given `slot` to the given `offset` (which might be `None`).
    RestoreCapture { slot: u32, offset: Option<NonMaxUsize> },
}

/// A sparse set used for representing ordered NFA states.
///
/// This supports constant time addition and membership testing. Clearing an
/// entire set can also be done in constant time. Iteration yields elements
/// in the order in which they were inserted.
///
/// The data structure is based on: https://research.swtch.com/sparse
/// Note though that we don't actually use uninitialized memory. We generally
/// reuse sparse sets, so the initial allocation cost is bearable. However, its
/// other properties listed above are extremely useful.
#[derive(Clone)]
struct SparseSet {
    /// The number of elements currently in this set.
    len: usize,
    /// Dense contains the ids in the order in which they were inserted.
    dense: Vec<StateID>,
    /// Sparse maps ids to their location in dense.
    ///
    /// A state ID is in the set if and only if
    /// sparse[id] < len && id == dense[sparse[id]].
    ///
    /// Note that these are indices into 'dense'. It's a little weird to use
    /// StateID here, but we know our length can never exceed the bounds of
    /// StateID (enforced by 'resize') and StateID will be at most 4 bytes
    /// where as a usize is likely double that in most cases.
    sparse: Vec<StateID>,
}

impl SparseSet {
    /// Create a new sparse set with the given capacity.
    ///
    /// Sparse sets have a fixed size and they cannot grow. Attempting to
    /// insert more distinct elements than the total capacity of the set will
    /// result in a panic.
    ///
    /// This panics if the capacity given is bigger than `StateID::LIMIT`.
    fn new(capacity: usize) -> SparseSet {
        let mut set = SparseSet { len: 0, dense: vec![], sparse: vec![] };
        set.resize(capacity);
        set
    }

    /// Resizes this sparse set to have the new capacity given.
    ///
    /// This set is automatically cleared.
    ///
    /// This panics if the capacity given is bigger than `StateID::LIMIT`.
    fn resize(&mut self, new_capacity: usize) {
        assert!(
            new_capacity <= u32::MAX.as_usize(),
            "sparse set capacity cannot exceed {:?}",
            u32::MAX,
        );
        self.clear();
        self.dense.resize(new_capacity, 0);
        self.sparse.resize(new_capacity, 0);
    }

    /// Returns the capacity of this set.
    ///
    /// The capacity represents a fixed limit on the number of distinct
    /// elements that are allowed in this set. The capacity cannot be changed.
    fn capacity(&self) -> usize {
        self.dense.len()
    }

    /// Returns the number of elements in this set.
    fn len(&self) -> usize {
        self.len
    }

    /// Returns true if and only if this set is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Insert the state ID value into this set and return true if the given
    /// state ID was not previously in this set.
    ///
    /// This operation is idempotent. If the given value is already in this
    /// set, then this is a no-op.
    ///
    /// If more than `capacity` ids are inserted, then this panics.
    fn insert(&mut self, id: StateID) -> bool {
        if self.contains(id) {
            return false;
        }

        let index = self.len();
        assert!(
            index < self.capacity(),
            "{:?} exceeds capacity of {:?} when inserting {:?}",
            index,
            self.capacity(),
            id,
        );
        self.dense[index] = id;
        // OK because we don't permit the capacity to be set higher than
        // u32::MAX.
        self.sparse[id.as_usize()] = u32::try_from(index).unwrap();
        self.len += 1;
        true
    }

    /// Returns true if and only if this set contains the given value.
    fn contains(&self, id: StateID) -> bool {
        let index = self.sparse[id.as_usize()];
        index.as_usize() < self.len() && self.dense[index.as_usize()] == id
    }

    /// Clear this set such that it has no members.
    fn clear(&mut self) {
        self.len = 0;
    }

    /// Returns an iterator over all the state IDs in this set in the order in
    /// which they were inserted.
    fn iter(&self) -> SparseSetIter<'_> {
        SparseSetIter(self.dense[..self.len()].iter())
    }
}

impl core::fmt::Debug for SparseSet {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let elements: Vec<StateID> = self.iter().collect();
        f.debug_tuple("SparseSet").field(&elements).finish()
    }
}

/// An iterator over all elements in a sparse set.
///
/// The lifetime `'a` refers to the lifetime of the set being iterated over.
#[derive(Debug)]
struct SparseSetIter<'a>(core::slice::Iter<'a, StateID>);

impl<'a> Iterator for SparseSetIter<'a> {
    type Item = StateID;

    fn next(&mut self) -> Option<StateID> {
        self.0.next().map(|&id| id)
    }
}
