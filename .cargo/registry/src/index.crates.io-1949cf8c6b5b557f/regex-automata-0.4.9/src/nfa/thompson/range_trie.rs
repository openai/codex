/*
I've called the primary data structure in this module a "range trie." As far
as I can tell, there is no prior art on a data structure like this, however,
it's likely someone somewhere has built something like it. Searching for
"range trie" turns up the paper "Range Tries for Scalable Address Lookup,"
but it does not appear relevant.

The range trie is just like a trie in that it is a special case of a
deterministic finite state machine. It has states and each state has a set
of transitions to other states. It is acyclic, and, like a normal trie,
it makes no attempt to reuse common suffixes among its elements. The key
difference between a normal trie and a range trie below is that a range trie
operates on *contiguous sequences* of bytes instead of singleton bytes.
One could say say that our alphabet is ranges of bytes instead of bytes
themselves, except a key part of range trie construction is splitting ranges
apart to ensure there is at most one transition that can be taken for any
byte in a given state.

I've tried to explain the details of how the range trie works below, so
for now, we are left with trying to understand what problem we're trying to
solve. Which is itself fairly involved!

At the highest level, here's what we want to do. We want to convert a
sequence of Unicode codepoints into a finite state machine whose transitions
are over *bytes* and *not* Unicode codepoints. We want this because it makes
said finite state machines much smaller and much faster to execute. As a
simple example, consider a byte oriented automaton for all Unicode scalar
values (0x00 through 0x10FFFF, not including surrogate codepoints):

    [00-7F]
    [C2-DF][80-BF]
    [E0-E0][A0-BF][80-BF]
    [E1-EC][80-BF][80-BF]
    [ED-ED][80-9F][80-BF]
    [EE-EF][80-BF][80-BF]
    [F0-F0][90-BF][80-BF][80-BF]
    [F1-F3][80-BF][80-BF][80-BF]
    [F4-F4][80-8F][80-BF][80-BF]

(These byte ranges are generated via the regex-syntax::utf8 module, which
was based on Russ Cox's code in RE2, which was in turn based on Ken
Thompson's implementation of the same idea in his Plan9 implementation of
grep.)

It should be fairly straight-forward to see how one could compile this into
a DFA. The sequences are sorted and non-overlapping. Essentially, you could
build a trie from this fairly easy. The problem comes when your initial
range (in this case, 0x00-0x10FFFF) isn't so nice. For example, the class
represented by '\w' contains only a tenth of the codepoints that
0x00-0x10FFFF contains, but if we were to write out the byte based ranges
as we did above, the list would stretch to 892 entries! This turns into
quite a large NFA with a few thousand states. Turning this beast into a DFA
takes quite a bit of time. We are thus left with trying to trim down the
number of states we produce as early as possible.

One approach (used by RE2 and still by the regex crate, at time of writing)
is to try to find common suffixes while building NFA states for the above
and reuse them. This is very cheap to do and one can control precisely how
much extra memory you want to use for the cache.

Another approach, however, is to reuse an algorithm for constructing a
*minimal* DFA from a sorted sequence of inputs. I don't want to go into
the full details here, but I explain it in more depth in my blog post on
FSTs[1]. Note that the algorithm was not invented by me, but was published
in paper by Daciuk et al. in 2000 called "Incremental Construction of
MinimalAcyclic Finite-State Automata." Like the suffix cache approach above,
it is also possible to control the amount of extra memory one uses, although
this usually comes with the cost of sacrificing true minimality. (But it's
typically close enough with a reasonably sized cache of states.)

The catch is that Daciuk's algorithm only works if you add your keys in
lexicographic ascending order. In our case, since we're dealing with ranges,
we also need the additional requirement that ranges are either equivalent
or do not overlap at all. For example, if one were given the following byte
ranges:

    [BC-BF][80-BF]
    [BC-BF][90-BF]

Then Daciuk's algorithm would not work, since there is nothing to handle the
fact that the ranges overlap. They would need to be split apart. Thankfully,
Thompson's algorithm for producing byte ranges for Unicode codepoint ranges
meets both of our requirements. (A proof for this eludes me, but it appears
true.)

... however, we would also like to be able to compile UTF-8 automata in
reverse. We want this because in order to find the starting location of a
match using a DFA, we need to run a second DFA---a reversed version of the
forward DFA---backwards to discover the match location. Unfortunately, if
we reverse our byte sequences for 0x00-0x10FFFF, we get sequences that are
can overlap, even if they are sorted:

    [00-7F]
    [80-BF][80-9F][ED-ED]
    [80-BF][80-BF][80-8F][F4-F4]
    [80-BF][80-BF][80-BF][F1-F3]
    [80-BF][80-BF][90-BF][F0-F0]
    [80-BF][80-BF][E1-EC]
    [80-BF][80-BF][EE-EF]
    [80-BF][A0-BF][E0-E0]
    [80-BF][C2-DF]

For example, '[80-BF][80-BF][EE-EF]' and '[80-BF][A0-BF][E0-E0]' have
overlapping ranges between '[80-BF]' and '[A0-BF]'. Thus, there is no
simple way to apply Daciuk's algorithm.

And thus, the range trie was born. The range trie's only purpose is to take
sequences of byte ranges like the ones above, collect them into a trie and then
spit them out in a sorted fashion with no overlapping ranges. For example,
0x00-0x10FFFF gets translated to:

    [0-7F]
    [80-BF][80-9F][80-8F][F1-F3]
    [80-BF][80-9F][80-8F][F4]
    [80-BF][80-9F][90-BF][F0]
    [80-BF][80-9F][90-BF][F1-F3]
    [80-BF][80-9F][E1-EC]
    [80-BF][80-9F][ED]
    [80-BF][80-9F][EE-EF]
    [80-BF][A0-BF][80-8F][F1-F3]
    [80-BF][A0-BF][80-8F][F4]
    [80-BF][A0-BF][90-BF][F0]
    [80-BF][A0-BF][90-BF][F1-F3]
    [80-BF][A0-BF][E0]
    [80-BF][A0-BF][E1-EC]
    [80-BF][A0-BF][EE-EF]
    [80-BF][C2-DF]

We've thus satisfied our requirements for running Daciuk's algorithm. All
sequences of ranges are sorted, and any corresponding ranges are either
exactly equivalent or non-overlapping.

In effect, a range trie is building a DFA from a sequence of arbitrary byte
ranges. But it uses an algorithm custom tailored to its input, so it is not as
costly as traditional DFA construction. While it is still quite a bit more
costly than the forward case (which only needs Daciuk's algorithm), it winds
up saving a substantial amount of time if one is doing a full DFA powerset
construction later by virtue of producing a much much smaller NFA.

[1] - https://blog.burntsushi.net/transducers/
[2] - https://www.mitpressjournals.org/doi/pdfplus/10.1162/089120100561601
*/

