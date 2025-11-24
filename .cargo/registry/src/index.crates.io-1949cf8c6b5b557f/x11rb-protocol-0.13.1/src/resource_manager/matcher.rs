//! Match Xrm entries against a query.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use std::cmp::Ordering;

use super::parser::parse_query;
use super::{Binding, Component, Entry};

mod zip_longest {
    /// Given two slices, produce an iterator that zips the two slices.
    ///
    /// Compared to std::iter::Iterator::zip(), this iterator does not stop at the end of the
    /// shorter of the two slices, but instead continues to the end of the longer slice. To make
    /// this possible, the individual items are wrapped in `Option`.
    ///
    /// See tests below to make this clearer.
    pub(super) fn zip_longest<'a, T>(
        a: &'a [T],
        b: &'a [T],
    ) -> impl Iterator<Item = (Option<&'a T>, Option<&'a T>)> + 'a {
        ZipLongest {
            a: a.iter(),
            b: b.iter(),
        }
    }

    #[derive(Debug)]
    struct ZipLongest<A, B> {
        a: A,
        b: B,
    }

    impl<A, B> Iterator for ZipLongest<A, B>
    where
        A: Iterator,
        B: Iterator,
    {
        type Item = (Option<A::Item>, Option<B::Item>);

        fn next(&mut self) -> Option<Self::Item> {
            match (self.a.next(), self.b.next()) {
                (None, None) => None,
                (a, b) => Some((a, b)),
            }
        }
    }

    #[cfg(test)]
    mod test_zip_longest {
        use super::zip_longest;
        use alloc::vec::Vec;

        #[test]
        fn empty() {
            let (a, b): ([u8; 0], [u8; 0]) = ([], []);
            let res = zip_longest(&a, &b).collect::<Vec<_>>();
            assert_eq!(res, []);
        }

        #[test]
        fn same_length() {
            let a = [0, 1, 2];
            let b = [4, 5, 6];
            let expected = [
                (Some(&0), Some(&4)),
                (Some(&1), Some(&5)),
                (Some(&2), Some(&6)),
            ];
            let res = zip_longest(&a, &b).collect::<Vec<_>>();
            assert_eq!(res, expected);
        }

        #[test]
        fn first_shorter() {
            let a = [0, 1];
            let b = [4, 5, 6, 7];
            let expected = [
                (Some(&0), Some(&4)),
                (Some(&1), Some(&5)),
                (None, Some(&6)),
                (None, Some(&7)),
            ];
            let res = zip_longest(&a, &b).collect::<Vec<_>>();
            assert_eq!(res, expected);
        }

        #[test]
        fn second_shorter() {
            let a = [0, 1, 2, 3];
            let b = [4, 5];
            let expected = [
                (Some(&0), Some(&4)),
                (Some(&1), Some(&5)),
                (Some(&2), None),
                (Some(&3), None),
            ];
            let res = zip_longest(&a, &b).collect::<Vec<_>>();
            assert_eq!(res, expected);
        }
    }
}

/// Info how a specific component was matched.
///
/// This information is used to decide which of two matches is "better" in `compare_matches()`.
#[derive(Debug, Copy, Clone)]
enum HowMatched {
    /// The component matched the instance of the query
    Instance,
    /// The component matched the class of the query
    Class,
    /// The component is a wildcard and thus matched by default
    Wildcard,
}

/// Info on how an (unskipped) component of the query was matched
///
/// This information is used to decide which of two matches is "better" in `compare_matches()`.
#[derive(Debug, Copy, Clone)]
struct MatchComponent {
    preceding_binding: Binding,
    how_matched: HowMatched,
}

/// Info how a (possibly skipped) component of the query was matched
///
/// This information is used to decide which of two matches is "better" in `compare_matches()`.
#[derive(Debug, Copy, Clone)]
enum MatchKind {
    /// The component was skipped via a loose binding ("*")
    SkippedViaLooseBinding,
    /// The component was matched against the entry.
    Matched(MatchComponent),
}

impl MatchKind {
    /// Create a new `MatchKind::Match` with the given entries.
    fn new_match(preceding_binding: Binding, how_matched: HowMatched) -> Self {
        Self::Matched(MatchComponent {
            preceding_binding,
            how_matched,
        })
    }
}

