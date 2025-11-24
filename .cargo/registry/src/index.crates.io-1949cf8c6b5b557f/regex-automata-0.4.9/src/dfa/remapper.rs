use alloc::vec::Vec;

use crate::util::primitives::StateID;

/// Remappable is a tightly coupled abstraction that facilitates remapping
/// state identifiers in DFAs.
///
/// The main idea behind remapping state IDs is that DFAs often need to check
/// if a certain state is a "special" state of some kind (like a match state)
/// during a search. Since this is extremely perf critical code, we want this
/// check to be as fast as possible. Partitioning state IDs into, for example,
/// into "non-match" and "match" states means one can tell if a state is a
/// match state via a simple comparison of the state ID.
///
/// The issue is that during the DFA construction process, it's not
/// particularly easy to partition the states. Instead, the simplest thing is
/// to often just do a pass over all of the states and shuffle them into their
/// desired partitionings. To do that, we need a mechanism for swapping states.
/// Hence, this abstraction.
///
/// Normally, for such little code, I would just duplicate it. But this is a
/// key optimization and the implementation is a bit subtle. So the abstraction
/// is basically a ham-fisted attempt at DRY. The only place we use this is in
/// the dense and one-pass DFAs.
///
/// See also src/dfa/special.rs for a more detailed explanation of how dense
/// DFAs are partitioned.
pub(super) trait Remappable: core::fmt::Debug {
    /// Return the total number of states.
    fn state_len(&self) -> usize;
    /// Return the power-of-2 exponent that yields the stride. The pertinent
    /// laws here are, where N=stride2: 2^N=stride and len(alphabet) <= stride.
    fn stride2(&self) -> usize;
    /// Swap the states pointed to by the given IDs. The underlying finite
    /// state machine should be mutated such that all of the transitions in
    /// `id1` are now in the memory region where the transitions for `id2`
    /// were, and all of the transitions in `id2` are now in the memory region
    /// where the transitions for `id1` were.
    ///
    /// Essentially, this "moves" `id1` to `id2` and `id2` to `id1`.
    ///
    /// It is expected that, after calling this, the underlying value will be
    /// left in an inconsistent state, since any other transitions pointing to,
    /// e.g., `id1` need to be updated to point to `id2`, since that's where
    /// `id1` moved to.
    ///
    /// In order to "fix" the underlying inconsistent state, a `Remapper`
    /// should be used to guarantee that `remap` is called at the appropriate
    /// time.
    fn swap_states(&mut self, id1: StateID, id2: StateID);
    /// This must remap every single state ID in the underlying value according
    /// to the function given. For example, in a DFA, this should remap every
    /// transition and every starting state ID.
    fn remap(&mut self, map: impl Fn(StateID) -> StateID);
}

/// Remapper is an abstraction the manages the remapping of state IDs in a
/// finite state machine. This is useful when one wants to shuffle states into
/// different positions in the machine.
///
/// One of the key complexities this manages is the ability to correctly move
/// one state multiple times.
///
/// Once shuffling is complete, `remap` must be called, which will rewrite
/// all pertinent transitions to updated state IDs. Neglecting to call `remap`
/// will almost certainly result in a corrupt machine.
#[derive(Debug)]
pub(super) struct Remapper {
    /// A map from the index of a state to its pre-multiplied identifier.
    ///
    /// When a state is swapped with another, then their corresponding
    /// locations in this map are also swapped. Thus, its new position will
    /// still point to its old pre-multiplied StateID.
    ///
    /// While there is a bit more to it, this then allows us to rewrite the
    /// state IDs in a DFA's transition table in a single pass. This is done
    /// by iterating over every ID in this map, then iterating over each
    /// transition for the state at that ID and re-mapping the transition from
    /// `old_id` to `map[dfa.to_index(old_id)]`. That is, we find the position
    /// in this map where `old_id` *started*, and set it to where it ended up
    /// after all swaps have been completed.
    map: Vec<StateID>,
    /// A mapper from state index to state ID (and back).
    idxmap: IndexMapper,
}

impl Remapper {
    /// Create a new remapper from the given remappable implementation. The
    /// remapper can then be used to swap states. The remappable value given
    /// here must the same one given to `swap` and `remap`.
    pub(super) fn new(r: &impl Remappable) -> Remapper {
        let idxmap = IndexMapper { stride2: r.stride2() };
        let map = (0..r.state_len()).map(|i| idxmap.to_state_id(i)).collect();
        Remapper { map, idxmap }
    }

