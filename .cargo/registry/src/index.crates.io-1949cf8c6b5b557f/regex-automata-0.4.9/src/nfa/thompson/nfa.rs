use core::{fmt, mem};

use alloc::{boxed::Box, format, string::String, sync::Arc, vec, vec::Vec};

#[cfg(feature = "syntax")]
use crate::nfa::thompson::{
    compiler::{Compiler, Config},
    error::BuildError,
};
use crate::{
    nfa::thompson::builder::Builder,
    util::{
        alphabet::{self, ByteClassSet, ByteClasses},
        captures::{GroupInfo, GroupInfoError},
        look::{Look, LookMatcher, LookSet},
        primitives::{
            IteratorIndexExt, PatternID, PatternIDIter, SmallIndex, StateID,
        },
        sparse_set::SparseSet,
    },
};

/// A byte oriented Thompson non-deterministic finite automaton (NFA).
///
/// A Thompson NFA is a finite state machine that permits unconditional epsilon
/// transitions, but guarantees that there exists at most one non-epsilon
/// transition for each element in the alphabet for each state.
///
/// An NFA may be used directly for searching, for analysis or to build
/// a deterministic finite automaton (DFA).
///
/// # Cheap clones
///
/// Since an NFA is a core data type in this crate that many other regex
/// engines are based on top of, it is convenient to give ownership of an NFA
/// to said regex engines. Because of this, an NFA uses reference counting
/// internally. Therefore, it is cheap to clone and it is encouraged to do so.
///
/// # Capabilities
///
/// Using an NFA for searching via the
/// [`PikeVM`](crate::nfa::thompson::pikevm::PikeVM) provides the most amount
/// of "power" of any regex engine in this crate. Namely, it supports the
/// following in all cases:
///
/// 1. Detection of a match.
/// 2. Location of a match, including both the start and end offset, in a
/// single pass of the haystack.
/// 3. Location of matching capturing groups.
/// 4. Handles multiple patterns, including (1)-(3) when multiple patterns are
/// present.
///
/// # Capturing Groups
///
/// Groups refer to parenthesized expressions inside a regex pattern. They look
/// like this, where `exp` is an arbitrary regex:
///
/// * `(exp)` - An unnamed capturing group.
/// * `(?P<name>exp)` or `(?<name>exp)` - A named capturing group.
/// * `(?:exp)` - A non-capturing group.
/// * `(?i:exp)` - A non-capturing group that sets flags.
///
/// Only the first two forms are said to be _capturing_. Capturing
/// means that the last position at which they match is reportable. The
/// [`Captures`](crate::util::captures::Captures) type provides convenient
/// access to the match positions of capturing groups, which includes looking
/// up capturing groups by their name.
///
/// # Byte oriented
///
/// This NFA is byte oriented, which means that all of its transitions are
/// defined on bytes. In other words, the alphabet of an NFA consists of the
/// 256 different byte values.
///
/// While DFAs nearly demand that they be byte oriented for performance
/// reasons, an NFA could conceivably be *Unicode codepoint* oriented. Indeed,
/// a previous version of this NFA supported both byte and codepoint oriented
/// modes. A codepoint oriented mode can work because an NFA fundamentally uses
/// a sparse representation of transitions, which works well with the large
/// sparse space of Unicode codepoints.
///
/// Nevertheless, this NFA is only byte oriented. This choice is primarily
/// driven by implementation simplicity, and also in part memory usage. In
/// practice, performance between the two is roughly comparable. However,
/// building a DFA (including a hybrid DFA) really wants a byte oriented NFA.
/// So if we do have a codepoint oriented NFA, then we also need to generate
/// byte oriented NFA in order to build an hybrid NFA/DFA. Thus, by only
/// generating byte oriented NFAs, we can produce one less NFA. In other words,
/// if we made our NFA codepoint oriented, we'd need to *also* make it support
/// a byte oriented mode, which is more complicated. But a byte oriented mode
/// can support everything.
///
/// # Differences with DFAs
///
/// At the theoretical level, the precise difference between an NFA and a DFA
/// is that, in a DFA, for every state, an input symbol unambiguously refers
/// to a single transition _and_ that an input symbol is required for each
/// transition. At a practical level, this permits DFA implementations to be
/// implemented at their core with a small constant number of CPU instructions
/// for each byte of input searched. In practice, this makes them quite a bit
/// faster than NFAs _in general_. Namely, in order to execute a search for any
/// Thompson NFA, one needs to keep track of a _set_ of states, and execute
/// the possible transitions on all of those states for each input symbol.
/// Overall, this results in much more overhead. To a first approximation, one
/// can expect DFA searches to be about an order of magnitude faster.
///
/// So why use an NFA at all? The main advantage of an NFA is that it takes
/// linear time (in the size of the pattern string after repetitions have been
/// expanded) to build and linear memory usage. A DFA, on the other hand, may
/// take exponential time and/or space to build. Even in non-pathological
/// cases, DFAs often take quite a bit more memory than their NFA counterparts,
/// _especially_ if large Unicode character classes are involved. Of course,
/// an NFA also provides additional capabilities. For example, it can match
/// Unicode word boundaries on non-ASCII text and resolve the positions of
/// capturing groups.
///
/// Note that a [`hybrid::regex::Regex`](crate::hybrid::regex::Regex) strikes a
/// good balance between an NFA and a DFA. It avoids the exponential build time
/// of a DFA while maintaining its fast search time. The downside of a hybrid
/// NFA/DFA is that in some cases it can be slower at search time than the NFA.
/// (It also has less functionality than a pure NFA. It cannot handle Unicode
/// word boundaries on non-ASCII text and cannot resolve capturing groups.)
///
/// # Example
///
/// This shows how to build an NFA with the default configuration and execute a
/// search using the Pike VM.
///
/// ```
/// use regex_automata::{nfa::thompson::pikevm::PikeVM, Match};
///
/// let re = PikeVM::new(r"foo[0-9]+")?;
/// let mut cache = re.create_cache();
/// let mut caps = re.create_captures();
///
/// let expected = Some(Match::must(0, 0..8));
/// re.captures(&mut cache, b"foo12345", &mut caps);
/// assert_eq!(expected, caps.get_match());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example: resolving capturing groups
///
/// This example shows how to parse some simple dates and extract the
/// components of each date via capturing groups.
///
/// ```
/// # if cfg!(miri) { return Ok(()); } // miri takes too long
/// use regex_automata::{
///     nfa::thompson::pikevm::PikeVM,
///     util::captures::Captures,
/// };
///
/// let vm = PikeVM::new(r"(?P<y>\d{4})-(?P<m>\d{2})-(?P<d>\d{2})")?;
/// let mut cache = vm.create_cache();
///
/// let haystack = "2012-03-14, 2013-01-01 and 2014-07-05";
/// let all: Vec<Captures> = vm.captures_iter(
///     &mut cache, haystack.as_bytes()
/// ).collect();
/// // There should be a total of 3 matches.
/// assert_eq!(3, all.len());
/// // The year from the second match is '2013'.
/// let span = all[1].get_group_by_name("y").unwrap();
/// assert_eq!("2013", &haystack[span]);
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// This example shows that only the last match of a capturing group is
/// reported, even if it had to match multiple times for an overall match
/// to occur.
///
/// ```
/// use regex_automata::{nfa::thompson::pikevm::PikeVM, Span};
///
/// let re = PikeVM::new(r"([a-z]){4}")?;
/// let mut cache = re.create_cache();
/// let mut caps = re.create_captures();
///
/// let haystack = b"quux";
/// re.captures(&mut cache, haystack, &mut caps);
/// assert!(caps.is_match());
/// assert_eq!(Some(Span::from(3..4)), caps.get_group(1));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct NFA(
    // We make NFAs reference counted primarily for two reasons. First is that
    // the NFA type itself is quite large (at least 0.5KB), and so it makes
    // sense to put it on the heap by default anyway. Second is that, for Arc
    // specifically, this enables cheap clones. This tends to be useful because
    // several structures (the backtracker, the Pike VM, the hybrid NFA/DFA)
    // all want to hang on to an NFA for use during search time. We could
    // provide the NFA at search time via a function argument, but this makes
    // for an unnecessarily annoying API. Instead, we just let each structure
    // share ownership of the NFA. Using a deep clone would not be smart, since
    // the NFA can use quite a bit of heap space.
    Arc<Inner>,
);