use core::{cell::RefCell, fmt, mem, ops::RangeInclusive};

use alloc::{format, string::String, vec, vec::Vec};

use regex_syntax::utf8::Utf8Range;

use crate::util::primitives::StateID;

/// There is only one final state in this trie. Every sequence of byte ranges
/// added shares the same final state.
const FINAL: StateID = StateID::ZERO;

/// The root state of the trie.
const ROOT: StateID = StateID::new_unchecked(1);

/// A range trie represents an ordered set of sequences of bytes.
///
/// A range trie accepts as input a sequence of byte ranges and merges
/// them into the existing set such that the trie can produce a sorted
/// non-overlapping sequence of byte ranges. The sequence emitted corresponds
/// precisely to the sequence of bytes matched by the given keys, although the
/// byte ranges themselves may be split at different boundaries.
///
/// The order complexity of this data structure seems difficult to analyze.
/// If the size of a byte is held as a constant, then insertion is clearly
/// O(n) where n is the number of byte ranges in the input key. However, if
/// k=256 is our alphabet size, then insertion could be O(k^2 * n). In
/// particular it seems possible for pathological inputs to cause insertion
/// to do a lot of work. However, for what we use this data structure for,
/// there should be no pathological inputs since the ultimate source is always
/// a sorted set of Unicode scalar value ranges.
///
/// Internally, this trie is setup like a finite state machine. Note though
/// that it is acyclic.
#[derive(Clone)]
pub struct RangeTrie {
    /// The states in this trie. The first is always the shared final state.
    /// The second is always the root state. Otherwise, there is no
    /// particular order.
    states: Vec<State>,
    /// A free-list of states. When a range trie is cleared, all of its states
    /// are added to this list. Creating a new state reuses states from this
    /// list before allocating a new one.
    free: Vec<State>,
    /// A stack for traversing this trie to yield sequences of byte ranges in
    /// lexicographic order.
    iter_stack: RefCell<Vec<NextIter>>,
    /// A buffer that stores the current sequence during iteration.
    iter_ranges: RefCell<Vec<Utf8Range>>,
    /// A stack used for traversing the trie in order to (deeply) duplicate
    /// a state. States are recursively duplicated when ranges are split.
    dupe_stack: Vec<NextDupe>,
    /// A stack used for traversing the trie during insertion of a new
    /// sequence of byte ranges.
    insert_stack: Vec<NextInsert>,
}

/// A single state in this trie.
#[derive(Clone)]
struct State {
    /// A sorted sequence of non-overlapping transitions to other states. Each
    /// transition corresponds to a single range of bytes.
    transitions: Vec<Transition>,
}

