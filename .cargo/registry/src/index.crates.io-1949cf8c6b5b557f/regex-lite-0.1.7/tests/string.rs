use {
    anyhow::Result,
    regex_lite::{Regex, RegexBuilder},
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
    let Some(pattern) = skip_or_get_pattern(test) else {
        return Ok(CompiledRegex::skip());
    };
    let re = RegexBuilder::new(pattern)
        .case_insensitive(test.case_insensitive())
        .build()?;
    Ok(CompiledRegex::compiled(move |test| run_test(&re, test)))
}

/// Whether we should skip the given test or not. If not, return the single
/// pattern from the given test.
fn skip_or_get_pattern(test: &RegexTest) -> Option<&str> {
    // We're only testing Regex here, which supports one pattern only.
    let pattern = match test.regexes().len() {
        1 => &test.regexes()[0],
        _ => return None,
    };
    // If the test name contains 'regex-lite', then we ALWAYS run it. Because
    // those tests are specifically designed for regex-lite. So if they fail,
    // then something needs attention.
    if test.full_name().contains("regex-lite/") {
        return Some(pattern);
    }
    // If the pattern has a \p in it, then we almost certainly don't support
    // it. This probably skips more than we intend, but there are likely very
    // few tests that contain a \p that isn't also a Unicode class.
    if pattern.contains(r"\p") || pattern.contains(r"\P") {
        return None;
    }
    // Similar deal for Perl classes, but we can abide them if the haystack
    // is ASCII-only.
    if !test.haystack().is_ascii() {
        if pattern.contains(r"\d") || pattern.contains(r"\D") {
            return None;
        }
        if pattern.contains(r"\s") || pattern.contains(r"\S") {
            return None;
        }
        if pattern.contains(r"\w") || pattern.contains(r"\W") {
            return None;
        }
    }
    // And also same deal for word boundaries.
    if !test.haystack().is_ascii() {
        if pattern.contains(r"\b") || pattern.contains(r"\B") {
            return None;
        }
    }
    // We only test is_match, find_iter and captures_iter. All of those are
    // leftmost searches.
    if !matches!(test.search_kind(), regex_test::SearchKind::Leftmost) {
        return None;
    }
    // The top-level single-pattern regex API always uses leftmost-first.
    if !matches!(test.match_kind(), regex_test::MatchKind::LeftmostFirst) {
        return None;
    }
    // The top-level regex API always runs unanchored searches. ... But we can
    // handle tests that are anchored but have only one match.
    if test.anchored() && test.match_limit() != Some(1) {
        return None;
    }
    // We don't support tests with explicit search bounds. We could probably
    // support this by using the 'find_at' (and such) APIs.
    let bounds = test.bounds();
    if !(bounds.start == 0 && bounds.end == test.haystack().len()) {
        return None;
    }
    // The Regex API specifically does not support disabling UTF-8 mode because
    // it can only search &str which is always valid UTF-8.
    if !test.utf8() {
        return None;
    }
    // regex-lite doesn't support Unicode-aware case insensitive matching.
    if test.case_insensitive()
        && (!pattern.is_ascii() || !test.haystack().is_ascii())
    {
        return None;
    }
    Some(pattern)
}

/// Convert `Captures` into the test suite's capture values.
fn testify_captures(caps: &regex_lite::Captures<'_>) -> regex_test::Captures {
    let spans = caps.iter().map(|group| {
        group.map(|m| regex_test::Span { start: m.start(), end: m.end() })
    });
    // This unwrap is OK because we assume our 'caps' represents a match, and
    // a match always gives a non-zero number of groups with the first group
    // being non-None.
    regex_test::Captures::new(0, spans).unwrap()
}
