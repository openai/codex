use std::collections::HashMap;

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use crate::{
    packed::{Config, MatchKind},
    util::search::Match,
};

/// A description of a single test against a multi-pattern searcher.
///
/// A single test may not necessarily pass on every configuration of a
/// searcher. The tests are categorized and grouped appropriately below.
#[derive(Clone, Debug, Eq, PartialEq)]
struct SearchTest {
    /// The name of this test, for debugging.
    name: &'static str,
    /// The patterns to search for.
    patterns: &'static [&'static str],
    /// The text to search.
    haystack: &'static str,
    /// Each match is a triple of (pattern_index, start, end), where
    /// pattern_index is an index into `patterns` and `start`/`end` are indices
    /// into `haystack`.
    matches: &'static [(usize, usize, usize)],
}

struct SearchTestOwned {
    offset: usize,
    name: String,
    patterns: Vec<String>,
    haystack: String,
    matches: Vec<(usize, usize, usize)>,
}

impl SearchTest {
    fn variations(&self) -> Vec<SearchTestOwned> {
        let count = if cfg!(miri) { 1 } else { 261 };
        let mut tests = vec![];
        for i in 0..count {
            tests.push(self.offset_prefix(i));
            tests.push(self.offset_suffix(i));
            tests.push(self.offset_both(i));
        }
        tests
    }

    fn offset_both(&self, off: usize) -> SearchTestOwned {
        SearchTestOwned {
            offset: off,
            name: self.name.to_string(),
            patterns: self.patterns.iter().map(|s| s.to_string()).collect(),
            haystack: format!(
                "{}{}{}",
                "Z".repeat(off),
                self.haystack,
                "Z".repeat(off)
            ),
            matches: self
                .matches
                .iter()
                .map(|&(id, s, e)| (id, s + off, e + off))
                .collect(),
        }
    }

    fn offset_prefix(&self, off: usize) -> SearchTestOwned {
        SearchTestOwned {
            offset: off,
            name: self.name.to_string(),
            patterns: self.patterns.iter().map(|s| s.to_string()).collect(),
            haystack: format!("{}{}", "Z".repeat(off), self.haystack),
            matches: self
                .matches
                .iter()
                .map(|&(id, s, e)| (id, s + off, e + off))
                .collect(),
        }
    }

    fn offset_suffix(&self, off: usize) -> SearchTestOwned {
        SearchTestOwned {
            offset: off,
            name: self.name.to_string(),
            patterns: self.patterns.iter().map(|s| s.to_string()).collect(),
            haystack: format!("{}{}", self.haystack, "Z".repeat(off)),
            matches: self.matches.to_vec(),
        }
    }
}

/// Short-hand constructor for SearchTest. We use it a lot below.
macro_rules! t {
    ($name:ident, $patterns:expr, $haystack:expr, $matches:expr) => {
        SearchTest {
            name: stringify!($name),
            patterns: $patterns,
            haystack: $haystack,
            matches: $matches,
        }
    };
}

/// A collection of test groups.
type TestCollection = &'static [&'static [SearchTest]];

// Define several collections corresponding to the different type of match
// semantics supported. These collections have some overlap, but each
// collection should have some tests that no other collection has.

/// Tests for leftmost-first match semantics.
const PACKED_LEFTMOST_FIRST: TestCollection =
    &[BASICS, LEFTMOST, LEFTMOST_FIRST, REGRESSION, TEDDY];

/// Tests for leftmost-longest match semantics.
const PACKED_LEFTMOST_LONGEST: TestCollection =
    &[BASICS, LEFTMOST, LEFTMOST_LONGEST, REGRESSION, TEDDY];

// Now define the individual tests that make up the collections above.

