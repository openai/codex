use crate::chars::Char;
use crate::pattern::{CaseMatching, Normalization, Pattern};
use crate::score::{
    BONUS_BOUNDARY, BONUS_CAMEL123, BONUS_CONSECUTIVE, BONUS_FIRST_CHAR_MULTIPLIER, BONUS_NON_WORD,
    MAX_PREFIX_BONUS, PENALTY_GAP_EXTENSION, PENALTY_GAP_START, SCORE_MATCH,
};
use crate::utf32_str::Utf32Str;
use crate::{Config, Matcher};

use Algorithm::*;

#[derive(Debug)]
enum Algorithm {
    FuzzyOptimal,
    FuzzyGreedy,
    Substring,
    Prefix,
    Postfix,
    Exact,
}

fn assert_matches(
    algorithm: &[Algorithm],
    normalize: bool,
    case_sensitive: bool,
    path: bool,
    prefer_prefix: bool,
    cases: &[(&str, &str, &[u32], u16)],
) {
    let mut config = Config {
        normalize,
        ignore_case: !case_sensitive,
        prefer_prefix,
        ..Config::DEFAULT
    };
    if path {
        config.set_match_paths();
    }
    let mut matcher = Matcher::new(config);
    let mut matched_indices = Vec::new();
    let mut needle_buf = Vec::new();
    let mut haystack_buf = Vec::new();
    for &(haystack, needle, indices, mut score) in cases {
        let needle = if !case_sensitive {
            needle.to_lowercase()
        } else {
            needle.to_owned()
        };
        let needle = Utf32Str::new(&needle, &mut needle_buf);
        let haystack = Utf32Str::new(haystack, &mut haystack_buf);
        score += needle.len() as u16 * SCORE_MATCH;
        for algo in algorithm {
            println!("xx {matched_indices:?} {algo:?}");
            matched_indices.clear();
            let res = match algo {
                FuzzyOptimal => matcher.fuzzy_indices(haystack, needle, &mut matched_indices),
                FuzzyGreedy => matcher.fuzzy_indices_greedy(haystack, needle, &mut matched_indices),
                Substring => matcher.substring_indices(haystack, needle, &mut matched_indices),
                Prefix => matcher.prefix_indices(haystack, needle, &mut matched_indices),
                Postfix => matcher.postfix_indices(haystack, needle, &mut matched_indices),
                Exact => matcher.exact_indices(haystack, needle, &mut matched_indices),
            };
            println!("{matched_indices:?}");
            let match_chars: Vec<_> = matched_indices
                .iter()
                .map(|&i| haystack.get(i).normalize(&matcher.config))
                .collect();
            let needle_chars: Vec<_> = needle.chars().collect();

            assert_eq!(
                res,
                Some(score),
                "{needle:?} did  not match {haystack:?}: matched {match_chars:?} {matched_indices:?} {algo:?}"
            );
            assert_eq!(
                matched_indices, indices,
                "{needle:?} match {haystack:?} {algo:?}"
            );
            assert_eq!(
                match_chars, needle_chars,
                "{needle:?} match {haystack:?} indices are incorrect {matched_indices:?} {algo:?}"
            );
        }
    }
}

fn assert_not_matches_with(
    normalize: bool,
    case_sensitive: bool,
    algorithm: &[Algorithm],
    cases: &[(&str, &str)],
) {
    let config = Config {
        normalize,
        ignore_case: !case_sensitive,
        ..Config::DEFAULT
    };
    let mut matcher = Matcher::new(config);
    let mut needle_buf = Vec::new();
    let mut haystack_buf = Vec::new();
    for &(haystack, needle) in cases {
        let needle = if !case_sensitive {
            needle.to_lowercase()
        } else {
            needle.to_owned()
        };
        let needle = Utf32Str::new(&needle, &mut needle_buf);
        let haystack = Utf32Str::new(haystack, &mut haystack_buf);

        for algo in algorithm {
            let res = match algo {
                FuzzyOptimal => matcher.fuzzy_match(haystack, needle),
                FuzzyGreedy => matcher.fuzzy_match_greedy(haystack, needle),
                Substring => matcher.substring_match(haystack, needle),
                Prefix => matcher.prefix_match(haystack, needle),
                Postfix => matcher.postfix_match(haystack, needle),
                Exact => matcher.exact_match(haystack, needle),
            };
            assert_eq!(
                res, None,
                "{needle:?} should not match {haystack:?} {algo:?}"
            );
        }
    }
}