impl NFA {
    /// Parse the given regular expression using a default configuration and
    /// build an NFA from it.
    ///
    /// If you want a non-default configuration, then use the NFA
    /// [`Compiler`] with a [`Config`].
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::pikevm::PikeVM, Match};
    ///
    /// let re = PikeVM::new(r"foo[0-9]+")?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// let expected = Some(Match::must(0, 0..8));
    /// re.captures(&mut cache, b"foo12345", &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "syntax")]
    pub fn new(pattern: &str) -> Result<NFA, BuildError> {
        NFA::compiler().build(pattern)
    }

    /// Parse the given regular expressions using a default configuration and
    /// build a multi-NFA from them.
    ///
    /// If you want a non-default configuration, then use the NFA
    /// [`Compiler`] with a [`Config`].
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::pikevm::PikeVM, Match};
    ///
    /// let re = PikeVM::new_many(&["[0-9]+", "[a-z]+"])?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// let expected = Some(Match::must(1, 0..3));
    /// re.captures(&mut cache, b"foo12345bar", &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "syntax")]
    pub fn new_many<P: AsRef<str>>(patterns: &[P]) -> Result<NFA, BuildError> {
        NFA::compiler().build_many(patterns)
    }

    /// Returns an NFA with a single regex pattern that always matches at every
    /// position.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::{NFA, pikevm::PikeVM}, Match};
    ///
    /// let re = PikeVM::new_from_nfa(NFA::always_match())?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// let expected = Some(Match::must(0, 0..0));
    /// re.captures(&mut cache, b"", &mut caps);
    /// assert_eq!(expected, caps.get_match());
    /// re.captures(&mut cache, b"foo", &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn always_match() -> NFA {
        // We could use NFA::new("") here and we'd get the same semantics, but
        // hand-assembling the NFA (as below) does the same thing with a fewer
        // number of states. It also avoids needing the 'syntax' feature
        // enabled.
        //
        // Technically all we need is the "match" state, but we add the
        // "capture" states so that the PikeVM can use this NFA.
        //
        // The unwraps below are OK because we add so few states that they will
        // never exhaust any default limits in any environment.
        let mut builder = Builder::new();
        let pid = builder.start_pattern().unwrap();
        assert_eq!(pid.as_usize(), 0);
        let start_id =
            builder.add_capture_start(StateID::ZERO, 0, None).unwrap();
        let end_id = builder.add_capture_end(StateID::ZERO, 0).unwrap();
        let match_id = builder.add_match().unwrap();
        builder.patch(start_id, end_id).unwrap();
        builder.patch(end_id, match_id).unwrap();
        let pid = builder.finish_pattern(start_id).unwrap();
        assert_eq!(pid.as_usize(), 0);
        builder.build(start_id, start_id).unwrap()
    }

    /// Returns an NFA that never matches at any position.
    ///
    /// This is a convenience routine for creating an NFA with zero patterns.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::nfa::thompson::{NFA, pikevm::PikeVM};
    ///
    /// let re = PikeVM::new_from_nfa(NFA::never_match())?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// re.captures(&mut cache, b"", &mut caps);
    /// assert!(!caps.is_match());
    /// re.captures(&mut cache, b"foo", &mut caps);
    /// assert!(!caps.is_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn never_match() -> NFA {
        // This always succeeds because it only requires one NFA state, which
        // will never exhaust any (default) limits.
        let mut builder = Builder::new();
        let sid = builder.add_fail().unwrap();
        builder.build(sid, sid).unwrap()
    }

    /// Return a default configuration for an `NFA`.
    ///
    /// This is a convenience routine to avoid needing to import the `Config`
    /// type when customizing the construction of an NFA.
    ///
    /// # Example
    ///
    /// This example shows how to build an NFA with a small size limit that
    /// results in a compilation error for any regex that tries to use more
    /// heap memory than the configured limit.
    ///
    /// ```
    /// use regex_automata::nfa::thompson::{NFA, pikevm::PikeVM};
    ///
    /// let result = PikeVM::builder()
    ///     .thompson(NFA::config().nfa_size_limit(Some(1_000)))
    ///     // Remember, \w is Unicode-aware by default and thus huge.
    ///     .build(r"\w+");
    /// assert!(result.is_err());
    /// ```
    #[cfg(feature = "syntax")]
    pub fn config() -> Config {
        Config::new()
    }

    /// Return a compiler for configuring the construction of an `NFA`.
    ///
    /// This is a convenience routine to avoid needing to import the
    /// [`Compiler`] type in common cases.
    ///
    /// # Example
    ///
    /// This example shows how to build an NFA that is permitted match invalid
    /// UTF-8. Without the additional syntax configuration here, compilation of
    /// `(?-u:.)` would fail because it is permitted to match invalid UTF-8.
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson::pikevm::PikeVM,
    ///     util::syntax,
    ///     Match,
    /// };
    ///
    /// let re = PikeVM::builder()
    ///     .syntax(syntax::Config::new().utf8(false))
    ///     .build(r"[a-z]+(?-u:.)")?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// let expected = Some(Match::must(0, 1..5));
    /// re.captures(&mut cache, b"\xFFabc\xFF", &mut caps);
    /// assert_eq!(expected, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "syntax")]
    pub fn compiler() -> Compiler {
        Compiler::new()
    }

    /// Returns an iterator over all pattern identifiers in this NFA.
    ///
    /// Pattern IDs are allocated in sequential order starting from zero,
    /// where the order corresponds to the order of patterns provided to the
    /// [`NFA::new_many`] constructor.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::NFA, PatternID};
    ///
    /// let nfa = NFA::new_many(&["[0-9]+", "[a-z]+", "[A-Z]+"])?;
    /// let pids: Vec<PatternID> = nfa.patterns().collect();
    /// assert_eq!(pids, vec![
    ///     PatternID::must(0),
    ///     PatternID::must(1),
    ///     PatternID::must(2),
    /// ]);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn patterns(&self) -> PatternIter<'_> {
        PatternIter {
            it: PatternID::iter(self.pattern_len()),
            _marker: core::marker::PhantomData,
        }
    }

    /// Returns the total number of regex patterns in this NFA.
    ///
    /// This may return zero if the NFA was constructed with no patterns. In
    /// this case, the NFA can never produce a match for any input.
    ///
    /// This is guaranteed to be no bigger than [`PatternID::LIMIT`] because
    /// NFA construction will fail if too many patterns are added.
    ///
    /// It is always true that `nfa.patterns().count() == nfa.pattern_len()`.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::nfa::thompson::NFA;
    ///
    /// let nfa = NFA::new_many(&["[0-9]+", "[a-z]+", "[A-Z]+"])?;
    /// assert_eq!(3, nfa.pattern_len());
    ///
    /// let nfa = NFA::never_match();
    /// assert_eq!(0, nfa.pattern_len());
    ///
    /// let nfa = NFA::always_match();
    /// assert_eq!(1, nfa.pattern_len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn pattern_len(&self) -> usize {
        self.0.start_pattern.len()
    }

    /// Return the state identifier of the initial anchored state of this NFA.
    ///
    /// The returned identifier is guaranteed to be a valid index into the
    /// slice returned by [`NFA::states`], and is also a valid argument to
    /// [`NFA::state`].
    ///
    /// # Example
    ///
    /// This example shows a somewhat contrived example where we can easily
    /// predict the anchored starting state.
    ///
    /// ```
    /// use regex_automata::nfa::thompson::{NFA, State, WhichCaptures};
    ///
    /// let nfa = NFA::compiler()
    ///     .configure(NFA::config().which_captures(WhichCaptures::None))
    ///     .build("a")?;
    /// let state = nfa.state(nfa.start_anchored());
    /// match *state {
    ///     State::ByteRange { trans } => {
    ///         assert_eq!(b'a', trans.start);
    ///         assert_eq!(b'a', trans.end);
    ///     }
    ///     _ => unreachable!("unexpected state"),
    /// }
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn start_anchored(&self) -> StateID {
        self.0.start_anchored
    }

    /// Return the state identifier of the initial unanchored state of this
    /// NFA.
    ///
    /// This is equivalent to the identifier returned by
    /// [`NFA::start_anchored`] when the NFA has no unanchored starting state.
    ///
    /// The returned identifier is guaranteed to be a valid index into the
    /// slice returned by [`NFA::states`], and is also a valid argument to
    /// [`NFA::state`].
    ///
    /// # Example
    ///
    /// This example shows that the anchored and unanchored starting states
    /// are equivalent when an anchored NFA is built.
    ///
    /// ```
    /// use regex_automata::nfa::thompson::NFA;
    ///
    /// let nfa = NFA::new("^a")?;
    /// assert_eq!(nfa.start_anchored(), nfa.start_unanchored());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn start_unanchored(&self) -> StateID {
        self.0.start_unanchored
    }

    /// Return the state identifier of the initial anchored state for the given
    /// pattern, or `None` if there is no pattern corresponding to the given
    /// identifier.
    ///
    /// If one uses the starting state for a particular pattern, then the only
    /// match that can be returned is for the corresponding pattern.
    ///
    /// The returned identifier is guaranteed to be a valid index into the
    /// slice returned by [`NFA::states`], and is also a valid argument to
    /// [`NFA::state`].
    ///
    /// # Errors
    ///
    /// If the pattern doesn't exist in this NFA, then this returns an error.
    /// This occurs when `pid.as_usize() >= nfa.pattern_len()`.
    ///
    /// # Example
    ///
    /// This example shows that the anchored and unanchored starting states
    /// are equivalent when an anchored NFA is built.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::NFA, PatternID};
    ///
    /// let nfa = NFA::new_many(&["^a", "^b"])?;
    /// // The anchored and unanchored states for the entire NFA are the same,
    /// // since all of the patterns are anchored.
    /// assert_eq!(nfa.start_anchored(), nfa.start_unanchored());
    /// // But the anchored starting states for each pattern are distinct,
    /// // because these starting states can only lead to matches for the
    /// // corresponding pattern.
    /// let anchored = Some(nfa.start_anchored());
    /// assert_ne!(anchored, nfa.start_pattern(PatternID::must(0)));
    /// assert_ne!(anchored, nfa.start_pattern(PatternID::must(1)));
    /// // Requesting a pattern not in the NFA will result in None:
    /// assert_eq!(None, nfa.start_pattern(PatternID::must(2)));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn start_pattern(&self, pid: PatternID) -> Option<StateID> {
        self.0.start_pattern.get(pid.as_usize()).copied()
    }

    /// Get the byte class set for this NFA.
    ///
    /// A byte class set is a partitioning of this NFA's alphabet into
    /// equivalence classes. Any two bytes in the same equivalence class are
    /// guaranteed to never discriminate between a match or a non-match. (The
    /// partitioning may not be minimal.)
    ///
    /// Byte classes are used internally by this crate when building DFAs.
    /// Namely, among other optimizations, they enable a space optimization
    /// where the DFA's internal alphabet is defined over the equivalence
    /// classes of bytes instead of all possible byte values. The former is
    /// often quite a bit smaller than the latter, which permits the DFA to use
    /// less space for its transition table.
    #[inline]
    pub(crate) fn byte_class_set(&self) -> &ByteClassSet {
        &self.0.byte_class_set
    }

    /// Get the byte classes for this NFA.
    ///
    /// Byte classes represent a partitioning of this NFA's alphabet into
    /// equivalence classes. Any two bytes in the same equivalence class are
    /// guaranteed to never discriminate between a match or a non-match. (The
    /// partitioning may not be minimal.)
    ///
    /// Byte classes are used internally by this crate when building DFAs.
    /// Namely, among other optimizations, they enable a space optimization
    /// where the DFA's internal alphabet is defined over the equivalence
    /// classes of bytes instead of all possible byte values. The former is
    /// often quite a bit smaller than the latter, which permits the DFA to use
    /// less space for its transition table.
    ///
    /// # Example
    ///
    /// This example shows how to query the class of various bytes.
    ///
    /// ```
    /// use regex_automata::nfa::thompson::NFA;
    ///
    /// let nfa = NFA::new("[a-z]+")?;
    /// let classes = nfa.byte_classes();
    /// // 'a' and 'z' are in the same class for this regex.
    /// assert_eq!(classes.get(b'a'), classes.get(b'z'));
    /// // But 'a' and 'A' are not.
    /// assert_ne!(classes.get(b'a'), classes.get(b'A'));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn byte_classes(&self) -> &ByteClasses {
        &self.0.byte_classes
    }

    /// Return a reference to the NFA state corresponding to the given ID.
    ///
    /// This is a convenience routine for `nfa.states()[id]`.
    ///
    /// # Panics
    ///
    /// This panics when the given identifier does not reference a valid state.
    /// That is, when `id.as_usize() >= nfa.states().len()`.
    ///
    /// # Example
    ///
    /// The anchored state for a pattern will typically correspond to a
    /// capturing state for that pattern. (Although, this is not an API
    /// guarantee!)
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::{NFA, State}, PatternID};
    ///
    /// let nfa = NFA::new("a")?;
    /// let state = nfa.state(nfa.start_pattern(PatternID::ZERO).unwrap());
    /// match *state {
    ///     State::Capture { slot, .. } => {
    ///         assert_eq!(0, slot.as_usize());
    ///     }
    ///     _ => unreachable!("unexpected state"),
    /// }
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn state(&self, id: StateID) -> &State {
        &self.states()[id]
    }

    /// Returns a slice of all states in this NFA.
    ///
    /// The slice returned is indexed by `StateID`. This provides a convenient
    /// way to access states while following transitions among those states.
    ///
    /// # Example
    ///
    /// This demonstrates that disabling UTF-8 mode can shrink the size of the
    /// NFA considerably in some cases, especially when using Unicode character
    /// classes.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::nfa::thompson::NFA;
    ///
    /// let nfa_unicode = NFA::new(r"\w")?;
    /// let nfa_ascii = NFA::new(r"(?-u)\w")?;
    /// // Yes, a factor of 45 difference. No lie.
    /// assert!(40 * nfa_ascii.states().len() < nfa_unicode.states().len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn states(&self) -> &[State] {
        &self.0.states
    }

    /// Returns the capturing group info for this NFA.
    ///
    /// The [`GroupInfo`] provides a way to map to and from capture index
    /// and capture name for each pattern. It also provides a mapping from
    /// each of the capturing groups in every pattern to their corresponding
    /// slot offsets encoded in [`State::Capture`] states.
    ///
    /// Note that `GroupInfo` uses reference counting internally, such that
    /// cloning a `GroupInfo` is very cheap.
    ///
    /// # Example
    ///
    /// This example shows how to get a list of all capture group names for
    /// a particular pattern.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::NFA, PatternID};
    ///
    /// let nfa = NFA::new(r"(a)(?P<foo>b)(c)(d)(?P<bar>e)")?;
    /// // The first is the implicit group that is always unnammed. The next
    /// // 5 groups are the explicit groups found in the concrete syntax above.
    /// let expected = vec![None, None, Some("foo"), None, None, Some("bar")];
    /// let got: Vec<Option<&str>> =
    ///     nfa.group_info().pattern_names(PatternID::ZERO).collect();
    /// assert_eq!(expected, got);
    ///
    /// // Using an invalid pattern ID will result in nothing yielded.
    /// let got = nfa.group_info().pattern_names(PatternID::must(999)).count();
    /// assert_eq!(0, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn group_info(&self) -> &GroupInfo {
        &self.0.group_info()
    }

    /// Returns true if and only if this NFA has at least one
    /// [`Capture`](State::Capture) in its sequence of states.
    ///
    /// This is useful as a way to perform a quick test before attempting
    /// something that does or does not require capture states. For example,
    /// some regex engines (like the PikeVM) require capture states in order to
    /// work at all.
    ///
    /// # Example
    ///
    /// This example shows a few different NFAs and whether they have captures
    /// or not.
    ///
    /// ```
    /// use regex_automata::nfa::thompson::{NFA, WhichCaptures};
    ///
    /// // Obviously has capture states.
    /// let nfa = NFA::new("(a)")?;
    /// assert!(nfa.has_capture());
    ///
    /// // Less obviously has capture states, because every pattern has at
    /// // least one anonymous capture group corresponding to the match for the
    /// // entire pattern.
    /// let nfa = NFA::new("a")?;
    /// assert!(nfa.has_capture());
    ///
    /// // Other than hand building your own NFA, this is the only way to build
    /// // an NFA without capturing groups. In general, you should only do this
    /// // if you don't intend to use any of the NFA-oriented regex engines.
    /// // Overall, capturing groups don't have many downsides. Although they
    /// // can add a bit of noise to simple NFAs, so it can be nice to disable
    /// // them for debugging purposes.
    /// //
    /// // Notice that 'has_capture' is false here even when we have an
    /// // explicit capture group in the pattern.
    /// let nfa = NFA::compiler()
    ///     .configure(NFA::config().which_captures(WhichCaptures::None))
    ///     .build("(a)")?;
    /// assert!(!nfa.has_capture());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn has_capture(&self) -> bool {
        self.0.has_capture
    }

    /// Returns true if and only if this NFA can match the empty string.
    /// When it returns false, all possible matches are guaranteed to have a
    /// non-zero length.
    ///
    /// This is useful as cheap way to know whether code needs to handle the
    /// case of a zero length match. This is particularly important when UTF-8
    /// modes are enabled, as when UTF-8 mode is enabled, empty matches that
    /// split a codepoint must never be reported. This extra handling can
    /// sometimes be costly, and since regexes matching an empty string are
    /// somewhat rare, it can be beneficial to treat such regexes specially.
    ///
    /// # Example
    ///
    /// This example shows a few different NFAs and whether they match the
    /// empty string or not. Notice the empty string isn't merely a matter
    /// of a string of length literally `0`, but rather, whether a match can
    /// occur between specific pairs of bytes.
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::NFA, util::syntax};
    ///
    /// // The empty regex matches the empty string.
    /// let nfa = NFA::new("")?;
    /// assert!(nfa.has_empty(), "empty matches empty");
    /// // The '+' repetition operator requires at least one match, and so
    /// // does not match the empty string.
    /// let nfa = NFA::new("a+")?;
    /// assert!(!nfa.has_empty(), "+ does not match empty");
    /// // But the '*' repetition operator does.
    /// let nfa = NFA::new("a*")?;
    /// assert!(nfa.has_empty(), "* does match empty");
    /// // And wrapping '+' in an operator that can match an empty string also
    /// // causes it to match the empty string too.
    /// let nfa = NFA::new("(a+)*")?;
    /// assert!(nfa.has_empty(), "+ inside of * matches empty");
    ///
    /// // If a regex is just made of a look-around assertion, even if the
    /// // assertion requires some kind of non-empty string around it (such as
    /// // \b), then it is still treated as if it matches the empty string.
    /// // Namely, if a match occurs of just a look-around assertion, then the
    /// // match returned is empty.
    /// let nfa = NFA::compiler()
    ///     .syntax(syntax::Config::new().utf8(false))
    ///     .build(r"^$\A\z\b\B(?-u:\b\B)")?;
    /// assert!(nfa.has_empty(), "assertions match empty");
    /// // Even when an assertion is wrapped in a '+', it still matches the
    /// // empty string.
    /// let nfa = NFA::new(r"\b+")?;
    /// assert!(nfa.has_empty(), "+ of an assertion matches empty");
    ///
    /// // An alternation with even one branch that can match the empty string
    /// // is also said to match the empty string overall.
    /// let nfa = NFA::new("foo|(bar)?|quux")?;
    /// assert!(nfa.has_empty(), "alternations can match empty");
    ///
    /// // An NFA that matches nothing does not match the empty string.
    /// let nfa = NFA::new("[a&&b]")?;
    /// assert!(!nfa.has_empty(), "never matching means not matching empty");
    /// // But if it's wrapped in something that doesn't require a match at
    /// // all, then it can match the empty string!
    /// let nfa = NFA::new("[a&&b]*")?;
    /// assert!(nfa.has_empty(), "* on never-match still matches empty");
    /// // Since a '+' requires a match, using it on something that can never
    /// // match will itself produce a regex that can never match anything,
    /// // and thus does not match the empty string.
    /// let nfa = NFA::new("[a&&b]+")?;
    /// assert!(!nfa.has_empty(), "+ on never-match still matches nothing");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn has_empty(&self) -> bool {
        self.0.has_empty
    }

    /// Whether UTF-8 mode is enabled for this NFA or not.
    ///
    /// When UTF-8 mode is enabled, all matches reported by a regex engine
    /// derived from this NFA are guaranteed to correspond to spans of valid
    /// UTF-8. This includes zero-width matches. For example, the regex engine
    /// must guarantee that the empty regex will not match at the positions
    /// between code units in the UTF-8 encoding of a single codepoint.
    ///
    /// See [`Config::utf8`] for more information.
    ///
    /// This is enabled by default.
    ///
    /// # Example
    ///
    /// This example shows how UTF-8 mode can impact the match spans that may
    /// be reported in certain cases.
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson::{self, pikevm::PikeVM},
    ///     Match, Input,
    /// };
    ///
    /// let re = PikeVM::new("")?;
    /// let (mut cache, mut caps) = (re.create_cache(), re.create_captures());
    ///
    /// // UTF-8 mode is enabled by default.
    /// let mut input = Input::new("☃");
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(Some(Match::must(0, 0..0)), caps.get_match());
    ///
    /// // Even though an empty regex matches at 1..1, our next match is
    /// // 3..3 because 1..1 and 2..2 split the snowman codepoint (which is
    /// // three bytes long).
    /// input.set_start(1);
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(Some(Match::must(0, 3..3)), caps.get_match());
    ///
    /// // But if we disable UTF-8, then we'll get matches at 1..1 and 2..2:
    /// let re = PikeVM::builder()
    ///     .thompson(thompson::Config::new().utf8(false))
    ///     .build("")?;
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(Some(Match::must(0, 1..1)), caps.get_match());
    ///
    /// input.set_start(2);
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(Some(Match::must(0, 2..2)), caps.get_match());
    ///
    /// input.set_start(3);
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(Some(Match::must(0, 3..3)), caps.get_match());
    ///
    /// input.set_start(4);
    /// re.search(&mut cache, &input, &mut caps);
    /// assert_eq!(None, caps.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn is_utf8(&self) -> bool {
        self.0.utf8
    }

    /// Returns true when this NFA is meant to be matched in reverse.
    ///
    /// Generally speaking, when this is true, it means the NFA is supposed to
    /// be used in conjunction with moving backwards through the haystack. That
    /// is, from a higher memory address to a lower memory address.
    ///
    /// It is often the case that lower level routines dealing with an NFA
    /// don't need to care about whether it is "meant" to be matched in reverse
    /// or not. However, there are some specific cases where it matters. For
    /// example, the implementation of CRLF-aware `^` and `$` line anchors
    /// needs to know whether the search is in the forward or reverse
    /// direction. In the forward direction, neither `^` nor `$` should match
    /// when a `\r` has been seen previously and a `\n` is next. However, in
    /// the reverse direction, neither `^` nor `$` should match when a `\n`
    /// has been seen previously and a `\r` is next. This fundamentally changes
    /// how the state machine is constructed, and thus needs to be altered
    /// based on the direction of the search.
    ///
    /// This is automatically set when using a [`Compiler`] with a configuration
    /// where [`Config::reverse`] is enabled. If you're building your own NFA
    /// by hand via a [`Builder`]
    #[inline]
    pub fn is_reverse(&self) -> bool {
        self.0.reverse
    }

    /// Returns true if and only if all starting states for this NFA correspond
    /// to the beginning of an anchored search.
    ///
    /// Typically, an NFA will have both an anchored and an unanchored starting
    /// state. Namely, because it tends to be useful to have both and the cost
    /// of having an unanchored starting state is almost zero (for an NFA).
    /// However, if all patterns in the NFA are themselves anchored, then even
    /// the unanchored starting state will correspond to an anchored search
    /// since the pattern doesn't permit anything else.
    ///
    /// # Example
    ///
    /// This example shows a few different scenarios where this method's
    /// return value varies.
    ///
    /// ```
    /// use regex_automata::nfa::thompson::NFA;
    ///
    /// // The unanchored starting state permits matching this pattern anywhere
    /// // in a haystack, instead of just at the beginning.
    /// let nfa = NFA::new("a")?;
    /// assert!(!nfa.is_always_start_anchored());
    ///
    /// // In this case, the pattern is itself anchored, so there is no way
    /// // to run an unanchored search.
    /// let nfa = NFA::new("^a")?;
    /// assert!(nfa.is_always_start_anchored());
    ///
    /// // When multiline mode is enabled, '^' can match at the start of a line
    /// // in addition to the start of a haystack, so an unanchored search is
    /// // actually possible.
    /// let nfa = NFA::new("(?m)^a")?;
    /// assert!(!nfa.is_always_start_anchored());
    ///
    /// // Weird cases also work. A pattern is only considered anchored if all
    /// // matches may only occur at the start of a haystack.
    /// let nfa = NFA::new("(^a)|a")?;
    /// assert!(!nfa.is_always_start_anchored());
    ///
    /// // When multiple patterns are present, if they are all anchored, then
    /// // the NFA is always anchored too.
    /// let nfa = NFA::new_many(&["^a", "^b", "^c"])?;
    /// assert!(nfa.is_always_start_anchored());
    ///
    /// // But if one pattern is unanchored, then the NFA must permit an
    /// // unanchored search.
    /// let nfa = NFA::new_many(&["^a", "b", "^c"])?;
    /// assert!(!nfa.is_always_start_anchored());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn is_always_start_anchored(&self) -> bool {
        self.start_anchored() == self.start_unanchored()
    }

    /// Returns the look-around matcher associated with this NFA.
    ///
    /// A look-around matcher determines how to match look-around assertions.
    /// In particular, some assertions are configurable. For example, the
    /// `(?m:^)` and `(?m:$)` assertions can have their line terminator changed
    /// from the default of `\n` to any other byte.
    ///
    /// If the NFA was built using a [`Compiler`], then this matcher
    /// can be set via the [`Config::look_matcher`] configuration
    /// knob. Otherwise, if you've built an NFA by hand, it is set via
    /// [`Builder::set_look_matcher`].
    ///
    /// # Example
    ///
    /// This shows how to change the line terminator for multi-line assertions.
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson::{self, pikevm::PikeVM},
    ///     util::look::LookMatcher,
    ///     Match, Input,
    /// };
    ///
    /// let mut lookm = LookMatcher::new();
    /// lookm.set_line_terminator(b'\x00');
    ///
    /// let re = PikeVM::builder()
    ///     .thompson(thompson::Config::new().look_matcher(lookm))
    ///     .build(r"(?m)^[a-z]+$")?;
    /// let mut cache = re.create_cache();
    ///
    /// // Multi-line assertions now use NUL as a terminator.
    /// assert_eq!(
    ///     Some(Match::must(0, 1..4)),
    ///     re.find(&mut cache, b"\x00abc\x00"),
    /// );
    /// // ... and \n is no longer recognized as a terminator.
    /// assert_eq!(
    ///     None,
    ///     re.find(&mut cache, b"\nabc\n"),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn look_matcher(&self) -> &LookMatcher {
        &self.0.look_matcher
    }

    /// Returns the union of all look-around assertions used throughout this
    /// NFA. When the returned set is empty, it implies that the NFA has no
    /// look-around assertions and thus zero conditional epsilon transitions.
    ///
    /// This is useful in some cases enabling optimizations. It is not
    /// unusual, for example, for optimizations to be of the form, "for any
    /// regex with zero conditional epsilon transitions, do ..." where "..."
    /// is some kind of optimization.
    ///
    /// This isn't only helpful for optimizations either. Sometimes look-around
    /// assertions are difficult to support. For example, many of the DFAs in
    /// this crate don't support Unicode word boundaries or handle them using
    /// heuristics. Handling that correctly typically requires some kind of
    /// cheap check of whether the NFA has a Unicode word boundary in the first
    /// place.
    ///
    /// # Example
    ///
    /// This example shows how this routine varies based on the regex pattern:
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::NFA, util::look::Look};
    ///
    /// // No look-around at all.
    /// let nfa = NFA::new("a")?;
    /// assert!(nfa.look_set_any().is_empty());
    ///
    /// // When multiple patterns are present, since this returns the union,
    /// // it will include look-around assertions that only appear in one
    /// // pattern.
    /// let nfa = NFA::new_many(&["a", "b", "a^b", "c"])?;
    /// assert!(nfa.look_set_any().contains(Look::Start));
    ///
    /// // Some groups of assertions have various shortcuts. For example:
    /// let nfa = NFA::new(r"(?-u:\b)")?;
    /// assert!(nfa.look_set_any().contains_word());
    /// assert!(!nfa.look_set_any().contains_word_unicode());
    /// assert!(nfa.look_set_any().contains_word_ascii());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn look_set_any(&self) -> LookSet {
        self.0.look_set_any
    }

    /// Returns the union of all prefix look-around assertions for every
    /// pattern in this NFA. When the returned set is empty, it implies none of
    /// the patterns require moving through a conditional epsilon transition
    /// before inspecting the first byte in the haystack.
    ///
    /// This can be useful for determining what kinds of assertions need to be
    /// satisfied at the beginning of a search. For example, typically DFAs
    /// in this crate will build a distinct starting state for each possible
    /// starting configuration that might result in look-around assertions
    /// being satisfied differently. However, if the set returned here is
    /// empty, then you know that the start state is invariant because there
    /// are no conditional epsilon transitions to consider.
    ///
    /// # Example
    ///
    /// This example shows how this routine varies based on the regex pattern:
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::NFA, util::look::Look};
    ///
    /// // No look-around at all.
    /// let nfa = NFA::new("a")?;
    /// assert!(nfa.look_set_prefix_any().is_empty());
    ///
    /// // When multiple patterns are present, since this returns the union,
    /// // it will include look-around assertions that only appear in one
    /// // pattern. But it will only include assertions that are in the prefix
    /// // of a pattern. For example, this includes '^' but not '$' even though
    /// // '$' does appear.
    /// let nfa = NFA::new_many(&["a", "b", "^ab$", "c"])?;
    /// assert!(nfa.look_set_prefix_any().contains(Look::Start));
    /// assert!(!nfa.look_set_prefix_any().contains(Look::End));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn look_set_prefix_any(&self) -> LookSet {
        self.0.look_set_prefix_any
    }

    // FIXME: The `look_set_prefix_all` computation was not correct, and it
    // seemed a little tricky to fix it. Since I wasn't actually using it for
    // anything, I just decided to remove it in the run up to the regex 1.9
    // release. If you need this, please file an issue.
    /*
    /// Returns the intersection of all prefix look-around assertions for every
    /// pattern in this NFA. When the returned set is empty, it implies at
    /// least one of the patterns does not require moving through a conditional
    /// epsilon transition before inspecting the first byte in the haystack.
    /// Conversely, when the set contains an assertion, it implies that every
    /// pattern in the NFA also contains that assertion in its prefix.
    ///
    /// This can be useful for determining what kinds of assertions need to be
    /// satisfied at the beginning of a search. For example, if you know that
    /// [`Look::Start`] is in the prefix intersection set returned here, then
    /// you know that all searches, regardless of input configuration, will be
    /// anchored.
    ///
    /// # Example
    ///
    /// This example shows how this routine varies based on the regex pattern:
    ///
    /// ```
    /// use regex_automata::{nfa::thompson::NFA, util::look::Look};
    ///
    /// // No look-around at all.
    /// let nfa = NFA::new("a")?;
    /// assert!(nfa.look_set_prefix_all().is_empty());
    ///
    /// // When multiple patterns are present, since this returns the
    /// // intersection, it will only include assertions present in every
    /// // prefix, and only the prefix.
    /// let nfa = NFA::new_many(&["^a$", "^b$", "$^ab$", "^c$"])?;
    /// assert!(nfa.look_set_prefix_all().contains(Look::Start));
    /// assert!(!nfa.look_set_prefix_all().contains(Look::End));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn look_set_prefix_all(&self) -> LookSet {
        self.0.look_set_prefix_all
    }
    */

    /// Returns the memory usage, in bytes, of this NFA.
    ///
    /// This does **not** include the stack size used up by this NFA. To
    /// compute that, use `std::mem::size_of::<NFA>()`.
    ///
    /// # Example
    ///
    /// This example shows that large Unicode character classes can use quite
    /// a bit of memory.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::nfa::thompson::NFA;
    ///
    /// let nfa_unicode = NFA::new(r"\w")?;
    /// let nfa_ascii = NFA::new(r"(?-u:\w)")?;
    ///
    /// assert!(10 * nfa_ascii.memory_usage() < nfa_unicode.memory_usage());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn memory_usage(&self) -> usize {
        use core::mem::size_of;

        size_of::<Inner>() // allocated on the heap via Arc
            + self.0.states.len() * size_of::<State>()
            + self.0.start_pattern.len() * size_of::<StateID>()
            + self.0.group_info.memory_usage()
            + self.0.memory_extra
    }
}