fn check_match(entry: &Entry, resource: &[String], class: &[String]) -> Vec<Vec<MatchKind>> {
    /// Current state of the matching machinery
    #[derive(Debug, Default)]
    struct MatchState {
        /// Index into the entry on where we have to continue matching
        index: usize,
        /// How did we get to this state?
        history: Vec<MatchKind>,
    }

    impl MatchState {
        /// Record that a component was skipped via a loose binding (`*`).
        fn skip_loose(&self) -> Self {
            let mut history = self.history.clone();
            history.push(MatchKind::SkippedViaLooseBinding);
            Self {
                index: self.index,
                history,
            }
        }

        /// Record that a component was matched in the given way.
        fn step(mut self, kind: MatchKind) -> Self {
            self.history.push(kind);
            self.index += 1;
            self
        }
    }

    // The idea is to check if a nondeterministic finite automaton accepts a given
    // word. We have a set of current states. This describes where in the
    // entry we are while trying to match. When we match a component, we go to the next
    // component in the entry (index + 1, `MatchState::step()`). When we have a loose binding, we
    // can accept the current component by staying in the same state (index,
    // `MatchState::skip_loose()`).
    let mut states = vec![MatchState::default()];

    // Go through the components and match them against the query
    for (resource, class) in zip_longest::zip_longest(resource, class) {
        let mut next_states = Vec::new();
        for state in states.into_iter() {
            if state.index == entry.components.len() {
                // We are at the end of the entry and thus cannot continue this match.
                // We drop this match state.
                continue;
            }
            let binding = entry.components[state.index].0;
            match binding {
                // We have to match here, no way around that.
                Binding::Tight => {}
                // We could "eat" this with the loose binding by staying in the state
                Binding::Loose => next_states.push(state.skip_loose()),
            }
            // Does the component match?
            let kind = match entry.components[state.index].1 {
                Component::Wildcard => Some(MatchKind::new_match(binding, HowMatched::Wildcard)),
                Component::Normal(ref s) => {
                    if Some(s) == resource {
                        Some(MatchKind::new_match(binding, HowMatched::Instance))
                    } else if Some(s) == class {
                        Some(MatchKind::new_match(binding, HowMatched::Class))
                    } else {
                        None
                    }
                }
            };
            if let Some(kind) = kind {
                // Yes, the component matches and we go to the next state
                next_states.push(state.step(kind));
            }
        }
        states = next_states;
    }
    // We have a match if we reached the end of the components
    states
        .into_iter()
        .filter(|s| s.index == entry.components.len())
        .map(|s| s.history)
        .collect()
}

/// Compare two matches and decide which one of the two is better (`Ordering::Greater`)
fn compare_matches(match1: &[MatchKind], match2: &[MatchKind]) -> Ordering {
    use Binding::*;
    use HowMatched::*;
    use MatchKind::*;

    fn rule1(match1: &MatchKind, match2: &MatchKind) -> Ordering {
        // Precedence rule #1: Matching components (including wildcard '?') outweighs loose bindings ('*')
        if let Matched(_) = match1 {
            if let SkippedViaLooseBinding = match2 {
                return Ordering::Greater;
            }
        }
        Ordering::Equal
    }

    fn rule2(match1: &MatchKind, match2: &MatchKind) -> Ordering {
        // Precedence rule #2a: Matching instance outweighs both matching class and wildcard
        if let Matched(MatchComponent {
            how_matched: Instance,
            preceding_binding: _,
        }) = match1
        {
            if let Matched(MatchComponent {
                how_matched: Class,
                preceding_binding: _,
            }) = match2
            {
                return Ordering::Greater;
            }
            if let Matched(MatchComponent {
                how_matched: Wildcard,
                ..
            }) = match2
            {
                return Ordering::Greater;
            }
        }
        // Precedence rule #2b: Matching class outweighs wildcard
        if let Matched(MatchComponent {
            how_matched: Class, ..
        }) = match1
        {
            if let Matched(MatchComponent {
                how_matched: Wildcard,
                ..
            }) = match2
            {
                return Ordering::Greater;
            }
        }
        Ordering::Equal
    }

    fn rule3(match1: &MatchKind, match2: &MatchKind) -> Ordering {
        // Precedence rule #3: A preceding exact match outweights a preceding '*'
        if let Matched(MatchComponent {
            preceding_binding: Tight,
            ..
        }) = match1
        {
            if let Matched(MatchComponent {
                preceding_binding: Loose,
                ..
            }) = match2
            {
                return Ordering::Greater;
            }
        }
        Ordering::Equal
    }

    assert_eq!(
        match1.len(),
        match2.len(),
        "Both matches should have the same length (which is guaranteed by the current \
         implementation of check_match())"
    );
    for (m1, m2) in match1.iter().zip(match2.iter()) {
        let ordering = rule1(m1, m2)
            .then_with(|| rule1(m2, m1).reverse())
            .then_with(|| rule2(m1, m2))
            .then_with(|| rule2(m2, m1).reverse())
            .then_with(|| rule3(m1, m2))
            .then_with(|| rule3(m2, m1).reverse());
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
}

/// Find the best match for the given query in the database, returning `None` when nothing matches.
pub(crate) fn match_entry<'a>(
    database: &'a [Entry],
    resource: &str,
    class: &str,
) -> Option<&'a [u8]> {
    let resource = parse_query(resource.as_bytes())?;
    let class = parse_query(class.as_bytes())?;
    database
        .iter()
        // Filter for entries that match the query (and record some info on how they match)
        .flat_map(|entry| {
            let matches = check_match(entry, &resource, &class);
            let best_match = matches
                .into_iter()
                .max_by(|match1, match2| compare_matches(match1, match2));
            best_match.map(|m| (entry, m))
        })
        .max_by(|(_, match1), (_, match2)| compare_matches(match1, match2))
        .map(|(entry, _)| &entry.value[..])
}