pub fn assert_not_matches(normalize: bool, case_sensitive: bool, cases: &[(&str, &str)]) {
    assert_not_matches_with(
        normalize,
        case_sensitive,
        &[FuzzyOptimal, FuzzyGreedy, Substring, Prefix, Postfix, Exact],
        cases,
    )
}

const BONUS_BOUNDARY_WHITE: u16 = Config::DEFAULT.bonus_boundary_white;
const BONUS_BOUNDARY_DELIMITER: u16 = Config::DEFAULT.bonus_boundary_delimiter;

#[test]
fn test_fuzzy() {
    assert_matches(
        &[FuzzyGreedy, FuzzyOptimal],
        false,
        false,
        false,
        false,
        &[
            (
                "fooBarbaz1",
                "obr",
                &[2, 3, 5],
                BONUS_CAMEL123 - PENALTY_GAP_START,
            ),
            (
                "/usr/share/doc/at/ChangeLog",
                "changelog",
                &[18, 19, 20, 21, 22, 23, 24, 25, 26],
                (BONUS_FIRST_CHAR_MULTIPLIER + 8) * BONUS_BOUNDARY_DELIMITER,
            ),
            (
                "fooBarbaz1",
                "br",
                &[3, 5],
                BONUS_CAMEL123 * BONUS_FIRST_CHAR_MULTIPLIER - PENALTY_GAP_START,
            ),
            (
                "foo bar baz",
                "fbb",
                &[0, 4, 8],
                BONUS_BOUNDARY_WHITE * BONUS_FIRST_CHAR_MULTIPLIER + BONUS_BOUNDARY_WHITE * 2
                    - 2 * PENALTY_GAP_START
                    - 4 * PENALTY_GAP_EXTENSION,
            ),
            (
                "/AutomatorDocument.icns",
                "rdoc",
                &[9, 10, 11, 12],
                BONUS_CAMEL123 + 2 * BONUS_CONSECUTIVE,
            ),
            (
                "/man1/zshcompctl.1",
                "zshc",
                &[6, 7, 8, 9],
                BONUS_BOUNDARY_DELIMITER * (BONUS_FIRST_CHAR_MULTIPLIER + 3),
            ),
            (
                "/.oh-my-zsh/cache",
                "zshc",
                &[8, 9, 10, 12],
                BONUS_BOUNDARY * (BONUS_FIRST_CHAR_MULTIPLIER + 2) - PENALTY_GAP_START
                    + BONUS_BOUNDARY_DELIMITER,
            ),
            (
                "ab0123 456",
                "12356",
                &[3, 4, 5, 8, 9],
                BONUS_CONSECUTIVE * 3 - PENALTY_GAP_START - PENALTY_GAP_EXTENSION,
            ),
            (
                "abc123 456",
                "12356",
                &[3, 4, 5, 8, 9],
                BONUS_CAMEL123 * (BONUS_FIRST_CHAR_MULTIPLIER + 2)
                    - PENALTY_GAP_START
                    - PENALTY_GAP_EXTENSION
                    + BONUS_CONSECUTIVE,
            ),
            (
                "foo/bar/baz",
                "fbb",
                &[0, 4, 8],
                BONUS_BOUNDARY_WHITE * BONUS_FIRST_CHAR_MULTIPLIER + BONUS_BOUNDARY_DELIMITER * 2
                    - 2 * PENALTY_GAP_START
                    - 4 * PENALTY_GAP_EXTENSION,
            ),
            (
                "fooBarBaz",
                "fbb",
                &[0, 3, 6],
                BONUS_BOUNDARY_WHITE * BONUS_FIRST_CHAR_MULTIPLIER + BONUS_CAMEL123 * 2
                    - 2 * PENALTY_GAP_START
                    - 2 * PENALTY_GAP_EXTENSION,
            ),
            (
                "foo barbaz",
                "fbb",
                &[0, 4, 7],
                BONUS_BOUNDARY_WHITE * BONUS_FIRST_CHAR_MULTIPLIER + BONUS_BOUNDARY_WHITE
                    - PENALTY_GAP_START * 2
                    - PENALTY_GAP_EXTENSION * 3,
            ),
            (
                "fooBar Baz",
                "foob",
                &[0, 1, 2, 3],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 3),
            ),
            (
                "xFoo-Bar Baz",
                "foo-b",
                &[1, 2, 3, 4, 5],
                BONUS_CAMEL123 * (BONUS_FIRST_CHAR_MULTIPLIER + 2) + 2 * BONUS_NON_WORD,
            ),
        ],
    );
}

