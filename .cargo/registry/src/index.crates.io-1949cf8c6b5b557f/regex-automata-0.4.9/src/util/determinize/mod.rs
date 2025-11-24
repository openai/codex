/*!
This module contains types and routines for implementing determinization.

In this crate, there are at least two places where we implement
determinization: fully ahead-of-time compiled DFAs in the `dfa` module and
lazily compiled DFAs in the `hybrid` module. The stuff in this module
corresponds to the things that are in common between these implementations.

There are three broad things that our implementations of determinization have
in common, as defined by this module:

* The classification of start states. That is, whether we're dealing with
word boundaries, line boundaries, etc., is all the same. This also includes
the look-behind assertions that are satisfied by each starting state
classification.
* The representation of DFA states as sets of NFA states, including
convenience types for building these DFA states that are amenable to reusing
allocations.
* Routines for the "classical" parts of determinization: computing the
epsilon closure, tracking match states (with corresponding pattern IDs, since
we support multi-pattern finite automata) and, of course, computing the
transition function between states for units of input.

I did consider a couple of alternatives to this particular form of code reuse:

1. Don't do any code reuse. The problem here is that we *really* want both
forms of determinization to do exactly identical things when it comes to
their handling of NFA states. While our tests generally ensure this, the code
is tricky and large enough where not reusing code is a pretty big bummer.

2. Implement all of determinization once and make it generic over fully
compiled DFAs and lazily compiled DFAs. While I didn't actually try this
approach, my instinct is that it would be more complex than is needed here.
And the interface required would be pretty hairy. Instead, I think splitting
it into logical sub-components works better.
*/

use alloc::vec::Vec;

pub(crate) use self::state::{
    State, StateBuilderEmpty, StateBuilderMatches, StateBuilderNFA,
};

use crate::{
    nfa::thompson,
    util::{
        alphabet,
        look::{Look, LookSet},
        primitives::StateID,
        search::MatchKind,
        sparse_set::{SparseSet, SparseSets},
        start::Start,
        utf8,
    },
};

mod state;

