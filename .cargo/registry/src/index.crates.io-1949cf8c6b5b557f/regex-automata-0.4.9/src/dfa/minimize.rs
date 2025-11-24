use core::{cell::RefCell, fmt, mem};

use alloc::{collections::BTreeMap, rc::Rc, vec, vec::Vec};

use crate::{
    dfa::{automaton::Automaton, dense, DEAD},
    util::{
        alphabet,
        primitives::{PatternID, StateID},
    },
};

/// An implementation of Hopcroft's algorithm for minimizing DFAs.
///
/// The algorithm implemented here is mostly taken from Wikipedia:
/// https://en.wikipedia.org/wiki/DFA_minimization#Hopcroft's_algorithm
///
/// This code has had some light optimization attention paid to it,
/// particularly in the form of reducing allocation as much as possible.
/// However, it is still generally slow. Future optimization work should
/// probably focus on the bigger picture rather than micro-optimizations. For
/// example:
///
/// 1. Figure out how to more intelligently create initial partitions. That is,
///    Hopcroft's algorithm starts by creating two partitions of DFA states
///    that are known to NOT be equivalent: match states and non-match states.
///    The algorithm proceeds by progressively refining these partitions into
///    smaller partitions. If we could start with more partitions, then we
///    could reduce the amount of work that Hopcroft's algorithm needs to do.
/// 2. For every partition that we visit, we find all incoming transitions to
///    every state in the partition for *every* element in the alphabet. (This
///    is why using byte classes can significantly decrease minimization times,
///    since byte classes shrink the alphabet.) This is quite costly and there
///    is perhaps some redundant work being performed depending on the specific
///    states in the set. For example, we might be able to only visit some
///    elements of the alphabet based on the transitions.
/// 3. Move parts of minimization into determinization. If minimization has
///    fewer states to deal with, then it should run faster. A prime example
///    of this might be large Unicode classes, which are generated in way that
///    can create a lot of redundant states. (Some work has been done on this
///    point during NFA compilation via the algorithm described in the
///    "Incremental Construction of MinimalAcyclic Finite-State Automata"
///    paper.)
pub(crate) struct Minimizer<'a> {
    dfa: &'a mut dense::OwnedDFA,
    in_transitions: Vec<Vec<Vec<StateID>>>,
    partitions: Vec<StateSet>,
    waiting: Vec<StateSet>,
}

impl<'a> fmt::Debug for Minimizer<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Minimizer")
            .field("dfa", &self.dfa)
            .field("in_transitions", &self.in_transitions)
            .field("partitions", &self.partitions)
            .field("waiting", &self.waiting)
            .finish()
    }
}

/// A set of states. A state set makes up a single partition in Hopcroft's
/// algorithm.
///
/// It is represented by an ordered set of state identifiers. We use shared
/// ownership so that a single state set can be in both the set of partitions
/// and in the set of waiting sets simultaneously without an additional
/// allocation. Generally, once a state set is built, it becomes immutable.
///
/// We use this representation because it avoids the overhead of more
/// traditional set data structures (HashSet/BTreeSet), and also because
/// computing intersection/subtraction on this representation is especially
/// fast.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct StateSet {
    ids: Rc<RefCell<Vec<StateID>>>,
}