#[test]
fn empty_needle() {
    assert_matches(
        &[Substring, Prefix, Postfix, FuzzyGreedy, FuzzyOptimal, Exact],
        false,
        false,
        false,
        false,
        &[("foo bar baz", "", &[], 0)],
    );
}

#[test]
fn test_substring() {
    assert_matches(
        &[Substring, Prefix],
        false,
        false,
        false,
        false,
        &[
            (
                "foo bar baz",
                "foo",
                &[0, 1, 2],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 2),
            ),
            (
                " foo bar baz",
                "FOO",
                &[1, 2, 3],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 2),
            ),
            (
                " foo bar baz",
                " FOO",
                &[0, 1, 2, 3],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 3),
            ),
        ],
    );
    assert_matches(
        &[Substring, Postfix],
        false,
        false,
        false,
        false,
        &[
            (
                "foo bar baz",
                "baz",
                &[8, 9, 10],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 2),
            ),
            (
                "foo bar baz ",
                "baz",
                &[8, 9, 10],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 2),
            ),
            (
                "foo bar baz ",
                "baz ",
                &[8, 9, 10, 11],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 3),
            ),
        ],
    );
    assert_matches(
        &[Substring, Prefix, Postfix, Exact, FuzzyGreedy, FuzzyOptimal],
        false,
        false,
        false,
        false,
        &[
            (
                "foo",
                "foo",
                &[0, 1, 2],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 2),
            ),
            (
                " foo",
                "foo",
                &[1, 2, 3],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 2),
            ),
            (
                " foo",
                " foo",
                &[0, 1, 2, 3],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 3),
            ),
        ],
    );
    assert_matches(
        &[Substring],
        false,
        false,
        false,
        false,
        &[
            (
                "fooBarbaz1",
                "oba",
                &[2, 3, 4],
                BONUS_CAMEL123 + BONUS_CONSECUTIVE,
            ),
            (
                "/AutomatorDocument.icns",
                "rdoc",
                &[9, 10, 11, 12],
                BONUS_CAMEL123 + 2 * BONUS_CONSECUTIVE,
            ),
            (
                "/man1/zshcompctl.1",
                "zshc",
                &[6, 7, 8, 9],
                BONUS_BOUNDARY_DELIMITER * (BONUS_FIRST_CHAR_MULTIPLIER + 3),
            ),
            (
                "/.oh-my-zsh/cache",
                "zsh/c",
                &[8, 9, 10, 11, 12],
                BONUS_BOUNDARY * (BONUS_FIRST_CHAR_MULTIPLIER + 2)
                    + BONUS_NON_WORD
                    + BONUS_BOUNDARY_DELIMITER,
            ),
        ],
    );
    assert_not_matches_with(
        true,
        false,
        &[Prefix, Substring, Postfix, Exact],
        &[(
            "At the Road’s End - Seeming - SOL: A Self-Banishment Ritual",
            "adi",
        )],
    )
}