/// Compute the set of all reachable NFA states, including the full epsilon
/// closure, from a DFA state for a single unit of input. The set of reachable
/// states is returned as a `StateBuilderNFA`. The `StateBuilderNFA` returned
/// also includes any look-behind assertions satisfied by `unit`, in addition
/// to whether it is a match state. For multi-pattern DFAs, the builder will
/// also include the pattern IDs that match (in the order seen).
///
/// `nfa` must be able to resolve any NFA state in `state` and any NFA state
/// reachable via the epsilon closure of any NFA state in `state`. `sparses`
/// must have capacity equivalent to `nfa.len()`.
///
/// `match_kind` should correspond to the match semantics implemented by the
/// DFA being built. Generally speaking, for leftmost-first match semantics,
/// states that appear after the first NFA match state will not be included in
/// the `StateBuilderNFA` returned since they are impossible to visit.
///
/// `sparses` is used as scratch space for NFA traversal. Other than their
/// capacity requirements (detailed above), there are no requirements on what's
/// contained within them (if anything). Similarly, what's inside of them once
/// this routine returns is unspecified.
///
/// `stack` must have length 0. It is used as scratch space for depth first
/// traversal. After returning, it is guaranteed that `stack` will have length
/// 0.
///
/// `state` corresponds to the current DFA state on which one wants to compute
/// the transition for the input `unit`.
///
/// `empty_builder` corresponds to the builder allocation to use to produce a
/// complete `StateBuilderNFA` state. If the state is not needed (or is already
/// cached), then it can be cleared and reused without needing to create a new
/// `State`. The `StateBuilderNFA` state returned is final and ready to be
/// turned into a `State` if necessary.
pub(crate) fn next(
    nfa: &thompson::NFA,
    match_kind: MatchKind,
    sparses: &mut SparseSets,
    stack: &mut Vec<StateID>,
    state: &State,
    unit: alphabet::Unit,
    empty_builder: StateBuilderEmpty,
) -> StateBuilderNFA {
    sparses.clear();

    // Whether the NFA is matched in reverse or not. We use this in some
    // conditional logic for dealing with the exceptionally annoying CRLF-aware
    // line anchors.
    let rev = nfa.is_reverse();
    // The look-around matcher that our NFA is configured with. We don't
    // actually use it to match look-around assertions, but we do need its
    // configuration for constructing states consistent with how it matches.
    let lookm = nfa.look_matcher();

    // Put the NFA state IDs into a sparse set in case we need to
    // re-compute their epsilon closure.
    //
    // Doing this state shuffling is technically not necessary unless some
    // kind of look-around is used in the DFA. Some ad hoc experiments
    // suggested that avoiding this didn't lead to much of an improvement,
    // but perhaps more rigorous experimentation should be done. And in
    // particular, avoiding this check requires some light refactoring of
    // the code below.
    state.iter_nfa_state_ids(|nfa_id| {
        sparses.set1.insert(nfa_id);
    });

    // Compute look-ahead assertions originating from the current state. Based
    // on the input unit we're transitioning over, some additional set of
    // assertions may be true. Thus, we re-compute this state's epsilon closure
    // (but only if necessary). Notably, when we build a DFA state initially,
    // we don't enable any look-ahead assertions because we don't know whether
    // they're true or not at that point.
    if !state.look_need().is_empty() {
        // Add look-ahead assertions that are now true based on the current
        // input unit.
        let mut look_have = state.look_have().clone();
        match unit.as_u8() {
            Some(b'\r') => {
                if !rev || !state.is_half_crlf() {
                    look_have = look_have.insert(Look::EndCRLF);
                }
            }
            Some(b'\n') => {
                if rev || !state.is_half_crlf() {
                    look_have = look_have.insert(Look::EndCRLF);
                }
            }
            Some(_) => {}
            None => {
                look_have = look_have
                    .insert(Look::End)
                    .insert(Look::EndLF)
                    .insert(Look::EndCRLF);
            }
        }
        if unit.is_byte(lookm.get_line_terminator()) {
            look_have = look_have.insert(Look::EndLF);
        }
        if state.is_half_crlf()
            && ((rev && !unit.is_byte(b'\r'))
                || (!rev && !unit.is_byte(b'\n')))
        {
            look_have = look_have.insert(Look::StartCRLF);
        }
        if state.is_from_word() == unit.is_word_byte() {
            look_have = look_have
                .insert(Look::WordAsciiNegate)
                .insert(Look::WordUnicodeNegate);
        } else {
            look_have =
                look_have.insert(Look::WordAscii).insert(Look::WordUnicode);
        }
        if !unit.is_word_byte() {
            look_have = look_have
                .insert(Look::WordEndHalfAscii)
                .insert(Look::WordEndHalfUnicode);
        }
        if state.is_from_word() && !unit.is_word_byte() {
            look_have = look_have
                .insert(Look::WordEndAscii)
                .insert(Look::WordEndUnicode);
        } else if !state.is_from_word() && unit.is_word_byte() {
            look_have = look_have
                .insert(Look::WordStartAscii)
                .insert(Look::WordStartUnicode);
        }
        // If we have new assertions satisfied that are among the set of
        // assertions that exist in this state (that is, just because we added
        // an EndLF assertion above doesn't mean there is an EndLF conditional
        // epsilon transition in this state), then we re-compute this state's
        // epsilon closure using the updated set of assertions.
        //
        // Note that since our DFA states omit unconditional epsilon
        // transitions, this check is necessary for correctness. If we re-did
        // the epsilon closure below needlessly, it could change based on the
        // fact that we omitted epsilon states originally.
        if !look_have
            .subtract(state.look_have())
            .intersect(state.look_need())
            .is_empty()
        {
            for nfa_id in sparses.set1.iter() {
                epsilon_closure(
                    nfa,
                    nfa_id,
                    look_have,
                    stack,
                    &mut sparses.set2,
                );
            }
            sparses.swap();
            sparses.set2.clear();
        }
    }

    // Convert our empty builder into one that can record assertions and match
    // pattern IDs.
    let mut builder = empty_builder.into_matches();
    // Set whether the StartLF look-behind assertion is true for this
    // transition or not. The look-behind assertion for ASCII word boundaries
    // is handled below.
    if nfa.look_set_any().contains_anchor_line()
        && unit.is_byte(lookm.get_line_terminator())
    {
        // Why only handle StartLF here and not Start? That's because Start
        // can only impact the starting state, which is special cased in
        // start state handling.
        builder.set_look_have(|have| have.insert(Look::StartLF));
    }
    // We also need to add StartCRLF to our assertions too, if we can. This
    // is unfortunately a bit more complicated, because it depends on the
    // direction of the search. In the forward direction, ^ matches after a
    // \n, but in the reverse direction, ^ only matches after a \r. (This is
    // further complicated by the fact that reverse a regex means changing a ^
    // to a $ and vice versa.)
    if nfa.look_set_any().contains_anchor_crlf()
        && ((rev && unit.is_byte(b'\r')) || (!rev && unit.is_byte(b'\n')))
    {
        builder.set_look_have(|have| have.insert(Look::StartCRLF));
    }
    // And also for the start-half word boundary assertions. As long as the
    // look-behind byte is not a word char, then the assertions are satisfied.
    if nfa.look_set_any().contains_word() && !unit.is_word_byte() {
        builder.set_look_have(|have| {
            have.insert(Look::WordStartHalfAscii)
                .insert(Look::WordStartHalfUnicode)
        });
    }
    for nfa_id in sparses.set1.iter() {
        match *nfa.state(nfa_id) {
            thompson::State::Union { .. }
            | thompson::State::BinaryUnion { .. }
            | thompson::State::Fail
            | thompson::State::Look { .. }
            | thompson::State::Capture { .. } => {}
            thompson::State::Match { pattern_id } => {
                // Notice here that we are calling the NEW state a match
                // state if the OLD state we are transitioning from
                // contains an NFA match state. This is precisely how we
                // delay all matches by one byte and also what therefore
                // guarantees that starting states cannot be match states.
                //
                // If we didn't delay matches by one byte, then whether
                // a DFA is a matching state or not would be determined
                // by whether one of its own constituent NFA states
                // was a match state. (And that would be done in
                // 'add_nfa_states'.)
                //
                // Also, 'add_match_pattern_id' requires that callers never
                // pass duplicative pattern IDs. We do in fact uphold that
                // guarantee here, but it's subtle. In particular, a Thompson
                // NFA guarantees that each pattern has exactly one match
                // state. Moreover, since we're iterating over the NFA state
                // IDs in a set, we are guarateed not to have any duplicative
                // match states. Thus, it is impossible to add the same pattern
                // ID more than once.
                //
                // N.B. We delay matches by 1 byte as a way to hack 1-byte
                // look-around into DFA searches. This lets us support ^, $
                // and ASCII-only \b. The delay is also why we need a special
                // "end-of-input" (EOI) sentinel and why we need to follow the
                // EOI sentinel at the end of every search. This final EOI
                // transition is necessary to report matches found at the end
                // of a haystack.
                builder.add_match_pattern_id(pattern_id);
                if !match_kind.continue_past_first_match() {
                    break;
                }
            }
            thompson::State::ByteRange { ref trans } => {
                if trans.matches_unit(unit) {
                    epsilon_closure(
                        nfa,
                        trans.next,
                        builder.look_have(),
                        stack,
                        &mut sparses.set2,
                    );
                }
            }
            thompson::State::Sparse(ref sparse) => {
                if let Some(next) = sparse.matches_unit(unit) {
                    epsilon_closure(
                        nfa,
                        next,
                        builder.look_have(),
                        stack,
                        &mut sparses.set2,
                    );
                }
            }
            thompson::State::Dense(ref dense) => {
                if let Some(next) = dense.matches_unit(unit) {
                    epsilon_closure(
                        nfa,
                        next,
                        builder.look_have(),
                        stack,
                        &mut sparses.set2,
                    );
                }
            }
        }
    }
    // We only set the word byte if there's a word boundary look-around
    // anywhere in this regex. Otherwise, there's no point in bloating the
    // number of states if we don't have one.
    //
    // We also only set it when the state has a non-zero number of NFA states.
    // Otherwise, we could wind up with states that *should* be DEAD states
    // but are otherwise distinct from DEAD states because of this look-behind
    // assertion being set. While this can't technically impact correctness *in
    // theory*, it can create pathological DFAs that consume input until EOI or
    // a quit byte is seen. Consuming until EOI isn't a correctness problem,
    // but a (serious) perf problem. Hitting a quit byte, however, could be a
    // correctness problem since it could cause search routines to report an
    // error instead of a detected match once the quit state is entered. (The
    // search routine could be made to be a bit smarter by reporting a match
    // if one was detected once it enters a quit state (and indeed, the search
    // routines in this crate do just that), but it seems better to prevent
    // these things by construction if possible.)
    if !sparses.set2.is_empty() {
        if nfa.look_set_any().contains_word() && unit.is_word_byte() {
            builder.set_is_from_word();
        }
        if nfa.look_set_any().contains_anchor_crlf()
            && ((rev && unit.is_byte(b'\n')) || (!rev && unit.is_byte(b'\r')))
        {
            builder.set_is_half_crlf();
        }
    }
    let mut builder_nfa = builder.into_nfa();
    add_nfa_states(nfa, &sparses.set2, &mut builder_nfa);
    builder_nfa
}

