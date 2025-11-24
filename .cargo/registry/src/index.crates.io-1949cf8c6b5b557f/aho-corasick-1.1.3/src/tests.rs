use std::{collections::HashMap, format, string::String, vec::Vec};

use crate::{
    AhoCorasick, AhoCorasickBuilder, AhoCorasickKind, Anchored, Input, Match,
    MatchKind, StartKind,
};

/// A description of a single test against an Aho-Corasick automaton.
///
/// A single test may not necessarily pass on every configuration of an
/// Aho-Corasick automaton. The tests are categorized and grouped appropriately
/// below.
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
// semantics supported by Aho-Corasick. These collections have some overlap,
// but each collection should have some tests that no other collection has.

/// Tests for Aho-Corasick's standard non-overlapping match semantics.
const AC_STANDARD_NON_OVERLAPPING: TestCollection =
    &[BASICS, NON_OVERLAPPING, STANDARD, REGRESSION];

/// Tests for Aho-Corasick's anchored standard non-overlapping match semantics.
const AC_STANDARD_ANCHORED_NON_OVERLAPPING: TestCollection =
    &[ANCHORED_BASICS, ANCHORED_NON_OVERLAPPING, STANDARD_ANCHORED];

/// Tests for Aho-Corasick's standard overlapping match semantics.
const AC_STANDARD_OVERLAPPING: TestCollection =
    &[BASICS, OVERLAPPING, REGRESSION];

/*
Iterators of anchored overlapping searches were removed from the API in
after 0.7, but we leave the tests commented out for posterity.
/// Tests for Aho-Corasick's anchored standard overlapping match semantics.
const AC_STANDARD_ANCHORED_OVERLAPPING: TestCollection =
    &[ANCHORED_BASICS, ANCHORED_OVERLAPPING];
*/

/// Tests for Aho-Corasick's leftmost-first match semantics.
const AC_LEFTMOST_FIRST: TestCollection =
    &[BASICS, NON_OVERLAPPING, LEFTMOST, LEFTMOST_FIRST, REGRESSION];

/// Tests for Aho-Corasick's anchored leftmost-first match semantics.
const AC_LEFTMOST_FIRST_ANCHORED: TestCollection = &[
    ANCHORED_BASICS,
    ANCHORED_NON_OVERLAPPING,
    ANCHORED_LEFTMOST,
    ANCHORED_LEFTMOST_FIRST,
];

/// Tests for Aho-Corasick's leftmost-longest match semantics.
const AC_LEFTMOST_LONGEST: TestCollection =
    &[BASICS, NON_OVERLAPPING, LEFTMOST, LEFTMOST_LONGEST, REGRESSION];

/// Tests for Aho-Corasick's anchored leftmost-longest match semantics.
const AC_LEFTMOST_LONGEST_ANCHORED: TestCollection = &[
    ANCHORED_BASICS,
    ANCHORED_NON_OVERLAPPING,
    ANCHORED_LEFTMOST,
    ANCHORED_LEFTMOST_LONGEST,
];

// Now define the individual tests that make up the collections above.

/// A collection of tests for the Aho-Corasick algorithm that should always be
/// true regardless of match semantics. That is, all combinations of
/// leftmost-{shortest, first, longest} x {overlapping, non-overlapping}
/// should produce the same answer.
const BASICS: &'static [SearchTest] = &[
    t!(basic000, &[], "", &[]),
    t!(basic001, &[""], "a", &[(0, 0, 0), (0, 1, 1)]),
    t!(basic002, &["a"], "", &[]),
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
    t!(basic200, &["abc"], "abc", &[(0, 0, 3)]),
    t!(basic210, &["abc"], "zazabzabcz", &[(0, 6, 9)]),
    t!(basic220, &["abc"], "zazabczabcz", &[(0, 3, 6), (0, 7, 10)]),
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
    t!(basic600, &[""], "", &[(0, 0, 0)]),
    t!(basic610, &[""], "a", &[(0, 0, 0), (0, 1, 1)]),
    t!(basic620, &[""], "abc", &[(0, 0, 0), (0, 1, 1), (0, 2, 2), (0, 3, 3)]),
    t!(basic700, &["yabcdef", "abcdezghi"], "yabcdefghi", &[(0, 0, 7),]),
    t!(basic710, &["yabcdef", "abcdezghi"], "yabcdezghi", &[(1, 1, 10),]),
    t!(
        basic720,
        &["yabcdef", "bcdeyabc", "abcdezghi"],
        "yabcdezghi",
        &[(2, 1, 10),]
    ),
];

/// A collection of *anchored* tests for the Aho-Corasick algorithm that should
/// always be true regardless of match semantics. That is, all combinations of
/// leftmost-{shortest, first, longest} x {overlapping, non-overlapping} should
/// produce the same answer.
const ANCHORED_BASICS: &'static [SearchTest] = &[
    t!(abasic000, &[], "", &[]),
    t!(abasic001, &[], "a", &[]),
    t!(abasic002, &[], "abc", &[]),
    t!(abasic010, &[""], "", &[(0, 0, 0)]),
    t!(abasic020, &[""], "a", &[(0, 0, 0), (0, 1, 1)]),
    t!(abasic030, &[""], "abc", &[(0, 0, 0), (0, 1, 1), (0, 2, 2), (0, 3, 3)]),
    t!(abasic100, &["a"], "a", &[(0, 0, 1)]),
    t!(abasic110, &["a"], "aa", &[(0, 0, 1), (0, 1, 2)]),
    t!(abasic120, &["a", "b"], "ab", &[(0, 0, 1), (1, 1, 2)]),
    t!(abasic130, &["a", "b"], "ba", &[(1, 0, 1), (0, 1, 2)]),
    t!(abasic140, &["foo", "foofoo"], "foo", &[(0, 0, 3)]),
    t!(abasic150, &["foofoo", "foo"], "foo", &[(1, 0, 3)]),
    t!(abasic200, &["foo"], "foofoo foo", &[(0, 0, 3), (0, 3, 6)]),
];

/// Tests for non-overlapping standard match semantics.
///
/// These tests generally shouldn't pass for leftmost-{first,longest}, although
/// some do in order to write clearer tests. For example, standard000 will
/// pass with leftmost-first semantics, but standard010 will not. We write
/// both to emphasize how the match semantics work.
const STANDARD: &'static [SearchTest] = &[
    t!(standard000, &["ab", "abcd"], "abcd", &[(0, 0, 2)]),
    t!(standard010, &["abcd", "ab"], "abcd", &[(1, 0, 2)]),
    t!(standard020, &["abcd", "ab", "abc"], "abcd", &[(1, 0, 2)]),
    t!(standard030, &["abcd", "abc", "ab"], "abcd", &[(2, 0, 2)]),
    t!(standard040, &["a", ""], "a", &[(1, 0, 0), (1, 1, 1)]),
    t!(
        standard400,
        &["abcd", "bcd", "cd", "b"],
        "abcd",
        &[(3, 1, 2), (2, 2, 4),]
    ),
    t!(standard410, &["", "a"], "a", &[(0, 0, 0), (0, 1, 1),]),
    t!(standard420, &["", "a"], "aa", &[(0, 0, 0), (0, 1, 1), (0, 2, 2),]),
    t!(standard430, &["", "a", ""], "a", &[(0, 0, 0), (0, 1, 1),]),
    t!(standard440, &["a", "", ""], "a", &[(1, 0, 0), (1, 1, 1),]),
    t!(standard450, &["", "", "a"], "a", &[(0, 0, 0), (0, 1, 1),]),
];