#[test]
fn test_fuzzy_case_sensitive() {
    assert_matches(
        &[FuzzyGreedy, FuzzyOptimal],
        false,
        true,
        false,
        false,
        &[
            (
                "fooBarbaz1",
                "oBr",
                &[2, 3, 5],
                BONUS_CAMEL123 - PENALTY_GAP_START,
            ),
            (
                "Foo/Bar/Baz",
                "FBB",
                &[0, 4, 8],
                BONUS_BOUNDARY_WHITE * BONUS_FIRST_CHAR_MULTIPLIER + BONUS_BOUNDARY_DELIMITER * 2
                    - 2 * PENALTY_GAP_START
                    - 4 * PENALTY_GAP_EXTENSION,
            ),
            (
                "FooBarBaz",
                "FBB",
                &[0, 3, 6],
                BONUS_BOUNDARY_WHITE * BONUS_FIRST_CHAR_MULTIPLIER + BONUS_CAMEL123 * 2
                    - 2 * PENALTY_GAP_START
                    - 2 * PENALTY_GAP_EXTENSION,
            ),
            (
                "FooBar Baz",
                "FooB",
                &[0, 1, 2, 3],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 3),
            ),
            ("foo-bar", "o-ba", &[2, 3, 4, 5], BONUS_NON_WORD * 3),
        ],
    );
}

#[test]
fn test_normalize() {
    assert_matches(
        &[FuzzyGreedy, FuzzyOptimal],
        true,
        false,
        false,
        false,
        &[
            (
                "Só Danço Samba",
                "So",
                &[0, 1],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 1),
            ),
            (
                "Só Danço Samba",
                "sodc",
                &[0, 1, 3, 6],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 1) - PENALTY_GAP_START
                    + BONUS_BOUNDARY_WHITE
                    - PENALTY_GAP_START
                    - PENALTY_GAP_EXTENSION,
            ),
            (
                "Danço",
                "danco",
                &[0, 1, 2, 3, 4],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 4),
            ),
            (
                "DanÇo",
                "danco",
                &[0, 1, 2, 3, 4],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 4),
            ),
            (
                "xÇando",
                "cando",
                &[1, 2, 3, 4, 5],
                BONUS_CAMEL123 * (BONUS_FIRST_CHAR_MULTIPLIER + 4),
            ),
            ("ۂ(GCGɴCG", "n", &[5], 0),
        ],
    )
}

#[test]
fn test_unicode() {
    assert_matches(
        &[FuzzyGreedy, FuzzyOptimal, Substring],
        true,
        false,
        false,
        false,
        &[
            (
                "你好世界",
                "你好",
                &[0, 1],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 1),
            ),
            (
                " 你好世界",
                "你好",
                &[1, 2],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 1),
            ),
        ],
    );
    assert_matches(
        &[FuzzyGreedy, FuzzyOptimal],
        true,
        false,
        false,
        false,
        &[(
            "你好世界",
            "你世",
            &[0, 2],
            BONUS_BOUNDARY_WHITE * BONUS_FIRST_CHAR_MULTIPLIER - PENALTY_GAP_START,
        )],
    );
    assert_not_matches(
        false,
        false,
        &[("Flibbertigibbet / イタズラっ子たち", "lying")],
    );
}

#[test]
fn test_long_str() {
    assert_matches(
        &[FuzzyGreedy, FuzzyOptimal],
        false,
        false,
        false,
        false,
        &[(
            &"x".repeat(u16::MAX as usize + 1),
            "xx",
            &[0, 1],
            BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 1),
        )],
    );
}

#[test]
fn test_casing() {
    assert_matches(
        &[FuzzyGreedy, FuzzyOptimal],
        false,
        false,
        false,
        false,
        &[
            // these two have the same score
            (
                "fooBar",
                "foobar",
                &[0, 1, 2, 3, 4, 5],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 5),
            ),
            (
                "foobar",
                "foobar",
                &[0, 1, 2, 3, 4, 5],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 5),
            ),
            // these two have the same score (slightly lower than the other two: 60 instead of 70)
            (
                "foo-bar",
                "foobar",
                &[0, 1, 2, 4, 5, 6],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 2) - PENALTY_GAP_START
                    + BONUS_BOUNDARY * 3,
            ),
            (
                "foo_bar",
                "foobar",
                &[0, 1, 2, 4, 5, 6],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 2) - PENALTY_GAP_START
                    + BONUS_BOUNDARY * 3,
            ),
        ],
    )
}