impl fmt::Debug for NFA {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// The "inner" part of the NFA. We split this part out so that we can easily
/// wrap it in an `Arc` above in the definition of `NFA`.
///
/// See builder.rs for the code that actually builds this type. This module
/// does provide (internal) mutable methods for adding things to this
/// NFA before finalizing it, but the high level construction process is
/// controlled by the builder abstraction. (Which is complicated enough to
/// get its own module.)
#[derive(Default)]
pub(super) struct Inner {
    /// The state sequence. This sequence is guaranteed to be indexable by all
    /// starting state IDs, and it is also guaranteed to contain at most one
    /// `Match` state for each pattern compiled into this NFA. (A pattern may
    /// not have a corresponding `Match` state if a `Match` state is impossible
    /// to reach.)
    states: Vec<State>,
    /// The anchored starting state of this NFA.
    start_anchored: StateID,
    /// The unanchored starting state of this NFA.
    start_unanchored: StateID,
    /// The starting states for each individual pattern. Starting at any
    /// of these states will result in only an anchored search for the
    /// corresponding pattern. The vec is indexed by pattern ID. When the NFA
    /// contains a single regex, then `start_pattern[0]` and `start_anchored`
    /// are always equivalent.
    start_pattern: Vec<StateID>,
    /// Info about the capturing groups in this NFA. This is responsible for
    /// mapping groups to slots, mapping groups to names and names to groups.
    group_info: GroupInfo,
    /// A representation of equivalence classes over the transitions in this
    /// NFA. Two bytes in the same equivalence class must not discriminate
    /// between a match or a non-match. This map can be used to shrink the
    /// total size of a DFA's transition table with a small match-time cost.
    ///
    /// Note that the NFA's transitions are *not* defined in terms of these
    /// equivalence classes. The NFA's transitions are defined on the original
    /// byte values. For the most part, this is because they wouldn't really
    /// help the NFA much since the NFA already uses a sparse representation
    /// to represent transitions. Byte classes are most effective in a dense
    /// representation.
    byte_class_set: ByteClassSet,
    /// This is generated from `byte_class_set`, and essentially represents the
    /// same thing but supports different access patterns. Namely, this permits
    /// looking up the equivalence class of a byte very cheaply.
    ///
    /// Ideally we would just store this, but because of annoying code
    /// structure reasons, we keep both this and `byte_class_set` around for
    /// now. I think I would prefer that `byte_class_set` were computed in the
    /// `Builder`, but right now, we compute it as states are added to the
    /// `NFA`.
    byte_classes: ByteClasses,
    /// Whether this NFA has a `Capture` state anywhere.
    has_capture: bool,
    /// When the empty string is in the language matched by this NFA.
    has_empty: bool,
    /// Whether UTF-8 mode is enabled for this NFA. Briefly, this means that
    /// all non-empty matches produced by this NFA correspond to spans of valid
    /// UTF-8, and any empty matches produced by this NFA that split a UTF-8
    /// encoded codepoint should be filtered out by the corresponding regex
    /// engine.
    utf8: bool,
    /// Whether this NFA is meant to be matched in reverse or not.
    reverse: bool,
    /// The matcher to be used for look-around assertions.
    look_matcher: LookMatcher,
    /// The union of all look-around assertions that occur anywhere within
    /// this NFA. If this set is empty, then it means there are precisely zero
    /// conditional epsilon transitions in the NFA.
    look_set_any: LookSet,
    /// The union of all look-around assertions that occur as a zero-length
    /// prefix for any of the patterns in this NFA.
    look_set_prefix_any: LookSet,
    /*
    /// The intersection of all look-around assertions that occur as a
    /// zero-length prefix for any of the patterns in this NFA.
    look_set_prefix_all: LookSet,
    */
    /// Heap memory used indirectly by NFA states and other things (like the
    /// various capturing group representations above). Since each state
    /// might use a different amount of heap, we need to keep track of this
    /// incrementally.
    memory_extra: usize,
}

impl Inner {
    /// Runs any last finalization bits and turns this into a full NFA.
    pub(super) fn into_nfa(mut self) -> NFA {
        self.byte_classes = self.byte_class_set.byte_classes();
        // Do epsilon closure from the start state of every pattern in order
        // to compute various properties such as look-around assertions and
        // whether the empty string can be matched.
        let mut stack = vec![];
        let mut seen = SparseSet::new(self.states.len());
        for &start_id in self.start_pattern.iter() {
            stack.push(start_id);
            seen.clear();
            // let mut prefix_all = LookSet::full();
            let mut prefix_any = LookSet::empty();
            while let Some(sid) = stack.pop() {
                if !seen.insert(sid) {
                    continue;
                }
                match self.states[sid] {
                    State::ByteRange { .. }
                    | State::Dense { .. }
                    | State::Fail => continue,
                    State::Sparse(_) => {
                        // This snippet below will rewrite this sparse state
                        // as a dense state. By doing it here, we apply this
                        // optimization to all hot "sparse" states since these
                        // are the states that are reachable from the start
                        // state via an epsilon closure.
                        //
                        // Unfortunately, this optimization did not seem to
                        // help much in some very limited ad hoc benchmarking.
                        //
                        // I left the 'Dense' state type in place in case we
                        // want to revisit this, but I suspect the real way
                        // to make forward progress is a more fundamental
                        // rearchitecting of how data in the NFA is laid out.
                        // I think we should consider a single contiguous
                        // allocation instead of all this indirection and
                        // potential heap allocations for every state. But this
                        // is a large re-design and will require API breaking
                        // changes.
                        // self.memory_extra -= self.states[sid].memory_usage();
                        // let trans = DenseTransitions::from_sparse(sparse);
                        // self.states[sid] = State::Dense(trans);
                        // self.memory_extra += self.states[sid].memory_usage();
                        continue;
                    }
                    State::Match { .. } => self.has_empty = true,
                    State::Look { look, next } => {
                        prefix_any = prefix_any.insert(look);
                        stack.push(next);
                    }
                    State::Union { ref alternates } => {
                        // Order doesn't matter here, since we're just dealing
                        // with look-around sets. But if we do richer analysis
                        // here that needs to care about preference order, then
                        // this should be done in reverse.
                        stack.extend(alternates.iter());
                    }
                    State::BinaryUnion { alt1, alt2 } => {
                        stack.push(alt2);
                        stack.push(alt1);
                    }
                    State::Capture { next, .. } => {
                        stack.push(next);
                    }
                }
            }
            self.look_set_prefix_any =
                self.look_set_prefix_any.union(prefix_any);
        }
        NFA(Arc::new(self))
    }

