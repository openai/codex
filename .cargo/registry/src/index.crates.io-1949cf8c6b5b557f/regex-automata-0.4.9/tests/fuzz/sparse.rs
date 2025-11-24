// This is a regression test for a bug in how special states are handled. The
// fuzzer found a case where a state returned true for 'is_special_state' but
// *didn't* return true for 'is_dead_state', 'is_quit_state', 'is_match_state',
// 'is_start_state' or 'is_accel_state'. This in turn tripped a debug assertion
// in the core matching loop that requires 'is_special_state' being true to
// imply that one of the other routines returns true.
//
// We fixed this by adding some validation to both dense and sparse DFAs that
// checks that this property is true for every state ID in the DFA.
#[test]
fn invalid_special_state() {
    let data = include_bytes!(
        "testdata/deserialize_sparse_crash-a1b839d899ced76d5d7d0f78f9edb7a421505838",
    );
    let _ = fuzz_run(data);
}

// This is an interesting case where a fuzzer generated a DFA with
// a transition to a state ID that decoded as a valid state, but
// where the ID itself did not point to one of the two existing
// states for this particular DFA. This combined with marking this
// transition's state ID as special but without actually making one of the
// 'is_{dead,quit,match,start,accel}_state' predicates return true ended up
// tripping the 'debug_assert(dfa.is_quit_state(sid))' code in the search
// routine.
//
// We fixed this in alloc mode by checking that every transition points to a
// valid state ID. Technically this bug still exists in core-only mode, but
// it's not clear how to fix it. And it's worth pointing out that the search
// routine won't panic in production. It will just provide invalid results. And
// that's acceptable within the contract of DFA::from_bytes.
#[test]
fn transition_to_invalid_but_valid_state() {
    let data = include_bytes!(
        "testdata/deserialize_sparse_crash-dbb8172d3984e7e7d03f4b5f8bb86ecd1460eff9",
    );
    let _ = fuzz_run(data);
}

// Another one caught by the fuzzer where it generated a DFA that reported a
// start state as a match state. Since matches are always delayed by one byte,
// start states specifically cannot be match states. And indeed, the search
// code relies on this.
#[test]
fn start_state_is_not_match_state() {
    let data = include_bytes!(
        "testdata/deserialize_sparse_crash-0da59c0434eaf35e5a6b470fa9244bb79c72b000",
    );
    let _ = fuzz_run(data);
}

// This is variation on 'transition_to_invalid_but_valid_state', but happens
// to a start state. Namely, the fuzz data here builds a DFA with a start
// state ID that is incorrect but points to a sequence of bytes that satisfies
// state decoding validation. This errant state in turn has a non-zero number
// of transitions, and its those transitions that point to a state that does
// *not* satisfy state decoding validation. But we never checked those. So the
// fix here was to add validation of the transitions off of the start state.
#[test]
fn start_state_has_valid_transitions() {
    let data = include_bytes!(
        "testdata/deserialize_sparse_crash-61fd8e3003bf9d99f6c1e5a8488727eefd234b98",
    );
    let _ = fuzz_run(data);
}

// This fuzz input generated a DFA with a state whose ID was in the match state
// ID range, but where the state itself was encoded with zero pattern IDs. We
// added validation code to check this case.
#[test]
fn match_state_inconsistency() {
    let data = include_bytes!(
        "testdata/deserialize_sparse_crash-c383ae07ec5e191422eadc492117439011816570",
    );
    let _ = fuzz_run(data);
}

// This fuzz input generated a DFA with a state whose ID was in the accelerator
// range, but who didn't have any accelerators. This violated an invariant that
// assumes that if 'dfa.is_accel_state(sid)' returns true, then the state must
// have some accelerators.
#[test]
fn invalid_accelerators() {
    let data = include_bytes!(
        "testdata/deserialize_sparse_crash-d07703ceb94b10dcd9e4acb809f2051420449e2b",
    );
    let _ = fuzz_run(data);
}

// This fuzz input generated a DFA with a state whose EOI transition led to
// a quit state, which is generally considered illegal. Why? Because the EOI
// transition is defined over a special sentinel alphabet element and one
// cannot configure a DFA to "quit" on that sentinel.
#[test]
fn eoi_transition_to_quit_state() {
    let data = include_bytes!(
        "testdata/deserialize_sparse_crash-18cfc246f2ddfc3dfc92b0c7893178c7cf65efa9",
    );
    let _ = fuzz_run(data);
}

// This is the code from the fuzz target. Kind of sucks to duplicate it here,
// but this is fundamentally how we interpret the date.
fn fuzz_run(given_data: &[u8]) -> Option<()> {
    use regex_automata::dfa::Automaton;

    if given_data.len() < 2 {
        return None;
    }
    let haystack_len = usize::from(given_data[0]);
    let haystack = given_data.get(1..1 + haystack_len)?;
    let given_dfa_bytes = given_data.get(1 + haystack_len..)?;

    // We help the fuzzer along by adding a preamble to the bytes that should
    // at least make these first parts valid. The preamble expects a very
    // specific sequence of bytes, so it makes sense to just force this.
    let label = "rust-regex-automata-dfa-sparse\x00\x00";
    assert_eq!(0, label.len() % 4);
    let endianness_check = 0xFEFFu32.to_ne_bytes().to_vec();
    let version_check = 2u32.to_ne_bytes().to_vec();
    let mut dfa_bytes: Vec<u8> = vec![];
    dfa_bytes.extend(label.as_bytes());
    dfa_bytes.extend(&endianness_check);
    dfa_bytes.extend(&version_check);
    dfa_bytes.extend(given_dfa_bytes);
    // This is the real test: checking that any input we give to
    // DFA::from_bytes will never result in a panic.
    let (dfa, _) =
        regex_automata::dfa::sparse::DFA::from_bytes(&dfa_bytes).ok()?;
    let _ = dfa.try_search_fwd(&regex_automata::Input::new(haystack));
    Some(())
}
