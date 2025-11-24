// A regression test for checking that minimization correctly translates
// whether a state is a match state or not. Previously, it was possible for
// minimization to mark a non-matching state as matching.
#[test]
#[cfg(not(miri))]
fn minimize_sets_correct_match_states() {
    use regex_automata::{
        dfa::{dense::DFA, Automaton, StartKind},
        Anchored, Input,
    };

    let pattern =
        // This is a subset of the grapheme matching regex. I couldn't seem
        // to get a repro any smaller than this unfortunately.
        r"(?x)
            (?:
                \p{gcb=Prepend}*
                (?:
                    (?:
                        (?:
                            \p{gcb=L}*
                            (?:\p{gcb=V}+|\p{gcb=LV}\p{gcb=V}*|\p{gcb=LVT})
                            \p{gcb=T}*
                        )
                        |
                        \p{gcb=L}+
                        |
                        \p{gcb=T}+
                    )
                    |
                    \p{Extended_Pictographic}
                    (?:\p{gcb=Extend}*\p{gcb=ZWJ}\p{Extended_Pictographic})*
                    |
                    [^\p{gcb=Control}\p{gcb=CR}\p{gcb=LF}]
                )
                [\p{gcb=Extend}\p{gcb=ZWJ}\p{gcb=SpacingMark}]*
            )
        ";

    let dfa = DFA::builder()
        .configure(
            DFA::config().start_kind(StartKind::Anchored).minimize(true),
        )
        .build(pattern)
        .unwrap();
    let input = Input::new(b"\xE2").anchored(Anchored::Yes);
    assert_eq!(Ok(None), dfa.try_search_fwd(&input));
}