/// Like STANDARD, but for anchored searches.
const STANDARD_ANCHORED: &'static [SearchTest] = &[
    t!(astandard000, &["ab", "abcd"], "abcd", &[(0, 0, 2)]),
    t!(astandard010, &["abcd", "ab"], "abcd", &[(1, 0, 2)]),
    t!(astandard020, &["abcd", "ab", "abc"], "abcd", &[(1, 0, 2)]),
    t!(astandard030, &["abcd", "abc", "ab"], "abcd", &[(2, 0, 2)]),
    t!(astandard040, &["a", ""], "a", &[(1, 0, 0), (1, 1, 1)]),
    t!(astandard050, &["abcd", "bcd", "cd", "b"], "abcd", &[(0, 0, 4)]),
    t!(astandard410, &["", "a"], "a", &[(0, 0, 0), (0, 1, 1)]),
    t!(astandard420, &["", "a"], "aa", &[(0, 0, 0), (0, 1, 1), (0, 2, 2)]),
    t!(astandard430, &["", "a", ""], "a", &[(0, 0, 0), (0, 1, 1)]),
    t!(astandard440, &["a", "", ""], "a", &[(1, 0, 0), (1, 1, 1)]),
    t!(astandard450, &["", "", "a"], "a", &[(0, 0, 0), (0, 1, 1)]),
];

/// Tests for non-overlapping leftmost match semantics. These should pass for
/// both leftmost-first and leftmost-longest match kinds. Stated differently,
/// among ambiguous matches, the longest match and the match that appeared
/// first when constructing the automaton should always be the same.
const LEFTMOST: &'static [SearchTest] = &[
    t!(leftmost000, &["ab", "ab"], "abcd", &[(0, 0, 2)]),
    t!(leftmost010, &["a", ""], "a", &[(0, 0, 1)]),
    t!(leftmost011, &["a", ""], "ab", &[(0, 0, 1), (1, 2, 2)]),
    t!(leftmost020, &["", ""], "a", &[(0, 0, 0), (0, 1, 1)]),
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

/// Like LEFTMOST, but for anchored searches.
const ANCHORED_LEFTMOST: &'static [SearchTest] = &[
    t!(aleftmost000, &["ab", "ab"], "abcd", &[(0, 0, 2)]),
    // We shouldn't allow an empty match immediately following a match, right?
    t!(aleftmost010, &["a", ""], "a", &[(0, 0, 1)]),
    t!(aleftmost020, &["", ""], "a", &[(0, 0, 0), (0, 1, 1)]),
    t!(aleftmost030, &["a", "ab"], "aa", &[(0, 0, 1), (0, 1, 2)]),
    t!(aleftmost031, &["ab", "a"], "aa", &[(1, 0, 1), (1, 1, 2)]),
    t!(aleftmost032, &["ab", "a"], "xayabbbz", &[]),
    t!(aleftmost300, &["abcd", "bce", "b"], "abce", &[]),
    t!(aleftmost301, &["abcd", "bcd", "cd", "b"], "abcd", &[(0, 0, 4)]),
    t!(aleftmost310, &["abcd", "ce", "bc"], "abce", &[]),
    t!(aleftmost320, &["abcd", "bce", "ce", "b"], "abce", &[]),
    t!(aleftmost330, &["abcd", "bce", "cz", "bc"], "abcz", &[]),
    t!(aleftmost340, &["bce", "cz", "bc"], "bcz", &[(2, 0, 2)]),
    t!(aleftmost350, &["abc", "bd", "ab"], "abd", &[(2, 0, 2)]),
    t!(
        aleftmost360,
        &["abcdefghi", "hz", "abcdefgh"],
        "abcdefghz",
        &[(2, 0, 8),]
    ),
    t!(
        aleftmost370,
        &["abcdefghi", "cde", "hz", "abcdefgh"],
        "abcdefghz",
        &[(3, 0, 8),]
    ),
    t!(
        aleftmost380,
        &["abcdefghi", "hz", "abcdefgh", "a"],
        "abcdefghz",
        &[(2, 0, 8),]
    ),
    t!(
        aleftmost390,
        &["b", "abcdefghi", "hz", "abcdefgh"],
        "abcdefghz",
        &[(3, 0, 8),]
    ),
    t!(
        aleftmost400,
        &["h", "abcdefghi", "hz", "abcdefgh"],
        "abcdefghz",
        &[(3, 0, 8),]
    ),
    t!(
        aleftmost410,
        &["z", "abcdefghi", "hz", "abcdefgh"],
        "abcdefghzyz",
        &[(3, 0, 8), (0, 8, 9)]
    ),
];

/// Tests for non-overlapping leftmost-first match semantics. These tests
/// should generally be specific to leftmost-first, which means they should
/// generally fail under leftmost-longest semantics.
const LEFTMOST_FIRST: &'static [SearchTest] = &[
    t!(leftfirst000, &["ab", "abcd"], "abcd", &[(0, 0, 2)]),
    t!(leftfirst010, &["", "a"], "a", &[(0, 0, 0), (0, 1, 1)]),
    t!(leftfirst011, &["", "a", ""], "a", &[(0, 0, 0), (0, 1, 1),]),
    t!(leftfirst012, &["a", "", ""], "a", &[(0, 0, 1)]),
    t!(leftfirst013, &["", "", "a"], "a", &[(0, 0, 0), (0, 1, 1)]),
    t!(leftfirst014, &["a", ""], "a", &[(0, 0, 1)]),
    t!(leftfirst015, &["a", ""], "ab", &[(0, 0, 1), (1, 2, 2)]),
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
    t!(leftfirst400, &["amwix", "samwise", "sam"], "Zsamwix", &[(2, 1, 4)]),
];