    /// Returns the capturing group info for this NFA.
    pub(super) fn group_info(&self) -> &GroupInfo {
        &self.group_info
    }

    /// Add the given state to this NFA after allocating a fresh identifier for
    /// it.
    ///
    /// This panics if too many states are added such that a fresh identifier
    /// could not be created. (Currently, the only caller of this routine is
    /// a `Builder`, and it upholds this invariant.)
    pub(super) fn add(&mut self, state: State) -> StateID {
        match state {
            State::ByteRange { ref trans } => {
                self.byte_class_set.set_range(trans.start, trans.end);
            }
            State::Sparse(ref sparse) => {
                for trans in sparse.transitions.iter() {
                    self.byte_class_set.set_range(trans.start, trans.end);
                }
            }
            State::Dense { .. } => unreachable!(),
            State::Look { look, .. } => {
                self.look_matcher
                    .add_to_byteset(look, &mut self.byte_class_set);
                self.look_set_any = self.look_set_any.insert(look);
            }
            State::Capture { .. } => {
                self.has_capture = true;
            }
            State::Union { .. }
            | State::BinaryUnion { .. }
            | State::Fail
            | State::Match { .. } => {}
        }

        let id = StateID::new(self.states.len()).unwrap();
        self.memory_extra += state.memory_usage();
        self.states.push(state);
        id
    }

