use regex_automata::{Input, Match};

mod multi_pattern_v2;

#[test]
fn multi_pattern_v2() {
    use multi_pattern_v2::MULTI_PATTERN_V2 as RE;

    assert_eq!(Some(Match::must(0, 0..4)), RE.find("abcd"));
    assert_eq!(Some(Match::must(0, 2..6)), RE.find("@ abcd @"));
    assert_eq!(Some(Match::must(1, 0..6)), RE.find("@abcd@"));
    assert_eq!(Some(Match::must(0, 1..5)), RE.find("\nabcd\n"));
    assert_eq!(Some(Match::must(0, 1..5)), RE.find("\nabcd wxyz\n"));
    assert_eq!(Some(Match::must(1, 1..7)), RE.find("\n@abcd@\n"));
    assert_eq!(Some(Match::must(2, 0..6)), RE.find("@abcd@\r\n"));
    assert_eq!(Some(Match::must(1, 2..8)), RE.find("\r\n@abcd@"));
    assert_eq!(Some(Match::must(2, 2..8)), RE.find("\r\n@abcd@\r\n"));

    // Fails because we have heuristic support for Unicode word boundaries
    // enabled.
    assert!(RE.try_search(&Input::new(b"\xFF@abcd@\xFF")).is_err());
}