/// Compute the epsilon closure for the given NFA state. The epsilon closure
/// consists of all NFA state IDs, including `start_nfa_id`, that can be
/// reached from `start_nfa_id` without consuming any input. These state IDs
/// are written to `set` in the order they are visited, but only if they are
/// not already in `set`. `start_nfa_id` must be a valid state ID for the NFA
/// given.
///
/// `look_have` consists of the satisfied assertions at the current
/// position. For conditional look-around epsilon transitions, these are
/// only followed if they are satisfied by `look_have`.
///
/// `stack` must have length 0. It is used as scratch space for depth first
/// traversal. After returning, it is guaranteed that `stack` will have length
/// 0.
pub(crate) fn epsilon_closure(
    nfa: &thompson::NFA,
    start_nfa_id: StateID,
    look_have: LookSet,
    stack: &mut Vec<StateID>,
    set: &mut SparseSet,
) {
    assert!(stack.is_empty());
    // If this isn't an epsilon state, then the epsilon closure is always just
    // itself, so there's no need to spin up the machinery below to handle it.
    if !nfa.state(start_nfa_id).is_epsilon() {
        set.insert(start_nfa_id);
        return;
    }

    stack.push(start_nfa_id);
    while let Some(mut id) = stack.pop() {
        // In many cases, we can avoid stack operations when an NFA state only
        // adds one new state to visit. In that case, we just set our ID to
        // that state and mush on. We only use the stack when an NFA state
        // introduces multiple new states to visit.
        loop {
            // Insert this NFA state, and if it's already in the set and thus
            // already visited, then we can move on to the next one.
            if !set.insert(id) {
                break;
            }
            match *nfa.state(id) {
                thompson::State::ByteRange { .. }
                | thompson::State::Sparse { .. }
                | thompson::State::Dense { .. }
                | thompson::State::Fail
                | thompson::State::Match { .. } => break,
                thompson::State::Look { look, next } => {
                    if !look_have.contains(look) {
                        break;
                    }
                    id = next;
                }
                thompson::State::Union { ref alternates } => {
                    id = match alternates.get(0) {
                        None => break,
                        Some(&id) => id,
                    };
                    // We need to process our alternates in order to preserve
                    // match preferences, so put the earliest alternates closer
                    // to the top of the stack.
                    stack.extend(alternates[1..].iter().rev());
                }
                thompson::State::BinaryUnion { alt1, alt2 } => {
                    id = alt1;
                    stack.push(alt2);
                }
                thompson::State::Capture { next, .. } => {
                    id = next;
                }
            }
        }
    }
}