/// Like LEFTMOST_FIRST, but for anchored searches.
const ANCHORED_LEFTMOST_FIRST: &'static [SearchTest] = &[
    t!(aleftfirst000, &["ab", "abcd"], "abcd", &[(0, 0, 2)]),
    t!(aleftfirst010, &["", "a"], "a", &[(0, 0, 0), (0, 1, 1)]),
    t!(aleftfirst011, &["", "a", ""], "a", &[(0, 0, 0), (0, 1, 1)]),
    t!(aleftfirst012, &["a", "", ""], "a", &[(0, 0, 1)]),
    t!(aleftfirst013, &["", "", "a"], "a", &[(0, 0, 0), (0, 1, 1)]),
    t!(aleftfirst020, &["abcd", "ab"], "abcd", &[(0, 0, 4)]),
    t!(aleftfirst030, &["ab", "ab"], "abcd", &[(0, 0, 2)]),
    t!(aleftfirst040, &["a", "ab"], "xayabbbz", &[]),
    t!(aleftfirst100, &["abcdefg", "bcde", "bcdef"], "abcdef", &[]),
    t!(aleftfirst110, &["abcdefg", "bcdef", "bcde"], "abcdef", &[]),
    t!(aleftfirst300, &["abcd", "b", "bce"], "abce", &[]),
    t!(aleftfirst310, &["abcd", "b", "bce", "ce"], "abce", &[]),
    t!(
        aleftfirst320,
        &["a", "abcdefghi", "hz", "abcdefgh"],
        "abcdefghz",
        &[(0, 0, 1)]
    ),
    t!(aleftfirst330, &["a", "abab"], "abab", &[(0, 0, 1)]),
    t!(aleftfirst400, &["wise", "samwise", "sam"], "samwix", &[(2, 0, 3)]),
];

/// Tests for non-overlapping leftmost-longest match semantics. These tests
/// should generally be specific to leftmost-longest, which means they should
/// generally fail under leftmost-first semantics.
const LEFTMOST_LONGEST: &'static [SearchTest] = &[
    t!(leftlong000, &["ab", "abcd"], "abcd", &[(1, 0, 4)]),
    t!(leftlong010, &["abcd", "bcd", "cd", "b"], "abcd", &[(0, 0, 4),]),
    t!(leftlong020, &["", "a"], "a", &[(1, 0, 1)]),
    t!(leftlong021, &["", "a", ""], "a", &[(1, 0, 1)]),
    t!(leftlong022, &["a", "", ""], "a", &[(0, 0, 1)]),
    t!(leftlong023, &["", "", "a"], "a", &[(2, 0, 1)]),
    t!(leftlong024, &["", "a"], "ab", &[(1, 0, 1), (0, 2, 2)]),
    t!(leftlong030, &["", "a"], "aa", &[(1, 0, 1), (1, 1, 2)]),
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

/// Like LEFTMOST_LONGEST, but for anchored searches.
const ANCHORED_LEFTMOST_LONGEST: &'static [SearchTest] = &[
    t!(aleftlong000, &["ab", "abcd"], "abcd", &[(1, 0, 4)]),
    t!(aleftlong010, &["abcd", "bcd", "cd", "b"], "abcd", &[(0, 0, 4),]),
    t!(aleftlong020, &["", "a"], "a", &[(1, 0, 1)]),
    t!(aleftlong021, &["", "a", ""], "a", &[(1, 0, 1)]),
    t!(aleftlong022, &["a", "", ""], "a", &[(0, 0, 1)]),
    t!(aleftlong023, &["", "", "a"], "a", &[(2, 0, 1)]),
    t!(aleftlong030, &["", "a"], "aa", &[(1, 0, 1), (1, 1, 2)]),
    t!(aleftlong040, &["a", "ab"], "a", &[(0, 0, 1)]),
    t!(aleftlong050, &["a", "ab"], "ab", &[(1, 0, 2)]),
    t!(aleftlong060, &["ab", "a"], "a", &[(1, 0, 1)]),
    t!(aleftlong070, &["ab", "a"], "ab", &[(0, 0, 2)]),
    t!(aleftlong100, &["abcdefg", "bcde", "bcdef"], "abcdef", &[]),
    t!(aleftlong110, &["abcdefg", "bcdef", "bcde"], "abcdef", &[]),
    t!(aleftlong300, &["abcd", "b", "bce"], "abce", &[]),
    t!(
        aleftlong310,
        &["a", "abcdefghi", "hz", "abcdefgh"],
        "abcdefghz",
        &[(3, 0, 8),]
    ),
    t!(aleftlong320, &["a", "abab"], "abab", &[(1, 0, 4)]),
    t!(aleftlong330, &["abcd", "b", "ce"], "abce", &[]),
    t!(aleftlong340, &["a", "ab"], "xayabbbz", &[]),
];

/// Tests for non-overlapping match semantics.
///
/// Generally these tests shouldn't pass when using overlapping semantics.
/// These should pass for both standard and leftmost match semantics.
const NON_OVERLAPPING: &'static [SearchTest] = &[
    t!(nover010, &["abcd", "bcd", "cd"], "abcd", &[(0, 0, 4),]),
    t!(nover020, &["bcd", "cd", "abcd"], "abcd", &[(2, 0, 4),]),
    t!(nover030, &["abc", "bc"], "zazabcz", &[(0, 3, 6),]),
    t!(
        nover100,
        &["ab", "ba"],
        "abababa",
        &[(0, 0, 2), (0, 2, 4), (0, 4, 6),]
    ),
    t!(nover200, &["foo", "foo"], "foobarfoo", &[(0, 0, 3), (0, 6, 9),]),
    t!(nover300, &["", ""], "", &[(0, 0, 0),]),
    t!(nover310, &["", ""], "a", &[(0, 0, 0), (0, 1, 1),]),
];

/// Like NON_OVERLAPPING, but for anchored searches.
const ANCHORED_NON_OVERLAPPING: &'static [SearchTest] = &[
    t!(anover010, &["abcd", "bcd", "cd"], "abcd", &[(0, 0, 4),]),
    t!(anover020, &["bcd", "cd", "abcd"], "abcd", &[(2, 0, 4),]),
    t!(anover030, &["abc", "bc"], "zazabcz", &[]),
    t!(
        anover100,
        &["ab", "ba"],
        "abababa",
        &[(0, 0, 2), (0, 2, 4), (0, 4, 6)]
    ),
    t!(anover200, &["foo", "foo"], "foobarfoo", &[(0, 0, 3)]),
    t!(anover300, &["", ""], "", &[(0, 0, 0)]),
    t!(anover310, &["", ""], "a", &[(0, 0, 0), (0, 1, 1)]),
];

