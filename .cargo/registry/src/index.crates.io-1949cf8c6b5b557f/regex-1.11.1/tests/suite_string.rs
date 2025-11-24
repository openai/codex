use {
    anyhow::Result,
    regex::{Regex, RegexBuilder},
    regex_test::{
        CompiledRegex, Match, RegexTest, Span, TestResult, TestRunner,
    },
};

/// Tests the default configuration of the hybrid NFA/DFA.
#[test]
fn default() -> Result<()> {
    let mut runner = TestRunner::new()?;
    runner
        .expand(&["is_match", "find", "captures"], |test| test.compiles())
        .blacklist_iter(super::BLACKLIST)
        .test_iter(crate::suite()?.iter(), compiler)
        .assert();
    Ok(())
}

fn run_test(re: &Regex, test: &RegexTest) -> TestResult {
    let hay = match std::str::from_utf8(test.haystack()) {
        Ok(hay) => hay,
        Err(err) => {
            return TestResult::fail(&format!(
                "haystack is not valid UTF-8: {}",
                err
            ));
        }
    };
    match test.additional_name() {
        "is_match" => TestResult::matched(re.is_match(hay)),
        "find" => TestResult::matches(
            re.find_iter(hay)
                .take(test.match_limit().unwrap_or(std::usize::MAX))
                .map(|m| Match {
                    id: 0,
                    span: Span { start: m.start(), end: m.end() },
                }),
        ),
        "captures" => {
            let it = re
                .captures_iter(hay)
                .take(test.match_limit().unwrap_or(std::usize::MAX))
                .map(|caps| testify_captures(&caps));
            TestResult::captures(it)
        }
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

    // We're only testing bytes::Regex here, which supports one pattern only.
    let pattern = match test.regexes().len() {
        1 => &test.regexes()[0],
        _ => return skip,
    };
    // We only test is_match, find_iter and captures_iter. All of those are
    // leftmost searches.
    if !matches!(test.search_kind(), regex_test::SearchKind::Leftmost) {
        return skip;
    }
    // The top-level single-pattern regex API always uses leftmost-first.
    if !matches!(test.match_kind(), regex_test::MatchKind::LeftmostFirst) {
        return skip;
    }
    // The top-level regex API always runs unanchored searches. ... But we can
    // handle tests that are anchored but have only one match.
    if test.anchored() && test.match_limit() != Some(1) {
        return skip;
    }
    // We don't support tests with explicit search bounds. We could probably
    // support this by using the 'find_at' (and such) APIs.
    let bounds = test.bounds();
    if !(bounds.start == 0 && bounds.end == test.haystack().len()) {
        return skip;
    }
    // The Regex API specifically does not support disabling UTF-8 mode because
    // it can only search &str which is always valid UTF-8.
    if !test.utf8() {
        return skip;
    }
    // If the test requires Unicode but the Unicode feature isn't enabled,
    // skip it. This is a little aggressive, but the test suite doesn't
    // have any easy way of communicating which Unicode features are needed.
    if test.unicode() && !cfg!(feature = "unicode") {
        return skip;
    }
    let re = RegexBuilder::new(pattern)
        .case_insensitive(test.case_insensitive())
        .unicode(test.unicode())
        .line_terminator(test.line_terminator())
        .build()?;
    Ok(CompiledRegex::compiled(move |test| run_test(&re, test)))
}

/// Convert `Captures` into the test suite's capture values.
fn testify_captures(caps: &regex::Captures<'_>) -> regex_test::Captures {
    let spans = caps.iter().map(|group| {
        group.map(|m| regex_test::Span { start: m.start(), end: m.end() })
    });
    // This unwrap is OK because we assume our 'caps' represents a match, and
    // a match always gives a non-zero number of groups with the first group
    // being non-None.
    regex_test::Captures::new(0, spans).unwrap()
}
