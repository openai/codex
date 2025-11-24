use crate::pattern::{Atom, AtomKind, CaseMatching, Normalization};

#[test]
fn negative() {
    let pat = Atom::parse("!foo", CaseMatching::Smart, Normalization::Smart);
    assert!(pat.negative);
    assert_eq!(pat.kind, AtomKind::Substring);
    assert_eq!(pat.needle.to_string(), "foo");
    let pat = Atom::parse("!^foo", CaseMatching::Smart, Normalization::Smart);
    assert!(pat.negative);
    assert_eq!(pat.kind, AtomKind::Prefix);
    assert_eq!(pat.needle.to_string(), "foo");
    let pat = Atom::parse("!foo$", CaseMatching::Smart, Normalization::Smart);
    assert!(pat.negative);
    assert_eq!(pat.kind, AtomKind::Postfix);
    assert_eq!(pat.needle.to_string(), "foo");
    let pat = Atom::parse("!^foo$", CaseMatching::Smart, Normalization::Smart);
    assert!(pat.negative);
    assert_eq!(pat.kind, AtomKind::Exact);
    assert_eq!(pat.needle.to_string(), "foo");
}

#[test]
fn pattern_kinds() {
    let pat = Atom::parse("foo", CaseMatching::Smart, Normalization::Smart);
    assert!(!pat.negative);
    assert_eq!(pat.kind, AtomKind::Fuzzy);
    assert_eq!(pat.needle.to_string(), "foo");
    let pat = Atom::parse("'foo", CaseMatching::Smart, Normalization::Smart);
    assert!(!pat.negative);
    assert_eq!(pat.kind, AtomKind::Substring);
    assert_eq!(pat.needle.to_string(), "foo");
    let pat = Atom::parse("^foo", CaseMatching::Smart, Normalization::Smart);
    assert!(!pat.negative);
    assert_eq!(pat.kind, AtomKind::Prefix);
    assert_eq!(pat.needle.to_string(), "foo");
    let pat = Atom::parse("foo$", CaseMatching::Smart, Normalization::Smart);
    assert!(!pat.negative);
    assert_eq!(pat.kind, AtomKind::Postfix);
    assert_eq!(pat.needle.to_string(), "foo");
    let pat = Atom::parse("^foo$", CaseMatching::Smart, Normalization::Smart);
    assert!(!pat.negative);
    assert_eq!(pat.kind, AtomKind::Exact);
    assert_eq!(pat.needle.to_string(), "foo");
}

#[test]
fn case_matching() {
    let pat = Atom::parse("foo", CaseMatching::Smart, Normalization::Smart);
    assert!(pat.ignore_case);
    assert_eq!(pat.needle.to_string(), "foo");
    let pat = Atom::parse("Foo", CaseMatching::Smart, Normalization::Smart);
    assert!(!pat.ignore_case);
    assert_eq!(pat.needle.to_string(), "Foo");
    let pat = Atom::parse("Foo", CaseMatching::Ignore, Normalization::Smart);
    assert!(pat.ignore_case);
    assert_eq!(pat.needle.to_string(), "foo");
    let pat = Atom::parse("Foo", CaseMatching::Respect, Normalization::Smart);
    assert!(!pat.ignore_case);
    assert_eq!(pat.needle.to_string(), "Foo");
    let pat = Atom::parse("Foo", CaseMatching::Respect, Normalization::Smart);
    assert!(!pat.ignore_case);
    assert_eq!(pat.needle.to_string(), "Foo");
    let pat = Atom::parse("Äxx", CaseMatching::Ignore, Normalization::Smart);
    assert!(pat.ignore_case);
    assert_eq!(pat.needle.to_string(), "äxx");
    let pat = Atom::parse("Äxx", CaseMatching::Respect, Normalization::Smart);
    assert!(!pat.ignore_case);
    let pat = Atom::parse("Axx", CaseMatching::Smart, Normalization::Smart);
    assert!(!pat.ignore_case);
    assert_eq!(pat.needle.to_string(), "Axx");
    let pat = Atom::parse("你xx", CaseMatching::Smart, Normalization::Smart);
    assert!(pat.ignore_case);
    assert_eq!(pat.needle.to_string(), "你xx");
    let pat = Atom::parse("你xx", CaseMatching::Ignore, Normalization::Smart);
    assert!(pat.ignore_case);
    assert_eq!(pat.needle.to_string(), "你xx");
    let pat = Atom::parse("Ⲽxx", CaseMatching::Smart, Normalization::Smart);
    assert!(!pat.ignore_case);
    assert_eq!(pat.needle.to_string(), "Ⲽxx");
    let pat = Atom::parse("Ⲽxx", CaseMatching::Ignore, Normalization::Smart);
    assert!(pat.ignore_case);
    assert_eq!(pat.needle.to_string(), "ⲽxx");
}

#[test]
fn escape() {
    let pat = Atom::parse("foo\\ bar", CaseMatching::Smart, Normalization::Smart);
    assert_eq!(pat.needle.to_string(), "foo bar");
    let pat = Atom::parse("\\!foo", CaseMatching::Smart, Normalization::Smart);
    assert_eq!(pat.needle.to_string(), "!foo");
    assert_eq!(pat.kind, AtomKind::Fuzzy);
    let pat = Atom::parse("\\'foo", CaseMatching::Smart, Normalization::Smart);
    assert_eq!(pat.needle.to_string(), "'foo");
    assert_eq!(pat.kind, AtomKind::Fuzzy);
    let pat = Atom::parse("\\^foo", CaseMatching::Smart, Normalization::Smart);
    assert_eq!(pat.needle.to_string(), "^foo");
    assert_eq!(pat.kind, AtomKind::Fuzzy);
    let pat = Atom::parse("foo\\$", CaseMatching::Smart, Normalization::Smart);
    assert_eq!(pat.needle.to_string(), "foo$");
    assert_eq!(pat.kind, AtomKind::Fuzzy);
    let pat = Atom::parse("^foo\\$", CaseMatching::Smart, Normalization::Smart);
    assert_eq!(pat.needle.to_string(), "foo$");
    assert_eq!(pat.kind, AtomKind::Prefix);
    let pat = Atom::parse("\\^foo\\$", CaseMatching::Smart, Normalization::Smart);
    assert_eq!(pat.needle.to_string(), "^foo$");
    assert_eq!(pat.kind, AtomKind::Fuzzy);
    let pat = Atom::parse("\\!^foo\\$", CaseMatching::Smart, Normalization::Smart);
    assert_eq!(pat.needle.to_string(), "!^foo$");
    assert_eq!(pat.kind, AtomKind::Fuzzy);
    let pat = Atom::parse("!\\^foo\\$", CaseMatching::Smart, Normalization::Smart);
    assert_eq!(pat.needle.to_string(), "^foo$");
    assert_eq!(pat.kind, AtomKind::Substring);
}
