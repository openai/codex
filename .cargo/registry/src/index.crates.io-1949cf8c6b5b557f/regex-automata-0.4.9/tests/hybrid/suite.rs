use {
    anyhow::Result,
    regex_automata::{
        hybrid::{
            dfa::{OverlappingState, DFA},
            regex::{self, Regex},
        },
        nfa::thompson,
        util::{prefilter::Prefilter, syntax},
        Anchored, Input, PatternSet,
    },
    regex_test::{
        CompiledRegex, Match, RegexTest, SearchKind, Span, TestResult,
        TestRunner,
    },
};

use crate::{create_input, suite, untestify_kind};

const EXPANSIONS: &[&str] = &["is_match", "find", "which"];

/// Tests the default configuration of the hybrid NFA/DFA.
#[test]
fn default() -> Result<()> {
    let builder = Regex::builder();
    TestRunner::new()?
        .expand(EXPANSIONS, |t| t.compiles())
        // Without NFA shrinking, this test blows the default cache capacity.
        .blacklist("expensive/regression-many-repeat-no-stack-overflow")
        .test_iter(suite()?.iter(), compiler(builder))
        .assert();
    Ok(())
}

/// Tests the hybrid NFA/DFA with prefilters enabled.
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
        let mut builder = Regex::builder();
        builder.dfa(DFA::config().prefilter(pre));
        compiler(builder)(test, regexes)
    };
    TestRunner::new()?
        .expand(EXPANSIONS, |t| t.compiles())
        // Without NFA shrinking, this test blows the default cache capacity.
        .blacklist("expensive/regression-many-repeat-no-stack-overflow")
        .test_iter(suite()?.iter(), my_compiler)
        .assert();
    Ok(())
}

/// Tests the hybrid NFA/DFA with NFA shrinking enabled.
///
/// This is *usually* not the configuration one wants for a lazy DFA. NFA
/// shrinking is mostly only advantageous when building a full DFA since it
/// can sharply decrease the amount of time determinization takes. But NFA
/// shrinking is itself otherwise fairly expensive currently. Since a lazy DFA
/// has no compilation time (other than for building the NFA of course) before
/// executing a search, it's usually worth it to forgo NFA shrinking.
///
/// Nevertheless, we test to make sure everything is OK with NFA shrinking. As
/// a bonus, there are some tests we don't need to skip because they now fit in
/// the default cache capacity.
#[test]
fn nfa_shrink() -> Result<()> {
    let mut builder = Regex::builder();
    builder.thompson(thompson::Config::new().shrink(true));
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
    let mut builder = Regex::builder();
    builder.dfa(DFA::config().starts_for_each_pattern(true));
    TestRunner::new()?
        .expand(EXPANSIONS, |t| t.compiles())
        // Without NFA shrinking, this test blows the default cache capacity.
        .blacklist("expensive/regression-many-repeat-no-stack-overflow")
        .test_iter(suite()?.iter(), compiler(builder))
        .assert();
    Ok(())
}

/// Tests the hybrid NFA/DFA when 'specialize_start_states' is enabled.
#[test]
fn specialize_start_states() -> Result<()> {
    let mut builder = Regex::builder();
    builder.dfa(DFA::config().specialize_start_states(true));
    TestRunner::new()?
        .expand(EXPANSIONS, |t| t.compiles())
        // Without NFA shrinking, this test blows the default cache capacity.
        .blacklist("expensive/regression-many-repeat-no-stack-overflow")
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
    let mut builder = Regex::builder();
    builder.dfa(DFA::config().byte_classes(false));
    TestRunner::new()?
        .expand(EXPANSIONS, |t| t.compiles())
        // Without NFA shrinking, this test blows the default cache capacity.
        .blacklist("expensive/regression-many-repeat-no-stack-overflow")
        .test_iter(suite()?.iter(), compiler(builder))
        .assert();
    Ok(())
}

/// Tests that hybrid NFA/DFA never clears its cache for any test with the
/// default capacity.
///
/// N.B. If a regex suite test is added that causes the cache to be cleared,
/// then this should just skip that test. (Which can be done by calling the
/// 'blacklist' method on 'TestRunner'.)
#[test]
fn no_cache_clearing() -> Result<()> {
    let mut builder = Regex::builder();
    builder.dfa(DFA::config().minimum_cache_clear_count(Some(0)));
    TestRunner::new()?
        .expand(EXPANSIONS, |t| t.compiles())
        // Without NFA shrinking, this test blows the default cache capacity.
        .blacklist("expensive/regression-many-repeat-no-stack-overflow")
        .test_iter(suite()?.iter(), compiler(builder))
        .assert();
    Ok(())
}

/// Tests the hybrid NFA/DFA when the minimum cache capacity is set.
#[test]
fn min_cache_capacity() -> Result<()> {
    let mut builder = Regex::builder();
    builder
        .dfa(DFA::config().cache_capacity(0).skip_cache_capacity_check(true));
    TestRunner::new()?
        .expand(EXPANSIONS, |t| t.compiles())
        .test_iter(suite()?.iter(), compiler(builder))
        .assert();
    Ok(())
}

