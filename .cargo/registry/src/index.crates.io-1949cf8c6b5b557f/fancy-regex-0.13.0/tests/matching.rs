use fancy_regex::{Error, RegexBuilder, RuntimeError};

mod common;

#[test]
fn control_character_escapes() {
    assert_match(r"\a", "\x07");
    assert_match(r"\e", "\x1B");
    assert_match(r"\f", "\x0C");
    assert_match(r"\n", "\x0A");
    assert_match(r"\r", "\x0D");
    assert_match(r"\t", "\x09");
    assert_match(r"\v", "\x0B");
}

#[test]
fn character_class_escapes() {
    assert_match(r"[\[]", "[");
    assert_match(r"[\^]", "^");

    // The regex crate would reject the following because it's not necessary to escape them.
    // Other engines allow to escape any non-alphanumeric character.
    assert_match(r"[\<]", "<");
    assert_match(r"[\>]", ">");
    assert_match(r"[\.]", ".");
    assert_match(r"[\ ]", " ");

    // Character class escape
    assert_match(r"[\d]", "1");

    // Control characters
    assert_match(r"[\e]", "\x1B");
    assert_match(r"[\n]", "\x0A");

    // `]` can be unescaped if it's right after `[`
    assert_match(r"[]]", "]");
    // `]` can be unescaped even after `[^`
    assert_match(r"[^]]", "a");
}

#[test]
fn character_class_nested() {
    assert_match(r"[[a][bc]]", "c");
    assert_match(r"[a[^b]]", "c");
}

#[test]
fn character_class_intersection() {
    assert_match(r"[\w&&a-c]", "c");
    assert_no_match(r"[\w&&a-c]", "d");

    assert_match(r"[[0-9]&&[^4]]", "1");
    assert_no_match(r"[[0-9]&&[^4]]", "4");
}

#[test]
fn alternation_with_empty_arm() {
    assert_match(r"^(a|)$", "a");
    assert_match(r"^(a|)$", "");
    assert_match(r"^(|a)$", "a");
    assert_match(r"^(|a)$", "");
    assert_match(r"a|", "a");
    assert_match(r"a|", "");
    assert_match(r"|a", "a");
    assert_match(r"|a", "");
    assert_no_match(r"^(a|)$", "b");
}

#[test]
fn case_insensitive_character_class() {
    assert_match(r"^(?i)[a-z]+$", "aB");
}

#[test]
fn case_insensitive_escape() {
    // `\x61` is lowercase `a`
    assert_match(r"(?i)\x61", "A");

    // `\p{Ll}` is the "Letter, lowercase" category
    assert_match(r"(?i)\p{Ll}", "A");
}

#[test]
fn atomic_group() {
    assert_match(r"^a(?>bc|b)c$", "abcc");
    assert_no_match(r"^a(?>bc|b)c$", "abc");

    // Look-ahead forces use of VM
    assert_match(r"^a(bc(?=d)|b)cd$", "abcd");
    assert_no_match(r"^a(?>bc(?=d)|b)cd$", "abcd");
}

#[test]
fn backtrack_limit() {
    let re = RegexBuilder::new("(?i)(a|b|ab)*(?=c)")
        .backtrack_limit(100_000)
        .build()
        .unwrap();
    let s = "abababababababababababababababababababababababababababab";
    let result = re.is_match(s);
    assert!(result.is_err());
    match result.err() {
        Some(Error::RuntimeError(RuntimeError::BacktrackLimitExceeded)) => {}
        _ => panic!("Expected RuntimeError::BacktrackLimitExceeded"),
    }
}

#[test]
fn end_of_hard_expression_cannot_be_delegated() {
    assert_match(r"(?!x)(?:a|ab)c", "abc");
    // If `(?:a|ab)` is delegated, there's no backtracking and `a` matches and `ab` is never tried.
    assert_match(r"((?!x)(?:a|ab))c", "abc");
}

#[test]
fn issue103() {
    assert_no_match(r"(([ab]+)\1b)", "babab");
}

#[test]
fn backreference_validity_checker() {
    assert_match(r"(a)(?(1))", "a");
    assert_match(r"(?<group1>a)(?('group1'))b", "ab");
    assert_match(r"(a)(b)?(?(2))", "ab");
    assert_no_match(r"(a)(b)?(?(2))", "a");
    assert_match(r"(a)(b?)(?(2))", "a");
}

#[test]
fn conditional_with_backref_validity() {
    assert_match(r"(a)?b(?(1)c|d)", "bd");
    assert_match(r"(a)?b(?(1)c|d)", "abc");
    assert_match(r"(?<group1>a)?b(?(<group1>)c|d)", "abc");
    assert_no_match(r"(a)?b(?(1)c|d)", "bc");
    assert_no_match(r"^(a)?b(?(1)c|d)$", "abd");
}

#[test]
fn conditional_with_consuming_condition() {
    assert_match(r"^(?(ab)c|d)$", "abc");
    assert_no_match(r"^(?(ab)c|d)$", "abd");
    assert_no_match(r"^(?(ab)c|d)$", "ad");

    assert_match(r"^(?(\d)abc|\d!)$", "5abc");
    assert_no_match(r"^(?(\d)abc|\d!)$", "5!");
}

#[test]
fn conditional_with_lookaround_condition() {
    assert_match(r"^(?((?=\d))\w+|!)$", "!");
    assert_match(r"^(?((?=\d))\w+|!)$", "5abc");
    assert_match(r"^(?((?=\d))abc)$", "");
    assert_match(r"^(?((?=\w))abc)$", "abc");
    assert_no_match(r"^(?((?=\d))\wabc|\d!)$", "5!");
}

#[cfg_attr(feature = "track_caller", track_caller)]
fn assert_match(re: &str, text: &str) {
    let result = match_text(re, text);
    assert_eq!(
        result, true,
        "Expected regex '{}' to match text '{}'",
        re, text
    );
}

#[cfg_attr(feature = "track_caller", track_caller)]
fn assert_no_match(re: &str, text: &str) {
    let result = match_text(re, text);
    assert_eq!(
        result, false,
        "Expected regex '{}' to not match text '{}'",
        re, text
    );
}

#[cfg_attr(feature = "track_caller", track_caller)]
fn match_text(re: &str, text: &str) -> bool {
    let regex = common::regex(re);
    let result = regex.is_match(text);
    assert!(
        result.is_ok(),
        "Expected match to succeed, but was {:?}",
        result
    );
    result.unwrap()
}
