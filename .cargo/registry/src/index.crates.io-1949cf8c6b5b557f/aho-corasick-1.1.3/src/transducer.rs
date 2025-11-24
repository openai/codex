/*!
Provides implementations of `fst::Automaton` for Aho-Corasick automata.

This works by providing two wrapper types, [`Anchored`] and [`Unanchored`].
The former executes an anchored search on an FST while the latter executes
an unanchored search. Building these wrappers is fallible and will fail if
the underlying Aho-Corasick automaton does not support the type of search it
represents.
*/

use crate::{
    automaton::{Automaton, StateID},
    Anchored as AcAnchored, Input, MatchError,
};

/// Represents an unanchored Aho-Corasick search of a finite state transducer.
///
/// Wrapping an Aho-Corasick automaton in `Unanchored` will fail if the
/// underlying automaton does not support unanchored searches.
///
/// # Example
///
/// This shows how to build an FST of keys and then run an unanchored search on
/// those keys using an Aho-Corasick automaton.
///
/// ```
/// use aho_corasick::{nfa::contiguous::NFA, transducer::Unanchored};
/// use fst::{Automaton, IntoStreamer, Set, Streamer};
///
/// let set = Set::from_iter(&["abcd", "bc", "bcd", "xyz"]).unwrap();
/// let nfa = NFA::new(&["bcd", "x"]).unwrap();
/// // NFAs always support both unanchored and anchored searches.
/// let searcher = Unanchored::new(&nfa).unwrap();
///
/// let mut stream = set.search(searcher).into_stream();
/// let mut results = vec![];
/// while let Some(key) = stream.next() {
///     results.push(std::str::from_utf8(key).unwrap().to_string());
/// }
/// assert_eq!(vec!["abcd", "bcd", "xyz"], results);
/// ```
#[derive(Clone, Debug)]
pub struct Unanchored<A>(A);

impl<A: Automaton> Unanchored<A> {
    /// Create a new `Unanchored` implementation of the `fst::Automaton` trait.
    ///
    /// If the given Aho-Corasick automaton does not support unanchored
    /// searches, then this returns an error.
    pub fn new(aut: A) -> Result<Unanchored<A>, MatchError> {
        let input = Input::new("").anchored(AcAnchored::No);
        let _ = aut.start_state(&input)?;
        Ok(Unanchored(aut))
    }

    /// Returns a borrow to the underlying automaton.
    pub fn as_ref(&self) -> &A {
        &self.0
    }

    /// Unwrap this value and return the inner automaton.
    pub fn into_inner(self) -> A {
        self.0
    }
}

impl<A: Automaton> fst::Automaton for Unanchored<A> {
    type State = StateID;

    #[inline]
    fn start(&self) -> StateID {
        let input = Input::new("").anchored(AcAnchored::No);
        self.0.start_state(&input).expect("support for unanchored searches")
    }

    #[inline]
    fn is_match(&self, state: &StateID) -> bool {
        self.0.is_match(*state)
    }

    #[inline]
    fn accept(&self, state: &StateID, byte: u8) -> StateID {
        if fst::Automaton::is_match(self, state) {
            return *state;
        }
        self.0.next_state(AcAnchored::No, *state, byte)
    }

    #[inline]
    fn can_match(&self, state: &StateID) -> bool {
        !self.0.is_dead(*state)
    }
}

/// Represents an anchored Aho-Corasick search of a finite state transducer.
///
/// Wrapping an Aho-Corasick automaton in `Unanchored` will fail if the
/// underlying automaton does not support unanchored searches.
///
/// # Example
///
/// This shows how to build an FST of keys and then run an anchored search on
/// those keys using an Aho-Corasick automaton.
///
/// ```
/// use aho_corasick::{nfa::contiguous::NFA, transducer::Anchored};
/// use fst::{Automaton, IntoStreamer, Set, Streamer};
///
/// let set = Set::from_iter(&["abcd", "bc", "bcd", "xyz"]).unwrap();
/// let nfa = NFA::new(&["bcd", "x"]).unwrap();
/// // NFAs always support both unanchored and anchored searches.
/// let searcher = Anchored::new(&nfa).unwrap();
///
/// let mut stream = set.search(searcher).into_stream();
/// let mut results = vec![];
/// while let Some(key) = stream.next() {
///     results.push(std::str::from_utf8(key).unwrap().to_string());
/// }
/// assert_eq!(vec!["bcd", "xyz"], results);
/// ```
///
/// This is like the example above, except we use an Aho-Corasick DFA, which
/// requires explicitly configuring it to support anchored searches. (NFAs
/// unconditionally support both unanchored and anchored searches.)
///
/// ```
/// use aho_corasick::{dfa::DFA, transducer::Anchored, StartKind};
/// use fst::{Automaton, IntoStreamer, Set, Streamer};
///
/// let set = Set::from_iter(&["abcd", "bc", "bcd", "xyz"]).unwrap();
/// let dfa = DFA::builder()
///     .start_kind(StartKind::Anchored)
///     .build(&["bcd", "x"])
///     .unwrap();
/// // We've explicitly configured our DFA to support anchored searches.
/// let searcher = Anchored::new(&dfa).unwrap();
///
/// let mut stream = set.search(searcher).into_stream();
/// let mut results = vec![];
/// while let Some(key) = stream.next() {
///     results.push(std::str::from_utf8(key).unwrap().to_string());
/// }
/// assert_eq!(vec!["bcd", "xyz"], results);
/// ```
#[derive(Clone, Debug)]
pub struct Anchored<A>(A);