/// Tests for overlapping match semantics.
///
/// This only supports standard match semantics, since leftmost-{first,longest}
/// do not support overlapping matches.
const OVERLAPPING: &'static [SearchTest] = &[
    t!(
        over000,
        &["abcd", "bcd", "cd", "b"],
        "abcd",
        &[(3, 1, 2), (0, 0, 4), (1, 1, 4), (2, 2, 4),]
    ),
    t!(
        over010,
        &["bcd", "cd", "b", "abcd"],
        "abcd",
        &[(2, 1, 2), (3, 0, 4), (0, 1, 4), (1, 2, 4),]
    ),
    t!(
        over020,
        &["abcd", "bcd", "cd"],
        "abcd",
        &[(0, 0, 4), (1, 1, 4), (2, 2, 4),]
    ),
    t!(
        over030,
        &["bcd", "abcd", "cd"],
        "abcd",
        &[(1, 0, 4), (0, 1, 4), (2, 2, 4),]
    ),
    t!(
        over040,
        &["bcd", "cd", "abcd"],
        "abcd",
        &[(2, 0, 4), (0, 1, 4), (1, 2, 4),]
    ),
    t!(over050, &["abc", "bc"], "zazabcz", &[(0, 3, 6), (1, 4, 6),]),
    t!(
        over100,
        &["ab", "ba"],
        "abababa",
        &[(0, 0, 2), (1, 1, 3), (0, 2, 4), (1, 3, 5), (0, 4, 6), (1, 5, 7),]
    ),
    t!(
        over200,
        &["foo", "foo"],
        "foobarfoo",
        &[(0, 0, 3), (1, 0, 3), (0, 6, 9), (1, 6, 9),]
    ),
    t!(over300, &["", ""], "", &[(0, 0, 0), (1, 0, 0),]),
    t!(
        over310,
        &["", ""],
        "a",
        &[(0, 0, 0), (1, 0, 0), (0, 1, 1), (1, 1, 1),]
    ),
    t!(over320, &["", "a"], "a", &[(0, 0, 0), (1, 0, 1), (0, 1, 1),]),
    t!(
        over330,
        &["", "a", ""],
        "a",
        &[(0, 0, 0), (2, 0, 0), (1, 0, 1), (0, 1, 1), (2, 1, 1),]
    ),
    t!(
        over340,
        &["a", "", ""],
        "a",
        &[(1, 0, 0), (2, 0, 0), (0, 0, 1), (1, 1, 1), (2, 1, 1),]
    ),
    t!(
        over350,
        &["", "", "a"],
        "a",
        &[(0, 0, 0), (1, 0, 0), (2, 0, 1), (0, 1, 1), (1, 1, 1),]
    ),
    t!(
        over360,
        &["foo", "foofoo"],
        "foofoo",
        &[(0, 0, 3), (1, 0, 6), (0, 3, 6)]
    ),
];

/*
Iterators of anchored overlapping searches were removed from the API in
after 0.7, but we leave the tests commented out for posterity.
/// Like OVERLAPPING, but for anchored searches.
const ANCHORED_OVERLAPPING: &'static [SearchTest] = &[
    t!(aover000, &["abcd", "bcd", "cd", "b"], "abcd", &[(0, 0, 4)]),
    t!(aover010, &["bcd", "cd", "b", "abcd"], "abcd", &[(3, 0, 4)]),
    t!(aover020, &["abcd", "bcd", "cd"], "abcd", &[(0, 0, 4)]),
    t!(aover030, &["bcd", "abcd", "cd"], "abcd", &[(1, 0, 4)]),
    t!(aover040, &["bcd", "cd", "abcd"], "abcd", &[(2, 0, 4)]),
    t!(aover050, &["abc", "bc"], "zazabcz", &[]),
    t!(aover100, &["ab", "ba"], "abababa", &[(0, 0, 2)]),
    t!(aover200, &["foo", "foo"], "foobarfoo", &[(0, 0, 3), (1, 0, 3)]),
    t!(aover300, &["", ""], "", &[(0, 0, 0), (1, 0, 0),]),
    t!(aover310, &["", ""], "a", &[(0, 0, 0), (1, 0, 0)]),
    t!(aover320, &["", "a"], "a", &[(0, 0, 0), (1, 0, 1)]),
    t!(aover330, &["", "a", ""], "a", &[(0, 0, 0), (2, 0, 0), (1, 0, 1)]),
    t!(aover340, &["a", "", ""], "a", &[(1, 0, 0), (2, 0, 0), (0, 0, 1)]),
    t!(aover350, &["", "", "a"], "a", &[(0, 0, 0), (1, 0, 0), (2, 0, 1)]),
    t!(aover360, &["foo", "foofoo"], "foofoo", &[(0, 0, 3), (1, 0, 6)]),
];
*/

/// Tests for ASCII case insensitivity.
///
/// These tests should all have the same behavior regardless of match semantics
/// or whether the search is overlapping.
const ASCII_CASE_INSENSITIVE: &'static [SearchTest] = &[
    t!(acasei000, &["a"], "A", &[(0, 0, 1)]),
    t!(acasei010, &["Samwise"], "SAMWISE", &[(0, 0, 7)]),
    t!(acasei011, &["Samwise"], "SAMWISE.abcd", &[(0, 0, 7)]),
    t!(acasei020, &["fOoBaR"], "quux foobar baz", &[(0, 5, 11)]),
];

/// Like ASCII_CASE_INSENSITIVE, but specifically for non-overlapping tests.
const ASCII_CASE_INSENSITIVE_NON_OVERLAPPING: &'static [SearchTest] = &[
    t!(acasei000, &["foo", "FOO"], "fOo", &[(0, 0, 3)]),
    t!(acasei000, &["FOO", "foo"], "fOo", &[(0, 0, 3)]),
    t!(acasei010, &["abc", "def"], "abcdef", &[(0, 0, 3), (1, 3, 6)]),
];

/// Like ASCII_CASE_INSENSITIVE, but specifically for overlapping tests.
const ASCII_CASE_INSENSITIVE_OVERLAPPING: &'static [SearchTest] = &[
    t!(acasei000, &["foo", "FOO"], "fOo", &[(0, 0, 3), (1, 0, 3)]),
    t!(acasei001, &["FOO", "foo"], "fOo", &[(0, 0, 3), (1, 0, 3)]),
    // This is a regression test from:
    // https://github.com/BurntSushi/aho-corasick/issues/68
    // Previously, it was reporting a duplicate (1, 3, 6) match.
    t!(
        acasei010,
        &["abc", "def", "abcdef"],
        "abcdef",
        &[(0, 0, 3), (2, 0, 6), (1, 3, 6)]
    ),
];

/// Regression tests that are applied to all Aho-Corasick combinations.
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

// Now define a test for each combination of things above that we want to run.
// Since there are a few different combinations for each collection of tests,
// we define a couple of macros to avoid repetition drudgery. The testconfig
// macro constructs the automaton from a given match kind, and runs the search
// tests one-by-one over the given collection. The `with` parameter allows one
// to configure the builder with additional parameters. The testcombo macro
// invokes testconfig in precisely this way: it sets up several tests where
// each one turns a different knob on AhoCorasickBuilder.

