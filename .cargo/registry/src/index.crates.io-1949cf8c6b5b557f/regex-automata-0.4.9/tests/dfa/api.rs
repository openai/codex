use std::error::Error;

use regex_automata::{
    dfa::{dense, Automaton, OverlappingState},
    nfa::thompson,
    HalfMatch, Input, MatchError,
};

// Tests that quit bytes in the forward direction work correctly.
#[test]
fn quit_fwd() -> Result<(), Box<dyn Error>> {
    let dfa = dense::Builder::new()
        .configure(dense::Config::new().quit(b'x', true))
        .build("[[:word:]]+$")?;

    assert_eq!(
        Err(MatchError::quit(b'x', 3)),
        dfa.try_search_fwd(&Input::new(b"abcxyz"))
    );
    assert_eq!(
        dfa.try_search_overlapping_fwd(
            &Input::new(b"abcxyz"),
            &mut OverlappingState::start()
        ),
        Err(MatchError::quit(b'x', 3)),
    );

    Ok(())
}

// Tests that quit bytes in the reverse direction work correctly.
#[test]
fn quit_rev() -> Result<(), Box<dyn Error>> {
    let dfa = dense::Builder::new()
        .configure(dense::Config::new().quit(b'x', true))
        .thompson(thompson::Config::new().reverse(true))
        .build("^[[:word:]]+")?;

    assert_eq!(
        Err(MatchError::quit(b'x', 3)),
        dfa.try_search_rev(&Input::new(b"abcxyz"))
    );

    Ok(())
}

// Tests that if we heuristically enable Unicode word boundaries but then
// instruct that a non-ASCII byte should NOT be a quit byte, then the builder
// will panic.
#[test]
#[should_panic]
fn quit_panics() {
    dense::Config::new().unicode_word_boundary(true).quit(b'\xFF', false);
}

// This tests an intesting case where even if the Unicode word boundary option
// is disabled, setting all non-ASCII bytes to be quit bytes will cause Unicode
// word boundaries to be enabled.
#[test]
fn unicode_word_implicitly_works() -> Result<(), Box<dyn Error>> {
    let mut config = dense::Config::new();
    for b in 0x80..=0xFF {
        config = config.quit(b, true);
    }
    let dfa = dense::Builder::new().configure(config).build(r"\b")?;
    let expected = HalfMatch::must(0, 1);
    assert_eq!(Ok(Some(expected)), dfa.try_search_fwd(&Input::new(b" a")));
    Ok(())
}
