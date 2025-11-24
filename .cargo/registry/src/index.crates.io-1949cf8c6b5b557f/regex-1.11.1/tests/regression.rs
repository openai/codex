use regex::Regex;

macro_rules! regex {
    ($pattern:expr) => {
        regex::Regex::new($pattern).unwrap()
    };
}

// See: https://github.com/rust-lang/regex/issues/48
#[test]
fn invalid_regexes_no_crash() {
    assert!(Regex::new("(*)").is_err());
    assert!(Regex::new("(?:?)").is_err());
    assert!(Regex::new("(?)").is_err());
    assert!(Regex::new("*").is_err());
}

// See: https://github.com/rust-lang/regex/issues/98
#[test]
fn regression_many_repeat_stack_overflow() {
    let re = regex!("^.{1,2500}");
    assert_eq!(
        vec![0..1],
        re.find_iter("a").map(|m| m.range()).collect::<Vec<_>>()
    );
}

// See: https://github.com/rust-lang/regex/issues/555
#[test]
fn regression_invalid_repetition_expr() {
    assert!(Regex::new("(?m){1,1}").is_err());
}

// See: https://github.com/rust-lang/regex/issues/527
#[test]
fn regression_invalid_flags_expression() {
    assert!(Regex::new("(((?x)))").is_ok());
}

// See: https://github.com/rust-lang/regex/issues/129
#[test]
fn regression_captures_rep() {
    let re = regex!(r"([a-f]){2}(?P<foo>[x-z])");
    let caps = re.captures("abx").unwrap();
    assert_eq!(&caps["foo"], "x");
}

// See: https://github.com/BurntSushi/ripgrep/issues/1247
#[cfg(feature = "unicode-perl")]
#[test]
fn regression_nfa_stops1() {
    let re = regex::bytes::Regex::new(r"\bs(?:[ab])").unwrap();
    assert_eq!(0, re.find_iter(b"s\xE4").count());
}

// See: https://github.com/rust-lang/regex/issues/981
#[cfg(feature = "unicode")]
#[test]
fn regression_bad_word_boundary() {
    let re = regex!(r#"(?i:(?:\b|_)win(?:32|64|dows)?(?:\b|_))"#);
    let hay = "ubi-Darwin-x86_64.tar.gz";
    assert!(!re.is_match(hay));
    let hay = "ubi-Windows-x86_64.zip";
    assert!(re.is_match(hay));
}

// See: https://github.com/rust-lang/regex/issues/982
#[cfg(feature = "unicode-perl")]
#[test]
fn regression_unicode_perl_not_enabled() {
    let pat = r"(\d+\s?(years|year|y))?\s?(\d+\s?(months|month|m))?\s?(\d+\s?(weeks|week|w))?\s?(\d+\s?(days|day|d))?\s?(\d+\s?(hours|hour|h))?";
    assert!(Regex::new(pat).is_ok());
}

// See: https://github.com/rust-lang/regex/issues/995
#[test]
fn regression_big_regex_overflow() {
    let pat = r" {2147483516}{2147483416}{5}";
    assert!(Regex::new(pat).is_err());
}

// See: https://github.com/rust-lang/regex/issues/999
#[test]
fn regression_complete_literals_suffix_incorrect() {
    let needles = vec![
        "aA", "bA", "cA", "dA", "eA", "fA", "gA", "hA", "iA", "jA", "kA",
        "lA", "mA", "nA", "oA", "pA", "qA", "rA", "sA", "tA", "uA", "vA",
        "wA", "xA", "yA", "zA",
    ];
    let pattern = needles.join("|");
    let re = regex!(&pattern);
    let hay = "FUBAR";
    assert_eq!(0, re.find_iter(hay).count());
}