macro_rules! testconfig {
    (anchored, $name:ident, $collection:expr, $kind:ident, $with:expr) => {
        #[test]
        fn $name() {
            run_search_tests($collection, |test| {
                let mut builder = AhoCorasick::builder();
                $with(&mut builder);
                let input = Input::new(test.haystack).anchored(Anchored::Yes);
                builder
                    .match_kind(MatchKind::$kind)
                    .build(test.patterns)
                    .unwrap()
                    .try_find_iter(input)
                    .unwrap()
                    .collect()
            });
        }
    };
    (overlapping, $name:ident, $collection:expr, $kind:ident, $with:expr) => {
        #[test]
        fn $name() {
            run_search_tests($collection, |test| {
                let mut builder = AhoCorasick::builder();
                $with(&mut builder);
                builder
                    .match_kind(MatchKind::$kind)
                    .build(test.patterns)
                    .unwrap()
                    .find_overlapping_iter(test.haystack)
                    .collect()
            });
        }
    };
    (stream, $name:ident, $collection:expr, $kind:ident, $with:expr) => {
        #[test]
        fn $name() {
            run_stream_search_tests($collection, |test| {
                let buf = std::io::BufReader::with_capacity(
                    1,
                    test.haystack.as_bytes(),
                );
                let mut builder = AhoCorasick::builder();
                $with(&mut builder);
                builder
                    .match_kind(MatchKind::$kind)
                    .build(test.patterns)
                    .unwrap()
                    .stream_find_iter(buf)
                    .map(|result| result.unwrap())
                    .collect()
            });
        }
    };
    ($name:ident, $collection:expr, $kind:ident, $with:expr) => {
        #[test]
        fn $name() {
            run_search_tests($collection, |test| {
                let mut builder = AhoCorasick::builder();
                $with(&mut builder);
                builder
                    .match_kind(MatchKind::$kind)
                    .build(test.patterns)
                    .unwrap()
                    .find_iter(test.haystack)
                    .collect()
            });
        }
    };
}

macro_rules! testcombo {
    ($name:ident, $collection:expr, $kind:ident) => {
        mod $name {
            use super::*;

            testconfig!(default, $collection, $kind, |_| ());
            testconfig!(
                nfa_default,
                $collection,
                $kind,
                |b: &mut AhoCorasickBuilder| {
                    b.kind(Some(AhoCorasickKind::NoncontiguousNFA));
                }
            );
            testconfig!(
                nfa_noncontig_no_prefilter,
                $collection,
                $kind,
                |b: &mut AhoCorasickBuilder| {
                    b.kind(Some(AhoCorasickKind::NoncontiguousNFA))
                        .prefilter(false);
                }
            );
            testconfig!(
                nfa_noncontig_all_sparse,
                $collection,
                $kind,
                |b: &mut AhoCorasickBuilder| {
                    b.kind(Some(AhoCorasickKind::NoncontiguousNFA))
                        .dense_depth(0);
                }
            );
            testconfig!(
                nfa_noncontig_all_dense,
                $collection,
                $kind,
                |b: &mut AhoCorasickBuilder| {
                    b.kind(Some(AhoCorasickKind::NoncontiguousNFA))
                        .dense_depth(usize::MAX);
                }
            );
            testconfig!(
                nfa_contig_default,
                $collection,
                $kind,
                |b: &mut AhoCorasickBuilder| {
                    b.kind(Some(AhoCorasickKind::ContiguousNFA));
                }
            );
            testconfig!(
                nfa_contig_no_prefilter,
                $collection,
                $kind,
                |b: &mut AhoCorasickBuilder| {
                    b.kind(Some(AhoCorasickKind::ContiguousNFA))
                        .prefilter(false);
                }
            );
            testconfig!(
                nfa_contig_all_sparse,
                $collection,
                $kind,
                |b: &mut AhoCorasickBuilder| {
                    b.kind(Some(AhoCorasickKind::ContiguousNFA))
                        .dense_depth(0);
                }
            );
            testconfig!(
                nfa_contig_all_dense,
                $collection,
                $kind,
                |b: &mut AhoCorasickBuilder| {
                    b.kind(Some(AhoCorasickKind::ContiguousNFA))
                        .dense_depth(usize::MAX);
                }
            );
            testconfig!(
                nfa_contig_no_byte_class,
                $collection,
                $kind,
                |b: &mut AhoCorasickBuilder| {
                    b.kind(Some(AhoCorasickKind::ContiguousNFA))
                        .byte_classes(false);
                }
            );
            testconfig!(
                dfa_default,
                $collection,
                $kind,
                |b: &mut AhoCorasickBuilder| {
                    b.kind(Some(AhoCorasickKind::DFA));
                }
            );
            testconfig!(
                dfa_start_both,
                $collection,
                $kind,
                |b: &mut AhoCorasickBuilder| {
                    b.kind(Some(AhoCorasickKind::DFA))
                        .start_kind(StartKind::Both);
                }
            );
            testconfig!(
                dfa_no_prefilter,
                $collection,
                $kind,
                |b: &mut AhoCorasickBuilder| {
                    b.kind(Some(AhoCorasickKind::DFA)).prefilter(false);
                }
            );
            testconfig!(
                dfa_start_both_no_prefilter,
                $collection,
                $kind,
                |b: &mut AhoCorasickBuilder| {
                    b.kind(Some(AhoCorasickKind::DFA))
                        .start_kind(StartKind::Both)
                        .prefilter(false);
                }
            );
            testconfig!(
                dfa_no_byte_class,
                $collection,
                $kind,
                |b: &mut AhoCorasickBuilder| {
                    b.kind(Some(AhoCorasickKind::DFA)).byte_classes(false);
                }
            );
            testconfig!(
                dfa_start_both_no_byte_class,
                $collection,
                $kind,
                |b: &mut AhoCorasickBuilder| {
                    b.kind(Some(AhoCorasickKind::DFA))
                        .start_kind(StartKind::Both)
                        .byte_classes(false);
                }
            );
        }
    };
}

// Write out the various combinations of match semantics given the variety of
// configurations tested by 'testcombo!'.
testcombo!(search_leftmost_longest, AC_LEFTMOST_LONGEST, LeftmostLongest);
testcombo!(search_leftmost_first, AC_LEFTMOST_FIRST, LeftmostFirst);
testcombo!(
    search_standard_nonoverlapping,
    AC_STANDARD_NON_OVERLAPPING,
    Standard
);

// Write out the overlapping combo by hand since there is only one of them.
testconfig!(
    overlapping,
    search_standard_overlapping_default,
    AC_STANDARD_OVERLAPPING,
    Standard,
    |_| ()
);
testconfig!(
    overlapping,
    search_standard_overlapping_nfa_noncontig_default,
    AC_STANDARD_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::NoncontiguousNFA));
    }
);
testconfig!(
    overlapping,
    search_standard_overlapping_nfa_noncontig_no_prefilter,
    AC_STANDARD_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::NoncontiguousNFA)).prefilter(false);
    }
);
testconfig!(
    overlapping,
    search_standard_overlapping_nfa_contig_default,
    AC_STANDARD_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::ContiguousNFA));
    }
);
testconfig!(
    overlapping,
    search_standard_overlapping_nfa_contig_no_prefilter,
    AC_STANDARD_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::ContiguousNFA)).prefilter(false);
    }
);
testconfig!(
    overlapping,
    search_standard_overlapping_nfa_contig_all_sparse,
    AC_STANDARD_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::ContiguousNFA)).dense_depth(0);
    }
);
testconfig!(
    overlapping,
    search_standard_overlapping_nfa_contig_all_dense,
    AC_STANDARD_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::ContiguousNFA)).dense_depth(usize::MAX);
    }
);
testconfig!(
    overlapping,
    search_standard_overlapping_dfa_default,
    AC_STANDARD_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::DFA));
    }
);
testconfig!(
    overlapping,
    search_standard_overlapping_dfa_start_both,
    AC_STANDARD_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::DFA)).start_kind(StartKind::Both);
    }
);
testconfig!(
    overlapping,
    search_standard_overlapping_dfa_no_prefilter,
    AC_STANDARD_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::DFA)).prefilter(false);
    }
);
testconfig!(
    overlapping,
    search_standard_overlapping_dfa_start_both_no_prefilter,
    AC_STANDARD_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::DFA))
            .start_kind(StartKind::Both)
            .prefilter(false);
    }
);
testconfig!(
    overlapping,
    search_standard_overlapping_dfa_no_byte_class,
    AC_STANDARD_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::DFA)).byte_classes(false);
    }
);
testconfig!(
    overlapping,
    search_standard_overlapping_dfa_start_both_no_byte_class,
    AC_STANDARD_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::DFA))
            .start_kind(StartKind::Both)
            .byte_classes(false);
    }
);