/// A transition is a single range of bytes. If a particular byte is in this
/// range, then the corresponding machine may transition to the state pointed
/// to by `next_id`.
#[derive(Clone)]
struct Transition {
    /// The byte range.
    range: Utf8Range,
    /// The next state to transition to.
    next_id: StateID,
}

impl RangeTrie {
    /// Create a new empty range trie.
    pub fn new() -> RangeTrie {
        let mut trie = RangeTrie {
            states: vec![],
            free: vec![],
            iter_stack: RefCell::new(vec![]),
            iter_ranges: RefCell::new(vec![]),
            dupe_stack: vec![],
            insert_stack: vec![],
        };
        trie.clear();
        trie
    }

    /// Clear this range trie such that it is empty. Clearing a range trie
    /// and reusing it can beneficial because this may reuse allocations.
    pub fn clear(&mut self) {
        self.free.extend(self.states.drain(..));
        self.add_empty(); // final
        self.add_empty(); // root
    }

    /// Iterate over all of the sequences of byte ranges in this trie, and
    /// call the provided function for each sequence. Iteration occurs in
    /// lexicographic order.
    pub fn iter<E, F: FnMut(&[Utf8Range]) -> Result<(), E>>(
        &self,
        mut f: F,
    ) -> Result<(), E> {
        let mut stack = self.iter_stack.borrow_mut();
        stack.clear();
        let mut ranges = self.iter_ranges.borrow_mut();
        ranges.clear();

        // We do iteration in a way that permits us to use a single buffer
        // for our keys. We iterate in a depth first fashion, while being
        // careful to expand our frontier as we move deeper in the trie.
        stack.push(NextIter { state_id: ROOT, tidx: 0 });
        while let Some(NextIter { mut state_id, mut tidx }) = stack.pop() {
            // This could be implemented more simply without an inner loop
            // here, but at the cost of more stack pushes.
            loop {
                let state = self.state(state_id);
                // If we've visited all transitions in this state, then pop
                // back to the parent state.
                if tidx >= state.transitions.len() {
                    ranges.pop();
                    break;
                }

                let t = &state.transitions[tidx];
                ranges.push(t.range);
                if t.next_id == FINAL {
                    f(&ranges)?;
                    ranges.pop();
                    tidx += 1;
                } else {
                    // Expand our frontier. Once we come back to this state
                    // via the stack, start in on the next transition.
                    stack.push(NextIter { state_id, tidx: tidx + 1 });
                    // Otherwise, move to the first transition of the next
                    // state.
                    state_id = t.next_id;
                    tidx = 0;
                }
            }
        }
        Ok(())
    }

