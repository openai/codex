use fancy_regex::{Captures, NoExpand};
use std::borrow::Cow;

mod common;

#[test]
fn replacer_string() {
    let regex = common::regex(
        r"\b([sS])uc(?:cs|s?)e(ed(?:ed|ing|s?)|ss(?:es|ful(?:ly)?|i(?:ons?|ve(?:ly)?)|ors?)?)\b",
    );

    // Replacer impl for &str
    let result = regex.replace("a sucessful b", "${1}ucce$2");
    assert_eq!(result, "a successful b");

    // Replacer impl for &String
    let repl_string = "${1}ucce$2".to_string();
    let result = regex.replace("a Suceeded b", &repl_string);
    assert_eq!(result, "a Succeeded b");

    // Replacer impl for String
    let result = regex.replace("a sucessor b", repl_string);
    assert_eq!(result, "a successor b");
}

#[test]
fn replacer_cow() {
    let regex = common::regex(r"\b([oO])mmi(?=t)t?(t(?:ed|ing)|s)\b");

    // Replacer impl for &Cow<str>
    let result = regex.replace("a ommiting b", &Cow::from("${1}mit$2"));
    assert_eq!(result, "a omitting b");

    // Replacer for Cow<str>::Borrowed
    let result = regex.replace("a ommited b", Cow::Borrowed("${1}mit$2"));
    assert_eq!(result, "a omitted b");

    // Replacer for Cow<str>::Owned
    let result = regex.replace("a Ommits b", Cow::Owned("${1}mit$2".to_string()));
    assert_eq!(result, "a Omits b");
}

#[test]
fn replacer_noexpand() {
    let regex = common::regex(r"\b([aA])n+ull(ar|ments?|s?)\b");

    // Replacer impl for NoExpand
    let result = regex.replace("a anullment b", NoExpand("${1}nnul$2"));
    assert_eq!(result, "a ${1}nnul$2 b");
}

#[test]
fn replacer_callback() {
    let regex = common::regex(r"\b([aA])p(?:p[or]|ro)x\.?(?=[ \)\n])");

    // Replacer impl for FnMut(&Captures)
    let result = regex.replace("a Aprox b", |cap: &Captures| {
        format!("{}pprox.", cap.get(1).unwrap().as_str())
    });
    assert_eq!(result, "a Approx. b");
}

/// `replace()` does only one replacement
#[test]
fn replace_one() {
    let regex = common::regex("bla");
    assert_eq!(regex.replace("blabla", "foo"), "foobla");
}

/// `replace_all()` replaces all non-overlapping matches
#[test]
fn replace_all() {
    let regex = common::regex("aa");
    assert_eq!(regex.replace_all("aaaa aaa aa a", "xx"), "xxxx xxa xx a");
}

/// `replacen()` replaces predefined number of times
#[test]
fn replacen() {
    let regex = common::regex("bla");
    assert_eq!(regex.replacen("blablabla", 2, "foo"), "foofoobla");
}