#[cfg(test)]
mod test {
    use super::super::parser::parse_database;
    use super::match_entry;

    use alloc::format;
    use alloc::string::{String, ToString};
    use alloc::vec::Vec;
    use std::eprintln;

    // Most tests in here are based on [1], which is: Copyright © 2016 Ingo Bürk
    // [1]: https://github.com/Airblader/xcb-util-xrm/blob/master/tests/tests_match.c

    #[test]
    fn test_matches() {
        let tests = [
            // Non-matches / Errors
            (&b""[..], "", "", None),
            // Xlib returns the match here, despite the query violating the specs (different number
            // of components in the query)
            (
                b"First.second: 1",
                "First.second",
                "First.second.third",
                None,
            ),
            (b"", "First.second", "", None),
            (b"First.second: 1", "First.third", "", None),
            (b"First.second: 1", "First", "", None),
            (b"First: 1", "First.second", "", None),
            (b"First.?.fourth: 1", "First.second.third.fourth", "", None),
            (b"First*?.third: 1", "First.third", "", None),
            (b"First: 1", "first", "", None),
            (b"First: 1", "", "first", None),
            // Duplicate entries
            (
                b"First: 1\nFirst: 2\nFirst: 3\n",
                "First",
                "",
                Some(&b"3"[..]),
            ),
            (
                b"First: 1\nSecond: 2\nSecond: 3\nThird: 4\n",
                "Second",
                "",
                Some(b"3"),
            ),
            // Basic matching
            (b"First: 1", "First", "", Some(b"1")),
            (b"First.second: 1", "First.second", "", Some(b"1")),
            (b"?.second: 1", "First.second", "", Some(b"1")),
            (b"First.?.third: 1", "First.second.third", "", Some(b"1")),
            (
                b"First.?.?.fourth: 1",
                "First.second.third.fourth",
                "",
                Some(b"1"),
            ),
            (b"*second: 1", "First.second", "", Some(b"1")),
            (b".second: 1", "First.second", "", None),
            (b"*third: 1", "First.second.third", "", Some(b"1")),
            (b"First*second: 1", "First.second", "", Some(b"1")),
            (b"First*third: 1", "First.second.third", "", Some(b"1")),
            (
                b"First*fourth: 1",
                "First.second.third.fourth",
                "",
                Some(b"1"),
            ),
            (b"First*?.third: 1", "First.second.third", "", Some(b"1")),
            (b"First: 1", "Second", "First", Some(b"1")),
            (
                b"First.second: 1",
                "First.third",
                "first.second",
                Some(b"1"),
            ),
            (
                b"First.second.third: 1",
                "First.third.third",
                "first.second.fourth",
                Some(b"1"),
            ),
            (
                b"First*third*fifth: 1",
                "First.second.third.fourth.third.fifth",
                "",
                Some(b"1"),
            ),
            (b"First: x\\\ny", "First", "", Some(b"xy")),
            (b"! First: x", "First", "", None),
            (b"# First: x", "First", "", None),
            (b"First:", "First", "", Some(b"")),
            (b"First: ", "First", "", Some(b"")),
            (b"First: \t ", "First", "", Some(b"")),
            // Consecutive bindings
            (b"*.bar: 1", "foo.foo.bar", "", Some(b"1")),
            (b"...bar: 1", "foo.bar", "", None),
            (b"...bar: 1", "foo.foo.foo.bar", "", None),
            (b"***bar: 1", "foo.bar", "", Some(b"1")),
            (b".*.bar: 1", "foo.bar", "", Some(b"1")),
            (b".*.bar: 1", "foo.foo.bar", "", Some(b"1")),
            (b"..*bar: 1", "foo.foo.foo.foo.bar", "", Some(b"1")),
            (b"a.*.z: 1", "a.b.c.d.e.f.z", "", Some(b"1")),
            (b"a...z: 1", "a.z", "", Some(b"1")),
            (b"a...z: 1", "a.b.z", "", None),
            // Matching among multiple entries
            (b"First: 1\nSecond: 2\n", "First", "", Some(b"1")),
            (b"First: 1\nSecond: 2\n", "Second", "", Some(b"2")),
            // Greediness
            (b"a*c.e: 1", "a.b.c.d.c.e", "", Some(b"1")),
            (b"a*c.e: 1", "a.b.c.c.e", "", Some(b"1")),
            (b"a*?.e: 1", "a.b.c.e", "", Some(b"1")),
            (b"a*c*e: 1", "a.b.c.d.c.d.e.d.e", "", Some(b"1")),
            // Precedence rules
            // Rule 1
            (
                b"First.second.third: 1\nFirst*third: 2\n",
                "First.second.third",
                "",
                Some(b"1"),
            ),
            (
                b"First*third: 2\nFirst.second.third: 1\n",
                "First.second.third",
                "",
                Some(b"1"),
            ),
            (
                b"First.second.third: 1\nFirst*third: 2\n",
                "x.x.x",
                "First.second.third",
                Some(b"1"),
            ),
            (
                b"First*third: 2\nFirst.second.third: 1\n",
                "x.x.x",
                "First.second.third",
                Some(b"1"),
            ),
            // Rule 2
            (
                b"First.second: 1\nFirst.third: 2\n",
                "First.second",
                "First.third",
                Some(b"1"),
            ),
            (
                b"First.third: 2\nFirst.second: 1\n",
                "First.second",
                "First.third",
                Some(b"1"),
            ),
            (
                b"First.second.third: 1\nFirst.?.third: 2\n",
                "First.second.third",
                "",
                Some(b"1"),
            ),
            (
                b"First.?.third: 2\nFirst.second.third: 1\n",
                "First.second.third",
                "",
                Some(b"1"),
            ),
            (
                b"First.second.third: 1\nFirst.?.third: 2\n",
                "x.x.x",
                "First.second.third",
                Some(b"1"),
            ),
            (
                b"First.?.third: 2\nFirst.second.third: 1\n",
                "x.x.x",
                "First.second.third",
                Some(b"1"),
            ),
            // Rule 3
            (
                b"First.second: 1\nFirst*second: 2\n",
                "First.second",
                "",
                Some(b"1"),
            ),
            (
                b"First*second: 2\nFirst.second: 1\n",
                "First.second",
                "",
                Some(b"1"),
            ),
            // Some real world examples. May contain duplicates to the above tests.

            // From the specification:
            // https://tronche.com/gui/x/xlib/resource-manager/matching-rules.html
            (
                b"xmh*Paned*activeForeground: red\n\
                  *incorporate.Foreground: blue\n\
                  xmh.toc*Command*activeForeground: green\n\
                  xmh.toc*?.Foreground: white\n\
                  xmh.toc*Command.activeForeground: black",
                "xmh.toc.messagefunctions.incorporate.activeForeground",
                "Xmh.Paned.Box.Command.Foreground",
                Some(b"black"),
            ),
            (
                b"urxvt*background: [95]#000",
                "urxvt.background",
                "",
                Some(b"[95]#000"),
            ),
            (
                b"urxvt*scrollBar_right:true",
                "urxvt.scrollBar_right",
                "",
                Some(b"true"),
            ),
            (
                b"urxvt*cutchars:    '\"'()*<>[]{|}",
                "urxvt.cutchars",
                "",
                Some(b"'\"'()*<>[]{|}"),
            ),
            (
                b"urxvt.keysym.Control-Shift-Up: perl:font:increment",
                "urxvt.keysym.Control-Shift-Up",
                "",
                Some(b"perl:font:increment"),
            ),
            (
                b"rofi.normal: #000000, #000000, #000000, #000000",
                "rofi.normal",
                "",
                Some(b"#000000, #000000, #000000, #000000"),
            ),
            // Own tests
            (b"*foo.bar: 1", "bar", "", None),
            (
                b"First.Second.Third: 1\nFirst.Second: 2",
                "First.Second.Third",
                "First.Second",
                Some(b"1"),
            ),
            (
                b"First.Second.Third: 1\nFirst.Second: 2",
                "First.Second",
                "First.Second.Third",
                Some(b"1"),
            ),
        ];
        let mut failures = 0;
        for &(data, resource, class, expected) in tests.iter() {
            let mut entries = Vec::new();
            parse_database(data, &mut entries, |_, _| unreachable!());
            let result = match_entry(&entries, resource, class);
            if result != expected {
                eprintln!(
                    "While testing resource '{}' and class '{}' with the following input:",
                    resource, class
                );
                eprintln!("{}", print_string(data));
                eprintln!("Expected: {:?}", expected.map(print_string));
                eprintln!("Got:      {:?}", result.map(print_string));
                eprintln!();
                failures += 1;
            }
        }
        if failures != 0 {
            panic!("Had {} failures", failures)
        }
    }

    fn print_string(data: &[u8]) -> String {
        std::str::from_utf8(data)
            .map(|s| s.to_string())
            .unwrap_or_else(|_| format!("{:?}", data))
    }
}
