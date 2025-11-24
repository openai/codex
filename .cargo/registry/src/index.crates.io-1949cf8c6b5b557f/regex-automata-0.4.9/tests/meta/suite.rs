use {
    anyhow::Result,
    regex_automata::{
        meta::{self, Regex},
        util::syntax,
        MatchKind, PatternSet,
    },
    regex_test::{
        CompiledRegex, Match, RegexTest, SearchKind, Span, TestResult,
        TestRunner,
    },
};

use crate::{create_input, suite, testify_captures};

const BLACKLIST: &[&str] = &[
    // These 'earliest' tests are blacklisted because the meta searcher doesn't
    // give the same offsets that the test expects. This is legal because the
    // 'earliest' routines don't guarantee a particular match offset other
    // than "the earliest the regex engine can report a match." Some regex
    // engines will quit earlier than others. The backtracker, for example,
    // can't really quit before finding the full leftmost-first match. Many of
    // the literal searchers also don't have the ability to quit fully or it's
    // otherwise not worth doing. (A literal searcher not quitting as early as
    // possible usually means looking at a few more bytes. That's no biggie.)
    "earliest/",
];

/// Tests the default configuration of the meta regex engine.
#[test]
fn default() -> Result<()> {
    let builder = Regex::builder();
    let mut runner = TestRunner::new()?;
    runner
        .expand(&["is_match", "find", "captures"], |test| test.compiles())
        .blacklist_iter(BLACKLIST)
        .test_iter(suite()?.iter(), compiler(builder))
        .assert();
    Ok(())
}

/// Tests the default configuration minus the full DFA.
#[test]
fn no_dfa() -> Result<()> {
    let mut builder = Regex::builder();
    builder.configure(Regex::config().dfa(false));
    let mut runner = TestRunner::new()?;
    runner
        .expand(&["is_match", "find", "captures"], |test| test.compiles())
        .blacklist_iter(BLACKLIST)
        .test_iter(suite()?.iter(), compiler(builder))
        .assert();
    Ok(())
}

/// Tests the default configuration minus the full DFA and lazy DFA.
#[test]
fn no_dfa_hybrid() -> Result<()> {
    let mut builder = Regex::builder();
    builder.configure(Regex::config().dfa(false).hybrid(false));
    let mut runner = TestRunner::new()?;
    runner
        .expand(&["is_match", "find", "captures"], |test| test.compiles())
        .blacklist_iter(BLACKLIST)
        .test_iter(suite()?.iter(), compiler(builder))
        .assert();
    Ok(())
}

/// Tests the default configuration minus the full DFA, lazy DFA and one-pass
/// DFA.
#[test]
fn no_dfa_hybrid_onepass() -> Result<()> {
    let mut builder = Regex::builder();
    builder.configure(Regex::config().dfa(false).hybrid(false).onepass(false));
    let mut runner = TestRunner::new()?;
    runner
        .expand(&["is_match", "find", "captures"], |test| test.compiles())
        .blacklist_iter(BLACKLIST)
        .test_iter(suite()?.iter(), compiler(builder))
        .assert();
    Ok(())
}

/// Tests the default configuration minus the full DFA, lazy DFA, one-pass
/// DFA and backtracker.
#[test]
fn no_dfa_hybrid_onepass_backtrack() -> Result<()> {
    let mut builder = Regex::builder();
    builder.configure(
        Regex::config()
            .dfa(false)
            .hybrid(false)
            .onepass(false)
            .backtrack(false),
    );
    let mut runner = TestRunner::new()?;
    runner
        .expand(&["is_match", "find", "captures"], |test| test.compiles())
        .blacklist_iter(BLACKLIST)
        .test_iter(suite()?.iter(), compiler(builder))
        .assert();
    Ok(())
}

fn compiler(
    mut builder: meta::Builder,
) -> impl FnMut(&RegexTest, &[String]) -> Result<CompiledRegex> {
    move |test, regexes| {
        if !configure_meta_builder(test, &mut builder) {
            return Ok(CompiledRegex::skip());
        }
        let re = builder.build_many(&regexes)?;
        Ok(CompiledRegex::compiled(move |test| -> TestResult {
            run_test(&re, test)
        }))
    }
}

fn run_test(re: &Regex, test: &RegexTest) -> TestResult {
    let input = create_input(test);
    match test.additional_name() {
        "is_match" => TestResult::matched(re.is_match(input)),
        "find" => match test.search_kind() {
            SearchKind::Earliest => TestResult::matches(
                re.find_iter(input.earliest(true))
                    .take(test.match_limit().unwrap_or(std::usize::MAX))
                    .map(|m| Match {
                        id: m.pattern().as_usize(),
                        span: Span { start: m.start(), end: m.end() },
                    }),
            ),
            SearchKind::Leftmost => TestResult::matches(
                re.find_iter(input)
                    .take(test.match_limit().unwrap_or(std::usize::MAX))
                    .map(|m| Match {
                        id: m.pattern().as_usize(),
                        span: Span { start: m.start(), end: m.end() },
                    }),
            ),
            SearchKind::Overlapping => {
                let mut patset = PatternSet::new(re.pattern_len());
                re.which_overlapping_matches(&input, &mut patset);
                TestResult::which(patset.iter().map(|p| p.as_usize()))
            }
        },
        "captures" => match test.search_kind() {
            SearchKind::Earliest => {
                let it = re
                    .captures_iter(input.earliest(true))
                    .take(test.match_limit().unwrap_or(std::usize::MAX))
                    .map(|caps| testify_captures(&caps));
                TestResult::captures(it)
            }
            SearchKind::Leftmost => {
                let it = re
                    .captures_iter(input)
                    .take(test.match_limit().unwrap_or(std::usize::MAX))
                    .map(|caps| testify_captures(&caps));
                TestResult::captures(it)
            }
            SearchKind::Overlapping => {
                // There is no overlapping regex API that supports captures.
                TestResult::skip()
            }
        },
        name => TestResult::fail(&format!("unrecognized test name: {}", name)),
    }
}

/// Configures the given regex builder with all relevant settings on the given
/// regex test.
///
/// If the regex test has a setting that is unsupported, then this returns
/// false (implying the test should be skipped).
fn configure_meta_builder(
    test: &RegexTest,
    builder: &mut meta::Builder,
) -> bool {
    let match_kind = match test.match_kind() {
        regex_test::MatchKind::All => MatchKind::All,
        regex_test::MatchKind::LeftmostFirst => MatchKind::LeftmostFirst,
        regex_test::MatchKind::LeftmostLongest => return false,
    };
    let meta_config = Regex::config()
        .match_kind(match_kind)
        .utf8_empty(test.utf8())
        .line_terminator(test.line_terminator());
    builder.configure(meta_config).syntax(config_syntax(test));
    true
}

/// Configuration of the regex parser from a regex test.
fn config_syntax(test: &RegexTest) -> syntax::Config {
    syntax::Config::new()
        .case_insensitive(test.case_insensitive())
        .unicode(test.unicode())
        .utf8(test.utf8())
        .line_terminator(test.line_terminator())
}