fn compiler(
    mut builder: regex::Builder,
) -> impl FnMut(&RegexTest, &[String]) -> Result<CompiledRegex> {
    move |test, regexes| {
        // Parse regexes as HIRs for some analysis below.
        let mut hirs = vec![];
        for pattern in regexes.iter() {
            hirs.push(syntax::parse_with(pattern, &config_syntax(test))?);
        }

        // Check if our regex contains things that aren't supported by DFAs.
        // That is, Unicode word boundaries when searching non-ASCII text.
        if !test.haystack().is_ascii() {
            for hir in hirs.iter() {
                if hir.properties().look_set().contains_word_unicode() {
                    return Ok(CompiledRegex::skip());
                }
            }
        }
        if !configure_regex_builder(test, &mut builder) {
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
    re: &Regex,
    cache: &mut regex::Cache,
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
                TestResult::matches(
                    re.find_iter(cache, input)
                        .take(test.match_limit().unwrap_or(std::usize::MAX))
                        .map(|m| Match {
                            id: m.pattern().as_usize(),
                            span: Span { start: m.start(), end: m.end() },
                        }),
                )
            }
            SearchKind::Overlapping => {
                try_search_overlapping(re, cache, &input).unwrap()
            }
        },
        "which" => match test.search_kind() {
            SearchKind::Earliest | SearchKind::Leftmost => {
                // There are no "which" APIs for standard searches.
                TestResult::skip()
            }
            SearchKind::Overlapping => {
                let dfa = re.forward();
                let cache = cache.as_parts_mut().0;
                let mut patset = PatternSet::new(dfa.pattern_len());
                dfa.try_which_overlapping_matches(cache, &input, &mut patset)
                    .unwrap();
                TestResult::which(patset.iter().map(|p| p.as_usize()))
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
fn configure_regex_builder(
    test: &RegexTest,
    builder: &mut regex::Builder,
) -> bool {
    let match_kind = match untestify_kind(test.match_kind()) {
        None => return false,
        Some(k) => k,
    };

    let mut dfa_config =
        DFA::config().match_kind(match_kind).unicode_word_boundary(true);
    // When doing an overlapping search, we might try to find the start of each
    // match with a custom search routine. In that case, we need to tell the
    // reverse search (for the start offset) which pattern to look for. The
    // only way that API works is when anchored starting states are compiled
    // for each pattern. This does technically also enable it for the forward
    // DFA, but we're okay with that.
    if test.search_kind() == SearchKind::Overlapping {
        dfa_config = dfa_config.starts_for_each_pattern(true);
    }
    builder
        .syntax(config_syntax(test))
        .thompson(config_thompson(test))
        .dfa(dfa_config);
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

/// Execute an overlapping search, and for each match found, also find its
/// overlapping starting positions.
///
/// N.B. This routine used to be part of the crate API, but 1) it wasn't clear
/// to me how useful it was and 2) it wasn't clear to me what its semantics
/// should be. In particular, a potentially surprising footgun of this routine
/// that it is worst case *quadratic* in the size of the haystack. Namely, it's
/// possible to report a match at every position, and for every such position,
/// scan all the way to the beginning of the haystack to find the starting
/// position. Typical leftmost non-overlapping searches don't suffer from this
/// because, well, matches can't overlap. So subsequent searches after a match
/// is found don't revisit previously scanned parts of the haystack.
///
/// Its semantics can be strange for other reasons too. For example, given
/// the regex '.*' and the haystack 'zz', the full set of overlapping matches
/// is: [0, 0], [1, 1], [0, 1], [2, 2], [1, 2], [0, 2]. The ordering of
/// those matches is quite strange, but makes sense when you think about the
/// implementation: an end offset is found left-to-right, and then one or more
/// starting offsets are found right-to-left.
///
/// Nevertheless, we provide this routine in our test suite because it's
/// useful to test the low level DFA overlapping search and our test suite
/// is written in a way that requires starting offsets.
fn try_search_overlapping(
    re: &Regex,
    cache: &mut regex::Cache,
    input: &Input<'_>,
) -> Result<TestResult> {
    let mut matches = vec![];
    let mut fwd_state = OverlappingState::start();
    let (fwd_dfa, rev_dfa) = (re.forward(), re.reverse());
    let (fwd_cache, rev_cache) = cache.as_parts_mut();
    while let Some(end) = {
        fwd_dfa.try_search_overlapping_fwd(
            fwd_cache,
            input,
            &mut fwd_state,
        )?;
        fwd_state.get_match()
    } {
        let revsearch = input
            .clone()
            .range(input.start()..end.offset())
            .anchored(Anchored::Pattern(end.pattern()))
            .earliest(false);
        let mut rev_state = OverlappingState::start();
        while let Some(start) = {
            rev_dfa.try_search_overlapping_rev(
                rev_cache,
                &revsearch,
                &mut rev_state,
            )?;
            rev_state.get_match()
        } {
            let span = Span { start: start.offset(), end: end.offset() };
            let mat = Match { id: end.pattern().as_usize(), span };
            matches.push(mat);
        }
    }
    Ok(TestResult::matches(matches))
}