/// A collection of tests for the that should always be true regardless of
/// match semantics. That is, all combinations of leftmost-{first, longest}
/// should produce the same answer.
const BASICS: &'static [SearchTest] = &[
    t!(basic001, &["a"], "", &[]),
    t!(basic010, &["a"], "a", &[(0, 0, 1)]),
    t!(basic020, &["a"], "aa", &[(0, 0, 1), (0, 1, 2)]),
    t!(basic030, &["a"], "aaa", &[(0, 0, 1), (0, 1, 2), (0, 2, 3)]),
    t!(basic040, &["a"], "aba", &[(0, 0, 1), (0, 2, 3)]),
    t!(basic050, &["a"], "bba", &[(0, 2, 3)]),
    t!(basic060, &["a"], "bbb", &[]),
    t!(basic070, &["a"], "bababbbba", &[(0, 1, 2), (0, 3, 4), (0, 8, 9)]),
    t!(basic100, &["aa"], "", &[]),
    t!(basic110, &["aa"], "aa", &[(0, 0, 2)]),
    t!(basic120, &["aa"], "aabbaa", &[(0, 0, 2), (0, 4, 6)]),
    t!(basic130, &["aa"], "abbab", &[]),
    t!(basic140, &["aa"], "abbabaa", &[(0, 5, 7)]),
    t!(basic150, &["aaa"], "aaa", &[(0, 0, 3)]),
    t!(basic200, &["abc"], "abc", &[(0, 0, 3)]),
    t!(basic210, &["abc"], "zazabzabcz", &[(0, 6, 9)]),
    t!(basic220, &["abc"], "zazabczabcz", &[(0, 3, 6), (0, 7, 10)]),
    t!(basic230, &["abcd"], "abcd", &[(0, 0, 4)]),
    t!(basic240, &["abcd"], "zazabzabcdz", &[(0, 6, 10)]),
    t!(basic250, &["abcd"], "zazabcdzabcdz", &[(0, 3, 7), (0, 8, 12)]),
    t!(basic300, &["a", "b"], "", &[]),
    t!(basic310, &["a", "b"], "z", &[]),
    t!(basic320, &["a", "b"], "b", &[(1, 0, 1)]),
    t!(basic330, &["a", "b"], "a", &[(0, 0, 1)]),
    t!(
        basic340,
        &["a", "b"],
        "abba",
        &[(0, 0, 1), (1, 1, 2), (1, 2, 3), (0, 3, 4),]
    ),
    t!(
        basic350,
        &["b", "a"],
        "abba",
        &[(1, 0, 1), (0, 1, 2), (0, 2, 3), (1, 3, 4),]
    ),
    t!(basic360, &["abc", "bc"], "xbc", &[(1, 1, 3),]),
    t!(basic400, &["foo", "bar"], "", &[]),
    t!(basic410, &["foo", "bar"], "foobar", &[(0, 0, 3), (1, 3, 6),]),
    t!(basic420, &["foo", "bar"], "barfoo", &[(1, 0, 3), (0, 3, 6),]),
    t!(basic430, &["foo", "bar"], "foofoo", &[(0, 0, 3), (0, 3, 6),]),
    t!(basic440, &["foo", "bar"], "barbar", &[(1, 0, 3), (1, 3, 6),]),
    t!(basic450, &["foo", "bar"], "bafofoo", &[(0, 4, 7),]),
    t!(basic460, &["bar", "foo"], "bafofoo", &[(1, 4, 7),]),
    t!(basic470, &["foo", "bar"], "fobabar", &[(1, 4, 7),]),
    t!(basic480, &["bar", "foo"], "fobabar", &[(0, 4, 7),]),
    t!(basic700, &["yabcdef", "abcdezghi"], "yabcdefghi", &[(0, 0, 7),]),
    t!(basic710, &["yabcdef", "abcdezghi"], "yabcdezghi", &[(1, 1, 10),]),
    t!(
        basic720,
        &["yabcdef", "bcdeyabc", "abcdezghi"],
        "yabcdezghi",
        &[(2, 1, 10),]
    ),
    t!(basic810, &["abcd", "bcd", "cd"], "abcd", &[(0, 0, 4),]),
    t!(basic820, &["bcd", "cd", "abcd"], "abcd", &[(2, 0, 4),]),
    t!(basic830, &["abc", "bc"], "zazabcz", &[(0, 3, 6),]),
    t!(
        basic840,
        &["ab", "ba"],
        "abababa",
        &[(0, 0, 2), (0, 2, 4), (0, 4, 6),]
    ),
    t!(basic850, &["foo", "foo"], "foobarfoo", &[(0, 0, 3), (0, 6, 9),]),
];

