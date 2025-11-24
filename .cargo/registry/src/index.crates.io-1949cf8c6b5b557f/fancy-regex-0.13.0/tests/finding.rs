mod common;

use fancy_regex::{Match, Regex};
use std::ops::Range;

#[test]
fn match_api() {
    let m = find_match(r"(\w+)", "... test").unwrap();
    assert_eq!(m.range(), (4..8));
    assert_eq!(Range::from(m), (4..8));
    assert_eq!(m.as_str(), "test");
}

#[test]
fn find_wrap() {
    assert_eq!(find(r"(\w+)", "... test"), Some((4, 8)));
    assert_eq!(find(r"(?m)^yes$", "foo\nyes\n"), Some((4, 7)));
}

#[test]
fn find_fancy_case_insensitive() {
    assert_eq!(find(r"(x|xy)\1", "XX"), None);
    assert_eq!(find(r"(x|xy)\1", "xx"), Some((0, 2)));
    assert_eq!(find(r"((?i:x|xy))\1", "XX"), Some((0, 2)));
}

#[test]
fn lookahead_grouping_single_expression() {
    // These would fail if the delegate expression was `^x|a` (if we didn't
    // group as `^(?:x|a)`).
    assert_eq!(find(r"(?=x|a)", "a"), Some((0, 0)));
    assert_eq!(find(r"(?=x|a)", "bbba"), Some((3, 3)));
}

#[test]
fn lookahead_grouping_multiple_expressions() {
    // These would fail if the delegate expression was `^ab|Bc` (if we didn't
    // preserve grouping of `(?:b|B)`).
    assert_eq!(find(r"(?=(?!x)a(?:b|B)c)", "aBc"), Some((0, 0)));
    assert_eq!(find(r"(?=(?!x)a(?:b|B)c)", "Bc"), None);
}

#[test]
fn lookbehind_grouping_single_expression() {
    assert_eq!(find(r"(?<=x|a)", "a"), Some((1, 1)));
    assert_eq!(find(r"(?<=x|a)", "ba"), Some((2, 2)));
    assert_eq!(find(r"(?<=^a)", "a"), Some((1, 1)));
    assert_eq!(find(r"(?<=^a)", "ba"), None);
}

#[test]
fn lookbehind_variable_sized_alt() {
    assert_eq!(find(r"(?<=a|bc)", "xxa"), Some((3, 3)));
    assert_eq!(find(r"(?<=a|bc)", "xxbc"), Some((4, 4)));
    assert_eq!(find(r"(?<=a|bc)", "xx"), None);
    assert_eq!(find(r"(?<=a|bc)", "xxb"), None);
    assert_eq!(find(r"(?<=a|bc)", "xxc"), None);

    assert!(Regex::new(r"(?<=a(?:b|cd))").is_err());
    assert!(Regex::new(r"(?<=a+b+))").is_err());
}

#[test]
fn negative_lookbehind_variable_sized_alt() {
    assert_eq!(find(r"(?<!a|bc)x", "axx"), Some((2, 3)));
    assert_eq!(find(r"(?<!a|bc)x", "bcxx"), Some((3, 4)));
    assert_eq!(find(r"(?<!a|bc)x", "x"), Some((0, 1)));
    assert_eq!(find(r"(?<!a|bc)x", "bx"), Some((1, 2)));
    assert_eq!(find(r"(?<!a|bc)x", "cx"), Some((1, 2)));
    assert_eq!(find(r"(?<!a|bc)x", "ax"), None);
    assert_eq!(find(r"(?<!a|bc)x", "bcx"), None);

    assert!(Regex::new(r"(?<!a(?:b|cd))").is_err());
    assert!(Regex::new(r"(?<!a+b+)").is_err());
}