impl<A: Automaton> Anchored<A> {
    /// Create a new `Anchored` implementation of the `fst::Automaton` trait.
    ///
    /// If the given Aho-Corasick automaton does not support anchored searches,
    /// then this returns an error.
    pub fn new(aut: A) -> Result<Anchored<A>, MatchError> {
        let input = Input::new("").anchored(AcAnchored::Yes);
        let _ = aut.start_state(&input)?;
        Ok(Anchored(aut))
    }

    /// Returns a borrow to the underlying automaton.
    pub fn as_ref(&self) -> &A {
        &self.0
    }

    /// Unwrap this value and return the inner automaton.
    pub fn into_inner(self) -> A {
        self.0
    }
}

impl<A: Automaton> fst::Automaton for Anchored<A> {
    type State = StateID;

    #[inline]
    fn start(&self) -> StateID {
        let input = Input::new("").anchored(AcAnchored::Yes);
        self.0.start_state(&input).expect("support for unanchored searches")
    }

    #[inline]
    fn is_match(&self, state: &StateID) -> bool {
        self.0.is_match(*state)
    }

    #[inline]
    fn accept(&self, state: &StateID, byte: u8) -> StateID {
        if fst::Automaton::is_match(self, state) {
            return *state;
        }
        self.0.next_state(AcAnchored::Yes, *state, byte)
    }

    #[inline]
    fn can_match(&self, state: &StateID) -> bool {
        !self.0.is_dead(*state)
    }
}

#[cfg(test)]
mod tests {
    use alloc::{string::String, vec, vec::Vec};

    use fst::{Automaton, IntoStreamer, Set, Streamer};

    use crate::{
        dfa::DFA,
        nfa::{contiguous, noncontiguous},
        StartKind,
    };

    use super::*;

    fn search<A: Automaton, D: AsRef<[u8]>>(
        set: &Set<D>,
        aut: A,
    ) -> Vec<String> {
        let mut stream = set.search(aut).into_stream();
        let mut results = vec![];
        while let Some(key) = stream.next() {
            results.push(String::from(core::str::from_utf8(key).unwrap()));
        }
        results
    }

    #[test]
    fn unanchored() {
        let set =
            Set::from_iter(&["a", "bar", "baz", "wat", "xba", "xbax", "z"])
                .unwrap();
        let patterns = vec!["baz", "bax"];
        let expected = vec!["baz", "xbax"];

        let aut = Unanchored(noncontiguous::NFA::new(&patterns).unwrap());
        let got = search(&set, &aut);
        assert_eq!(got, expected);

        let aut = Unanchored(contiguous::NFA::new(&patterns).unwrap());
        let got = search(&set, &aut);
        assert_eq!(got, expected);

        let aut = Unanchored(DFA::new(&patterns).unwrap());
        let got = search(&set, &aut);
        assert_eq!(got, expected);
    }

    #[test]
    fn anchored() {
        let set =
            Set::from_iter(&["a", "bar", "baz", "wat", "xba", "xbax", "z"])
                .unwrap();
        let patterns = vec!["baz", "bax"];
        let expected = vec!["baz"];

        let aut = Anchored(noncontiguous::NFA::new(&patterns).unwrap());
        let got = search(&set, &aut);
        assert_eq!(got, expected);

        let aut = Anchored(contiguous::NFA::new(&patterns).unwrap());
        let got = search(&set, &aut);
        assert_eq!(got, expected);

        let aut = Anchored(
            DFA::builder()
                .start_kind(StartKind::Anchored)
                .build(&patterns)
                .unwrap(),
        );
        let got = search(&set, &aut);
        assert_eq!(got, expected);
    }
}
