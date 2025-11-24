/*!
This module defines tests and test helpers for substring implementations.
*/

use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
};

pub(crate) mod naive;
#[macro_use]
pub(crate) mod prop;

const SEEDS: &'static [Seed] = &[
    Seed::new("", "", Some(0), Some(0)),
    Seed::new("", "a", Some(0), Some(1)),
    Seed::new("", "ab", Some(0), Some(2)),
    Seed::new("", "abc", Some(0), Some(3)),
    Seed::new("a", "", None, None),
    Seed::new("a", "a", Some(0), Some(0)),
    Seed::new("a", "aa", Some(0), Some(1)),
    Seed::new("a", "ba", Some(1), Some(1)),
    Seed::new("a", "bba", Some(2), Some(2)),
    Seed::new("a", "bbba", Some(3), Some(3)),
    Seed::new("a", "bbbab", Some(3), Some(3)),
    Seed::new("a", "bbbabb", Some(3), Some(3)),
    Seed::new("a", "bbbabbb", Some(3), Some(3)),
    Seed::new("a", "bbbbbb", None, None),
    Seed::new("ab", "", None, None),
    Seed::new("ab", "a", None, None),
    Seed::new("ab", "b", None, None),
    Seed::new("ab", "ab", Some(0), Some(0)),
    Seed::new("ab", "aab", Some(1), Some(1)),
    Seed::new("ab", "aaab", Some(2), Some(2)),
    Seed::new("ab", "abaab", Some(0), Some(3)),
    Seed::new("ab", "baaab", Some(3), Some(3)),
    Seed::new("ab", "acb", None, None),
    Seed::new("ab", "abba", Some(0), Some(0)),
    Seed::new("abc", "ab", None, None),
    Seed::new("abc", "abc", Some(0), Some(0)),
    Seed::new("abc", "abcz", Some(0), Some(0)),
    Seed::new("abc", "abczz", Some(0), Some(0)),
    Seed::new("abc", "zabc", Some(1), Some(1)),
    Seed::new("abc", "zzabc", Some(2), Some(2)),
    Seed::new("abc", "azbc", None, None),
    Seed::new("abc", "abzc", None, None),
    Seed::new("abczdef", "abczdefzzzzzzzzzzzzzzzzzzzz", Some(0), Some(0)),
    Seed::new("abczdef", "zzzzzzzzzzzzzzzzzzzzabczdef", Some(20), Some(20)),
    Seed::new(
        "xyz",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaxyz",
        Some(32),
        Some(32),
    ),
    Seed::new("\u{0}\u{15}", "\u{0}\u{15}\u{15}\u{0}", Some(0), Some(0)),
    Seed::new("\u{0}\u{1e}", "\u{1e}\u{0}", None, None),
];

/// Runs a host of substring search tests.
///
/// This has support for "partial" substring search implementations only work
/// for a subset of needles/haystacks. For example, the "packed pair" substring
/// search implementation only works for haystacks of some minimum length based
/// of the pair of bytes selected and the size of the vector used.
pub(crate) struct Runner {
    fwd: Option<
        Box<dyn FnMut(&[u8], &[u8]) -> Option<Option<usize>> + 'static>,
    >,
    rev: Option<
        Box<dyn FnMut(&[u8], &[u8]) -> Option<Option<usize>> + 'static>,
    >,
}

impl Runner {
    /// Create a new test runner for forward and reverse substring search
    /// implementations.
    pub(crate) fn new() -> Runner {
        Runner { fwd: None, rev: None }
    }