#[test]
fn lookahead_looks_left() {
    assert_eq!(find(r"a(?=\b)", "ab"), None);
    assert_eq!(find(r"a(?=\b)", "a."), Some((0, 1)));
    assert_eq!(find(r"a(?=\b|_)", "a."), Some((0, 1)));
    assert_eq!(find(r"a(?=_|\b)", "a."), Some((0, 1)));
}

#[test]
fn negative_lookahead_fail() {
    // This was a tricky one. There's a negative lookahead that contains a
    // "hard" alternative (because of the lookahead). When the VM gets to the
    // point where the body of the negative lookahead matched, it needs to fail
    // the negative lookahead match. That means it needs to pop the stack until
    // before the negative lookahead, and then fail. But how many times it has
    // to pop is not fixed, it depends on the body/VM state.
    assert_eq!(find(r"(?!a(?=b)|x)", "ab"), Some((1, 1)));
    assert_eq!(find(r"(?!`(?:[^`]+(?=`)|x)`)", "`a`"), Some((1, 1)));
}

#[test]
fn backref_for_unmatched_group() {
    assert_eq!(find(r"(a)?\1", "bb"), None);
}

#[test]
fn backref_with_multibyte() {
    assert_eq!(
        find(r"(.+)\1+", "x\u{1F431}\u{1F436}\u{1F431}\u{1F436}"),
        Some((1, 17))
    );
}

#[test]
fn repeat_non_greedy() {
    // (?=a) to make it fancy and use VM
    assert_eq!(find(r"(a(?=a)){2,}?", "aaa"), Some((0, 2)));
    assert_eq!(find(r"(a(?=a)){2}?a", "aaa"), Some((0, 3)));
}

#[test]
fn empty_repeat_non_greedy() {
    // (?=b) to make it fancy and use VM
    assert_eq!(find(r"(a(?=b)|)+?", "ab"), Some((0, 1)));
    // This tests the "prevent zero-length match on repeat" logic
    assert_eq!(find(r"(a(?=b)|)+?x", "ab"), None);
}

#[test]
fn any_match_unicode_scalar_value() {
    assert_eq!(find(r"(.)\1", "\u{1F60A}\u{1F60A}"), Some((0, 8)));
    assert_eq!(find(r"(?s)(.)\1", "\u{1F60A}\u{1F60A}"), Some((0, 8)));
}

#[test]
fn delegates_match_unicode_scalar_value() {
    assert_eq!(find(r".(?=a)", "\u{1F60A}a"), Some((0, 4)));
    assert_eq!(find(r".(?=\ba+)", "\u{1F60A}a"), Some((0, 4)));
}

#[test]
fn keepout_matches_in_correct_place() {
    assert_eq!(find(r"a\Kb", "aaab"), Some((3, 4)));
    assert_eq!(find(r".+\Kb", "aaab"), Some((3, 4)));
    assert_eq!(find(r"(?:aaa\K)b", "aaab"), Some((3, 4)));
}

#[test]
fn keepout_in_lookarounds_match_in_correct_place() {
    assert_eq!(find(r"(?<=a\Kb)c", "abc"), Some((1, 3)));
    assert_eq!(find(r"(?<!a\Kb)c", "axc"), Some((2, 3)));
    assert_eq!(find(r"a(?=b\Kc)", "abc"), Some((1, 1)));
    assert_eq!(find(r"a(?=b\Kc)..", "abc"), Some((2, 3)));
    assert_eq!(find(r"a(?!b\Kc)", "abx"), Some((0, 1)));
}

#[test]
fn find_no_matches_when_continuing_from_previous_match_end_and_no_match_at_start_of_text() {
    assert_eq!(find(r"\G(\d)\d", " 1122 33"), None);
}

#[test]
fn find_iter() {
    let text = "11 22 33";

    for (i, mat) in common::regex(r"(\d)\d").find_iter(text).enumerate() {
        let mat = mat.unwrap();

        match i {
            0 => assert_eq!((mat.start(), mat.end()), (0, 2)),
            1 => assert_eq!((mat.start(), mat.end()), (3, 5)),
            2 => assert_eq!((mat.start(), mat.end()), (6, 8)),
            i => panic!("Expected 3 captures, got {}", i + 1),
        }
    }
}