    /// Swap two states. Once this is called, callers must follow through to
    /// call `remap`, or else it's possible for the underlying remappable
    /// value to be in a corrupt state.
    pub(super) fn swap(
        &mut self,
        r: &mut impl Remappable,
        id1: StateID,
        id2: StateID,
    ) {
        if id1 == id2 {
            return;
        }
        r.swap_states(id1, id2);
        self.map.swap(self.idxmap.to_index(id1), self.idxmap.to_index(id2));
    }

    /// Complete the remapping process by rewriting all state IDs in the
    /// remappable value according to the swaps performed.
    pub(super) fn remap(mut self, r: &mut impl Remappable) {
        // Update the map to account for states that have been swapped
        // multiple times. For example, if (A, C) and (C, G) are swapped, then
        // transitions previously pointing to A should now point to G. But if
        // we don't update our map, they will erroneously be set to C. All we
        // do is follow the swaps in our map until we see our original state
        // ID.
        //
        // The intuition here is to think about how changes are made to the
        // map: only through pairwise swaps. That means that starting at any
        // given state, it is always possible to find the loop back to that
        // state by following the swaps represented in the map (which might be
        // 0 swaps).
        //
        // We are also careful to clone the map before starting in order to
        // freeze it. We use the frozen map to find our loops, since we need to
        // update our map as well. Without freezing it, our updates could break
        // the loops referenced above and produce incorrect results.
        let oldmap = self.map.clone();
        for i in 0..r.state_len() {
            let cur_id = self.idxmap.to_state_id(i);
            let mut new_id = oldmap[i];
            if cur_id == new_id {
                continue;
            }
            loop {
                let id = oldmap[self.idxmap.to_index(new_id)];
                if cur_id == id {
                    self.map[i] = new_id;
                    break;
                }
                new_id = id;
            }
        }
        r.remap(|next| self.map[self.idxmap.to_index(next)]);
    }
}

/// A simple type for mapping between state indices and state IDs.
///
/// The reason why this exists is because state IDs are "premultiplied." That
/// is, in order to get to the transitions for a particular state, one need
/// only use the state ID as-is, instead of having to multiple it by transition
/// table's stride.
///
/// The downside of this is that it's inconvenient to map between state IDs
/// using a dense map, e.g., Vec<StateID>. That's because state IDs look like
/// `0`, `0+stride`, `0+2*stride`, `0+3*stride`, etc., instead of `0`, `1`,
/// `2`, `3`, etc.
///
/// Since our state IDs are premultiplied, we can convert back-and-forth
/// between IDs and indices by simply unmultiplying the IDs and multiplying the
/// indices.
#[derive(Debug)]
struct IndexMapper {
    /// The power of 2 corresponding to the stride of the corresponding
    /// transition table. 'id >> stride2' de-multiplies an ID while 'index <<
    /// stride2' pre-multiplies an index to an ID.
    stride2: usize,
}

impl IndexMapper {
    /// Convert a state ID to a state index.
    fn to_index(&self, id: StateID) -> usize {
        id.as_usize() >> self.stride2
    }

    /// Convert a state index to a state ID.
    fn to_state_id(&self, index: usize) -> StateID {
        // CORRECTNESS: If the given index is not valid, then it is not
        // required for this to panic or return a valid state ID. We'll "just"
        // wind up with panics or silent logic errors at some other point.
        StateID::new_unchecked(index << self.stride2)
    }
}

#[cfg(feature = "dfa-build")]
mod dense {
    use crate::{dfa::dense::OwnedDFA, util::primitives::StateID};

    use super::Remappable;

    impl Remappable for OwnedDFA {
        fn state_len(&self) -> usize {
            OwnedDFA::state_len(self)
        }

        fn stride2(&self) -> usize {
            OwnedDFA::stride2(self)
        }

        fn swap_states(&mut self, id1: StateID, id2: StateID) {
            OwnedDFA::swap_states(self, id1, id2)
        }

        fn remap(&mut self, map: impl Fn(StateID) -> StateID) {
            OwnedDFA::remap(self, map)
        }
    }
}

#[cfg(feature = "dfa-onepass")]
mod onepass {
    use crate::{dfa::onepass::DFA, util::primitives::StateID};

    use super::Remappable;

    impl Remappable for DFA {
        fn state_len(&self) -> usize {
            DFA::state_len(self)
        }

        fn stride2(&self) -> usize {
            // We don't do pre-multiplication for the one-pass DFA, so
            // returning 0 has the effect of making state IDs and state indices
            // equivalent.
            0
        }

        fn swap_states(&mut self, id1: StateID, id2: StateID) {
            DFA::swap_states(self, id1, id2)
        }

        fn remap(&mut self, map: impl Fn(StateID) -> StateID) {
            DFA::remap(self, map)
        }
    }
}