    /// Inserts a new sequence of ranges into this trie.
    ///
    /// The sequence given must be non-empty and must not have a length
    /// exceeding 4.
    pub fn insert(&mut self, ranges: &[Utf8Range]) {
        assert!(!ranges.is_empty());
        assert!(ranges.len() <= 4);

        let mut stack = mem::replace(&mut self.insert_stack, vec![]);
        stack.clear();

        stack.push(NextInsert::new(ROOT, ranges));
        while let Some(next) = stack.pop() {
            let (state_id, ranges) = (next.state_id(), next.ranges());
            assert!(!ranges.is_empty());

            let (mut new, rest) = (ranges[0], &ranges[1..]);

            // i corresponds to the position of the existing transition on
            // which we are operating. Typically, the result is to remove the
            // transition and replace it with two or more new transitions
            // corresponding to the partitions generated by splitting the
            // 'new' with the ith transition's range.
            let mut i = self.state(state_id).find(new);

            // In this case, there is no overlap *and* the new range is greater
            // than all existing ranges. So we can just add it to the end.
            if i == self.state(state_id).transitions.len() {
                let next_id = NextInsert::push(self, &mut stack, rest);
                self.add_transition(state_id, new, next_id);
                continue;
            }

            // The need for this loop is a bit subtle, buf basically, after
            // we've handled the partitions from our initial split, it's
            // possible that there will be a partition leftover that overlaps
            // with a subsequent transition. If so, then we have to repeat
            // the split process again with the leftovers and that subsequent
            // transition.
            'OUTER: loop {
                let old = self.state(state_id).transitions[i].clone();
                let split = match Split::new(old.range, new) {
                    Some(split) => split,
                    None => {
                        let next_id = NextInsert::push(self, &mut stack, rest);
                        self.add_transition_at(i, state_id, new, next_id);
                        continue;
                    }
                };
                let splits = split.as_slice();
                // If we only have one partition, then the ranges must be
                // equivalent. There's nothing to do here for this state, so
                // just move on to the next one.
                if splits.len() == 1 {
                    // ... but only if we have anything left to do.
                    if !rest.is_empty() {
                        stack.push(NextInsert::new(old.next_id, rest));
                    }
                    break;
                }
                // At this point, we know that 'split' is non-empty and there
                // must be some overlap AND that the two ranges are not
                // equivalent. Therefore, the existing range MUST be removed
                // and split up somehow. Instead of actually doing the removal
                // and then a subsequent insertion---with all the memory
                // shuffling that entails---we simply overwrite the transition
                // at position `i` for the first new transition we want to
                // insert. After that, we're forced to do expensive inserts.
                let mut first = true;
                let mut add_trans =
                    |trie: &mut RangeTrie, pos, from, range, to| {
                        if first {
                            trie.set_transition_at(pos, from, range, to);
                            first = false;
                        } else {
                            trie.add_transition_at(pos, from, range, to);
                        }
                    };
                for (j, &srange) in splits.iter().enumerate() {
                    match srange {
                        SplitRange::Old(r) => {
                            // Deep clone the state pointed to by the ith
                            // transition. This is always necessary since 'old'
                            // is always coupled with at least a 'both'
                            // partition. We don't want any new changes made
                            // via the 'both' partition to impact the part of
                            // the transition that doesn't overlap with the
                            // new range.
                            let dup_id = self.duplicate(old.next_id);
                            add_trans(self, i, state_id, r, dup_id);
                        }
                        SplitRange::New(r) => {
                            // This is a bit subtle, but if this happens to be
                            // the last partition in our split, it is possible
                            // that this overlaps with a subsequent transition.
                            // If it does, then we must repeat the whole
                            // splitting process over again with `r` and the
                            // subsequent transition.
                            {
                                let trans = &self.state(state_id).transitions;
                                if j + 1 == splits.len()
                                    && i < trans.len()
                                    && intersects(r, trans[i].range)
                                {
                                    new = r;
                                    continue 'OUTER;
                                }
                            }

                            // ... otherwise, setup exploration for a new
                            // empty state and add a brand new transition for
                            // this new range.
                            let next_id =
                                NextInsert::push(self, &mut stack, rest);
                            add_trans(self, i, state_id, r, next_id);
                        }
                        SplitRange::Both(r) => {
                            // Continue adding the remaining ranges on this
                            // path and update the transition with the new
                            // range.
                            if !rest.is_empty() {
                                stack.push(NextInsert::new(old.next_id, rest));
                            }
                            add_trans(self, i, state_id, r, old.next_id);
                        }
                    }
                    i += 1;
                }
                // If we've reached this point, then we know that there are
                // no subsequent transitions with any overlap. Therefore, we
                // can stop processing this range and move on to the next one.
                break;
            }
        }
        self.insert_stack = stack;
    }

    pub fn add_empty(&mut self) -> StateID {
        let id = match StateID::try_from(self.states.len()) {
            Ok(id) => id,
            Err(_) => {
                // This generally should not happen since a range trie is
                // only ever used to compile a single sequence of Unicode
                // scalar values. If we ever got to this point, we would, at
                // *minimum*, be using 96GB in just the range trie alone.
                panic!("too many sequences added to range trie");
            }
        };
        // If we have some free states available, then use them to avoid
        // more allocations.
        if let Some(mut state) = self.free.pop() {
            state.clear();
            self.states.push(state);
        } else {
            self.states.push(State { transitions: vec![] });
        }
        id
    }

    /// Performs a deep clone of the given state and returns the duplicate's
    /// state ID.
    ///
    /// A "deep clone" in this context means that the state given along with
    /// recursively all states that it points to are copied. Once complete,
    /// the given state ID and the returned state ID share nothing.
    ///
    /// This is useful during range trie insertion when a new range overlaps
    /// with an existing range that is bigger than the new one. The part
    /// of the existing range that does *not* overlap with the new one is
    /// duplicated so that adding the new range to the overlap doesn't disturb
    /// the non-overlapping portion.
    ///
    /// There's one exception: if old_id is the final state, then it is not
    /// duplicated and the same final state is returned. This is because all
    /// final states in this trie are equivalent.
    fn duplicate(&mut self, old_id: StateID) -> StateID {
        if old_id == FINAL {
            return FINAL;
        }

        let mut stack = mem::replace(&mut self.dupe_stack, vec![]);
        stack.clear();

        let new_id = self.add_empty();
        // old_id is the state we're cloning and new_id is the ID of the
        // duplicated state for old_id.
        stack.push(NextDupe { old_id, new_id });
        while let Some(NextDupe { old_id, new_id }) = stack.pop() {
            for i in 0..self.state(old_id).transitions.len() {
                let t = self.state(old_id).transitions[i].clone();
                if t.next_id == FINAL {
                    // All final states are the same, so there's no need to
                    // duplicate it.
                    self.add_transition(new_id, t.range, FINAL);
                    continue;
                }

                let new_child_id = self.add_empty();
                self.add_transition(new_id, t.range, new_child_id);
                stack.push(NextDupe {
                    old_id: t.next_id,
                    new_id: new_child_id,
                });
            }
        }
        self.dupe_stack = stack;
        new_id
    }

    /// Adds the given transition to the given state.
    ///
    /// Callers must ensure that all previous transitions in this state
    /// are lexicographically smaller than the given range.
    fn add_transition(
        &mut self,
        from_id: StateID,
        range: Utf8Range,
        next_id: StateID,
    ) {
        self.state_mut(from_id)
            .transitions
            .push(Transition { range, next_id });
    }

    /// Like `add_transition`, except this inserts the transition just before
    /// the ith transition.
    fn add_transition_at(
        &mut self,
        i: usize,
        from_id: StateID,
        range: Utf8Range,
        next_id: StateID,
    ) {
        self.state_mut(from_id)
            .transitions
            .insert(i, Transition { range, next_id });
    }

    /// Overwrites the transition at position i with the given transition.
    fn set_transition_at(
        &mut self,
        i: usize,
        from_id: StateID,
        range: Utf8Range,
        next_id: StateID,
    ) {
        self.state_mut(from_id).transitions[i] = Transition { range, next_id };
    }

    /// Return an immutable borrow for the state with the given ID.
    fn state(&self, id: StateID) -> &State {
        &self.states[id]
    }

    /// Return a mutable borrow for the state with the given ID.
    fn state_mut(&mut self, id: StateID) -> &mut State {
        &mut self.states[id]
    }
}