/// Tests for leftmost match semantics. These should pass for both
/// leftmost-first and leftmost-longest match kinds. Stated differently, among
/// ambiguous matches, the longest match and the match that appeared first when
/// constructing the automaton should always be the same.
const LEFTMOST: &'static [SearchTest] = &[
    t!(leftmost000, &["ab", "ab"], "abcd", &[(0, 0, 2)]),
    t!(leftmost030, &["a", "ab"], "aa", &[(0, 0, 1), (0, 1, 2)]),
    t!(leftmost031, &["ab", "a"], "aa", &[(1, 0, 1), (1, 1, 2)]),
    t!(leftmost032, &["ab", "a"], "xayabbbz", &[(1, 1, 2), (0, 3, 5)]),
    t!(leftmost300, &["abcd", "bce", "b"], "abce", &[(1, 1, 4)]),
    t!(leftmost310, &["abcd", "ce", "bc"], "abce", &[(2, 1, 3)]),
    t!(leftmost320, &["abcd", "bce", "ce", "b"], "abce", &[(1, 1, 4)]),
    t!(leftmost330, &["abcd", "bce", "cz", "bc"], "abcz", &[(3, 1, 3)]),
    t!(leftmost340, &["bce", "cz", "bc"], "bcz", &[(2, 0, 2)]),
    t!(leftmost350, &["abc", "bd", "ab"], "abd", &[(2, 0, 2)]),
    t!(
        leftmost360,
        &["abcdefghi", "hz", "abcdefgh"],
        "abcdefghz",
        &[(2, 0, 8),]
    ),
    t!(
        leftmost370,
        &["abcdefghi", "cde", "hz", "abcdefgh"],
        "abcdefghz",
        &[(3, 0, 8),]
    ),
    t!(
        leftmost380,
        &["abcdefghi", "hz", "abcdefgh", "a"],
        "abcdefghz",
        &[(2, 0, 8),]
    ),
    t!(
        leftmost390,
        &["b", "abcdefghi", "hz", "abcdefgh"],
        "abcdefghz",
        &[(3, 0, 8),]
    ),
    t!(
        leftmost400,
        &["h", "abcdefghi", "hz", "abcdefgh"],
        "abcdefghz",
        &[(3, 0, 8),]
    ),
    t!(
        leftmost410,
        &["z", "abcdefghi", "hz", "abcdefgh"],
        "abcdefghz",
        &[(3, 0, 8), (0, 8, 9),]
    ),
];

/// Tests for non-overlapping leftmost-first match semantics. These tests
/// should generally be specific to leftmost-first, which means they should
/// generally fail under leftmost-longest semantics.
const LEFTMOST_FIRST: &'static [SearchTest] = &[
    t!(leftfirst000, &["ab", "abcd"], "abcd", &[(0, 0, 2)]),
    t!(leftfirst020, &["abcd", "ab"], "abcd", &[(0, 0, 4)]),
    t!(leftfirst030, &["ab", "ab"], "abcd", &[(0, 0, 2)]),
    t!(leftfirst040, &["a", "ab"], "xayabbbz", &[(0, 1, 2), (0, 3, 4)]),
    t!(leftfirst100, &["abcdefg", "bcde", "bcdef"], "abcdef", &[(1, 1, 5)]),
    t!(leftfirst110, &["abcdefg", "bcdef", "bcde"], "abcdef", &[(1, 1, 6)]),
    t!(leftfirst300, &["abcd", "b", "bce"], "abce", &[(1, 1, 2)]),
    t!(
        leftfirst310,
        &["abcd", "b", "bce", "ce"],
        "abce",
        &[(1, 1, 2), (3, 2, 4),]
    ),
    t!(
        leftfirst320,
        &["a", "abcdefghi", "hz", "abcdefgh"],
        "abcdefghz",
        &[(0, 0, 1), (2, 7, 9),]
    ),
    t!(leftfirst330, &["a", "abab"], "abab", &[(0, 0, 1), (0, 2, 3)]),
    t!(
        leftfirst340,
        &["abcdef", "x", "x", "x", "x", "x", "x", "abcde"],
        "abcdef",
        &[(0, 0, 6)]
    ),
];

