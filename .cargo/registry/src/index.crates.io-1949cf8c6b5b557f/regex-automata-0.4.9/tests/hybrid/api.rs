use std::error::Error;

use regex_automata::{
    hybrid::dfa::{OverlappingState, DFA},
    nfa::thompson,
    HalfMatch, Input, MatchError,
};

// Tests that too many cache resets cause the lazy DFA to quit.
//
// We only test this on 64-bit because the test is gingerly crafted based on
// implementation details of cache sizes. It's not a great test because of
// that, but it does check some interesting properties around how positions are
// reported when a search "gives up."
//
// NOTE: If you change something in lazy DFA implementation that causes this
// test to fail by reporting different "gave up" positions, then it's generally
// okay to update the positions in the test below as long as you're sure your
// changes are correct. Namely, it is expected that if there are changes in the
// cache size (or changes in how big things are inside the cache), then its
// utilization may change slightly and thus impact where a search gives up.
// Precisely where a search gives up is not an API guarantee, so changing the
// offsets here is OK.
#[test]
#[cfg(target_pointer_width = "64")]
#[cfg(not(miri))]
fn too_many_cache_resets_cause_quit() -> Result<(), Box<dyn Error>> {
    // This is a carefully chosen regex. The idea is to pick one that requires
    // some decent number of states (hence the bounded repetition). But we
    // specifically choose to create a class with an ASCII letter and a
    // non-ASCII letter so that we can check that no new states are created
    // once the cache is full. Namely, if we fill up the cache on a haystack
    // of 'a's, then in order to match one 'β', a new state will need to be
    // created since a 'β' is encoded with multiple bytes.
    //
    // So we proceed by "filling" up the cache by searching a haystack of just
    // 'a's. The cache won't have enough room to add enough states to find the
    // match (because of the bounded repetition), which should result in it
    // giving up before it finds a match.
    //
    // Since there's now no more room to create states, we search a haystack
    // of 'β' and confirm that it gives up immediately.
    let pattern = r"[aβ]{99}";
    let dfa = DFA::builder()
        .configure(
            // Configure it so that we have the minimum cache capacity
            // possible. And that if any resets occur, the search quits.
            DFA::config()
                .skip_cache_capacity_check(true)
                .cache_capacity(0)
                .minimum_cache_clear_count(Some(0)),
        )
        .thompson(thompson::NFA::config())
        .build(pattern)?;
    let mut cache = dfa.create_cache();

    let haystack = "a".repeat(101).into_bytes();
    let err = MatchError::gave_up(24);
    // Notice that we make the same amount of progress in each search! That's
    // because the cache is reused and already has states to handle the first
    // N bytes.
    assert_eq!(
        Err(err.clone()),
        dfa.try_search_fwd(&mut cache, &Input::new(&haystack))
    );
    assert_eq!(
        Err(err.clone()),
        dfa.try_search_overlapping_fwd(
            &mut cache,
            &Input::new(&haystack),
            &mut OverlappingState::start()
        ),
    );

    let haystack = "β".repeat(101).into_bytes();
    let err = MatchError::gave_up(2);
    assert_eq!(
        Err(err),
        dfa.try_search_fwd(&mut cache, &Input::new(&haystack))
    );
    // no need to test that other find routines quit, since we did that above

    // OK, if we reset the cache, then we should be able to create more states
    // and make more progress with searching for betas.
    cache.reset(&dfa);
    let err = MatchError::gave_up(26);
    assert_eq!(
        Err(err),
        dfa.try_search_fwd(&mut cache, &Input::new(&haystack))
    );

    // ... switching back to ASCII still makes progress since it just needs to
    // set transitions on existing states!
    let haystack = "a".repeat(101).into_bytes();
    let err = MatchError::gave_up(13);
    assert_eq!(
        Err(err),
        dfa.try_search_fwd(&mut cache, &Input::new(&haystack))
    );

    Ok(())
}

// Tests that quit bytes in the forward direction work correctly.
#[test]
fn quit_fwd() -> Result<(), Box<dyn Error>> {
    let dfa = DFA::builder()
        .configure(DFA::config().quit(b'x', true))
        .build("[[:word:]]+$")?;
    let mut cache = dfa.create_cache();

    assert_eq!(
        dfa.try_search_fwd(&mut cache, &Input::new("abcxyz")),
        Err(MatchError::quit(b'x', 3)),
    );
    assert_eq!(
        dfa.try_search_overlapping_fwd(
            &mut cache,
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
    let dfa = DFA::builder()
        .configure(DFA::config().quit(b'x', true))
        .thompson(thompson::Config::new().reverse(true))
        .build("^[[:word:]]+")?;
    let mut cache = dfa.create_cache();

    assert_eq!(
        dfa.try_search_rev(&mut cache, &Input::new("abcxyz")),
        Err(MatchError::quit(b'x', 3)),
    );

    Ok(())
}

// Tests that if we heuristically enable Unicode word boundaries but then
// instruct that a non-ASCII byte should NOT be a quit byte, then the builder
// will panic.
#[test]
#[should_panic]
fn quit_panics() {
    DFA::config().unicode_word_boundary(true).quit(b'\xFF', false);
}

// This tests an intesting case where even if the Unicode word boundary option
// is disabled, setting all non-ASCII bytes to be quit bytes will cause Unicode
// word boundaries to be enabled.
#[test]
fn unicode_word_implicitly_works() -> Result<(), Box<dyn Error>> {
    let mut config = DFA::config();
    for b in 0x80..=0xFF {
        config = config.quit(b, true);
    }
    let dfa = DFA::builder().configure(config).build(r"\b")?;
    let mut cache = dfa.create_cache();
    let expected = HalfMatch::must(0, 1);
    assert_eq!(
        Ok(Some(expected)),
        dfa.try_search_fwd(&mut cache, &Input::new(" a")),
    );
    Ok(())
}