#[test]
fn test_optimal() {
    assert_matches(
        &[FuzzyOptimal],
        false,
        false,
        false,
        false,
        &[
            (
                "axxx xx ",
                "xx",
                &[5, 6],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 1),
            ),
            (
                "SS!H",
                "S!",
                &[0, 2],
                BONUS_BOUNDARY_WHITE * BONUS_FIRST_CHAR_MULTIPLIER - PENALTY_GAP_START
                    + BONUS_NON_WORD,
            ),
            // this case is a cool example of why our algorithm is more than fzf
            // we handle this corretly detect that it's better to match
            // the second f instead of the third yielding a higher score
            // (despite using the same scoring function!)
            (
                "xf.foo",
                "xfoo",
                &[0, 3, 4, 5],
                BONUS_BOUNDARY_WHITE * BONUS_FIRST_CHAR_MULTIPLIER
                    - PENALTY_GAP_START
                    - PENALTY_GAP_EXTENSION
                    + BONUS_BOUNDARY * 3,
            ),
            (
                "xf fo",
                "xfo",
                &[0, 3, 4],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 2)
                    - PENALTY_GAP_START
                    - PENALTY_GAP_EXTENSION,
            ),
        ],
    );
}

#[test]
fn test_reject() {
    assert_not_matches(
        true,
        false,
        &[
            ("你好界", "abc"),
            ("你好界", "a"),
            ("你好世界", "富"),
            ("Só Danço Samba", "sox"),
            ("fooBarbaz", "fooBarbazz"),
            ("fooBarbaz", "c"),
        ],
    );
    assert_not_matches(
        true,
        true,
        &[
            ("你好界", "abc"),
            ("abc", "你"),
            ("abc", "A"),
            ("abc", "d"),
            ("你好世界", "富"),
            ("Só Danço Samba", "sox"),
            ("fooBarbaz", "oBZ"),
            ("Foo Bar Baz", "fbb"),
            ("fooBarbaz", "fooBarbazz"),
        ],
    );
    assert_not_matches(
        false,
        true,
        &[
            ("Só Danço Samba", "sod"),
            ("Só Danço Samba", "soc"),
            ("Só Danç", "So"),
        ],
    );
    assert_not_matches(false, false, &[("ۂۂfoۂۂ", "foo")]);
}

#[test]
fn test_prefer_prefix() {
    assert_matches(
        &[FuzzyOptimal, FuzzyGreedy],
        false,
        false,
        false,
        true,
        &[
            (
                "Moby Dick",
                "md",
                &[0, 5],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 1)  + MAX_PREFIX_BONUS
                    - PENALTY_GAP_START
                    - 3 * PENALTY_GAP_EXTENSION,
            ),
            (
                "Though I cannot tell why it was exactly that those stage managers, the Fates, put me down for this shabby part of a whaling voyage",
                "md",
                &[82, 85],
                BONUS_BOUNDARY_WHITE * (BONUS_FIRST_CHAR_MULTIPLIER + 1)
                    - PENALTY_GAP_START
                    - PENALTY_GAP_EXTENSION,
            ),
        ],
    );
}

#[test]
fn test_single_char_needle() {
    assert_matches(
        &[FuzzyOptimal],
        false,
        false,
        false,
        false,
        &[(
            "foO",
            "o",
            &[2],
            BONUS_FIRST_CHAR_MULTIPLIER * BONUS_CAMEL123,
        )],
    );
    assert_matches(
        &[FuzzyOptimal],
        false,
        false,
        false,
        false,
        &[(
            "föÖ",
            "ö",
            &[2],
            BONUS_FIRST_CHAR_MULTIPLIER * BONUS_CAMEL123,
        )],
    );
}

#[test]
fn umlaut() {
    let paths = ["be", "bë"];
    let mut matcher = Matcher::new(Config::DEFAULT);
    let matches = Pattern::parse("ë", CaseMatching::Ignore, Normalization::Smart)
        .match_list(paths, &mut matcher);
    assert_eq!(matches.len(), 1);
    let matches = Pattern::parse("e", CaseMatching::Ignore, Normalization::Never)
        .match_list(paths, &mut matcher);
    assert_eq!(matches.len(), 1);
    let matches = Pattern::parse("e", CaseMatching::Ignore, Normalization::Smart)
        .match_list(paths, &mut matcher);
    assert_eq!(matches.len(), 2);
}