    /// Set the starting state identifiers for this NFA.
    ///
    /// `start_anchored` and `start_unanchored` may be equivalent. When they
    /// are, then the NFA can only execute anchored searches. This might
    /// occur, for example, for patterns that are unconditionally anchored.
    /// e.g., `^foo`.
    pub(super) fn set_starts(
        &mut self,
        start_anchored: StateID,
        start_unanchored: StateID,
        start_pattern: &[StateID],
    ) {
        self.start_anchored = start_anchored;
        self.start_unanchored = start_unanchored;
        self.start_pattern = start_pattern.to_vec();
    }

    /// Sets the UTF-8 mode of this NFA.
    pub(super) fn set_utf8(&mut self, yes: bool) {
        self.utf8 = yes;
    }

    /// Sets the reverse mode of this NFA.
    pub(super) fn set_reverse(&mut self, yes: bool) {
        self.reverse = yes;
    }

    /// Sets the look-around assertion matcher for this NFA.
    pub(super) fn set_look_matcher(&mut self, m: LookMatcher) {
        self.look_matcher = m;
    }

    /// Set the capturing groups for this NFA.
    ///
    /// The given slice should contain the capturing groups for each pattern,
    /// The capturing groups in turn should correspond to the total number of
    /// capturing groups in the pattern, including the anonymous first capture
    /// group for each pattern. If a capturing group does have a name, then it
    /// should be provided as a Arc<str>.
    ///
    /// This returns an error if a corresponding `GroupInfo` could not be
    /// built.
    pub(super) fn set_captures(
        &mut self,
        captures: &[Vec<Option<Arc<str>>>],
    ) -> Result<(), GroupInfoError> {
        self.group_info = GroupInfo::new(
            captures.iter().map(|x| x.iter().map(|y| y.as_ref())),
        )?;
        Ok(())
    }