#[test]
fn find_iter_overlapping_lookahead() {
    let text = "abcdef";

    for (i, mat) in common::regex(r"[a-z]{2}(?=[a-z])")
        .find_iter(text)
        .enumerate()
    {
        let mat = mat.unwrap();

        match i {
            0 => assert_eq!((mat.start(), mat.end()), (0, 2)),
            1 => assert_eq!((mat.start(), mat.end()), (2, 4)),
            i => panic!("Expected 2 captures, got {}", i + 1),
        }
    }
}

#[test]
fn find_iter_zero_length() {
    let text = "ab1c2";

    for (i, mat) in common::regex(r"\d*(?=[a-z])").find_iter(text).enumerate() {
        let mat = mat.unwrap();

        match i {
            0 => assert_eq!((mat.start(), mat.end()), (0, 0)),
            1 => assert_eq!((mat.start(), mat.end()), (1, 1)),
            2 => assert_eq!((mat.start(), mat.end()), (2, 3)),
            i => panic!("Expected 3 captures, got {}", i + 1),
        }
    }
}

#[test]
fn find_iter_zero_length_longer_codepoint() {
    let text = "é1é";

    for (i, mat) in common::regex(r"\d*(?=é)").find_iter(text).enumerate() {
        let mat = mat.unwrap();

        match i {
            0 => assert_eq!((mat.start(), mat.end()), (0, 0)),
            1 => assert_eq!((mat.start(), mat.end()), (2, 3)),
            i => panic!("Expected 2 captures, got {}", i + 1),
        }
    }
}

#[test]
fn find_iter_continue_from_previous_match_end() {
    let text = "1122 33";

    for (i, mat) in common::regex(r"\G(\d)\d").find_iter(text).enumerate() {
        let mat = mat.unwrap();

        match i {
            0 => assert_eq!((mat.start(), mat.end()), (0, 2)),
            1 => assert_eq!((mat.start(), mat.end()), (2, 4)),
            i => panic!("Expected 2 results, got {}", i + 1),
        }
    }
}

#[test]
fn find_iter_continue_from_previous_match_end_with_zero_width_match() {
    let text = "1122 33";

    for (i, mat) in common::regex(r"\G\d*").find_iter(text).enumerate() {
        let mat = mat.unwrap();

        match i {
            0 => assert_eq!((mat.start(), mat.end()), (0, 4)),
            i => panic!("Expected 1 result, got {}", i + 1),
        }
    }
}

#[test]
fn find_iter_attributes() {
    let text = "ab1c2";
    let regex = common::regex(r"\d*(?=[a-z])");

    let matches = regex.find_iter(text);

    assert_eq!(matches.text(), text);
    assert_eq!(regex.as_str(), matches.regex().as_str());
}

#[test]
fn find_conditional() {
    assert_eq!(find(r"(?(ab)c|d)", "acd"), Some((2, 3)));
    assert_eq!(find(r"(a)?b(?(1)c|d)", "abc"), Some((0, 3)));
    assert_eq!(find(r"(a)?b(?(1)c|d)", "abd"), Some((1, 3)));
}

fn find(re: &str, text: &str) -> Option<(usize, usize)> {
    find_match(re, text).map(|m| (m.start(), m.end()))
}

fn find_match<'t>(re: &str, text: &'t str) -> Option<Match<'t>> {
    let regex = common::regex(re);
    let result = regex.find(text);
    assert!(
        result.is_ok(),
        "Expected find to succeed, but was {:?}",
        result
    );
    result.unwrap()
}

#[test]
fn incomplete_escape_sequences() {
    // See GH-76
    assert!(Regex::new("\\u").is_err());
    assert!(Regex::new("\\U").is_err());
    assert!(Regex::new("\\x").is_err());
}