/// Add the NFA state IDs in the given `set` to the given DFA builder state.
/// The order in which states are added corresponds to the order in which they
/// were added to `set`.
///
/// The DFA builder state given should already have its complete set of match
/// pattern IDs added (if any) and any look-behind assertions (StartLF, Start
/// and whether this state is being generated for a transition over a word byte
/// when applicable) that are true immediately prior to transitioning into this
/// state (via `builder.look_have()`). The match pattern IDs should correspond
/// to matches that occurred on the previous transition, since all matches are
/// delayed by one byte. The things that should _not_ be set are look-ahead
/// assertions (EndLF, End and whether the next byte is a word byte or not).
/// The builder state should also not have anything in `look_need` set, as this
/// routine will compute that for you.
///
/// The given NFA should be able to resolve all identifiers in `set` to a
/// particular NFA state. Additionally, `set` must have capacity equivalent
/// to `nfa.len()`.
pub(crate) fn add_nfa_states(
    nfa: &thompson::NFA,
    set: &SparseSet,
    builder: &mut StateBuilderNFA,
) {
    for nfa_id in set.iter() {
        match *nfa.state(nfa_id) {
            thompson::State::ByteRange { .. } => {
                builder.add_nfa_state_id(nfa_id);
            }
            thompson::State::Sparse { .. } => {
                builder.add_nfa_state_id(nfa_id);
            }
            thompson::State::Dense { .. } => {
                builder.add_nfa_state_id(nfa_id);
            }
            thompson::State::Look { look, .. } => {
                builder.add_nfa_state_id(nfa_id);
                builder.set_look_need(|need| need.insert(look));
            }
            thompson::State::Union { .. }
            | thompson::State::BinaryUnion { .. } => {
                // Pure epsilon transitions don't need to be tracked as part
                // of the DFA state. Tracking them is actually superfluous;
                // they won't cause any harm other than making determinization
                // slower.
                //
                // Why aren't these needed? Well, in an NFA, epsilon
                // transitions are really just jumping points to other states.
                // So once you hit an epsilon transition, the same set of
                // resulting states always appears. Therefore, putting them in
                // a DFA's set of ordered NFA states is strictly redundant.
                //
                // Look-around states are also epsilon transitions, but
                // they are *conditional*. So their presence could be
                // discriminatory, and thus, they are tracked above.
                //
                // But wait... why are epsilon states in our `set` in the first
                // place? Why not just leave them out? They're in our `set`
                // because it was generated by computing an epsilon closure,
                // and we want to keep track of all states we visited to avoid
                // re-visiting them. In exchange, we have to do this second
                // iteration over our collected states to finalize our DFA
                // state. In theory, we could avoid this second iteration if
                // we maintained two sets during epsilon closure: the set of
                // visited states (to avoid cycles) and the set of states that
                // will actually be used to construct the next DFA state.
                //
                // Note that this optimization requires that we re-compute the
                // epsilon closure to account for look-ahead in 'next' *only
                // when necessary*. Namely, only when the set of look-around
                // assertions changes and only when those changes are within
                // the set of assertions that are needed in order to step
                // through the closure correctly. Otherwise, if we re-do the
                // epsilon closure needlessly, it could change based on the
                // fact that we are omitting epsilon states here.
                //
                // -----
                //
                // Welp, scratch the above. It turns out that recording these
                // is in fact necessary to seemingly handle one particularly
                // annoying case: when a conditional epsilon transition is
                // put inside of a repetition operator. One specific case I
                // ran into was the regex `(?:\b|%)+` on the haystack `z%`.
                // The correct leftmost first matches are: [0, 0] and [1, 1].
                // But the DFA was reporting [0, 0] and [1, 2]. To understand
                // why this happens, consider the NFA for the aforementioned
                // regex:
                //
                //     >000000: binary-union(4, 1)
                //      000001: \x00-\xFF => 0
                //      000002: WordAscii => 5
                //      000003: % => 5
                //     ^000004: binary-union(2, 3)
                //      000005: binary-union(4, 6)
                //      000006: MATCH(0)
                //
                // The problem here is that one of the DFA start states is
                // going to consist of the NFA states [2, 3] by computing the
                // epsilon closure of state 4. State 4 isn't included because
                // we previously were not keeping track of union states. But
                // only a subset of transitions out of this state will be able
                // to follow WordAscii, and in those cases, the epsilon closure
                // is redone. The only problem is that computing the epsilon
                // closure from [2, 3] is different than computing the epsilon
                // closure from [4]. In the former case, assuming the WordAscii
                // assertion is satisfied, you get: [2, 3, 6]. In the latter
                // case, you get: [2, 6, 3]. Notice that '6' is the match state
                // and appears AFTER '3' in the former case. This leads to a
                // preferential but incorrect match of '%' before returning
                // a match. In the latter case, the match is preferred over
                // continuing to accept the '%'.
                //
                // It almost feels like we might be able to fix the NFA states
                // to avoid this, or to at least only keep track of union
                // states where this actually matters, since in the vast
                // majority of cases, this doesn't matter.
                //
                // Another alternative would be to define a new HIR property
                // called "assertion is repeated anywhere" and compute it
                // inductively over the entire pattern. If it happens anywhere,
                // which is probably pretty rare, then we record union states.
                // Otherwise we don't.
                builder.add_nfa_state_id(nfa_id);
            }
            // Capture states we definitely do not need to record, since they
            // are unconditional epsilon transitions with no branching.
            thompson::State::Capture { .. } => {}
            // It's not totally clear whether we need to record fail states or
            // not, but we do so out of an abundance of caution. Since they are
            // quite rare in practice, there isn't much cost to recording them.
            thompson::State::Fail => {
                builder.add_nfa_state_id(nfa_id);
            }
            thompson::State::Match { .. } => {
                // Normally, the NFA match state doesn't actually need to
                // be inside the DFA state. But since we delay matches by
                // one byte, the matching DFA state corresponds to states
                // that transition from the one we're building here. And
                // the way we detect those cases is by looking for an NFA
                // match state. See 'next' for how this is handled.
                builder.add_nfa_state_id(nfa_id);
            }
        }
    }
    // If we know this state contains no look-around assertions, then
    // there's no reason to track which look-around assertions were
    // satisfied when this state was created.
    if builder.look_need().is_empty() {
        builder.set_look_have(|_| LookSet::empty());
    }
}

