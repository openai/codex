use crate::util::primitives::StateID;

/// A collection of sentinel state IDs for Aho-Corasick automata.
///
/// This specifically enables the technique by which we determine which states
/// are dead, matches or start states. Namely, by arranging states in a
/// particular order, we can determine the type of a state simply by looking at
/// its ID.
#[derive(Clone, Debug)]
pub(crate) struct Special {
    /// The maximum ID of all the "special" states. This corresponds either to
    /// start_anchored_id when a prefilter is active and max_match_id when a
    /// prefilter is not active. The idea here is that if there is no prefilter,
    /// then there is no point in treating start states as special.
    pub(crate) max_special_id: StateID,
    /// The maximum ID of all the match states. Any state ID bigger than this
    /// is guaranteed to be a non-match ID.
    ///
    /// It is possible and legal for max_match_id to be equal to
    /// start_anchored_id, which occurs precisely in the case where the empty
    /// string is a pattern that was added to the underlying automaton.
    pub(crate) max_match_id: StateID,
    /// The state ID of the start state used for unanchored searches.
    pub(crate) start_unanchored_id: StateID,
    /// The state ID of the start state used for anchored searches. This is
    /// always start_unanchored_id+1.
    pub(crate) start_anchored_id: StateID,
}

impl Special {
    /// Create a new set of "special" state IDs with all IDs initialized to
    /// zero. The general idea here is that they will be updated and set to
    /// correct values later.
    pub(crate) fn zero() -> Special {
        Special {
            max_special_id: StateID::ZERO,
            max_match_id: StateID::ZERO,
            start_unanchored_id: StateID::ZERO,
            start_anchored_id: StateID::ZERO,
        }
    }
}