impl State {
    /// Find the position at which the given range should be inserted in this
    /// state.
    ///
    /// The position returned is always in the inclusive range
    /// [0, transitions.len()]. If 'transitions.len()' is returned, then the
    /// given range overlaps with no other range in this state *and* is greater
    /// than all of them.
    ///
    /// For all other possible positions, the given range either overlaps
    /// with the transition at that position or is otherwise less than it
    /// with no overlap (and is greater than the previous transition). In the
    /// former case, careful attention must be paid to inserting this range
    /// as a new transition. In the latter case, the range can be inserted as
    /// a new transition at the given position without disrupting any other
    /// transitions.
    fn find(&self, range: Utf8Range) -> usize {
        /// Returns the position `i` at which `pred(xs[i])` first returns true
        /// such that for all `j >= i`, `pred(xs[j]) == true`. If `pred` never
        /// returns true, then `xs.len()` is returned.
        ///
        /// We roll our own binary search because it doesn't seem like the
        /// standard library's binary search can be used here. Namely, if
        /// there is an overlapping range, then we want to find the first such
        /// occurrence, but there may be many. Or at least, it's not quite
        /// clear to me how to do it.
        fn binary_search<T, F>(xs: &[T], mut pred: F) -> usize
        where
            F: FnMut(&T) -> bool,
        {
            let (mut left, mut right) = (0, xs.len());
            while left < right {
                // Overflow is impossible because xs.len() <= 256.
                let mid = (left + right) / 2;
                if pred(&xs[mid]) {
                    right = mid;
                } else {
                    left = mid + 1;
                }
            }
            left
        }

        // Benchmarks suggest that binary search is just a bit faster than
        // straight linear search. Specifically when using the debug tool:
        //
        //   hyperfine "regex-cli debug thompson -qr --captures none '\w{90} ecurB'"
        binary_search(&self.transitions, |t| range.start <= t.range.end)
    }

    /// Clear this state such that it has zero transitions.
    fn clear(&mut self) {
        self.transitions.clear();
    }
}

/// The next state to process during duplication.
#[derive(Clone, Debug)]
struct NextDupe {
    /// The state we want to duplicate.
    old_id: StateID,
    /// The ID of the new state that is a duplicate of old_id.
    new_id: StateID,
}

/// The next state (and its corresponding transition) that we want to visit
/// during iteration in lexicographic order.
#[derive(Clone, Debug)]
struct NextIter {
    state_id: StateID,
    tidx: usize,
}

/// The next state to process during insertion and any remaining ranges that we
/// want to add for a particular sequence of ranges. The first such instance
/// is always the root state along with all ranges given.
#[derive(Clone, Debug)]
struct NextInsert {
    /// The next state to begin inserting ranges. This state should be the
    /// state at which `ranges[0]` should be inserted.
    state_id: StateID,
    /// The ranges to insert. We used a fixed-size array here to avoid an
    /// allocation.
    ranges: [Utf8Range; 4],
    /// The number of valid ranges in the above array.
    len: u8,
}

impl NextInsert {
    /// Create the next item to visit. The given state ID should correspond
    /// to the state at which the first range in the given slice should be
    /// inserted. The slice given must not be empty and it must be no longer
    /// than 4.
    fn new(state_id: StateID, ranges: &[Utf8Range]) -> NextInsert {
        let len = ranges.len();
        assert!(len > 0);
        assert!(len <= 4);

        let mut tmp = [Utf8Range { start: 0, end: 0 }; 4];
        tmp[..len].copy_from_slice(ranges);
        NextInsert { state_id, ranges: tmp, len: u8::try_from(len).unwrap() }
    }

