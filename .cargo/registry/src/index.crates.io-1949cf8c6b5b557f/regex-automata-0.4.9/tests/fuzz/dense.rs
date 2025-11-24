// This test was found by a fuzzer input that crafted a way to provide
// an invalid serialization of ByteClasses that passed our verification.
// Specifically, the verification step in the deserialization of ByteClasses
// used an iterator that depends on part of the serialized bytes being correct.
// (Specifically, the encoding of the number of classes.)
#[test]
fn invalid_byte_classes() {
    let data = include_bytes!(
        "testdata/deserialize_dense_crash-9486fb7c8a93b12c12a62166b43d31640c0208a9",
    );
    let _ = fuzz_run(data);
}

#[test]
fn invalid_byte_classes_min() {
    let data = include_bytes!(
        "testdata/deserialize_dense_minimized-from-9486fb7c8a93b12c12a62166b43d31640c0208a9",
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
    let label = "rust-regex-automata-dfa-dense\x00\x00\x00";
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
        regex_automata::dfa::dense::DFA::from_bytes(&dfa_bytes).ok()?;
    let _ = dfa.try_search_fwd(&regex_automata::Input::new(haystack));
    Some(())
}