// Also write out tests manually for streams, since we only test the standard
// match semantics. We also don't bother testing different automaton
// configurations, since those are well covered by tests above.
#[cfg(feature = "std")]
testconfig!(
    stream,
    search_standard_stream_default,
    AC_STANDARD_NON_OVERLAPPING,
    Standard,
    |_| ()
);
#[cfg(feature = "std")]
testconfig!(
    stream,
    search_standard_stream_nfa_noncontig_default,
    AC_STANDARD_NON_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::NoncontiguousNFA));
    }
);
#[cfg(feature = "std")]
testconfig!(
    stream,
    search_standard_stream_nfa_contig_default,
    AC_STANDARD_NON_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::ContiguousNFA));
    }
);
#[cfg(feature = "std")]
testconfig!(
    stream,
    search_standard_stream_dfa_default,
    AC_STANDARD_NON_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::DFA));
    }
);

// Same thing for anchored searches. Write them out manually.
testconfig!(
    anchored,
    search_standard_anchored_default,
    AC_STANDARD_ANCHORED_NON_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.start_kind(StartKind::Anchored);
    }
);
testconfig!(
    anchored,
    search_standard_anchored_nfa_noncontig_default,
    AC_STANDARD_ANCHORED_NON_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.start_kind(StartKind::Anchored)
            .kind(Some(AhoCorasickKind::NoncontiguousNFA));
    }
);
testconfig!(
    anchored,
    search_standard_anchored_nfa_contig_default,
    AC_STANDARD_ANCHORED_NON_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.start_kind(StartKind::Anchored)
            .kind(Some(AhoCorasickKind::ContiguousNFA));
    }
);
testconfig!(
    anchored,
    search_standard_anchored_dfa_default,
    AC_STANDARD_ANCHORED_NON_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.start_kind(StartKind::Anchored).kind(Some(AhoCorasickKind::DFA));
    }
);
testconfig!(
    anchored,
    search_standard_anchored_dfa_start_both,
    AC_STANDARD_ANCHORED_NON_OVERLAPPING,
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.start_kind(StartKind::Both).kind(Some(AhoCorasickKind::DFA));
    }
);
testconfig!(
    anchored,
    search_leftmost_first_anchored_default,
    AC_LEFTMOST_FIRST_ANCHORED,
    LeftmostFirst,
    |b: &mut AhoCorasickBuilder| {
        b.start_kind(StartKind::Anchored);
    }
);
testconfig!(
    anchored,
    search_leftmost_first_anchored_nfa_noncontig_default,
    AC_LEFTMOST_FIRST_ANCHORED,
    LeftmostFirst,
    |b: &mut AhoCorasickBuilder| {
        b.start_kind(StartKind::Anchored)
            .kind(Some(AhoCorasickKind::NoncontiguousNFA));
    }
);
testconfig!(
    anchored,
    search_leftmost_first_anchored_nfa_contig_default,
    AC_LEFTMOST_FIRST_ANCHORED,
    LeftmostFirst,
    |b: &mut AhoCorasickBuilder| {
        b.start_kind(StartKind::Anchored)
            .kind(Some(AhoCorasickKind::ContiguousNFA));
    }
);
testconfig!(
    anchored,
    search_leftmost_first_anchored_dfa_default,
    AC_LEFTMOST_FIRST_ANCHORED,
    LeftmostFirst,
    |b: &mut AhoCorasickBuilder| {
        b.start_kind(StartKind::Anchored).kind(Some(AhoCorasickKind::DFA));
    }
);
testconfig!(
    anchored,
    search_leftmost_first_anchored_dfa_start_both,
    AC_LEFTMOST_FIRST_ANCHORED,
    LeftmostFirst,
    |b: &mut AhoCorasickBuilder| {
        b.start_kind(StartKind::Both).kind(Some(AhoCorasickKind::DFA));
    }
);
testconfig!(
    anchored,
    search_leftmost_longest_anchored_default,
    AC_LEFTMOST_LONGEST_ANCHORED,
    LeftmostLongest,
    |b: &mut AhoCorasickBuilder| {
        b.start_kind(StartKind::Anchored);
    }
);
testconfig!(
    anchored,
    search_leftmost_longest_anchored_nfa_noncontig_default,
    AC_LEFTMOST_LONGEST_ANCHORED,
    LeftmostLongest,
    |b: &mut AhoCorasickBuilder| {
        b.start_kind(StartKind::Anchored)
            .kind(Some(AhoCorasickKind::NoncontiguousNFA));
    }
);
testconfig!(
    anchored,
    search_leftmost_longest_anchored_nfa_contig_default,
    AC_LEFTMOST_LONGEST_ANCHORED,
    LeftmostLongest,
    |b: &mut AhoCorasickBuilder| {
        b.start_kind(StartKind::Anchored)
            .kind(Some(AhoCorasickKind::ContiguousNFA));
    }
);
testconfig!(
    anchored,
    search_leftmost_longest_anchored_dfa_default,
    AC_LEFTMOST_LONGEST_ANCHORED,
    LeftmostLongest,
    |b: &mut AhoCorasickBuilder| {
        b.start_kind(StartKind::Anchored).kind(Some(AhoCorasickKind::DFA));
    }
);
testconfig!(
    anchored,
    search_leftmost_longest_anchored_dfa_start_both,
    AC_LEFTMOST_LONGEST_ANCHORED,
    LeftmostLongest,
    |b: &mut AhoCorasickBuilder| {
        b.start_kind(StartKind::Both).kind(Some(AhoCorasickKind::DFA));
    }
);