    /// Push a new empty state to visit along with any remaining ranges that
    /// still need to be inserted. The ID of the new empty state is returned.
    ///
    /// If ranges is empty, then no new state is created and FINAL is returned.
    fn push(
        trie: &mut RangeTrie,
        stack: &mut Vec<NextInsert>,
        ranges: &[Utf8Range],
    ) -> StateID {
        if ranges.is_empty() {
            FINAL
        } else {
            let next_id = trie.add_empty();
            stack.push(NextInsert::new(next_id, ranges));
            next_id
        }
    }

    /// Return the ID of the state to visit.
    fn state_id(&self) -> StateID {
        self.state_id
    }

    /// Return the remaining ranges to insert.
    fn ranges(&self) -> &[Utf8Range] {
        &self.ranges[..usize::try_from(self.len).unwrap()]
    }
}

/// Split represents a partitioning of two ranges into one or more ranges. This
/// is the secret sauce that makes a range trie work, as it's what tells us
/// how to deal with two overlapping but unequal ranges during insertion.
///
/// Essentially, either two ranges overlap or they don't. If they don't, then
/// handling insertion is easy: just insert the new range into its
/// lexicographically correct position. Since it does not overlap with anything
/// else, no other transitions are impacted by the new range.
///
/// If they do overlap though, there are generally three possible cases to
/// handle:
///
/// 1. The part where the two ranges actually overlap. i.e., The intersection.
/// 2. The part of the existing range that is not in the new range.
/// 3. The part of the new range that is not in the old range.
///
/// (1) is guaranteed to always occur since all overlapping ranges have a
/// non-empty intersection. If the two ranges are not equivalent, then at
/// least one of (2) or (3) is guaranteed to occur as well. In some cases,
/// e.g., `[0-4]` and `[4-9]`, all three cases will occur.
///
/// This `Split` type is responsible for providing (1), (2) and (3) for any
/// possible pair of byte ranges.
///
/// As for insertion, for the overlap in (1), the remaining ranges to insert
/// should be added by following the corresponding transition. However, this
/// should only be done for the overlapping parts of the range. If there was
/// a part of the existing range that was not in the new range, then that
/// existing part must be split off from the transition and duplicated. The
/// remaining parts of the overlap can then be added to using the new ranges
/// without disturbing the existing range.
///
/// Handling the case for the part of a new range that is not in an existing
/// range is seemingly easy. Just treat it as if it were a non-overlapping
/// range. The problem here is that if this new non-overlapping range occurs
/// after both (1) and (2), then it's possible that it can overlap with the
/// next transition in the current state. If it does, then the whole process
/// must be repeated!
///
/// # Details of the 3 cases
///
/// The following details the various cases that are implemented in code
/// below. It's plausible that the number of cases is not actually minimal,
/// but it's important for this code to remain at least somewhat readable.
///
/// Given [a,b] and [x,y], where a <= b, x <= y, b < 256 and y < 256, we define
/// the follow distinct relationships where at least one must apply. The order
/// of these matters, since multiple can match. The first to match applies.
///
///   1. b < x <=> [a,b] < [x,y]
///   2. y < a <=> [x,y] < [a,b]
///
/// In the case of (1) and (2), these are the only cases where there is no
/// overlap. Or otherwise, the intersection of [a,b] and [x,y] is empty. In
/// order to compute the intersection, one can do [max(a,x), min(b,y)]. The
/// intersection in all of the following cases is non-empty.
///
///    3. a = x && b = y <=> [a,b] == [x,y]
///    4. a = x && b < y <=> [x,y] right-extends [a,b]
///    5. b = y && a > x <=> [x,y] left-extends [a,b]
///    6. x = a && y < b <=> [a,b] right-extends [x,y]
///    7. y = b && x > a <=> [a,b] left-extends [x,y]
///    8. a > x && b < y <=> [x,y] covers [a,b]
///    9. x > a && y < b <=> [a,b] covers [x,y]
///   10. b = x && a < y <=> [a,b] is left-adjacent to [x,y]
///   11. y = a && x < b <=> [x,y] is left-adjacent to [a,b]
///   12. b > x && b < y <=> [a,b] left-overlaps [x,y]
///   13. y > a && y < b <=> [x,y] left-overlaps [a,b]
///
/// In cases 3-13, we can form rules that partition the ranges into a
/// non-overlapping ordered sequence of ranges:
///
///    3. [a,b]
///    4. [a,b], [b+1,y]
///    5. [x,a-1], [a,b]
///    6. [x,y], [y+1,b]
///    7. [a,x-1], [x,y]
///    8. [x,a-1], [a,b], [b+1,y]
///    9. [a,x-1], [x,y], [y+1,b]
///   10. [a,b-1], [b,b], [b+1,y]
///   11. [x,y-1], [y,y], [y+1,b]
///   12. [a,x-1], [x,b], [b+1,y]
///   13. [x,a-1], [a,y], [y+1,b]
///
/// In the code below, we go a step further and identify each of the above
/// outputs as belonging either to the overlap of the two ranges or to one
/// of [a,b] or [x,y] exclusively.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Split {
    partitions: [SplitRange; 3],
    len: usize,
}

