// We have a similar config in the regex-automata crate root. Basically, it is
// just too annoying to deal with dead code when a subset of features is
// enabled.
#![cfg_attr(
    not(all(
        feature = "std",
        feature = "nfa",
        feature = "dfa",
        feature = "hybrid",
        feature = "perf-literal-substring",
        feature = "perf-literal-multisubstring",
    )),
    allow(dead_code, unused_imports, unused_variables)
)]
// Similar deal with Miri. Just let dead code warnings be.
#![cfg_attr(miri, allow(dead_code, unused_imports, unused_variables))]

#[cfg(any(feature = "dfa-search", feature = "dfa-onepass"))]
mod dfa;
#[cfg(feature = "dfa-search")]
mod fuzz;
#[cfg(feature = "dfa-search")]
mod gen;
#[cfg(feature = "hybrid")]
mod hybrid;
#[cfg(feature = "meta")]
mod meta;
#[cfg(any(feature = "nfa-backtrack", feature = "nfa-pikevm"))]
mod nfa;

fn suite() -> anyhow::Result<regex_test::RegexTests> {
    let _ = env_logger::try_init();

    let mut tests = regex_test::RegexTests::new();
    macro_rules! load {
        ($name:expr) => {{
            const DATA: &[u8] =
                include_bytes!(concat!("../../testdata/", $name, ".toml"));
            tests.load_slice($name, DATA)?;
        }};
    }

    load!("anchored");
    load!("bytes");
    load!("crazy");
    load!("crlf");
    load!("earliest");
    load!("empty");
    load!("expensive");
    load!("flags");
    load!("iter");
    load!("leftmost-all");
    load!("line-terminator");
    load!("misc");
    load!("multiline");
    load!("no-unicode");
    load!("overlapping");
    load!("regression");
    load!("set");
    load!("substring");
    load!("unicode");
    load!("utf8");
    load!("word-boundary");
    load!("word-boundary-special");
    load!("fowler/basic");
    load!("fowler/nullsubexpr");
    load!("fowler/repetition");

    Ok(tests)
}

/// Configure a regex_automata::Input with the given test configuration.
fn create_input<'h>(
    test: &'h regex_test::RegexTest,
) -> regex_automata::Input<'h> {
    use regex_automata::Anchored;

    let bounds = test.bounds();
    let anchored = if test.anchored() { Anchored::Yes } else { Anchored::No };
    regex_automata::Input::new(test.haystack())
        .range(bounds.start..bounds.end)
        .anchored(anchored)
}

/// Convert capture matches into the test suite's capture values.
///
/// The given captures must represent a valid match, where the first capturing
/// group has a non-None span. Otherwise this panics.
fn testify_captures(
    caps: &regex_automata::util::captures::Captures,
) -> regex_test::Captures {
    assert!(caps.is_match(), "expected captures to represent a match");
    let spans = caps.iter().map(|group| {
        group.map(|m| regex_test::Span { start: m.start, end: m.end })
    });
    // These unwraps are OK because we assume our 'caps' represents a match,
    // and a match always gives a non-zero number of groups with the first
    // group being non-None.
    regex_test::Captures::new(caps.pattern().unwrap().as_usize(), spans)
        .unwrap()
}

/// Convert a test harness match kind to a regex-automata match kind. If
/// regex-automata doesn't support the harness kind, then `None` is returned.
fn untestify_kind(
    kind: regex_test::MatchKind,
) -> Option<regex_automata::MatchKind> {
    match kind {
        regex_test::MatchKind::All => Some(regex_automata::MatchKind::All),
        regex_test::MatchKind::LeftmostFirst => {
            Some(regex_automata::MatchKind::LeftmostFirst)
        }
        regex_test::MatchKind::LeftmostLongest => None,
    }
}