// And also write out the test combinations for ASCII case insensitivity.
testconfig!(
    acasei_standard_default,
    &[ASCII_CASE_INSENSITIVE],
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.prefilter(false).ascii_case_insensitive(true);
    }
);
testconfig!(
    acasei_standard_nfa_noncontig_default,
    &[ASCII_CASE_INSENSITIVE],
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::NoncontiguousNFA))
            .prefilter(false)
            .ascii_case_insensitive(true);
    }
);
testconfig!(
    acasei_standard_nfa_contig_default,
    &[ASCII_CASE_INSENSITIVE],
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::ContiguousNFA))
            .prefilter(false)
            .ascii_case_insensitive(true);
    }
);
testconfig!(
    acasei_standard_dfa_default,
    &[ASCII_CASE_INSENSITIVE, ASCII_CASE_INSENSITIVE_NON_OVERLAPPING],
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::DFA)).ascii_case_insensitive(true);
    }
);
testconfig!(
    overlapping,
    acasei_standard_overlapping_default,
    &[ASCII_CASE_INSENSITIVE, ASCII_CASE_INSENSITIVE_OVERLAPPING],
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.ascii_case_insensitive(true);
    }
);
testconfig!(
    overlapping,
    acasei_standard_overlapping_nfa_noncontig_default,
    &[ASCII_CASE_INSENSITIVE, ASCII_CASE_INSENSITIVE_OVERLAPPING],
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::NoncontiguousNFA))
            .ascii_case_insensitive(true);
    }
);
testconfig!(
    overlapping,
    acasei_standard_overlapping_nfa_contig_default,
    &[ASCII_CASE_INSENSITIVE, ASCII_CASE_INSENSITIVE_OVERLAPPING],
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::ContiguousNFA))
            .ascii_case_insensitive(true);
    }
);
testconfig!(
    overlapping,
    acasei_standard_overlapping_dfa_default,
    &[ASCII_CASE_INSENSITIVE, ASCII_CASE_INSENSITIVE_OVERLAPPING],
    Standard,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::DFA)).ascii_case_insensitive(true);
    }
);
testconfig!(
    acasei_leftmost_first_default,
    &[ASCII_CASE_INSENSITIVE, ASCII_CASE_INSENSITIVE_NON_OVERLAPPING],
    LeftmostFirst,
    |b: &mut AhoCorasickBuilder| {
        b.ascii_case_insensitive(true);
    }
);
testconfig!(
    acasei_leftmost_first_nfa_noncontig_default,
    &[ASCII_CASE_INSENSITIVE, ASCII_CASE_INSENSITIVE_NON_OVERLAPPING],
    LeftmostFirst,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::NoncontiguousNFA))
            .ascii_case_insensitive(true);
    }
);
testconfig!(
    acasei_leftmost_first_nfa_contig_default,
    &[ASCII_CASE_INSENSITIVE, ASCII_CASE_INSENSITIVE_NON_OVERLAPPING],
    LeftmostFirst,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::ContiguousNFA))
            .ascii_case_insensitive(true);
    }
);
testconfig!(
    acasei_leftmost_first_dfa_default,
    &[ASCII_CASE_INSENSITIVE, ASCII_CASE_INSENSITIVE_NON_OVERLAPPING],
    LeftmostFirst,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::DFA)).ascii_case_insensitive(true);
    }
);
testconfig!(
    acasei_leftmost_longest_default,
    &[ASCII_CASE_INSENSITIVE, ASCII_CASE_INSENSITIVE_NON_OVERLAPPING],
    LeftmostLongest,
    |b: &mut AhoCorasickBuilder| {
        b.ascii_case_insensitive(true);
    }
);
testconfig!(
    acasei_leftmost_longest_nfa_noncontig_default,
    &[ASCII_CASE_INSENSITIVE, ASCII_CASE_INSENSITIVE_NON_OVERLAPPING],
    LeftmostLongest,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::NoncontiguousNFA))
            .ascii_case_insensitive(true);
    }
);
testconfig!(
    acasei_leftmost_longest_nfa_contig_default,
    &[ASCII_CASE_INSENSITIVE, ASCII_CASE_INSENSITIVE_NON_OVERLAPPING],
    LeftmostLongest,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::ContiguousNFA))
            .ascii_case_insensitive(true);
    }
);
testconfig!(
    acasei_leftmost_longest_dfa_default,
    &[ASCII_CASE_INSENSITIVE, ASCII_CASE_INSENSITIVE_NON_OVERLAPPING],
    LeftmostLongest,
    |b: &mut AhoCorasickBuilder| {
        b.kind(Some(AhoCorasickKind::DFA)).ascii_case_insensitive(true);
    }
);

fn run_search_tests<F: FnMut(&SearchTest) -> Vec<Match>>(
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
        for test in tests {
            assert_eq!(
                test.matches,
                get_match_triples(f(&test)).as_slice(),
                "test: {}, patterns: {:?}, haystack: {:?}",
                test.name,
                test.patterns,
                test.haystack
            );
        }
    }
}

// Like 'run_search_tests', but we skip any tests that contain the empty
// pattern because stream searching doesn't support it.
#[cfg(feature = "std")]
fn run_stream_search_tests<F: FnMut(&SearchTest) -> Vec<Match>>(
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
        for test in tests {
            if test.patterns.iter().any(|p| p.is_empty()) {
                continue;
            }
            assert_eq!(
                test.matches,
                get_match_triples(f(&test)).as_slice(),
                "test: {}, patterns: {:?}, haystack: {:?}",
                test.name,
                test.patterns,
                test.haystack
            );
        }
    }
}

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
    assert("STANDARD", STANDARD);
    assert("LEFTMOST", LEFTMOST);
    assert("LEFTMOST_FIRST", LEFTMOST_FIRST);
    assert("LEFTMOST_LONGEST", LEFTMOST_LONGEST);
    assert("NON_OVERLAPPING", NON_OVERLAPPING);
    assert("OVERLAPPING", OVERLAPPING);
    assert("REGRESSION", REGRESSION);
}

#[cfg(feature = "std")]
#[test]
#[should_panic]
fn stream_not_allowed_leftmost_first() {
    let fsm = AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostFirst)
        .build(None::<String>)
        .unwrap();
    assert_eq!(fsm.stream_find_iter(&b""[..]).count(), 0);
}

#[cfg(feature = "std")]
#[test]
#[should_panic]
fn stream_not_allowed_leftmost_longest() {
    let fsm = AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostLongest)
        .build(None::<String>)
        .unwrap();
    assert_eq!(fsm.stream_find_iter(&b""[..]).count(), 0);
}

#[test]
#[should_panic]
fn overlapping_not_allowed_leftmost_first() {
    let fsm = AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostFirst)
        .build(None::<String>)
        .unwrap();
    assert_eq!(fsm.find_overlapping_iter("").count(), 0);
}

#[test]
#[should_panic]
fn overlapping_not_allowed_leftmost_longest() {
    let fsm = AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostLongest)
        .build(None::<String>)
        .unwrap();
    assert_eq!(fsm.find_overlapping_iter("").count(), 0);
}

