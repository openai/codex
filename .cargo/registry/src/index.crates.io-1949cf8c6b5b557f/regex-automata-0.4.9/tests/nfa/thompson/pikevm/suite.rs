use {
    anyhow::Result,
    regex_automata::{
        nfa::thompson::{
            self,
            pikevm::{self, PikeVM},
        },
        util::{prefilter::Prefilter, syntax},
        PatternSet,
    },
    regex_test::{
        CompiledRegex, Match, RegexTest, SearchKind, Span, TestResult,
        TestRunner,
    },
};

use crate::{create_input, suite, testify_captures, untestify_kind};

/// Tests the default configuration of the hybrid NFA/DFA.
#[test]
fn default() -> Result<()> {
    let builder = PikeVM::builder();
    let mut runner = TestRunner::new()?;
    runner.expand(&["is_match", "find", "captures"], |test| test.compiles());
    runner.test_iter(suite()?.iter(), compiler(builder)).assert();
    Ok(())
}

/// Tests the PikeVM with prefilters enabled.
#[test]
fn prefilter() -> Result<()> {
    let my_compiler = |test: &RegexTest, regexes: &[String]| {
        // Parse regexes as HIRs so we can get literals to build a prefilter.
        let mut hirs = vec![];
        for pattern in regexes.iter() {
            hirs.push(syntax::parse_with(pattern, &config_syntax(test))?);
        }
        let kind = match untestify_kind(test.match_kind()) {
            None => return Ok(CompiledRegex::skip()),
            Some(kind) => kind,
        };
        let pre = Prefilter::from_hirs_prefix(kind, &hirs);
        let mut builder = PikeVM::builder();
        builder.configure(PikeVM::config().prefilter(pre));
        compiler(builder)(test, regexes)
    };
    let mut runner = TestRunner::new()?;
    runner.expand(&["is_match", "find", "captures"], |test| test.compiles());
    runner.test_iter(suite()?.iter(), my_compiler).assert();
    Ok(())
}

fn compiler(
    mut builder: pikevm::Builder,
) -> impl FnMut(&RegexTest, &[String]) -> Result<CompiledRegex> {
    move |test, regexes| {
        if !configure_pikevm_builder(test, &mut builder) {
            return Ok(CompiledRegex::skip());
        }
        let re = builder.build_many(&regexes)?;
        let mut cache = re.create_cache();
        Ok(CompiledRegex::compiled(move |test| -> TestResult {
            run_test(&re, &mut cache, test)
        }))
    }
}

fn run_test(
    re: &PikeVM,
    cache: &mut pikevm::Cache,
    test: &RegexTest,
) -> TestResult {
    let input = create_input(test);
    match test.additional_name() {
        "is_match" => TestResult::matched(re.is_match(cache, input)),
        "find" => match test.search_kind() {
            SearchKind::Earliest => {
                let it = re
                    .find_iter(cache, input.earliest(true))
                    .take(test.match_limit().unwrap_or(std::usize::MAX))
                    .map(|m| Match {
                        id: m.pattern().as_usize(),
                        span: Span { start: m.start(), end: m.end() },
                    });
                TestResult::matches(it)
            }
            SearchKind::Leftmost => {
                let it = re
                    .find_iter(cache, input)
                    .take(test.match_limit().unwrap_or(std::usize::MAX))
                    .map(|m| Match {
                        id: m.pattern().as_usize(),
                        span: Span { start: m.start(), end: m.end() },
                    });
                TestResult::matches(it)
            }
            SearchKind::Overlapping => {
                let mut patset = PatternSet::new(re.get_nfa().pattern_len());
                re.which_overlapping_matches(cache, &input, &mut patset);
                TestResult::which(patset.iter().map(|p| p.as_usize()))
            }
        },
        "captures" => match test.search_kind() {
            SearchKind::Earliest => {
                let it = re
                    .captures_iter(cache, input.earliest(true))
                    .take(test.match_limit().unwrap_or(std::usize::MAX))
                    .map(|caps| testify_captures(&caps));
                TestResult::captures(it)
            }
            SearchKind::Leftmost => {
                let it = re
                    .captures_iter(cache, input)
                    .take(test.match_limit().unwrap_or(std::usize::MAX))
                    .map(|caps| testify_captures(&caps));
                TestResult::captures(it)
            }
            SearchKind::Overlapping => {
                // There is no overlapping PikeVM API that supports captures.
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
fn configure_pikevm_builder(
    test: &RegexTest,
    builder: &mut pikevm::Builder,
) -> bool {
    let match_kind = match untestify_kind(test.match_kind()) {
        None => return false,
        Some(k) => k,
    };
    let pikevm_config = PikeVM::config().match_kind(match_kind);
    builder
        .configure(pikevm_config)
        .syntax(config_syntax(test))
        .thompson(config_thompson(test));
    true
}

/// Configuration of a Thompson NFA compiler from a regex test.
fn config_thompson(test: &RegexTest) -> thompson::Config {
    let mut lookm = regex_automata::util::look::LookMatcher::new();
    lookm.set_line_terminator(test.line_terminator());
    thompson::Config::new().utf8(test.utf8()).look_matcher(lookm)
}

/// Configuration of the regex parser from a regex test.
fn config_syntax(test: &RegexTest) -> syntax::Config {
    syntax::Config::new()
        .case_insensitive(test.case_insensitive())
        .unicode(test.unicode())
        .utf8(test.utf8())
        .line_terminator(test.line_terminator())
}