    /// Run all tests. This panics on the first failure.
    ///
    /// If the implementation being tested returns `None` for a particular
    /// haystack/needle combination, then that test is skipped.
    ///
    /// This runs tests on both the forward and reverse implementations given.
    /// If either (or both) are missing, then tests for that implementation are
    /// skipped.
    pub(crate) fn run(self) {
        if let Some(mut fwd) = self.fwd {
            for seed in SEEDS.iter() {
                for t in seed.generate() {
                    match fwd(t.haystack.as_bytes(), t.needle.as_bytes()) {
                        None => continue,
                        Some(result) => {
                            assert_eq!(
                                t.fwd, result,
                                "FORWARD, needle: {:?}, haystack: {:?}",
                                t.needle, t.haystack,
                            );
                        }
                    }
                }
            }
        }
        if let Some(mut rev) = self.rev {
            for seed in SEEDS.iter() {
                for t in seed.generate() {
                    match rev(t.haystack.as_bytes(), t.needle.as_bytes()) {
                        None => continue,
                        Some(result) => {
                            assert_eq!(
                                t.rev, result,
                                "REVERSE, needle: {:?}, haystack: {:?}",
                                t.needle, t.haystack,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Set the implementation for forward substring search.
    ///
    /// If the closure returns `None`, then it is assumed that the given
    /// test cannot be applied to the particular implementation and it is
    /// skipped. For example, if a particular implementation only supports
    /// needles or haystacks for some minimum length.
    ///
    /// If this is not set, then forward substring search is not tested.
    pub(crate) fn fwd(
        mut self,
        search: impl FnMut(&[u8], &[u8]) -> Option<Option<usize>> + 'static,
    ) -> Runner {
        self.fwd = Some(Box::new(search));
        self
    }

    /// Set the implementation for reverse substring search.
    ///
    /// If the closure returns `None`, then it is assumed that the given
    /// test cannot be applied to the particular implementation and it is
    /// skipped. For example, if a particular implementation only supports
    /// needles or haystacks for some minimum length.
    ///
    /// If this is not set, then reverse substring search is not tested.
    pub(crate) fn rev(
        mut self,
        search: impl FnMut(&[u8], &[u8]) -> Option<Option<usize>> + 'static,
    ) -> Runner {
        self.rev = Some(Box::new(search));
        self
    }
}

/// A single substring test for forward and reverse searches.
#[derive(Clone, Debug)]
struct Test {
    needle: String,
    haystack: String,
    fwd: Option<usize>,
    rev: Option<usize>,
}

/// A single substring test for forward and reverse searches.
///
/// Each seed is valid on its own, but it also serves as a starting point
/// to generate more tests. Namely, we pad out the haystacks with other
/// characters so that we get more complete coverage. This is especially useful
/// for testing vector algorithms that tend to have weird special cases for
/// alignment and loop unrolling.
///
/// Padding works by assuming certain characters never otherwise appear in a
/// needle or a haystack. Neither should contain a `#` character.
#[derive(Clone, Copy, Debug)]
struct Seed {
    needle: &'static str,
    haystack: &'static str,
    fwd: Option<usize>,
    rev: Option<usize>,
}

impl Seed {
    const MAX_PAD: usize = 34;

    const fn new(
        needle: &'static str,
        haystack: &'static str,
        fwd: Option<usize>,
        rev: Option<usize>,
    ) -> Seed {
        Seed { needle, haystack, fwd, rev }
    }

    fn generate(self) -> impl Iterator<Item = Test> {
        assert!(!self.needle.contains('#'), "needle must not contain '#'");
        assert!(!self.haystack.contains('#'), "haystack must not contain '#'");
        (0..=Seed::MAX_PAD)
            // Generate tests for padding at the beginning of haystack.
            .map(move |pad| {
                let needle = self.needle.to_string();
                let prefix = "#".repeat(pad);
                let haystack = format!("{}{}", prefix, self.haystack);
                let fwd = if needle.is_empty() {
                    Some(0)
                } else {
                    self.fwd.map(|i| pad + i)
                };
                let rev = if needle.is_empty() {
                    Some(haystack.len())
                } else {
                    self.rev.map(|i| pad + i)
                };
                Test { needle, haystack, fwd, rev }
            })
            // Generate tests for padding at the end of haystack.
            .chain((1..=Seed::MAX_PAD).map(move |pad| {
                let needle = self.needle.to_string();
                let suffix = "#".repeat(pad);
                let haystack = format!("{}{}", self.haystack, suffix);
                let fwd = if needle.is_empty() { Some(0) } else { self.fwd };
                let rev = if needle.is_empty() {
                    Some(haystack.len())
                } else {
                    self.rev
                };
                Test { needle, haystack, fwd, rev }
            }))
    }
}