    /// Remap the transitions in every state of this NFA using the given map.
    /// The given map should be indexed according to state ID namespace used by
    /// the transitions of the states currently in this NFA.
    ///
    /// This is particularly useful to the NFA builder, since it is convenient
    /// to add NFA states in order to produce their final IDs. Then, after all
    /// of the intermediate "empty" states (unconditional epsilon transitions)
    /// have been removed from the builder's representation, we can re-map all
    /// of the transitions in the states already added to their final IDs.
    pub(super) fn remap(&mut self, old_to_new: &[StateID]) {
        for state in &mut self.states {
            state.remap(old_to_new);
        }
        self.start_anchored = old_to_new[self.start_anchored];
        self.start_unanchored = old_to_new[self.start_unanchored];
        for id in self.start_pattern.iter_mut() {
            *id = old_to_new[*id];
        }
    }
}

impl fmt::Debug for Inner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "thompson::NFA(")?;
        for (sid, state) in self.states.iter().with_state_ids() {
            let status = if sid == self.start_anchored {
                '^'
            } else if sid == self.start_unanchored {
                '>'
            } else {
                ' '
            };
            writeln!(f, "{}{:06?}: {:?}", status, sid.as_usize(), state)?;
        }
        let pattern_len = self.start_pattern.len();
        if pattern_len > 1 {
            writeln!(f, "")?;
            for pid in 0..pattern_len {
                let sid = self.start_pattern[pid];
                writeln!(f, "START({:06?}): {:?}", pid, sid.as_usize())?;
            }
        }
        writeln!(f, "")?;
        writeln!(
            f,
            "transition equivalence classes: {:?}",
            self.byte_classes,
        )?;
        writeln!(f, ")")?;
        Ok(())
    }
}