impl<'a> Minimizer<'a> {
    pub fn new(dfa: &'a mut dense::OwnedDFA) -> Minimizer<'a> {
        let in_transitions = Minimizer::incoming_transitions(dfa);
        let partitions = Minimizer::initial_partitions(dfa);
        let waiting = partitions.clone();
        Minimizer { dfa, in_transitions, partitions, waiting }
    }

    pub fn run(mut self) {
        let stride2 = self.dfa.stride2();
        let as_state_id = |index: usize| -> StateID {
            StateID::new(index << stride2).unwrap()
        };
        let as_index = |id: StateID| -> usize { id.as_usize() >> stride2 };

        let mut incoming = StateSet::empty();
        let mut scratch1 = StateSet::empty();
        let mut scratch2 = StateSet::empty();
        let mut newparts = vec![];

        // This loop is basically Hopcroft's algorithm. Everything else is just
        // shuffling data around to fit our representation.
        while let Some(set) = self.waiting.pop() {
            for b in self.dfa.byte_classes().iter() {
                self.find_incoming_to(b, &set, &mut incoming);
                // If incoming is empty, then the intersection with any other
                // set must also be empty. So 'newparts' just ends up being
                // 'self.partitions'. So there's no need to go through the loop
                // below.
                //
                // This actually turns out to be rather large optimization. On
                // the order of making minimization 4-5x faster. It's likely
                // that the vast majority of all states have very few incoming
                // transitions.
                if incoming.is_empty() {
                    continue;
                }

                for p in 0..self.partitions.len() {
                    self.partitions[p].intersection(&incoming, &mut scratch1);
                    if scratch1.is_empty() {
                        newparts.push(self.partitions[p].clone());
                        continue;
                    }

                    self.partitions[p].subtract(&incoming, &mut scratch2);
                    if scratch2.is_empty() {
                        newparts.push(self.partitions[p].clone());
                        continue;
                    }

                    let (x, y) =
                        (scratch1.deep_clone(), scratch2.deep_clone());
                    newparts.push(x.clone());
                    newparts.push(y.clone());
                    match self.find_waiting(&self.partitions[p]) {
                        Some(i) => {
                            self.waiting[i] = x;
                            self.waiting.push(y);
                        }
                        None => {
                            if x.len() <= y.len() {
                                self.waiting.push(x);
                            } else {
                                self.waiting.push(y);
                            }
                        }
                    }
                }
                newparts = mem::replace(&mut self.partitions, newparts);
                newparts.clear();
            }
        }

        // At this point, we now have a minimal partitioning of states, where
        // each partition is an equivalence class of DFA states. Now we need to
        // use this partitioning to update the DFA to only contain one state for
        // each partition.

        // Create a map from DFA state ID to the representative ID of the
        // equivalence class to which it belongs. The representative ID of an
        // equivalence class of states is the minimum ID in that class.
        let mut state_to_part = vec![DEAD; self.dfa.state_len()];
        for p in &self.partitions {
            p.iter(|id| state_to_part[as_index(id)] = p.min());
        }

        // Generate a new contiguous sequence of IDs for minimal states, and
        // create a map from equivalence IDs to the new IDs. Thus, the new
        // minimal ID of *any* state in the unminimized DFA can be obtained
        // with minimals_ids[state_to_part[old_id]].
        let mut minimal_ids = vec![DEAD; self.dfa.state_len()];
        let mut new_index = 0;
        for state in self.dfa.states() {
            if state_to_part[as_index(state.id())] == state.id() {
                minimal_ids[as_index(state.id())] = as_state_id(new_index);
                new_index += 1;
            }
        }
        // The total number of states in the minimal DFA.
        let minimal_count = new_index;
        // Convenience function for remapping state IDs. This takes an old ID,
        // looks up its Hopcroft partition and then maps that to the new ID
        // range.
        let remap = |old| minimal_ids[as_index(state_to_part[as_index(old)])];

        // Re-map this DFA in place such that the only states remaining
        // correspond to the representative states of every equivalence class.
        for id in (0..self.dfa.state_len()).map(as_state_id) {
            // If this state isn't a representative for an equivalence class,
            // then we skip it since it won't appear in the minimal DFA.
            if state_to_part[as_index(id)] != id {
                continue;
            }
            self.dfa.remap_state(id, remap);
            self.dfa.swap_states(id, minimal_ids[as_index(id)]);
        }
        // Trim off all unused states from the pre-minimized DFA. This
        // represents all states that were merged into a non-singleton
        // equivalence class of states, and appeared after the first state
        // in each such class. (Because the state with the smallest ID in each
        // equivalence class is its representative ID.)
        self.dfa.truncate_states(minimal_count);

        // Update the new start states, which is now just the minimal ID of
        // whatever state the old start state was collapsed into. Also, we
        // collect everything before-hand to work around the borrow checker.
        // We're already allocating so much that this is probably fine. If this
        // turns out to be costly, then I guess add a `starts_mut` iterator.
        let starts: Vec<_> = self.dfa.starts().collect();
        for (old_start_id, anchored, start_type) in starts {
            self.dfa.set_start_state(
                anchored,
                start_type,
                remap(old_start_id),
            );
        }

        // Update the match state pattern ID list for multi-regexes. All we
        // need to do is remap the match state IDs. The pattern ID lists are
        // always the same as they were since match states with distinct
        // pattern ID lists are always considered distinct states.
        let mut pmap = BTreeMap::new();
        for (match_id, pattern_ids) in self.dfa.pattern_map() {
            let new_id = remap(match_id);
            pmap.insert(new_id, pattern_ids);
        }
        // This unwrap is OK because minimization never increases the number of
        // match states or patterns in those match states. Since minimization
        // runs after the pattern map has already been set at least once, we
        // know that our match states cannot error.
        self.dfa.set_pattern_map(&pmap).unwrap();

        // In order to update the ID of the maximum match state, we need to
        // find the maximum ID among all of the match states in the minimized
        // DFA. This is not necessarily the new ID of the unminimized maximum
        // match state, since that could have been collapsed with a much
        // earlier match state. Therefore, to find the new max match state,
        // we iterate over all previous match states, find their corresponding
        // new minimal ID, and take the maximum of those.
        let old = self.dfa.special().clone();
        let new = self.dfa.special_mut();
        // ... but only remap if we had match states.
        if old.matches() {
            new.min_match = StateID::MAX;
            new.max_match = StateID::ZERO;
            for i in as_index(old.min_match)..=as_index(old.max_match) {
                let new_id = remap(as_state_id(i));
                if new_id < new.min_match {
                    new.min_match = new_id;
                }
                if new_id > new.max_match {
                    new.max_match = new_id;
                }
            }
        }
        // ... same, but for start states.
        if old.starts() {
            new.min_start = StateID::MAX;
            new.max_start = StateID::ZERO;
            for i in as_index(old.min_start)..=as_index(old.max_start) {
                let new_id = remap(as_state_id(i));
                if new_id == DEAD {
                    continue;
                }
                if new_id < new.min_start {
                    new.min_start = new_id;
                }
                if new_id > new.max_start {
                    new.max_start = new_id;
                }
            }
            if new.max_start == DEAD {
                new.min_start = DEAD;
            }
        }
        new.quit_id = remap(new.quit_id);
        new.set_max();
    }

    fn find_waiting(&self, set: &StateSet) -> Option<usize> {
        self.waiting.iter().position(|s| s == set)
    }

    fn find_incoming_to(
        &self,
        b: alphabet::Unit,
        set: &StateSet,
        incoming: &mut StateSet,
    ) {
        incoming.clear();
        set.iter(|id| {
            for &inid in
                &self.in_transitions[self.dfa.to_index(id)][b.as_usize()]
            {
                incoming.add(inid);
            }
        });
        incoming.canonicalize();
    }

    fn initial_partitions(dfa: &dense::OwnedDFA) -> Vec<StateSet> {
        // For match states, we know that two match states with different
        // pattern ID lists will *always* be distinct, so we can partition them
        // initially based on that.
        let mut matching: BTreeMap<Vec<PatternID>, StateSet> = BTreeMap::new();
        let mut is_quit = StateSet::empty();
        let mut no_match = StateSet::empty();
        for state in dfa.states() {
            if dfa.is_match_state(state.id()) {
                let mut pids = vec![];
                for i in 0..dfa.match_len(state.id()) {
                    pids.push(dfa.match_pattern(state.id(), i));
                }
                matching
                    .entry(pids)
                    .or_insert(StateSet::empty())
                    .add(state.id());
            } else if dfa.is_quit_state(state.id()) {
                is_quit.add(state.id());
            } else {
                no_match.add(state.id());
            }
        }

        let mut sets: Vec<StateSet> =
            matching.into_iter().map(|(_, set)| set).collect();
        sets.push(no_match);
        sets.push(is_quit);
        sets
    }

    fn incoming_transitions(dfa: &dense::OwnedDFA) -> Vec<Vec<Vec<StateID>>> {
        let mut incoming = vec![];
        for _ in dfa.states() {
            incoming.push(vec![vec![]; dfa.alphabet_len()]);
        }
        for state in dfa.states() {
            for (b, next) in state.transitions() {
                incoming[dfa.to_index(next)][b.as_usize()].push(state.id());
            }
        }
        incoming
    }
}

impl StateSet {
    fn empty() -> StateSet {
        StateSet { ids: Rc::new(RefCell::new(vec![])) }
    }

