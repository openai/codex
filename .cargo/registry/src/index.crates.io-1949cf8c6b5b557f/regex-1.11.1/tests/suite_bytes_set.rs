use {
    anyhow::Result,
    regex::bytes::{RegexSet, RegexSetBuilder},
    regex_test::{CompiledRegex, RegexTest, TestResult, TestRunner},
};

/// Tests the default configuration of the hybrid NFA/DFA.
#[test]
fn default() -> Result<()> {
    let mut runner = TestRunner::new()?;
    runner
        .expand(&["is_match", "which"], |test| test.compiles())
        .blacklist_iter(super::BLACKLIST)
        .test_iter(crate::suite()?.iter(), compiler)
        .assert();
    Ok(())
}

fn run_test(re: &RegexSet, test: &RegexTest) -> TestResult {
    match test.additional_name() {
        "is_match" => TestResult::matched(re.is_match(test.haystack())),
        "which" => TestResult::which(re.matches(test.haystack()).iter()),
        name => TestResult::fail(&format!("unrecognized test name: {}", name)),
    }
}

/// Converts the given regex test to a closure that searches with a
/// `bytes::Regex`. If the test configuration is unsupported, then a
/// `CompiledRegex` that skips the test is returned.
fn compiler(
    test: &RegexTest,
    _patterns: &[String],
) -> anyhow::Result<CompiledRegex> {
    let skip = Ok(CompiledRegex::skip());

    // The top-level RegexSet API only supports "overlapping" semantics.
    if !matches!(test.search_kind(), regex_test::SearchKind::Overlapping) {
        return skip;
    }
    // The top-level RegexSet API only supports "all" semantics.
    if !matches!(test.match_kind(), regex_test::MatchKind::All) {
        return skip;
    }
    // The top-level RegexSet API always runs unanchored searches.
    if test.anchored() {
        return skip;
    }
    // We don't support tests with explicit search bounds.
    let bounds = test.bounds();
    if !(bounds.start == 0 && bounds.end == test.haystack().len()) {
        return skip;
    }
    // The bytes::Regex API specifically does not support enabling UTF-8 mode.
    // It could I suppose, but currently it does not. That is, it permits
    // matches to have offsets that split codepoints.
    if test.utf8() {
        return skip;
    }
    // If the test requires Unicode but the Unicode feature isn't enabled,
    // skip it. This is a little aggressive, but the test suite doesn't
    // have any easy way of communicating which Unicode features are needed.
    if test.unicode() && !cfg!(feature = "unicode") {
        return skip;
    }
    let re = RegexSetBuilder::new(test.regexes())
        .case_insensitive(test.case_insensitive())
        .unicode(test.unicode())
        .line_terminator(test.line_terminator())
        .build()?;
    Ok(CompiledRegex::compiled(move |test| run_test(&re, test)))
}