/// Tests for non-overlapping leftmost-longest match semantics. These tests
/// should generally be specific to leftmost-longest, which means they should
/// generally fail under leftmost-first semantics.
const LEFTMOST_LONGEST: &'static [SearchTest] = &[
    t!(leftlong000, &["ab", "abcd"], "abcd", &[(1, 0, 4)]),
    t!(leftlong010, &["abcd", "bcd", "cd", "b"], "abcd", &[(0, 0, 4),]),
    t!(leftlong040, &["a", "ab"], "a", &[(0, 0, 1)]),
    t!(leftlong050, &["a", "ab"], "ab", &[(1, 0, 2)]),
    t!(leftlong060, &["ab", "a"], "a", &[(1, 0, 1)]),
    t!(leftlong070, &["ab", "a"], "ab", &[(0, 0, 2)]),
    t!(leftlong100, &["abcdefg", "bcde", "bcdef"], "abcdef", &[(2, 1, 6)]),
    t!(leftlong110, &["abcdefg", "bcdef", "bcde"], "abcdef", &[(1, 1, 6)]),
    t!(leftlong300, &["abcd", "b", "bce"], "abce", &[(2, 1, 4)]),
    t!(
        leftlong310,
        &["a", "abcdefghi", "hz", "abcdefgh"],
        "abcdefghz",
        &[(3, 0, 8),]
    ),
    t!(leftlong320, &["a", "abab"], "abab", &[(1, 0, 4)]),
    t!(leftlong330, &["abcd", "b", "ce"], "abce", &[(1, 1, 2), (2, 2, 4),]),
    t!(leftlong340, &["a", "ab"], "xayabbbz", &[(0, 1, 2), (1, 3, 5)]),
];

/// Regression tests that are applied to all combinations.
///
/// If regression tests are needed for specific match semantics, then add them
/// to the appropriate group above.
const REGRESSION: &'static [SearchTest] = &[
    t!(regression010, &["inf", "ind"], "infind", &[(0, 0, 3), (1, 3, 6),]),
    t!(regression020, &["ind", "inf"], "infind", &[(1, 0, 3), (0, 3, 6),]),
    t!(
        regression030,
        &["libcore/", "libstd/"],
        "libcore/char/methods.rs",
        &[(0, 0, 8),]
    ),
    t!(
        regression040,
        &["libstd/", "libcore/"],
        "libcore/char/methods.rs",
        &[(1, 0, 8),]
    ),
    t!(
        regression050,
        &["\x00\x00\x01", "\x00\x00\x00"],
        "\x00\x00\x00",
        &[(1, 0, 3),]
    ),
    t!(
        regression060,
        &["\x00\x00\x00", "\x00\x00\x01"],
        "\x00\x00\x00",
        &[(0, 0, 3),]
    ),
];

const TEDDY: &'static [SearchTest] = &[
    t!(
        teddy010,
        &["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"],
        "abcdefghijk",
        &[
            (0, 0, 1),
            (1, 1, 2),
            (2, 2, 3),
            (3, 3, 4),
            (4, 4, 5),
            (5, 5, 6),
            (6, 6, 7),
            (7, 7, 8),
            (8, 8, 9),
            (9, 9, 10),
            (10, 10, 11)
        ]
    ),
    t!(
        teddy020,
        &["ab", "bc", "cd", "de", "ef", "fg", "gh", "hi", "ij", "jk", "kl"],
        "abcdefghijk",
        &[(0, 0, 2), (2, 2, 4), (4, 4, 6), (6, 6, 8), (8, 8, 10),]
    ),
    t!(
        teddy030,
        &["abc"],
        "abcdefghijklmnopqrstuvwxyzabcdefghijk",
        &[(0, 0, 3), (0, 26, 29)]
    ),
];

// Now define a test for each combination of things above that we want to run.
// Since there are a few different combinations for each collection of tests,
// we define a couple of macros to avoid repetition drudgery. The testconfig
// macro constructs the automaton from a given match kind, and runs the search
// tests one-by-one over the given collection. The `with` parameter allows one
// to configure the config with additional parameters. The testcombo macro
// invokes testconfig in precisely this way: it sets up several tests where
// each one turns a different knob on Config.

macro_rules! testconfig {
    ($name:ident, $collection:expr, $with:expr) => {
        #[test]
        fn $name() {
            run_search_tests($collection, |test| {
                let mut config = Config::new();
                $with(&mut config);
                let mut builder = config.builder();
                builder.extend(test.patterns.iter().map(|p| p.as_bytes()));
                let searcher = match builder.build() {
                    Some(searcher) => searcher,
                    None => {
                        // For x86-64 and aarch64, not building a searcher is
                        // probably a bug, so be loud.
                        if cfg!(any(
                            target_arch = "x86_64",
                            target_arch = "aarch64"
                        )) {
                            panic!("failed to build packed searcher")
                        }
                        return None;
                    }
                };
                Some(searcher.find_iter(&test.haystack).collect())
            });
        }
    };
}

testconfig!(
    search_default_leftmost_first,
    PACKED_LEFTMOST_FIRST,
    |_: &mut Config| {}
);