    fn add(&mut self, id: StateID) {
        self.ids.borrow_mut().push(id);
    }

    fn min(&self) -> StateID {
        self.ids.borrow()[0]
    }

    fn canonicalize(&mut self) {
        self.ids.borrow_mut().sort();
        self.ids.borrow_mut().dedup();
    }

    fn clear(&mut self) {
        self.ids.borrow_mut().clear();
    }

    fn len(&self) -> usize {
        self.ids.borrow().len()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn deep_clone(&self) -> StateSet {
        let ids = self.ids.borrow().iter().cloned().collect();
        StateSet { ids: Rc::new(RefCell::new(ids)) }
    }

    fn iter<F: FnMut(StateID)>(&self, mut f: F) {
        for &id in self.ids.borrow().iter() {
            f(id);
        }
    }

    fn intersection(&self, other: &StateSet, dest: &mut StateSet) {
        dest.clear();
        if self.is_empty() || other.is_empty() {
            return;
        }

        let (seta, setb) = (self.ids.borrow(), other.ids.borrow());
        let (mut ita, mut itb) = (seta.iter().cloned(), setb.iter().cloned());
        let (mut a, mut b) = (ita.next().unwrap(), itb.next().unwrap());
        loop {
            if a == b {
                dest.add(a);
                a = match ita.next() {
                    None => break,
                    Some(a) => a,
                };
                b = match itb.next() {
                    None => break,
                    Some(b) => b,
                };
            } else if a < b {
                a = match ita.next() {
                    None => break,
                    Some(a) => a,
                };
            } else {
                b = match itb.next() {
                    None => break,
                    Some(b) => b,
                };
            }
        }
    }

    fn subtract(&self, other: &StateSet, dest: &mut StateSet) {
        dest.clear();
        if self.is_empty() || other.is_empty() {
            self.iter(|s| dest.add(s));
            return;
        }

        let (seta, setb) = (self.ids.borrow(), other.ids.borrow());
        let (mut ita, mut itb) = (seta.iter().cloned(), setb.iter().cloned());
        let (mut a, mut b) = (ita.next().unwrap(), itb.next().unwrap());
        loop {
            if a == b {
                a = match ita.next() {
                    None => break,
                    Some(a) => a,
                };
                b = match itb.next() {
                    None => {
                        dest.add(a);
                        break;
                    }
                    Some(b) => b,
                };
            } else if a < b {
                dest.add(a);
                a = match ita.next() {
                    None => break,
                    Some(a) => a,
                };
            } else {
                b = match itb.next() {
                    None => {
                        dest.add(a);
                        break;
                    }
                    Some(b) => b,
                };
            }
        }
        for a in ita {
            dest.add(a);
        }
    }
}