// This tests that if we build an AC matcher with an "unanchored" start kind,
// then we can't run an anchored search even if the underlying searcher
// supports it.
//
// The key bit here is that both of the NFAs in this crate unconditionally
// support both unanchored and anchored searches, but the DFA does not because
// of the added cost of doing so. To avoid the top-level AC matcher sometimes
// supporting anchored and sometimes not (depending on which searcher it
// chooses to use internally), we ensure that the given 'StartKind' is always
// respected.
#[test]
fn anchored_not_allowed_even_if_technically_available() {
    let ac = AhoCorasick::builder()
        .kind(Some(AhoCorasickKind::NoncontiguousNFA))
        .start_kind(StartKind::Unanchored)
        .build(&["foo"])
        .unwrap();
    assert!(ac.try_find(Input::new("foo").anchored(Anchored::Yes)).is_err());

    let ac = AhoCorasick::builder()
        .kind(Some(AhoCorasickKind::ContiguousNFA))
        .start_kind(StartKind::Unanchored)
        .build(&["foo"])
        .unwrap();
    assert!(ac.try_find(Input::new("foo").anchored(Anchored::Yes)).is_err());

    // For completeness, check that the DFA returns an error too.
    let ac = AhoCorasick::builder()
        .kind(Some(AhoCorasickKind::DFA))
        .start_kind(StartKind::Unanchored)
        .build(&["foo"])
        .unwrap();
    assert!(ac.try_find(Input::new("foo").anchored(Anchored::Yes)).is_err());
}

// This is like the test aboved, but with unanchored and anchored flipped. That
// is, we asked for an AC searcher with anchored support and we check that
// unanchored searches return an error even if the underlying searcher would
// technically support it.
#[test]
fn unanchored_not_allowed_even_if_technically_available() {
    let ac = AhoCorasick::builder()
        .kind(Some(AhoCorasickKind::NoncontiguousNFA))
        .start_kind(StartKind::Anchored)
        .build(&["foo"])
        .unwrap();
    assert!(ac.try_find(Input::new("foo").anchored(Anchored::No)).is_err());

    let ac = AhoCorasick::builder()
        .kind(Some(AhoCorasickKind::ContiguousNFA))
        .start_kind(StartKind::Anchored)
        .build(&["foo"])
        .unwrap();
    assert!(ac.try_find(Input::new("foo").anchored(Anchored::No)).is_err());

    // For completeness, check that the DFA returns an error too.
    let ac = AhoCorasick::builder()
        .kind(Some(AhoCorasickKind::DFA))
        .start_kind(StartKind::Anchored)
        .build(&["foo"])
        .unwrap();
    assert!(ac.try_find(Input::new("foo").anchored(Anchored::No)).is_err());
}

// This tests that a prefilter does not cause a search to report a match
// outside the bounds provided by the caller.
//
// This is a regression test for a bug I introduced during the rewrite of most
// of the crate after 0.7. It was never released. The tricky part here is
// ensuring we get a prefilter that can report matches on its own (such as the
// packed searcher). Otherwise, prefilters that report false positives might
// have searched past the bounds provided by the caller, but confirming the
// match would subsequently fail.
#[test]
fn prefilter_stays_in_bounds() {
    let ac = AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostFirst)
        .build(&["sam", "frodo", "pippin", "merry", "gandalf", "sauron"])
        .unwrap();
    let haystack = "foo gandalf";
    assert_eq!(None, ac.find(Input::new(haystack).range(0..10)));
}

// See: https://github.com/BurntSushi/aho-corasick/issues/44
//
// In short, this test ensures that enabling ASCII case insensitivity does not
// visit an exponential number of states when filling in failure transitions.
#[test]
fn regression_ascii_case_insensitive_no_exponential() {
    let ac = AhoCorasick::builder()
        .ascii_case_insensitive(true)
        .build(&["Tsubaki House-Triple Shot Vol01校花三姐妹"])
        .unwrap();
    assert!(ac.find("").is_none());
}

// See: https://github.com/BurntSushi/aho-corasick/issues/53
//
// This test ensures that the rare byte prefilter works in a particular corner
// case. In particular, the shift offset detected for '/' in the patterns below
// was incorrect, leading to a false negative.
#[test]
fn regression_rare_byte_prefilter() {
    use crate::AhoCorasick;

    let ac = AhoCorasick::new(&["ab/j/", "x/"]).unwrap();
    assert!(ac.is_match("ab/j/"));
}

#[test]
fn regression_case_insensitive_prefilter() {
    for c in b'a'..b'z' {
        for c2 in b'a'..b'z' {
            let c = c as char;
            let c2 = c2 as char;
            let needle = format!("{}{}", c, c2).to_lowercase();
            let haystack = needle.to_uppercase();
            let ac = AhoCorasick::builder()
                .ascii_case_insensitive(true)
                .prefilter(true)
                .build(&[&needle])
                .unwrap();
            assert_eq!(
                1,
                ac.find_iter(&haystack).count(),
                "failed to find {:?} in {:?}\n\nautomaton:\n{:?}",
                needle,
                haystack,
                ac,
            );
        }
    }
}

// See: https://github.com/BurntSushi/aho-corasick/issues/64
//
// This occurs when the rare byte prefilter is active.
#[cfg(feature = "std")]
#[test]
fn regression_stream_rare_byte_prefilter() {
    use std::io::Read;

    // NOTE: The test only fails if this ends with j.
    const MAGIC: [u8; 5] = *b"1234j";

    // NOTE: The test fails for value in 8188..=8191 These value put the string
    // to search accross two call to read because the buffer size is 64KB by
    // default.
    const BEGIN: usize = 65_535;

    /// This is just a structure that implements Reader. The reader
    /// implementation will simulate a file filled with 0, except for the MAGIC
    /// string at offset BEGIN.
    #[derive(Default)]
    struct R {
        read: usize,
    }

    impl Read for R {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.read > 100000 {
                return Ok(0);
            }
            let mut from = 0;
            if self.read < BEGIN {
                from = buf.len().min(BEGIN - self.read);
                for x in 0..from {
                    buf[x] = 0;
                }
                self.read += from;
            }
            if self.read >= BEGIN && self.read <= BEGIN + MAGIC.len() {
                let to = buf.len().min(BEGIN + MAGIC.len() - self.read + from);
                if to > from {
                    buf[from..to].copy_from_slice(
                        &MAGIC
                            [self.read - BEGIN..self.read - BEGIN + to - from],
                    );
                    self.read += to - from;
                    from = to;
                }
            }
            for x in from..buf.len() {
                buf[x] = 0;
                self.read += 1;
            }
            Ok(buf.len())
        }
    }

    fn run() -> std::io::Result<()> {
        let aut = AhoCorasick::builder()
            // Enable byte classes to make debugging the automaton easier. It
            // should have no effect on the test result.
            .byte_classes(false)
            .build(&[&MAGIC])
            .unwrap();

        // While reading from a vector, it works:
        let mut buf = alloc::vec![];
        R::default().read_to_end(&mut buf)?;
        let from_whole = aut.find_iter(&buf).next().unwrap().start();

        // But using stream_find_iter fails!
        let mut file = std::io::BufReader::new(R::default());
        let begin = aut
            .stream_find_iter(&mut file)
            .next()
            .expect("NOT FOUND!!!!")? // Panic here
            .start();
        assert_eq!(from_whole, begin);
        Ok(())
    }

    run().unwrap()
}