testconfig!(
    search_default_leftmost_longest,
    PACKED_LEFTMOST_LONGEST,
    |c: &mut Config| {
        c.match_kind(MatchKind::LeftmostLongest);
    }
);

testconfig!(
    search_teddy_leftmost_first,
    PACKED_LEFTMOST_FIRST,
    |c: &mut Config| {
        c.only_teddy(true);
    }
);

testconfig!(
    search_teddy_leftmost_longest,
    PACKED_LEFTMOST_LONGEST,
    |c: &mut Config| {
        c.only_teddy(true).match_kind(MatchKind::LeftmostLongest);
    }
);

testconfig!(
    search_teddy_ssse3_leftmost_first,
    PACKED_LEFTMOST_FIRST,
    |c: &mut Config| {
        c.only_teddy(true);
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("ssse3") {
            c.only_teddy_256bit(Some(false));
        }
    }
);

testconfig!(
    search_teddy_ssse3_leftmost_longest,
    PACKED_LEFTMOST_LONGEST,
    |c: &mut Config| {
        c.only_teddy(true).match_kind(MatchKind::LeftmostLongest);
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("ssse3") {
            c.only_teddy_256bit(Some(false));
        }
    }
);

testconfig!(
    search_teddy_avx2_leftmost_first,
    PACKED_LEFTMOST_FIRST,
    |c: &mut Config| {
        c.only_teddy(true);
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("avx2") {
            c.only_teddy_256bit(Some(true));
        }
    }
);

testconfig!(
    search_teddy_avx2_leftmost_longest,
    PACKED_LEFTMOST_LONGEST,
    |c: &mut Config| {
        c.only_teddy(true).match_kind(MatchKind::LeftmostLongest);
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("avx2") {
            c.only_teddy_256bit(Some(true));
        }
    }
);

testconfig!(
    search_teddy_fat_leftmost_first,
    PACKED_LEFTMOST_FIRST,
    |c: &mut Config| {
        c.only_teddy(true);
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("avx2") {
            c.only_teddy_fat(Some(true));
        }
    }
);

testconfig!(
    search_teddy_fat_leftmost_longest,
    PACKED_LEFTMOST_LONGEST,
    |c: &mut Config| {
        c.only_teddy(true).match_kind(MatchKind::LeftmostLongest);
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("avx2") {
            c.only_teddy_fat(Some(true));
        }
    }
);

testconfig!(
    search_rabinkarp_leftmost_first,
    PACKED_LEFTMOST_FIRST,
    |c: &mut Config| {
        c.only_rabin_karp(true);
    }
);

testconfig!(
    search_rabinkarp_leftmost_longest,
    PACKED_LEFTMOST_LONGEST,
    |c: &mut Config| {
        c.only_rabin_karp(true).match_kind(MatchKind::LeftmostLongest);
    }
);

#[test]
fn search_tests_have_unique_names() {
    let assert = |constname, tests: &[SearchTest]| {
        let mut seen = HashMap::new(); // map from test name to position
        for (i, test) in tests.iter().enumerate() {
            if !seen.contains_key(test.name) {
                seen.insert(test.name, i);
            } else {
                let last = seen[test.name];
                panic!(
                    "{} tests have duplicate names at positions {} and {}",
                    constname, last, i
                );
            }
        }
    };
    assert("BASICS", BASICS);
    assert("LEFTMOST", LEFTMOST);
    assert("LEFTMOST_FIRST", LEFTMOST_FIRST);
    assert("LEFTMOST_LONGEST", LEFTMOST_LONGEST);
    assert("REGRESSION", REGRESSION);
    assert("TEDDY", TEDDY);
}

fn run_search_tests<F: FnMut(&SearchTestOwned) -> Option<Vec<Match>>>(
    which: TestCollection,
    mut f: F,
) {
    let get_match_triples =
        |matches: Vec<Match>| -> Vec<(usize, usize, usize)> {
            matches
                .into_iter()
                .map(|m| (m.pattern().as_usize(), m.start(), m.end()))
                .collect()
        };
    for &tests in which {
        for spec in tests {
            for test in spec.variations() {
                let results = match f(&test) {
                    None => continue,
                    Some(results) => results,
                };
                assert_eq!(
                    test.matches,
                    get_match_triples(results).as_slice(),
                    "test: {}, patterns: {:?}, haystack(len={:?}): {:?}, \
                     offset: {:?}",
                    test.name,
                    test.patterns,
                    test.haystack.len(),
                    test.haystack,
                    test.offset,
                );
            }
        }
    }
}