/// Sets the appropriate look-behind assertions on the given state based on
/// this starting configuration.
pub(crate) fn set_lookbehind_from_start(
    nfa: &thompson::NFA,
    start: &Start,
    builder: &mut StateBuilderMatches,
) {
    let rev = nfa.is_reverse();
    let lineterm = nfa.look_matcher().get_line_terminator();
    let lookset = nfa.look_set_any();
    match *start {
        Start::NonWordByte => {
            if lookset.contains_word() {
                builder.set_look_have(|have| {
                    have.insert(Look::WordStartHalfAscii)
                        .insert(Look::WordStartHalfUnicode)
                });
            }
        }
        Start::WordByte => {
            if lookset.contains_word() {
                builder.set_is_from_word();
            }
        }
        Start::Text => {
            if lookset.contains_anchor_haystack() {
                builder.set_look_have(|have| have.insert(Look::Start));
            }
            if lookset.contains_anchor_line() {
                builder.set_look_have(|have| {
                    have.insert(Look::StartLF).insert(Look::StartCRLF)
                });
            }
            if lookset.contains_word() {
                builder.set_look_have(|have| {
                    have.insert(Look::WordStartHalfAscii)
                        .insert(Look::WordStartHalfUnicode)
                });
            }
        }
        Start::LineLF => {
            if rev {
                if lookset.contains_anchor_crlf() {
                    builder.set_is_half_crlf();
                }
                if lookset.contains_anchor_line() {
                    builder.set_look_have(|have| have.insert(Look::StartLF));
                }
            } else {
                if lookset.contains_anchor_line() {
                    builder.set_look_have(|have| have.insert(Look::StartCRLF));
                }
            }
            if lookset.contains_anchor_line() && lineterm == b'\n' {
                builder.set_look_have(|have| have.insert(Look::StartLF));
            }
            if lookset.contains_word() {
                builder.set_look_have(|have| {
                    have.insert(Look::WordStartHalfAscii)
                        .insert(Look::WordStartHalfUnicode)
                });
            }
        }
        Start::LineCR => {
            if lookset.contains_anchor_crlf() {
                if rev {
                    builder.set_look_have(|have| have.insert(Look::StartCRLF));
                } else {
                    builder.set_is_half_crlf();
                }
            }
            if lookset.contains_anchor_line() && lineterm == b'\r' {
                builder.set_look_have(|have| have.insert(Look::StartLF));
            }
            if lookset.contains_word() {
                builder.set_look_have(|have| {
                    have.insert(Look::WordStartHalfAscii)
                        .insert(Look::WordStartHalfUnicode)
                });
            }
        }
        Start::CustomLineTerminator => {
            if lookset.contains_anchor_line() {
                builder.set_look_have(|have| have.insert(Look::StartLF));
            }
            // This is a bit of a tricky case, but if the line terminator was
            // set to a word byte, then we also need to behave as if the start
            // configuration is Start::WordByte. That is, we need to mark our
            // state as having come from a word byte.
            if lookset.contains_word() {
                if utf8::is_word_byte(lineterm) {
                    builder.set_is_from_word();
                } else {
                    builder.set_look_have(|have| {
                        have.insert(Look::WordStartHalfAscii)
                            .insert(Look::WordStartHalfUnicode)
                    });
                }
            }
        }
    }
}