/// A state in an NFA.
///
/// In theory, it can help to conceptualize an `NFA` as a graph consisting of
/// `State`s. Each `State` contains its complete set of outgoing transitions.
///
/// In practice, it can help to conceptualize an `NFA` as a sequence of
/// instructions for a virtual machine. Each `State` says what to do and where
/// to go next.
///
/// Strictly speaking, the practical interpretation is the most correct one,
/// because of the [`Capture`](State::Capture) state. Namely, a `Capture`
/// state always forwards execution to another state unconditionally. Its only
/// purpose is to cause a side effect: the recording of the current input
/// position at a particular location in memory. In this sense, an `NFA`
/// has more power than a theoretical non-deterministic finite automaton.
///
/// For most uses of this crate, it is likely that one may never even need to
/// be aware of this type at all. The main use cases for looking at `State`s
/// directly are if you need to write your own search implementation or if you
/// need to do some kind of analysis on the NFA.
#[derive(Clone, Eq, PartialEq)]
pub enum State {
    /// A state with a single transition that can only be taken if the current
    /// input symbol is in a particular range of bytes.
    ByteRange {
        /// The transition from this state to the next.
        trans: Transition,
    },
    /// A state with possibly many transitions represented in a sparse fashion.
    /// Transitions are non-overlapping and ordered lexicographically by input
    /// range.
    ///
    /// In practice, this is used for encoding UTF-8 automata. Its presence is
    /// primarily an optimization that avoids many additional unconditional
    /// epsilon transitions (via [`Union`](State::Union) states), and thus
    /// decreases the overhead of traversing the NFA. This can improve both
    /// matching time and DFA construction time.
    Sparse(SparseTransitions),
    /// A dense representation of a state with multiple transitions.
    Dense(DenseTransitions),
    /// A conditional epsilon transition satisfied via some sort of
    /// look-around. Look-around is limited to anchor and word boundary
    /// assertions.
    ///
    /// Look-around states are meant to be evaluated while performing epsilon
    /// closure (computing the set of states reachable from a particular state
    /// via only epsilon transitions). If the current position in the haystack
    /// satisfies the look-around assertion, then you're permitted to follow
    /// that epsilon transition.
    Look {
        /// The look-around assertion that must be satisfied before moving
        /// to `next`.
        look: Look,
        /// The state to transition to if the look-around assertion is
        /// satisfied.
        next: StateID,
    },
    /// An alternation such that there exists an epsilon transition to all
    /// states in `alternates`, where matches found via earlier transitions
    /// are preferred over later transitions.
    Union {
        /// An ordered sequence of unconditional epsilon transitions to other
        /// states. Transitions earlier in the sequence are preferred over
        /// transitions later in the sequence.
        alternates: Box<[StateID]>,
    },
    /// An alternation such that there exists precisely two unconditional
    /// epsilon transitions, where matches found via `alt1` are preferred over
    /// matches found via `alt2`.
    ///
    /// This state exists as a common special case of Union where there are
    /// only two alternates. In this case, we don't need any allocations to
    /// represent the state. This saves a bit of memory and also saves an
    /// additional memory access when traversing the NFA.
    BinaryUnion {
        /// An unconditional epsilon transition to another NFA state. This
        /// is preferred over `alt2`.
        alt1: StateID,
        /// An unconditional epsilon transition to another NFA state. Matches
        /// reported via this transition should only be reported if no matches
        /// were found by following `alt1`.
        alt2: StateID,
    },
    /// An empty state that records a capture location.
    ///
    /// From the perspective of finite automata, this is precisely equivalent
    /// to an unconditional epsilon transition, but serves the purpose of
    /// instructing NFA simulations to record additional state when the finite
    /// state machine passes through this epsilon transition.
    ///
    /// `slot` in this context refers to the specific capture group slot
    /// offset that is being recorded. Each capturing group has two slots
    /// corresponding to the start and end of the matching portion of that
    /// group.
    ///
    /// The pattern ID and capture group index are also included in this state
    /// in case they are useful. But mostly, all you'll need is `next` and
    /// `slot`.
    Capture {
        /// The state to transition to, unconditionally.
        next: StateID,
        /// The pattern ID that this capture belongs to.
        pattern_id: PatternID,
        /// The capture group index that this capture belongs to. Capture group
        /// indices are local to each pattern. For example, when capturing
        /// groups are enabled, every pattern has a capture group at index
        /// `0`.
        group_index: SmallIndex,
        /// The slot index for this capture. Every capturing group has two
        /// slots: one for the start haystack offset and one for the end
        /// haystack offset. Unlike capture group indices, slot indices are
        /// global across all patterns in this NFA. That is, each slot belongs
        /// to a single pattern, but there is only one slot at index `i`.
        slot: SmallIndex,
    },
    /// A state that cannot be transitioned out of. This is useful for cases
    /// where you want to prevent matching from occurring. For example, if your
    /// regex parser permits empty character classes, then one could choose
    /// a `Fail` state to represent them. (An empty character class can be
    /// thought of as an empty set. Since nothing is in an empty set, they can
    /// never match anything.)
    Fail,
    /// A match state. There is at least one such occurrence of this state for
    /// each regex that can match that is in this NFA.
    Match {
        /// The matching pattern ID.
        pattern_id: PatternID,
    },
}

impl State {
    /// Returns true if and only if this state contains one or more epsilon
    /// transitions.
    ///
    /// In practice, a state has no outgoing transitions (like `Match`), has
    /// only non-epsilon transitions (like `ByteRange`) or has only epsilon
    /// transitions (like `Union`).
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson::{State, Transition},
    ///     util::primitives::{PatternID, StateID, SmallIndex},
    /// };
    ///
    /// // Capture states are epsilon transitions.
    /// let state = State::Capture {
    ///     next: StateID::ZERO,
    ///     pattern_id: PatternID::ZERO,
    ///     group_index: SmallIndex::ZERO,
    ///     slot: SmallIndex::ZERO,
    /// };
    /// assert!(state.is_epsilon());
    ///
    /// // ByteRange states are not.
    /// let state = State::ByteRange {
    ///     trans: Transition { start: b'a', end: b'z', next: StateID::ZERO },
    /// };
    /// assert!(!state.is_epsilon());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn is_epsilon(&self) -> bool {
        match *self {
            State::ByteRange { .. }
            | State::Sparse { .. }
            | State::Dense { .. }
            | State::Fail
            | State::Match { .. } => false,
            State::Look { .. }
            | State::Union { .. }
            | State::BinaryUnion { .. }
            | State::Capture { .. } => true,
        }
    }

    /// Returns the heap memory usage of this NFA state in bytes.
    fn memory_usage(&self) -> usize {
        match *self {
            State::ByteRange { .. }
            | State::Look { .. }
            | State::BinaryUnion { .. }
            | State::Capture { .. }
            | State::Match { .. }
            | State::Fail => 0,
            State::Sparse(SparseTransitions { ref transitions }) => {
                transitions.len() * mem::size_of::<Transition>()
            }
            State::Dense { .. } => 256 * mem::size_of::<StateID>(),
            State::Union { ref alternates } => {
                alternates.len() * mem::size_of::<StateID>()
            }
        }
    }

    /// Remap the transitions in this state using the given map. Namely, the
    /// given map should be indexed according to the transitions currently
    /// in this state.
    ///
    /// This is used during the final phase of the NFA compiler, which turns
    /// its intermediate NFA into the final NFA.
    fn remap(&mut self, remap: &[StateID]) {
        match *self {
            State::ByteRange { ref mut trans } => {
                trans.next = remap[trans.next]
            }
            State::Sparse(SparseTransitions { ref mut transitions }) => {
                for t in transitions.iter_mut() {
                    t.next = remap[t.next];
                }
            }
            State::Dense(DenseTransitions { ref mut transitions }) => {
                for sid in transitions.iter_mut() {
                    *sid = remap[*sid];
                }
            }
            State::Look { ref mut next, .. } => *next = remap[*next],
            State::Union { ref mut alternates } => {
                for alt in alternates.iter_mut() {
                    *alt = remap[*alt];
                }
            }
            State::BinaryUnion { ref mut alt1, ref mut alt2 } => {
                *alt1 = remap[*alt1];
                *alt2 = remap[*alt2];
            }
            State::Capture { ref mut next, .. } => *next = remap[*next],
            State::Fail => {}
            State::Match { .. } => {}
        }
    }
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            State::ByteRange { ref trans } => trans.fmt(f),
            State::Sparse(SparseTransitions { ref transitions }) => {
                let rs = transitions
                    .iter()
                    .map(|t| format!("{:?}", t))
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "sparse({})", rs)
            }
            State::Dense(ref dense) => {
                write!(f, "dense(")?;
                for (i, t) in dense.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:?}", t)?;
                }
                write!(f, ")")
            }
            State::Look { ref look, next } => {
                write!(f, "{:?} => {:?}", look, next.as_usize())
            }
            State::Union { ref alternates } => {
                let alts = alternates
                    .iter()
                    .map(|id| format!("{:?}", id.as_usize()))
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "union({})", alts)
            }
            State::BinaryUnion { alt1, alt2 } => {
                write!(
                    f,
                    "binary-union({}, {})",
                    alt1.as_usize(),
                    alt2.as_usize()
                )
            }
            State::Capture { next, pattern_id, group_index, slot } => {
                write!(
                    f,
                    "capture(pid={:?}, group={:?}, slot={:?}) => {:?}",
                    pattern_id.as_usize(),
                    group_index.as_usize(),
                    slot.as_usize(),
                    next.as_usize(),
                )
            }
            State::Fail => write!(f, "FAIL"),
            State::Match { pattern_id } => {
                write!(f, "MATCH({:?})", pattern_id.as_usize())
            }
        }
    }
}

/// A sequence of transitions used to represent a sparse state.
///
/// This is the primary representation of a [`Sparse`](State::Sparse) state.
/// It corresponds to a sorted sequence of transitions with non-overlapping
/// byte ranges. If the byte at the current position in the haystack matches
/// one of the byte ranges, then the finite state machine should take the
/// corresponding transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseTransitions {
    /// The sorted sequence of non-overlapping transitions.
    pub transitions: Box<[Transition]>,
}

impl SparseTransitions {
    /// This follows the matching transition for a particular byte.
    ///
    /// The matching transition is found by looking for a matching byte
    /// range (there is at most one) corresponding to the position `at` in
    /// `haystack`.
    ///
    /// If `at >= haystack.len()`, then this returns `None`.
    #[inline]
    pub fn matches(&self, haystack: &[u8], at: usize) -> Option<StateID> {
        haystack.get(at).and_then(|&b| self.matches_byte(b))
    }

    /// This follows the matching transition for any member of the alphabet.
    ///
    /// The matching transition is found by looking for a matching byte
    /// range (there is at most one) corresponding to the position `at` in
    /// `haystack`. If the given alphabet unit is [`EOI`](alphabet::Unit::eoi),
    /// then this always returns `None`.
    #[inline]
    pub(crate) fn matches_unit(
        &self,
        unit: alphabet::Unit,
    ) -> Option<StateID> {
        unit.as_u8().map_or(None, |byte| self.matches_byte(byte))
    }