/// A tagged range indicating how it was derived from a pair of ranges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SplitRange {
    Old(Utf8Range),
    New(Utf8Range),
    Both(Utf8Range),
}

impl Split {
    /// Create a partitioning of the given ranges.
    ///
    /// If the given ranges have an empty intersection, then None is returned.
    fn new(o: Utf8Range, n: Utf8Range) -> Option<Split> {
        let range = |r: RangeInclusive<u8>| Utf8Range {
            start: *r.start(),
            end: *r.end(),
        };
        let old = |r| SplitRange::Old(range(r));
        let new = |r| SplitRange::New(range(r));
        let both = |r| SplitRange::Both(range(r));

        // Use same names as the comment above to make it easier to compare.
        let (a, b, x, y) = (o.start, o.end, n.start, n.end);

        if b < x || y < a {
            // case 1, case 2
            None
        } else if a == x && b == y {
            // case 3
            Some(Split::parts1(both(a..=b)))
        } else if a == x && b < y {
            // case 4
            Some(Split::parts2(both(a..=b), new(b + 1..=y)))
        } else if b == y && a > x {
            // case 5
            Some(Split::parts2(new(x..=a - 1), both(a..=b)))
        } else if x == a && y < b {
            // case 6
            Some(Split::parts2(both(x..=y), old(y + 1..=b)))
        } else if y == b && x > a {
            // case 7
            Some(Split::parts2(old(a..=x - 1), both(x..=y)))
        } else if a > x && b < y {
            // case 8
            Some(Split::parts3(new(x..=a - 1), both(a..=b), new(b + 1..=y)))
        } else if x > a && y < b {
            // case 9
            Some(Split::parts3(old(a..=x - 1), both(x..=y), old(y + 1..=b)))
        } else if b == x && a < y {
            // case 10
            Some(Split::parts3(old(a..=b - 1), both(b..=b), new(b + 1..=y)))
        } else if y == a && x < b {
            // case 11
            Some(Split::parts3(new(x..=y - 1), both(y..=y), old(y + 1..=b)))
        } else if b > x && b < y {
            // case 12
            Some(Split::parts3(old(a..=x - 1), both(x..=b), new(b + 1..=y)))
        } else if y > a && y < b {
            // case 13
            Some(Split::parts3(new(x..=a - 1), both(a..=y), old(y + 1..=b)))
        } else {
            unreachable!()
        }
    }

    /// Create a new split with a single partition. This only occurs when two
    /// ranges are equivalent.
    fn parts1(r1: SplitRange) -> Split {
        // This value doesn't matter since it is never accessed.
        let nada = SplitRange::Old(Utf8Range { start: 0, end: 0 });
        Split { partitions: [r1, nada, nada], len: 1 }
    }

    /// Create a new split with two partitions.
    fn parts2(r1: SplitRange, r2: SplitRange) -> Split {
        // This value doesn't matter since it is never accessed.
        let nada = SplitRange::Old(Utf8Range { start: 0, end: 0 });
        Split { partitions: [r1, r2, nada], len: 2 }
    }

    /// Create a new split with three partitions.
    fn parts3(r1: SplitRange, r2: SplitRange, r3: SplitRange) -> Split {
        Split { partitions: [r1, r2, r3], len: 3 }
    }

    /// Return the partitions in this split as a slice.
    fn as_slice(&self) -> &[SplitRange] {
        &self.partitions[..self.len]
    }
}

impl fmt::Debug for RangeTrie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "")?;
        for (i, state) in self.states.iter().enumerate() {
            let status = if i == FINAL.as_usize() { '*' } else { ' ' };
            writeln!(f, "{}{:06}: {:?}", status, i, state)?;
        }
        Ok(())
    }
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rs = self
            .transitions
            .iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<String>>()
            .join(", ");
        write!(f, "{}", rs)
    }
}

impl fmt::Debug for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.range.start == self.range.end {
            write!(
                f,
                "{:02X} => {:02X}",
                self.range.start,
                self.next_id.as_usize(),
            )
        } else {
            write!(
                f,
                "{:02X}-{:02X} => {:02X}",
                self.range.start,
                self.range.end,
                self.next_id.as_usize(),
            )
        }
    }
}

