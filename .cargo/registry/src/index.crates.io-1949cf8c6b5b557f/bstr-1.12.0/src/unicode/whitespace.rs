use regex_automata::{dfa::Automaton, Anchored, Input};

use crate::unicode::fsm::{
    whitespace_anchored_fwd::WHITESPACE_ANCHORED_FWD,
    whitespace_anchored_rev::WHITESPACE_ANCHORED_REV,
};

/// Return the first position of a non-whitespace character.
pub fn whitespace_len_fwd(slice: &[u8]) -> usize {
    let input = Input::new(slice).anchored(Anchored::Yes);
    WHITESPACE_ANCHORED_FWD
        .try_search_fwd(&input)
        .unwrap()
        .map_or(0, |hm| hm.offset())
}

/// Return the last position of a non-whitespace character.
pub fn whitespace_len_rev(slice: &[u8]) -> usize {
    let input = Input::new(slice).anchored(Anchored::Yes);
    WHITESPACE_ANCHORED_REV
        .try_search_rev(&input)
        .unwrap()
        .map_or(slice.len(), |hm| hm.offset())
}