    /// This follows the matching transition for a particular byte.
    ///
    /// The matching transition is found by looking for a matching byte range
    /// (there is at most one) corresponding to the byte given.
    #[inline]
    pub fn matches_byte(&self, byte: u8) -> Option<StateID> {
        for t in self.transitions.iter() {
            if t.start > byte {
                break;
            } else if t.matches_byte(byte) {
                return Some(t.next);
            }
        }
        None

        /*
        // This is an alternative implementation that uses binary search. In
        // some ad hoc experiments, like
        //
        //   regex-cli find match pikevm -b -p '\b\w+\b' non-ascii-file
        //
        // I could not observe any improvement, and in fact, things seemed to
        // be a bit slower. I can see an improvement in at least one benchmark:
        //
        //   regex-cli find match pikevm -b -p '\pL{100}' all-codepoints-utf8
        //
        // Where total search time goes from 3.2s to 2.4s when using binary
        // search.
        self.transitions
            .binary_search_by(|t| {
                if t.end < byte {
                    core::cmp::Ordering::Less
                } else if t.start > byte {
                    core::cmp::Ordering::Greater
                } else {
                    core::cmp::Ordering::Equal
                }
            })
            .ok()
            .map(|i| self.transitions[i].next)
        */
    }
}

/// A sequence of transitions used to represent a dense state.
///
/// This is the primary representation of a [`Dense`](State::Dense) state. It
/// provides constant time matching. That is, given a byte in a haystack and
/// a `DenseTransitions`, one can determine if the state matches in constant
/// time.
///
/// This is in contrast to `SparseTransitions`, whose time complexity is
/// necessarily bigger than constant time. Also in contrast, `DenseTransitions`
/// usually requires (much) more heap memory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DenseTransitions {
    /// A dense representation of this state's transitions on the heap. This
    /// always has length 256.
    pub transitions: Box<[StateID]>,
}

impl DenseTransitions {
    /// This follows the matching transition for a particular byte.
    ///
    /// The matching transition is found by looking for a transition that
    /// doesn't correspond to `StateID::ZERO` for the byte `at` the given
    /// position in `haystack`.
    ///
    /// If `at >= haystack.len()`, then this returns `None`.
    #[inline]
    pub fn matches(&self, haystack: &[u8], at: usize) -> Option<StateID> {
        haystack.get(at).and_then(|&b| self.matches_byte(b))
    }

    /// This follows the matching transition for any member of the alphabet.
    ///
    /// The matching transition is found by looking for a transition that
    /// doesn't correspond to `StateID::ZERO` for the byte `at` the given
    /// position in `haystack`.
    ///
    /// If `at >= haystack.len()` or if the given alphabet unit is
    /// [`EOI`](alphabet::Unit::eoi), then this returns `None`.
    #[inline]
    pub(crate) fn matches_unit(
        &self,
        unit: alphabet::Unit,
    ) -> Option<StateID> {
        unit.as_u8().map_or(None, |byte| self.matches_byte(byte))
    }

    /// This follows the matching transition for a particular byte.
    ///
    /// The matching transition is found by looking for a transition that
    /// doesn't correspond to `StateID::ZERO` for the given `byte`.
    ///
    /// If `at >= haystack.len()`, then this returns `None`.
    #[inline]
    pub fn matches_byte(&self, byte: u8) -> Option<StateID> {
        let next = self.transitions[usize::from(byte)];
        if next == StateID::ZERO {
            None
        } else {
            Some(next)
        }
    }

    /*
    /// The dense state optimization isn't currently enabled, so permit a
    /// little bit of dead code.
    pub(crate) fn from_sparse(sparse: &SparseTransitions) -> DenseTransitions {
        let mut dense = vec![StateID::ZERO; 256];
        for t in sparse.transitions.iter() {
            for b in t.start..=t.end {
                dense[usize::from(b)] = t.next;
            }
        }
        DenseTransitions { transitions: dense.into_boxed_slice() }
    }
    */

    /// Returns an iterator over all transitions that don't point to
    /// `StateID::ZERO`.
    pub(crate) fn iter(&self) -> impl Iterator<Item = Transition> + '_ {
        use crate::util::int::Usize;
        self.transitions
            .iter()
            .enumerate()
            .filter(|&(_, &sid)| sid != StateID::ZERO)
            .map(|(byte, &next)| Transition {
                start: byte.as_u8(),
                end: byte.as_u8(),
                next,
            })
    }
}

/// A single transition to another state.
///
/// This transition may only be followed if the current byte in the haystack
/// falls in the inclusive range of bytes specified.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct Transition {
    /// The inclusive start of the byte range.
    pub start: u8,
    /// The inclusive end of the byte range.
    pub end: u8,
    /// The identifier of the state to transition to.
    pub next: StateID,
}

impl Transition {
    /// Returns true if the position `at` in `haystack` falls in this
    /// transition's range of bytes.
    ///
    /// If `at >= haystack.len()`, then this returns `false`.
    pub fn matches(&self, haystack: &[u8], at: usize) -> bool {
        haystack.get(at).map_or(false, |&b| self.matches_byte(b))
    }

    /// Returns true if the given alphabet unit falls in this transition's
    /// range of bytes. If the given unit is [`EOI`](alphabet::Unit::eoi), then
    /// this returns `false`.
    pub fn matches_unit(&self, unit: alphabet::Unit) -> bool {
        unit.as_u8().map_or(false, |byte| self.matches_byte(byte))
    }

    /// Returns true if the given byte falls in this transition's range of
    /// bytes.
    pub fn matches_byte(&self, byte: u8) -> bool {
        self.start <= byte && byte <= self.end
    }
}

impl fmt::Debug for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use crate::util::escape::DebugByte;

        let Transition { start, end, next } = *self;
        if self.start == self.end {
            write!(f, "{:?} => {:?}", DebugByte(start), next.as_usize())
        } else {
            write!(
                f,
                "{:?}-{:?} => {:?}",
                DebugByte(start),
                DebugByte(end),
                next.as_usize(),
            )
        }
    }
}

/// An iterator over all pattern IDs in an NFA.
///
/// This iterator is created by [`NFA::patterns`].
///
/// The lifetime parameter `'a` refers to the lifetime of the NFA from which
/// this pattern iterator was created.
#[derive(Debug)]
pub struct PatternIter<'a> {
    it: PatternIDIter,
    /// We explicitly associate a lifetime with this iterator even though we
    /// don't actually borrow anything from the NFA. We do this for backward
    /// compatibility purposes. If we ever do need to borrow something from
    /// the NFA, then we can and just get rid of this marker without breaking
    /// the public API.
    _marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> Iterator for PatternIter<'a> {
    type Item = PatternID;

    fn next(&mut self) -> Option<PatternID> {
        self.it.next()
    }
}

#[cfg(all(test, feature = "nfa-pikevm"))]
mod tests {
    use super::*;
    use crate::{nfa::thompson::pikevm::PikeVM, Input};

    // This asserts that an NFA state doesn't have its size changed. It is
    // *really* easy to accidentally increase the size, and thus potentially
    // dramatically increase the memory usage of every NFA.
    //
    // This assert doesn't mean we absolutely cannot increase the size of an
    // NFA state. We can. It's just here to make sure we do it knowingly and
    // intentionally.
    #[test]
    fn state_has_small_size() {
        #[cfg(target_pointer_width = "64")]
        assert_eq!(24, core::mem::size_of::<State>());
        #[cfg(target_pointer_width = "32")]
        assert_eq!(20, core::mem::size_of::<State>());
    }

    #[test]
    fn always_match() {
        let re = PikeVM::new_from_nfa(NFA::always_match()).unwrap();
        let mut cache = re.create_cache();
        let mut caps = re.create_captures();
        let mut find = |haystack, start, end| {
            let input = Input::new(haystack).range(start..end);
            re.search(&mut cache, &input, &mut caps);
            caps.get_match().map(|m| m.end())
        };

        assert_eq!(Some(0), find("", 0, 0));
        assert_eq!(Some(0), find("a", 0, 1));
        assert_eq!(Some(1), find("a", 1, 1));
        assert_eq!(Some(0), find("ab", 0, 2));
        assert_eq!(Some(1), find("ab", 1, 2));
        assert_eq!(Some(2), find("ab", 2, 2));
    }

    #[test]
    fn never_match() {
        let re = PikeVM::new_from_nfa(NFA::never_match()).unwrap();
        let mut cache = re.create_cache();
        let mut caps = re.create_captures();
        let mut find = |haystack, start, end| {
            let input = Input::new(haystack).range(start..end);
            re.search(&mut cache, &input, &mut caps);
            caps.get_match().map(|m| m.end())
        };

        assert_eq!(None, find("", 0, 0));
        assert_eq!(None, find("a", 0, 1));
        assert_eq!(None, find("a", 1, 1));
        assert_eq!(None, find("ab", 0, 2));
        assert_eq!(None, find("ab", 1, 2));
        assert_eq!(None, find("ab", 2, 2));
    }
}