/// Returns true if and only if the given ranges intersect.
fn intersects(r1: Utf8Range, r2: Utf8Range) -> bool {
    !(r1.end < r2.start || r2.end < r1.start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(range: RangeInclusive<u8>) -> Utf8Range {
        Utf8Range { start: *range.start(), end: *range.end() }
    }

    fn split_maybe(
        old: RangeInclusive<u8>,
        new: RangeInclusive<u8>,
    ) -> Option<Split> {
        Split::new(r(old), r(new))
    }

    fn split(
        old: RangeInclusive<u8>,
        new: RangeInclusive<u8>,
    ) -> Vec<SplitRange> {
        split_maybe(old, new).unwrap().as_slice().to_vec()
    }

    #[test]
    fn no_splits() {
        // case 1
        assert_eq!(None, split_maybe(0..=1, 2..=3));
        // case 2
        assert_eq!(None, split_maybe(2..=3, 0..=1));
    }

    #[test]
    fn splits() {
        let range = |r: RangeInclusive<u8>| Utf8Range {
            start: *r.start(),
            end: *r.end(),
        };
        let old = |r| SplitRange::Old(range(r));
        let new = |r| SplitRange::New(range(r));
        let both = |r| SplitRange::Both(range(r));

        // case 3
        assert_eq!(split(0..=0, 0..=0), vec![both(0..=0)]);
        assert_eq!(split(9..=9, 9..=9), vec![both(9..=9)]);

        // case 4
        assert_eq!(split(0..=5, 0..=6), vec![both(0..=5), new(6..=6)]);
        assert_eq!(split(0..=5, 0..=8), vec![both(0..=5), new(6..=8)]);
        assert_eq!(split(5..=5, 5..=8), vec![both(5..=5), new(6..=8)]);

        // case 5
        assert_eq!(split(1..=5, 0..=5), vec![new(0..=0), both(1..=5)]);
        assert_eq!(split(3..=5, 0..=5), vec![new(0..=2), both(3..=5)]);
        assert_eq!(split(5..=5, 0..=5), vec![new(0..=4), both(5..=5)]);

        // case 6
        assert_eq!(split(0..=6, 0..=5), vec![both(0..=5), old(6..=6)]);
        assert_eq!(split(0..=8, 0..=5), vec![both(0..=5), old(6..=8)]);
        assert_eq!(split(5..=8, 5..=5), vec![both(5..=5), old(6..=8)]);

        // case 7
        assert_eq!(split(0..=5, 1..=5), vec![old(0..=0), both(1..=5)]);
        assert_eq!(split(0..=5, 3..=5), vec![old(0..=2), both(3..=5)]);
        assert_eq!(split(0..=5, 5..=5), vec![old(0..=4), both(5..=5)]);

        // case 8
        assert_eq!(
            split(3..=6, 2..=7),
            vec![new(2..=2), both(3..=6), new(7..=7)],
        );
        assert_eq!(
            split(3..=6, 1..=8),
            vec![new(1..=2), both(3..=6), new(7..=8)],
        );

        // case 9
        assert_eq!(
            split(2..=7, 3..=6),
            vec![old(2..=2), both(3..=6), old(7..=7)],
        );
        assert_eq!(
            split(1..=8, 3..=6),
            vec![old(1..=2), both(3..=6), old(7..=8)],
        );

        // case 10
        assert_eq!(
            split(3..=6, 6..=7),
            vec![old(3..=5), both(6..=6), new(7..=7)],
        );
        assert_eq!(
            split(3..=6, 6..=8),
            vec![old(3..=5), both(6..=6), new(7..=8)],
        );
        assert_eq!(
            split(5..=6, 6..=7),
            vec![old(5..=5), both(6..=6), new(7..=7)],
        );

        // case 11
        assert_eq!(
            split(6..=7, 3..=6),
            vec![new(3..=5), both(6..=6), old(7..=7)],
        );
        assert_eq!(
            split(6..=8, 3..=6),
            vec![new(3..=5), both(6..=6), old(7..=8)],
        );
        assert_eq!(
            split(6..=7, 5..=6),
            vec![new(5..=5), both(6..=6), old(7..=7)],
        );

        // case 12
        assert_eq!(
            split(3..=7, 5..=9),
            vec![old(3..=4), both(5..=7), new(8..=9)],
        );
        assert_eq!(
            split(3..=5, 4..=6),
            vec![old(3..=3), both(4..=5), new(6..=6)],
        );

        // case 13
        assert_eq!(
            split(5..=9, 3..=7),
            vec![new(3..=4), both(5..=7), old(8..=9)],
        );
        assert_eq!(
            split(4..=6, 3..=5),
            vec![new(3..=3), both(4..=5), old(6..=6)],
        );
    }

    // Arguably there should be more tests here, but in practice, this data
    // structure is well covered by the huge number of regex tests.
}
