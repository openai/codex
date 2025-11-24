use {
    anyhow::Result,
    regex_automata::{
        dfa::onepass::{self, DFA},
        nfa::thompson,
        util::{iter, syntax},
    },
    regex_test::{
        CompiledRegex, Match, RegexTest, SearchKind, Span, TestResult,
        TestRunner,
    },
};

use crate::{create_input, suite, testify_captures, untestify_kind};

const EXPANSIONS: &[&str] = &["is_match", "find", "captures"];

/// Tests the default configuration of the hybrid NFA/DFA.
#[test]
fn default() -> Result<()> {
    let builder = DFA::builder();
    TestRunner::new()?
        .expand(EXPANSIONS, |t| t.compiles())
        .test_iter(suite()?.iter(), compiler(builder))
        .assert();
    Ok(())
}

/// Tests the hybrid NFA/DFA when 'starts_for_each_pattern' is enabled for all
/// tests.
#[test]
fn starts_for_each_pattern() -> Result<()> {
    let mut builder = DFA::builder();
    builder.configure(DFA::config().starts_for_each_pattern(true));
    TestRunner::new()?
        .expand(EXPANSIONS, |t| t.compiles())
        .test_iter(suite()?.iter(), compiler(builder))
        .assert();
    Ok(())
}

/// Tests the hybrid NFA/DFA when byte classes are disabled.
///
/// N.B. Disabling byte classes doesn't avoid any indirection at search time.
/// All it does is cause every byte value to be its own distinct equivalence
/// class.
#[test]
fn no_byte_classes() -> Result<()> {
    let mut builder = DFA::builder();
    builder.configure(DFA::config().byte_classes(false));
    TestRunner::new()?
        .expand(EXPANSIONS, |t| t.compiles())
        .test_iter(suite()?.iter(), compiler(builder))
        .assert();
    Ok(())
}

fn compiler(
    mut builder: onepass::Builder,
) -> impl FnMut(&RegexTest, &[String]) -> Result<CompiledRegex> {
    move |test, regexes| {
        // Check if our regex contains things that aren't supported by DFAs.
        // That is, Unicode word boundaries when searching non-ASCII text.
        if !configure_onepass_builder(test, &mut builder) {
            return Ok(CompiledRegex::skip());
        }
        let re = match builder.build_many(&regexes) {
            Ok(re) => re,
            Err(err) => {
                let msg = err.to_string();
                // This is pretty gross, but when a regex fails to compile as
                // a one-pass regex, then we want to be OK with that and just
                // skip the test. But we have to be careful to only skip it
                // when the expected result is that the regex compiles. If
                // the test is specifically checking that the regex does not
                // compile, then we should bubble up that error and allow the
                // test to pass.
                //
                // Since our error types are all generally opaque, we just
                // look for an error string. Not great, but not the end of the
                // world.
                if test.compiles() && msg.contains("not one-pass") {
                    return Ok(CompiledRegex::skip());
                }
                return Err(err.into());
            }
        };
        let mut cache = re.create_cache();
        Ok(CompiledRegex::compiled(move |test| -> TestResult {
            run_test(&re, &mut cache, test)
        }))
    }
}

fn run_test(
    re: &DFA,
    cache: &mut onepass::Cache,
    test: &RegexTest,
) -> TestResult {
    let input = create_input(test);
    match test.additional_name() {
        "is_match" => {
            TestResult::matched(re.is_match(cache, input.earliest(true)))
        }
        "find" => match test.search_kind() {
            SearchKind::Earliest | SearchKind::Leftmost => {
                let input =
                    input.earliest(test.search_kind() == SearchKind::Earliest);
                let mut caps = re.create_captures();
                let it = iter::Searcher::new(input)
                    .into_matches_iter(|input| {
                        re.try_search(cache, input, &mut caps)?;
                        Ok(caps.get_match())
                    })
                    .infallible()
                    .take(test.match_limit().unwrap_or(std::usize::MAX))
                    .map(|m| Match {
                        id: m.pattern().as_usize(),
                        span: Span { start: m.start(), end: m.end() },
                    });
                TestResult::matches(it)
            }
            SearchKind::Overlapping => {
                // The one-pass DFA does not support any kind of overlapping
                // search. This is not just a matter of not having the API.
                // It's fundamentally incompatible with the one-pass concept.
                // If overlapping matches were possible, then the one-pass DFA
                // would fail to build.
                TestResult::skip()
            }
        },
        "captures" => match test.search_kind() {
            SearchKind::Earliest | SearchKind::Leftmost => {
                let input =
                    input.earliest(test.search_kind() == SearchKind::Earliest);
                let it = iter::Searcher::new(input)
                    .into_captures_iter(re.create_captures(), |input, caps| {
                        re.try_search(cache, input, caps)
                    })
                    .infallible()
                    .take(test.match_limit().unwrap_or(std::usize::MAX))
                    .map(|caps| testify_captures(&caps));
                TestResult::captures(it)
            }
            SearchKind::Overlapping => {
                // The one-pass DFA does not support any kind of overlapping
                // search. This is not just a matter of not having the API.
                // It's fundamentally incompatible with the one-pass concept.
                // If overlapping matches were possible, then the one-pass DFA
                // would fail to build.
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
fn configure_onepass_builder(
    test: &RegexTest,
    builder: &mut onepass::Builder,
) -> bool {
    if !test.anchored() {
        return false;
    }
    let match_kind = match untestify_kind(test.match_kind()) {
        None => return false,
        Some(k) => k,
    };

    let config = DFA::config().match_kind(match_kind);
    builder
        .configure(config)
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
