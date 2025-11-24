#[cfg(feature = "alloc")]
use crate::util::search::PatternSet;
use crate::{
    dfa::search,
    util::{
        empty,
        prefilter::Prefilter,
        primitives::{PatternID, StateID},
        search::{Anchored, HalfMatch, Input, MatchError},
        start,
    },
};

/// A trait describing the interface of a deterministic finite automaton (DFA).
///
/// The complexity of this trait probably means that it's unlikely for others
/// to implement it. The primary purpose of the trait is to provide for a way
/// of abstracting over different types of DFAs. In this crate, that means
/// dense DFAs and sparse DFAs. (Dense DFAs are fast but memory hungry, where
/// as sparse DFAs are slower but come with a smaller memory footprint. But
/// they otherwise provide exactly equivalent expressive power.) For example, a
/// [`dfa::regex::Regex`](crate::dfa::regex::Regex) is generic over this trait.
///
/// Normally, a DFA's execution model is very simple. You might have a single
/// start state, zero or more final or "match" states and a function that
/// transitions from one state to the next given the next byte of input.
/// Unfortunately, the interface described by this trait is significantly
/// more complicated than this. The complexity has a number of different
/// reasons, mostly motivated by performance, functionality or space savings:
///
/// * A DFA can search for multiple patterns simultaneously. This
/// means extra information is returned when a match occurs. Namely,
/// a match is not just an offset, but an offset plus a pattern ID.
/// [`Automaton::pattern_len`] returns the number of patterns compiled into
/// the DFA, [`Automaton::match_len`] returns the total number of patterns
/// that match in a particular state and [`Automaton::match_pattern`] permits
/// iterating over the patterns that match in a particular state.
/// * A DFA can have multiple start states, and the choice of which start
/// state to use depends on the content of the string being searched and
/// position of the search, as well as whether the search is an anchored
/// search for a specific pattern in the DFA. Moreover, computing the start
/// state also depends on whether you're doing a forward or a reverse search.
/// [`Automaton::start_state_forward`] and [`Automaton::start_state_reverse`]
/// are used to compute the start state for forward and reverse searches,
/// respectively.
/// * All matches are delayed by one byte to support things like `$` and `\b`
/// at the end of a pattern. Therefore, every use of a DFA is required to use
/// [`Automaton::next_eoi_state`]
/// at the end of the search to compute the final transition.
/// * For optimization reasons, some states are treated specially. Every
/// state is either special or not, which can be determined via the
/// [`Automaton::is_special_state`] method. If it's special, then the state
/// must be at least one of a few possible types of states. (Note that some
/// types can overlap, for example, a match state can also be an accel state.
/// But some types can't. If a state is a dead state, then it can never be any
/// other type of state.) Those types are:
///     * A dead state. A dead state means the DFA will never enter a match
///     state. This can be queried via the [`Automaton::is_dead_state`] method.
///     * A quit state. A quit state occurs if the DFA had to stop the search
///     prematurely for some reason. This can be queried via the
///     [`Automaton::is_quit_state`] method.
///     * A match state. A match state occurs when a match is found. When a DFA
///     enters a match state, the search may stop immediately (when looking
///     for the earliest match), or it may continue to find the leftmost-first
///     match. This can be queried via the [`Automaton::is_match_state`]
///     method.
///     * A start state. A start state is where a search begins. For every
///     search, there is exactly one start state that is used, however, a
///     DFA may contain many start states. When the search is in a start
///     state, it may use a prefilter to quickly skip to candidate matches
///     without executing the DFA on every byte. This can be queried via the
///     [`Automaton::is_start_state`] method.
///     * An accel state. An accel state is a state that is accelerated.
///     That is, it is a state where _most_ of its transitions loop back to
///     itself and only a small number of transitions lead to other states.
///     This kind of state is said to be accelerated because a search routine
///     can quickly look for the bytes leading out of the state instead of
///     continuing to execute the DFA on each byte. This can be queried via the
///     [`Automaton::is_accel_state`] method. And the bytes that lead out of
///     the state can be queried via the [`Automaton::accelerator`] method.
///
/// There are a number of provided methods on this trait that implement
/// efficient searching (for forwards and backwards) with a DFA using
/// all of the above features of this trait. In particular, given the
/// complexity of all these features, implementing a search routine in
/// this trait can be a little subtle. With that said, it is possible to
/// somewhat simplify the search routine. For example, handling accelerated
/// states is strictly optional, since it is always correct to assume that
/// `Automaton::is_accel_state` returns false. However, one complex part of
/// writing a search routine using this trait is handling the 1-byte delay of a
/// match. That is not optional.
///
/// # Safety
///
/// This trait is not safe to implement so that code may rely on the
/// correctness of implementations of this trait to avoid undefined behavior.
/// The primary correctness guarantees are:
///
/// * `Automaton::start_state` always returns a valid state ID or an error or
/// panics.
/// * `Automaton::next_state`, when given a valid state ID, always returns
/// a valid state ID for all values of `anchored` and `byte`, or otherwise
/// panics.
///
/// In general, the rest of the methods on `Automaton` need to uphold their
/// contracts as well. For example, `Automaton::is_dead` should only returns
/// true if the given state ID is actually a dead state.
pub unsafe trait Automaton {
    /// Transitions from the current state to the next state, given the next
    /// byte of input.
    ///
    /// Implementations must guarantee that the returned ID is always a valid
    /// ID when `current` refers to a valid ID. Moreover, the transition
    /// function must be defined for all possible values of `input`.
    ///
    /// # Panics
    ///
    /// If the given ID does not refer to a valid state, then this routine
    /// may panic but it also may not panic and instead return an invalid ID.
    /// However, if the caller provides an invalid ID then this must never
    /// sacrifice memory safety.
    ///
    /// # Example
    ///
    /// This shows a simplistic example for walking a DFA for a given haystack
    /// by using the `next_state` method.
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense}, Input};
    ///
    /// let dfa = dense::DFA::new(r"[a-z]+r")?;
    /// let haystack = "bar".as_bytes();
    ///
    /// // The start state is determined by inspecting the position and the
    /// // initial bytes of the haystack.
    /// let mut state = dfa.start_state_forward(&Input::new(haystack))?;
    /// // Walk all the bytes in the haystack.
    /// for &b in haystack {
    ///     state = dfa.next_state(state, b);
    /// }
    /// // Matches are always delayed by 1 byte, so we must explicitly walk the
    /// // special "EOI" transition at the end of the search.
    /// state = dfa.next_eoi_state(state);
    /// assert!(dfa.is_match_state(state));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn next_state(&self, current: StateID, input: u8) -> StateID;

    /// Transitions from the current state to the next state, given the next
    /// byte of input.
    ///
    /// Unlike [`Automaton::next_state`], implementations may implement this
    /// more efficiently by assuming that the `current` state ID is valid.
    /// Typically, this manifests by eliding bounds checks.
    ///
    /// # Safety
    ///
    /// Callers of this method must guarantee that `current` refers to a valid
    /// state ID. If `current` is not a valid state ID for this automaton, then
    /// calling this routine may result in undefined behavior.
    ///
    /// If `current` is valid, then implementations must guarantee that the ID
    /// returned is valid for all possible values of `input`.
    unsafe fn next_state_unchecked(
        &self,
        current: StateID,
        input: u8,
    ) -> StateID;

    /// Transitions from the current state to the next state for the special
    /// EOI symbol.
    ///
    /// Implementations must guarantee that the returned ID is always a valid
    /// ID when `current` refers to a valid ID.
    ///
    /// This routine must be called at the end of every search in a correct
    /// implementation of search. Namely, DFAs in this crate delay matches
    /// by one byte in order to support look-around operators. Thus, after
    /// reaching the end of a haystack, a search implementation must follow one
    /// last EOI transition.
    ///
    /// It is best to think of EOI as an additional symbol in the alphabet of
    /// a DFA that is distinct from every other symbol. That is, the alphabet
    /// of DFAs in this crate has a logical size of 257 instead of 256, where
    /// 256 corresponds to every possible inhabitant of `u8`. (In practice, the
    /// physical alphabet size may be smaller because of alphabet compression
    /// via equivalence classes, but EOI is always represented somehow in the
    /// alphabet.)
    ///
    /// # Panics
    ///
    /// If the given ID does not refer to a valid state, then this routine
    /// may panic but it also may not panic and instead return an invalid ID.
    /// However, if the caller provides an invalid ID then this must never
    /// sacrifice memory safety.
    ///
    /// # Example
    ///
    /// This shows a simplistic example for walking a DFA for a given haystack,
    /// and then finishing the search with the final EOI transition.
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense}, Input};
    ///
    /// let dfa = dense::DFA::new(r"[a-z]+r")?;
    /// let haystack = "bar".as_bytes();
    ///
    /// // The start state is determined by inspecting the position and the
    /// // initial bytes of the haystack.
    /// //
    /// // The unwrap is OK because we aren't requesting a start state for a
    /// // specific pattern.
    /// let mut state = dfa.start_state_forward(&Input::new(haystack))?;
    /// // Walk all the bytes in the haystack.
    /// for &b in haystack {
    ///     state = dfa.next_state(state, b);
    /// }
    /// // Matches are always delayed by 1 byte, so we must explicitly walk
    /// // the special "EOI" transition at the end of the search. Without this
    /// // final transition, the assert below will fail since the DFA will not
    /// // have entered a match state yet!
    /// state = dfa.next_eoi_state(state);
    /// assert!(dfa.is_match_state(state));
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn next_eoi_state(&self, current: StateID) -> StateID;

    /// Return the ID of the start state for this DFA for the given starting
    /// configuration.
    ///
    /// Unlike typical DFA implementations, the start state for DFAs in this
    /// crate is dependent on a few different factors:
    ///
    /// * The [`Anchored`] mode of the search. Unanchored, anchored and
    /// anchored searches for a specific [`PatternID`] all use different start
    /// states.
    /// * Whether a "look-behind" byte exists. For example, the `^` anchor
    /// matches if and only if there is no look-behind byte.
    /// * The specific value of that look-behind byte. For example, a `(?m:^)`
    /// assertion only matches when there is either no look-behind byte, or
    /// when the look-behind byte is a line terminator.
    ///
    /// The [starting configuration](start::Config) provides the above
    /// information.
    ///
    /// This routine can be used for either forward or reverse searches.
    /// Although, as a convenience, if you have an [`Input`], then it may
    /// be more succinct to use [`Automaton::start_state_forward`] or
    /// [`Automaton::start_state_reverse`]. Note, for example, that the
    /// convenience routines return a [`MatchError`] on failure where as this
    /// routine returns a [`StartError`].
    ///
    /// # Errors
    ///
    /// This may return a [`StartError`] if the search needs to give up when
    /// determining the start state (for example, if it sees a "quit" byte).
    /// This can also return an error if the given configuration contains an
    /// unsupported [`Anchored`] configuration.
    fn start_state(
        &self,
        config: &start::Config,
    ) -> Result<StateID, StartError>;

    /// Return the ID of the start state for this DFA when executing a forward
    /// search.
    ///
    /// This is a convenience routine for calling [`Automaton::start_state`]
    /// that converts the given [`Input`] to a [start
    /// configuration](start::Config). Additionally, if an error occurs, it is
    /// converted from a [`StartError`] to a [`MatchError`] using the offset
    /// information in the given [`Input`].
    ///
    /// # Errors
    ///
    /// This may return a [`MatchError`] if the search needs to give up
    /// when determining the start state (for example, if it sees a "quit"
    /// byte). This can also return an error if the given `Input` contains an
    /// unsupported [`Anchored`] configuration.
    fn start_state_forward(
        &self,
        input: &Input<'_>,
    ) -> Result<StateID, MatchError> {
        let config = start::Config::from_input_forward(input);
        self.start_state(&config).map_err(|err| match err {
            StartError::Quit { byte } => {
                let offset = input
                    .start()
                    .checked_sub(1)
                    .expect("no quit in start without look-behind");
                MatchError::quit(byte, offset)
            }
            StartError::UnsupportedAnchored { mode } => {
                MatchError::unsupported_anchored(mode)
            }
        })
    }

    /// Return the ID of the start state for this DFA when executing a reverse
    /// search.
    ///
    /// This is a convenience routine for calling [`Automaton::start_state`]
    /// that converts the given [`Input`] to a [start
    /// configuration](start::Config). Additionally, if an error occurs, it is
    /// converted from a [`StartError`] to a [`MatchError`] using the offset
    /// information in the given [`Input`].
    ///
    /// # Errors
    ///
    /// This may return a [`MatchError`] if the search needs to give up
    /// when determining the start state (for example, if it sees a "quit"
    /// byte). This can also return an error if the given `Input` contains an
    /// unsupported [`Anchored`] configuration.
    fn start_state_reverse(
        &self,
        input: &Input<'_>,
    ) -> Result<StateID, MatchError> {
        let config = start::Config::from_input_reverse(input);
        self.start_state(&config).map_err(|err| match err {
            StartError::Quit { byte } => {
                let offset = input.end();
                MatchError::quit(byte, offset)
            }
            StartError::UnsupportedAnchored { mode } => {
                MatchError::unsupported_anchored(mode)
            }
        })
    }

    /// If this DFA has a universal starting state for the given anchor mode
    /// and the DFA supports universal starting states, then this returns that
    /// state's identifier.
    ///
    /// A DFA is said to have a universal starting state when the starting
    /// state is invariant with respect to the haystack. Usually, the starting
    /// state is chosen depending on the bytes immediately surrounding the
    /// starting position of a search. However, the starting state only differs
    /// when one or more of the patterns in the DFA have look-around assertions
    /// in its prefix.
    ///
    /// Stated differently, if none of the patterns in a DFA have look-around
    /// assertions in their prefix, then the DFA has a universal starting state
    /// and _may_ be returned by this method.
    ///
    /// It is always correct for implementations to return `None`, and indeed,
    /// this is what the default implementation does. When this returns `None`,
    /// callers must use either `start_state_forward` or `start_state_reverse`
    /// to get the starting state.
    ///
    /// # Use case
    ///
    /// There are a few reasons why one might want to use this:
    ///
    /// * If you know your regex patterns have no look-around assertions in
    /// their prefix, then calling this routine is likely cheaper and perhaps
    /// more semantically meaningful.
    /// * When implementing prefilter support in a DFA regex implementation,
    /// it is necessary to re-compute the start state after a candidate
    /// is returned from the prefilter. However, this is only needed when
    /// there isn't a universal start state. When one exists, one can avoid
    /// re-computing the start state.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{Automaton, dense::DFA},
    ///     Anchored,
    /// };
    ///
    /// // There are no look-around assertions in the prefixes of any of the
    /// // patterns, so we get a universal start state.
    /// let dfa = DFA::new_many(&["[0-9]+", "[a-z]+$", "[A-Z]+"])?;
    /// assert!(dfa.universal_start_state(Anchored::No).is_some());
    /// assert!(dfa.universal_start_state(Anchored::Yes).is_some());
    ///
    /// // One of the patterns has a look-around assertion in its prefix,
    /// // so this means there is no longer a universal start state.
    /// let dfa = DFA::new_many(&["[0-9]+", "^[a-z]+$", "[A-Z]+"])?;
    /// assert!(!dfa.universal_start_state(Anchored::No).is_some());
    /// assert!(!dfa.universal_start_state(Anchored::Yes).is_some());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    fn universal_start_state(&self, _mode: Anchored) -> Option<StateID> {
        None
    }

    /// Returns true if and only if the given identifier corresponds to a
    /// "special" state. A special state is one or more of the following:
    /// a dead state, a quit state, a match state, a start state or an
    /// accelerated state.
    ///
    /// A correct implementation _may_ always return false for states that
    /// are either start states or accelerated states, since that information
    /// is only intended to be used for optimization purposes. Correct
    /// implementations must return true if the state is a dead, quit or match
    /// state. This is because search routines using this trait must be able
    /// to rely on `is_special_state` as an indicator that a state may need
    /// special treatment. (For example, when a search routine sees a dead
    /// state, it must terminate.)
    ///
    /// This routine permits search implementations to use a single branch to
    /// check whether a state needs special attention before executing the next
    /// transition. The example below shows how to do this.
    ///
    /// # Example
    ///
    /// This example shows how `is_special_state` can be used to implement a
    /// correct search routine with minimal branching. In particular, this
    /// search routine implements "leftmost" matching, which means that it
    /// doesn't immediately stop once a match is found. Instead, it continues
    /// until it reaches a dead state.
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{Automaton, dense},
    ///     HalfMatch, MatchError, Input,
    /// };
    ///
    /// fn find<A: Automaton>(
    ///     dfa: &A,
    ///     haystack: &[u8],
    /// ) -> Result<Option<HalfMatch>, MatchError> {
    ///     // The start state is determined by inspecting the position and the
    ///     // initial bytes of the haystack. Note that start states can never
    ///     // be match states (since DFAs in this crate delay matches by 1
    ///     // byte), so we don't need to check if the start state is a match.
    ///     let mut state = dfa.start_state_forward(&Input::new(haystack))?;
    ///     let mut last_match = None;
    ///     // Walk all the bytes in the haystack. We can quit early if we see
    ///     // a dead or a quit state. The former means the automaton will
    ///     // never transition to any other state. The latter means that the
    ///     // automaton entered a condition in which its search failed.
    ///     for (i, &b) in haystack.iter().enumerate() {
    ///         state = dfa.next_state(state, b);
    ///         if dfa.is_special_state(state) {
    ///             if dfa.is_match_state(state) {
    ///                 last_match = Some(HalfMatch::new(
    ///                     dfa.match_pattern(state, 0),
    ///                     i,
    ///                 ));
    ///             } else if dfa.is_dead_state(state) {
    ///                 return Ok(last_match);
    ///             } else if dfa.is_quit_state(state) {
    ///                 // It is possible to enter into a quit state after
    ///                 // observing a match has occurred. In that case, we
    ///                 // should return the match instead of an error.
    ///                 if last_match.is_some() {
    ///                     return Ok(last_match);
    ///                 }
    ///                 return Err(MatchError::quit(b, i));
    ///             }
    ///             // Implementors may also want to check for start or accel
    ///             // states and handle them differently for performance
    ///             // reasons. But it is not necessary for correctness.
    ///         }
    ///     }
    ///     // Matches are always delayed by 1 byte, so we must explicitly walk
    ///     // the special "EOI" transition at the end of the search.
    ///     state = dfa.next_eoi_state(state);
    ///     if dfa.is_match_state(state) {
    ///         last_match = Some(HalfMatch::new(
    ///             dfa.match_pattern(state, 0),
    ///             haystack.len(),
    ///         ));
    ///     }
    ///     Ok(last_match)
    /// }
    ///
    /// // We use a greedy '+' operator to show how the search doesn't just
    /// // stop once a match is detected. It continues extending the match.
    /// // Using '[a-z]+?' would also work as expected and stop the search
    /// // early. Greediness is built into the automaton.
    /// let dfa = dense::DFA::new(r"[a-z]+")?;
    /// let haystack = "123 foobar 4567".as_bytes();
    /// let mat = find(&dfa, haystack)?.unwrap();
    /// assert_eq!(mat.pattern().as_usize(), 0);
    /// assert_eq!(mat.offset(), 10);
    ///
    /// // Here's another example that tests our handling of the special EOI
    /// // transition. This will fail to find a match if we don't call
    /// // 'next_eoi_state' at the end of the search since the match isn't
    /// // found until the final byte in the haystack.
    /// let dfa = dense::DFA::new(r"[0-9]{4}")?;
    /// let haystack = "123 foobar 4567".as_bytes();
    /// let mat = find(&dfa, haystack)?.unwrap();
    /// assert_eq!(mat.pattern().as_usize(), 0);
    /// assert_eq!(mat.offset(), 15);
    ///
    /// // And note that our search implementation above automatically works
    /// // with multi-DFAs. Namely, `dfa.match_pattern(match_state, 0)` selects
    /// // the appropriate pattern ID for us.
    /// let dfa = dense::DFA::new_many(&[r"[a-z]+", r"[0-9]+"])?;
    /// let haystack = "123 foobar 4567".as_bytes();
    /// let mat = find(&dfa, haystack)?.unwrap();
    /// assert_eq!(mat.pattern().as_usize(), 1);
    /// assert_eq!(mat.offset(), 3);
    /// let mat = find(&dfa, &haystack[3..])?.unwrap();
    /// assert_eq!(mat.pattern().as_usize(), 0);
    /// assert_eq!(mat.offset(), 7);
    /// let mat = find(&dfa, &haystack[10..])?.unwrap();
    /// assert_eq!(mat.pattern().as_usize(), 1);
    /// assert_eq!(mat.offset(), 5);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn is_special_state(&self, id: StateID) -> bool;

    /// Returns true if and only if the given identifier corresponds to a dead
    /// state. When a DFA enters a dead state, it is impossible to leave. That
    /// is, every transition on a dead state by definition leads back to the
    /// same dead state.
    ///
    /// In practice, the dead state always corresponds to the identifier `0`.
    /// Moreover, in practice, there is only one dead state.
    ///
    /// The existence of a dead state is not strictly required in the classical
    /// model of finite state machines, where one generally only cares about
    /// the question of whether an input sequence matches or not. Dead states
    /// are not needed to answer that question, since one can immediately quit
    /// as soon as one enters a final or "match" state. However, we don't just
    /// care about matches but also care about the location of matches, and
    /// more specifically, care about semantics like "greedy" matching.
    ///
    /// For example, given the pattern `a+` and the input `aaaz`, the dead
    /// state won't be entered until the state machine reaches `z` in the
    /// input, at which point, the search routine can quit. But without the
    /// dead state, the search routine wouldn't know when to quit. In a
    /// classical representation, the search routine would stop after seeing
    /// the first `a` (which is when the search would enter a match state). But
    /// this wouldn't implement "greedy" matching where `a+` matches as many
    /// `a`'s as possible.
    ///
    /// # Example
    ///
    /// See the example for [`Automaton::is_special_state`] for how to use this
    /// method correctly.
    fn is_dead_state(&self, id: StateID) -> bool;

    /// Returns true if and only if the given identifier corresponds to a quit
    /// state. A quit state is like a dead state (it has no transitions other
    /// than to itself), except it indicates that the DFA failed to complete
    /// the search. When this occurs, callers can neither accept or reject that
    /// a match occurred.
    ///
    /// In practice, the quit state always corresponds to the state immediately
    /// following the dead state. (Which is not usually represented by `1`,
    /// since state identifiers are pre-multiplied by the state machine's
    /// alphabet stride, and the alphabet stride varies between DFAs.)
    ///
    /// The typical way in which a quit state can occur is when heuristic
    /// support for Unicode word boundaries is enabled via the
    /// [`dense::Config::unicode_word_boundary`](crate::dfa::dense::Config::unicode_word_boundary)
    /// option. But other options, like the lower level
    /// [`dense::Config::quit`](crate::dfa::dense::Config::quit)
    /// configuration, can also result in a quit state being entered. The
    /// purpose of the quit state is to provide a way to execute a fast DFA
    /// in common cases while delegating to slower routines when the DFA quits.
    ///
    /// The default search implementations provided by this crate will return a
    /// [`MatchError::quit`] error when a quit state is entered.
    ///
    /// # Example
    ///
    /// See the example for [`Automaton::is_special_state`] for how to use this
    /// method correctly.
    fn is_quit_state(&self, id: StateID) -> bool;

    /// Returns true if and only if the given identifier corresponds to a
    /// match state. A match state is also referred to as a "final" state and
    /// indicates that a match has been found.
    ///
    /// If all you care about is whether a particular pattern matches in the
    /// input sequence, then a search routine can quit early as soon as the
    /// machine enters a match state. However, if you're looking for the
    /// standard "leftmost-first" match location, then search _must_ continue
    /// until either the end of the input or until the machine enters a dead
    /// state. (Since either condition implies that no other useful work can
    /// be done.) Namely, when looking for the location of a match, then
    /// search implementations should record the most recent location in
    /// which a match state was entered, but otherwise continue executing the
    /// search as normal. (The search may even leave the match state.) Once
    /// the termination condition is reached, the most recently recorded match
    /// location should be returned.
    ///
    /// Finally, one additional power given to match states in this crate
    /// is that they are always associated with a specific pattern in order
    /// to support multi-DFAs. See [`Automaton::match_pattern`] for more
    /// details and an example for how to query the pattern associated with a
    /// particular match state.
    ///
    /// # Example
    ///
    /// See the example for [`Automaton::is_special_state`] for how to use this
    /// method correctly.
    fn is_match_state(&self, id: StateID) -> bool;

    /// Returns true only if the given identifier corresponds to a start
    /// state
    ///
    /// A start state is a state in which a DFA begins a search.
    /// All searches begin in a start state. Moreover, since all matches are
    /// delayed by one byte, a start state can never be a match state.
    ///
    /// The main role of a start state is, as mentioned, to be a starting
    /// point for a DFA. This starting point is determined via one of
    /// [`Automaton::start_state_forward`] or
    /// [`Automaton::start_state_reverse`], depending on whether one is doing
    /// a forward or a reverse search, respectively.
    ///
    /// A secondary use of start states is for prefix acceleration. Namely,
    /// while executing a search, if one detects that you're in a start state,
    /// then it may be faster to look for the next match of a prefix of the
    /// pattern, if one exists. If a prefix exists and since all matches must
    /// begin with that prefix, then skipping ahead to occurrences of that
    /// prefix may be much faster than executing the DFA.
    ///
    /// As mentioned in the documentation for
    /// [`is_special_state`](Automaton::is_special_state) implementations
    /// _may_ always return false, even if the given identifier is a start
    /// state. This is because knowing whether a state is a start state or not
    /// is not necessary for correctness and is only treated as a potential
    /// performance optimization. (For example, the implementations of this
    /// trait in this crate will only return true when the given identifier
    /// corresponds to a start state and when [specialization of start
    /// states](crate::dfa::dense::Config::specialize_start_states) was enabled
    /// during DFA construction. If start state specialization is disabled
    /// (which is the default), then this method will always return false.)
    ///
    /// # Example
    ///
    /// This example shows how to implement your own search routine that does
    /// a prefix search whenever the search enters a start state.
    ///
    /// Note that you do not need to implement your own search routine
    /// to make use of prefilters like this. The search routines
    /// provided by this crate already implement prefilter support via
    /// the [`Prefilter`](crate::util::prefilter::Prefilter) trait.
    /// A prefilter can be added to your search configuration with
    /// [`dense::Config::prefilter`](crate::dfa::dense::Config::prefilter) for
    /// dense and sparse DFAs in this crate.
    ///
    /// This example is meant to show how you might deal with prefilters in a
    /// simplified case if you are implementing your own search routine.
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{Automaton, dense},
    ///     HalfMatch, MatchError, Input,
    /// };
    ///
    /// fn find_byte(slice: &[u8], at: usize, byte: u8) -> Option<usize> {
    ///     // Would be faster to use the memchr crate, but this is still
    ///     // faster than running through the DFA.
    ///     slice[at..].iter().position(|&b| b == byte).map(|i| at + i)
    /// }
    ///
    /// fn find<A: Automaton>(
    ///     dfa: &A,
    ///     haystack: &[u8],
    ///     prefix_byte: Option<u8>,
    /// ) -> Result<Option<HalfMatch>, MatchError> {
    ///     // See the Automaton::is_special_state example for similar code
    ///     // with more comments.
    ///
    ///     let mut state = dfa.start_state_forward(&Input::new(haystack))?;
    ///     let mut last_match = None;
    ///     let mut pos = 0;
    ///     while pos < haystack.len() {
    ///         let b = haystack[pos];
    ///         state = dfa.next_state(state, b);
    ///         pos += 1;
    ///         if dfa.is_special_state(state) {
    ///             if dfa.is_match_state(state) {
    ///                 last_match = Some(HalfMatch::new(
    ///                     dfa.match_pattern(state, 0),
    ///                     pos - 1,
    ///                 ));
    ///             } else if dfa.is_dead_state(state) {
    ///                 return Ok(last_match);
    ///             } else if dfa.is_quit_state(state) {
    ///                 // It is possible to enter into a quit state after
    ///                 // observing a match has occurred. In that case, we
    ///                 // should return the match instead of an error.
    ///                 if last_match.is_some() {
    ///                     return Ok(last_match);
    ///                 }
    ///                 return Err(MatchError::quit(b, pos - 1));
    ///             } else if dfa.is_start_state(state) {
    ///                 // If we're in a start state and know all matches begin
    ///                 // with a particular byte, then we can quickly skip to
    ///                 // candidate matches without running the DFA through
    ///                 // every byte inbetween.
    ///                 if let Some(prefix_byte) = prefix_byte {
    ///                     pos = match find_byte(haystack, pos, prefix_byte) {
    ///                         Some(pos) => pos,
    ///                         None => break,
    ///                     };
    ///                 }
    ///             }
    ///         }
    ///     }
    ///     // Matches are always delayed by 1 byte, so we must explicitly walk
    ///     // the special "EOI" transition at the end of the search.
    ///     state = dfa.next_eoi_state(state);
    ///     if dfa.is_match_state(state) {
    ///         last_match = Some(HalfMatch::new(
    ///             dfa.match_pattern(state, 0),
    ///             haystack.len(),
    ///         ));
    ///     }
    ///     Ok(last_match)
    /// }
    ///
    /// // In this example, it's obvious that all occurrences of our pattern
    /// // begin with 'Z', so we pass in 'Z'. Note also that we need to
    /// // enable start state specialization, or else it won't be possible to
    /// // detect start states during a search. ('is_start_state' would always
    /// // return false.)
    /// let dfa = dense::DFA::builder()
    ///     .configure(dense::DFA::config().specialize_start_states(true))
    ///     .build(r"Z[a-z]+")?;
    /// let haystack = "123 foobar Zbaz quux".as_bytes();
    /// let mat = find(&dfa, haystack, Some(b'Z'))?.unwrap();
    /// assert_eq!(mat.pattern().as_usize(), 0);
    /// assert_eq!(mat.offset(), 15);
    ///
    /// // But note that we don't need to pass in a prefix byte. If we don't,
    /// // then the search routine does no acceleration.
    /// let mat = find(&dfa, haystack, None)?.unwrap();
    /// assert_eq!(mat.pattern().as_usize(), 0);
    /// assert_eq!(mat.offset(), 15);
    ///
    /// // However, if we pass an incorrect byte, then the prefix search will
    /// // result in incorrect results.
    /// assert_eq!(find(&dfa, haystack, Some(b'X'))?, None);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn is_start_state(&self, id: StateID) -> bool;

    /// Returns true if and only if the given identifier corresponds to an
    /// accelerated state.
    ///
    /// An accelerated state is a special optimization
    /// trick implemented by this crate. Namely, if
    /// [`dense::Config::accelerate`](crate::dfa::dense::Config::accelerate) is
    /// enabled (and it is by default), then DFAs generated by this crate will
    /// tag states meeting certain characteristics as accelerated. States meet
    /// this criteria whenever most of their transitions are self-transitions.
    /// That is, transitions that loop back to the same state. When a small
    /// number of transitions aren't self-transitions, then it follows that
    /// there are only a small number of bytes that can cause the DFA to leave
    /// that state. Thus, there is an opportunity to look for those bytes
    /// using more optimized routines rather than continuing to run through
    /// the DFA. This trick is similar to the prefilter idea described in
    /// the documentation of [`Automaton::is_start_state`] with two main
    /// differences:
    ///
    /// 1. It is more limited since acceleration only applies to single bytes.
    /// This means states are rarely accelerated when Unicode mode is enabled
    /// (which is enabled by default).
    /// 2. It can occur anywhere in the DFA, which increases optimization
    /// opportunities.
    ///
    /// Like the prefilter idea, the main downside (and a possible reason to
    /// disable it) is that it can lead to worse performance in some cases.
    /// Namely, if a state is accelerated for very common bytes, then the
    /// overhead of checking for acceleration and using the more optimized
    /// routines to look for those bytes can cause overall performance to be
    /// worse than if acceleration wasn't enabled at all.
    ///
    /// A simple example of a regex that has an accelerated state is
    /// `(?-u)[^a]+a`. Namely, the `[^a]+` sub-expression gets compiled down
    /// into a single state where all transitions except for `a` loop back to
    /// itself, and where `a` is the only transition (other than the special
    /// EOI transition) that goes to some other state. Thus, this state can
    /// be accelerated and implemented more efficiently by calling an
    /// optimized routine like `memchr` with `a` as the needle. Notice that
    /// the `(?-u)` to disable Unicode is necessary here, as without it,
    /// `[^a]` will match any UTF-8 encoding of any Unicode scalar value other
    /// than `a`. This more complicated expression compiles down to many DFA
    /// states and the simple acceleration optimization is no longer available.
    ///
    /// Typically, this routine is used to guard calls to
    /// [`Automaton::accelerator`], which returns the accelerated bytes for
    /// the specified state.
    fn is_accel_state(&self, id: StateID) -> bool;

    /// Returns the total number of patterns compiled into this DFA.
    ///
    /// In the case of a DFA that contains no patterns, this must return `0`.
    ///
    /// # Example
    ///
    /// This example shows the pattern length for a DFA that never matches:
    ///
    /// ```
    /// use regex_automata::dfa::{Automaton, dense::DFA};
    ///
    /// let dfa: DFA<Vec<u32>> = DFA::never_match()?;
    /// assert_eq!(dfa.pattern_len(), 0);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// And another example for a DFA that matches at every position:
    ///
    /// ```
    /// use regex_automata::dfa::{Automaton, dense::DFA};
    ///
    /// let dfa: DFA<Vec<u32>> = DFA::always_match()?;
    /// assert_eq!(dfa.pattern_len(), 1);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// And finally, a DFA that was constructed from multiple patterns:
    ///
    /// ```
    /// use regex_automata::dfa::{Automaton, dense::DFA};
    ///
    /// let dfa = DFA::new_many(&["[0-9]+", "[a-z]+", "[A-Z]+"])?;
    /// assert_eq!(dfa.pattern_len(), 3);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn pattern_len(&self) -> usize;

    /// Returns the total number of patterns that match in this state.
    ///
    /// If the given state is not a match state, then implementations may
    /// panic.
    ///
    /// If the DFA was compiled with one pattern, then this must necessarily
    /// always return `1` for all match states.
    ///
    /// Implementations must guarantee that [`Automaton::match_pattern`] can be
    /// called with indices up to (but not including) the length returned by
    /// this routine without panicking.
    ///
    /// # Panics
    ///
    /// Implementations are permitted to panic if the provided state ID does
    /// not correspond to a match state.
    ///
    /// # Example
    ///
    /// This example shows a simple instance of implementing overlapping
    /// matches. In particular, it shows not only how to determine how many
    /// patterns have matched in a particular state, but also how to access
    /// which specific patterns have matched.
    ///
    /// Notice that we must use
    /// [`MatchKind::All`](crate::MatchKind::All)
    /// when building the DFA. If we used
    /// [`MatchKind::LeftmostFirst`](crate::MatchKind::LeftmostFirst)
    /// instead, then the DFA would not be constructed in a way that
    /// supports overlapping matches. (It would only report a single pattern
    /// that matches at any particular point in time.)
    ///
    /// Another thing to take note of is the patterns used and the order in
    /// which the pattern IDs are reported. In the example below, pattern `3`
    /// is yielded first. Why? Because it corresponds to the match that
    /// appears first. Namely, the `@` symbol is part of `\S+` but not part
    /// of any of the other patterns. Since the `\S+` pattern has a match that
    /// starts to the left of any other pattern, its ID is returned before any
    /// other.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{dfa::{Automaton, dense}, Input, MatchKind};
    ///
    /// let dfa = dense::Builder::new()
    ///     .configure(dense::Config::new().match_kind(MatchKind::All))
    ///     .build_many(&[
    ///         r"[[:word:]]+", r"[a-z]+", r"[A-Z]+", r"[[:^space:]]+",
    ///     ])?;
    /// let haystack = "@bar".as_bytes();
    ///
    /// // The start state is determined by inspecting the position and the
    /// // initial bytes of the haystack.
    /// let mut state = dfa.start_state_forward(&Input::new(haystack))?;
    /// // Walk all the bytes in the haystack.
    /// for &b in haystack {
    ///     state = dfa.next_state(state, b);
    /// }
    /// state = dfa.next_eoi_state(state);
    ///
    /// assert!(dfa.is_match_state(state));
    /// assert_eq!(dfa.match_len(state), 3);
    /// // The following calls are guaranteed to not panic since `match_len`
    /// // returned `3` above.
    /// assert_eq!(dfa.match_pattern(state, 0).as_usize(), 3);
    /// assert_eq!(dfa.match_pattern(state, 1).as_usize(), 0);
    /// assert_eq!(dfa.match_pattern(state, 2).as_usize(), 1);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn match_len(&self, id: StateID) -> usize;

    /// Returns the pattern ID corresponding to the given match index in the
    /// given state.
    ///
    /// See [`Automaton::match_len`] for an example of how to use this
    /// method correctly. Note that if you know your DFA is compiled with a
    /// single pattern, then this routine is never necessary since it will
    /// always return a pattern ID of `0` for an index of `0` when `id`
    /// corresponds to a match state.
    ///
    /// Typically, this routine is used when implementing an overlapping
    /// search, as the example for `Automaton::match_len` does.
    ///
    /// # Panics
    ///
    /// If the state ID is not a match state or if the match index is out
    /// of bounds for the given state, then this routine may either panic
    /// or produce an incorrect result. If the state ID is correct and the
    /// match index is correct, then this routine must always produce a valid
    /// `PatternID`.
    fn match_pattern(&self, id: StateID, index: usize) -> PatternID;

    /// Returns true if and only if this automaton can match the empty string.
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
    /// This example shows a few different DFAs and whether they match the
    /// empty string or not. Notice the empty string isn't merely a matter
    /// of a string of length literally `0`, but rather, whether a match can
    /// occur between specific pairs of bytes.
    ///
    /// ```
    /// use regex_automata::{dfa::{dense::DFA, Automaton}, util::syntax};
    ///
    /// // The empty regex matches the empty string.
    /// let dfa = DFA::new("")?;
    /// assert!(dfa.has_empty(), "empty matches empty");
    /// // The '+' repetition operator requires at least one match, and so
    /// // does not match the empty string.
    /// let dfa = DFA::new("a+")?;
    /// assert!(!dfa.has_empty(), "+ does not match empty");
    /// // But the '*' repetition operator does.
    /// let dfa = DFA::new("a*")?;
    /// assert!(dfa.has_empty(), "* does match empty");
    /// // And wrapping '+' in an operator that can match an empty string also
    /// // causes it to match the empty string too.
    /// let dfa = DFA::new("(a+)*")?;
    /// assert!(dfa.has_empty(), "+ inside of * matches empty");
    ///
    /// // If a regex is just made of a look-around assertion, even if the
    /// // assertion requires some kind of non-empty string around it (such as
    /// // \b), then it is still treated as if it matches the empty string.
    /// // Namely, if a match occurs of just a look-around assertion, then the
    /// // match returned is empty.
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().unicode_word_boundary(true))
    ///     .syntax(syntax::Config::new().utf8(false))
    ///     .build(r"^$\A\z\b\B(?-u:\b\B)")?;
    /// assert!(dfa.has_empty(), "assertions match empty");
    /// // Even when an assertion is wrapped in a '+', it still matches the
    /// // empty string.
    /// let dfa = DFA::new(r"^+")?;
    /// assert!(dfa.has_empty(), "+ of an assertion matches empty");
    ///
    /// // An alternation with even one branch that can match the empty string
    /// // is also said to match the empty string overall.
    /// let dfa = DFA::new("foo|(bar)?|quux")?;
    /// assert!(dfa.has_empty(), "alternations can match empty");
    ///
    /// // An NFA that matches nothing does not match the empty string.
    /// let dfa = DFA::new("[a&&b]")?;
    /// assert!(!dfa.has_empty(), "never matching means not matching empty");
    /// // But if it's wrapped in something that doesn't require a match at
    /// // all, then it can match the empty string!
    /// let dfa = DFA::new("[a&&b]*")?;
    /// assert!(dfa.has_empty(), "* on never-match still matches empty");
    /// // Since a '+' requires a match, using it on something that can never
    /// // match will itself produce a regex that can never match anything,
    /// // and thus does not match the empty string.
    /// let dfa = DFA::new("[a&&b]+")?;
    /// assert!(!dfa.has_empty(), "+ on never-match still matches nothing");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn has_empty(&self) -> bool;

    /// Whether UTF-8 mode is enabled for this DFA or not.
    ///
    /// When UTF-8 mode is enabled, all matches reported by a DFA are
    /// guaranteed to correspond to spans of valid UTF-8. This includes
    /// zero-width matches. For example, the DFA must guarantee that the empty
    /// regex will not match at the positions between code units in the UTF-8
    /// encoding of a single codepoint.
    ///
    /// See [`thompson::Config::utf8`](crate::nfa::thompson::Config::utf8) for
    /// more information.
    ///
    /// # Example
    ///
    /// This example shows how UTF-8 mode can impact the match spans that may
    /// be reported in certain cases.
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{dense::DFA, Automaton},
    ///     nfa::thompson,
    ///     HalfMatch, Input,
    /// };
    ///
    /// // UTF-8 mode is enabled by default.
    /// let re = DFA::new("")?;
    /// assert!(re.is_utf8());
    /// let mut input = Input::new("☃");
    /// let got = re.try_search_fwd(&input)?;
    /// assert_eq!(Some(HalfMatch::must(0, 0)), got);
    ///
    /// // Even though an empty regex matches at 1..1, our next match is
    /// // 3..3 because 1..1 and 2..2 split the snowman codepoint (which is
    /// // three bytes long).
    /// input.set_start(1);
    /// let got = re.try_search_fwd(&input)?;
    /// assert_eq!(Some(HalfMatch::must(0, 3)), got);
    ///
    /// // But if we disable UTF-8, then we'll get matches at 1..1 and 2..2:
    /// let re = DFA::builder()
    ///     .thompson(thompson::Config::new().utf8(false))
    ///     .build("")?;
    /// assert!(!re.is_utf8());
    /// let got = re.try_search_fwd(&input)?;
    /// assert_eq!(Some(HalfMatch::must(0, 1)), got);
    ///
    /// input.set_start(2);
    /// let got = re.try_search_fwd(&input)?;
    /// assert_eq!(Some(HalfMatch::must(0, 2)), got);
    ///
    /// input.set_start(3);
    /// let got = re.try_search_fwd(&input)?;
    /// assert_eq!(Some(HalfMatch::must(0, 3)), got);
    ///
    /// input.set_start(4);
    /// let got = re.try_search_fwd(&input)?;
    /// assert_eq!(None, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn is_utf8(&self) -> bool;

    /// Returns true if and only if this DFA is limited to returning matches
    /// whose start position is `0`.
    ///
    /// Note that if you're using DFAs provided by
    /// this crate, then this is _orthogonal_ to
    /// [`Config::start_kind`](crate::dfa::dense::Config::start_kind).
    ///
    /// This is useful in some cases because if a DFA is limited to producing
    /// matches that start at offset `0`, then a reverse search is never
    /// required for finding the start of a match.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::dfa::{dense::DFA, Automaton};
    ///
    /// // The empty regex matches anywhere
    /// let dfa = DFA::new("")?;
    /// assert!(!dfa.is_always_start_anchored(), "empty matches anywhere");
    /// // 'a' matches anywhere.
    /// let dfa = DFA::new("a")?;
    /// assert!(!dfa.is_always_start_anchored(), "'a' matches anywhere");
    /// // '^' only matches at offset 0!
    /// let dfa = DFA::new("^a")?;
    /// assert!(dfa.is_always_start_anchored(), "'^a' matches only at 0");
    /// // But '(?m:^)' matches at 0 but at other offsets too.
    /// let dfa = DFA::new("(?m:^)a")?;
    /// assert!(!dfa.is_always_start_anchored(), "'(?m:^)a' matches anywhere");
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn is_always_start_anchored(&self) -> bool;

    /// Return a slice of bytes to accelerate for the given state, if possible.
    ///
    /// If the given state has no accelerator, then an empty slice must be
    /// returned. If `Automaton::is_accel_state` returns true for the given ID,
    /// then this routine _must_ return a non-empty slice. But note that it is
    /// not required for an implementation of this trait to ever return `true`
    /// for `is_accel_state`, even if the state _could_ be accelerated. That
    /// is, acceleration is an optional optimization. But the return values of
    /// `is_accel_state` and `accelerator` must be in sync.
    ///
    /// If the given ID is not a valid state ID for this automaton, then
    /// implementations may panic or produce incorrect results.
    ///
    /// See [`Automaton::is_accel_state`] for more details on state
    /// acceleration.
    ///
    /// By default, this method will always return an empty slice.
    ///
    /// # Example
    ///
    /// This example shows a contrived case in which we build a regex that we
    /// know is accelerated and extract the accelerator from a state.
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{Automaton, dense},
    ///     util::{primitives::StateID, syntax},
    /// };
    ///
    /// let dfa = dense::Builder::new()
    ///     // We disable Unicode everywhere and permit the regex to match
    ///     // invalid UTF-8. e.g., [^abc] matches \xFF, which is not valid
    ///     // UTF-8. If we left Unicode enabled, [^abc] would match any UTF-8
    ///     // encoding of any Unicode scalar value except for 'a', 'b' or 'c'.
    ///     // That translates to a much more complicated DFA, and also
    ///     // inhibits the 'accelerator' optimization that we are trying to
    ///     // demonstrate in this example.
    ///     .syntax(syntax::Config::new().unicode(false).utf8(false))
    ///     .build("[^abc]+a")?;
    ///
    /// // Here we just pluck out the state that we know is accelerated.
    /// // While the stride calculations are something that can be relied
    /// // on by callers, the specific position of the accelerated state is
    /// // implementation defined.
    /// //
    /// // N.B. We get '3' by inspecting the state machine using 'regex-cli'.
    /// // e.g., try `regex-cli debug dense dfa -p '[^abc]+a' -BbUC`.
    /// let id = StateID::new(3 * dfa.stride()).unwrap();
    /// let accelerator = dfa.accelerator(id);
    /// // The `[^abc]+` sub-expression permits [a, b, c] to be accelerated.
    /// assert_eq!(accelerator, &[b'a', b'b', b'c']);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    fn accelerator(&self, _id: StateID) -> &[u8] {
        &[]
    }

    /// Returns the prefilter associated with a DFA, if one exists.
    ///
    /// The default implementation of this trait always returns `None`. And
    /// indeed, it is always correct to return `None`.
    ///
    /// For DFAs in this crate, a prefilter can be attached to a DFA via
    /// [`dense::Config::prefilter`](crate::dfa::dense::Config::prefilter).
    ///
    /// Do note that prefilters are not serialized by DFAs in this crate.
    /// So if you deserialize a DFA that had a prefilter attached to it
    /// at serialization time, then it will not have a prefilter after
    /// deserialization.
    #[inline]
    fn get_prefilter(&self) -> Option<&Prefilter> {
        None
    }

    /// Executes a forward search and returns the end position of the leftmost
    /// match that is found. If no match exists, then `None` is returned.
    ///
    /// In particular, this method continues searching even after it enters
    /// a match state. The search only terminates once it has reached the
    /// end of the input or when it has entered a dead or quit state. Upon
    /// termination, the position of the last byte seen while still in a match
    /// state is returned.
    ///
    /// # Errors
    ///
    /// This routine errors if the search could not complete. This can occur
    /// in a number of circumstances:
    ///
    /// * The configuration of the DFA may permit it to "quit" the search.
    /// For example, setting quit bytes or enabling heuristic support for
    /// Unicode word boundaries. The default configuration does not enable any
    /// option that could result in the DFA quitting.
    /// * When the provided `Input` configuration is not supported. For
    /// example, by providing an unsupported anchor mode.
    ///
    /// When a search returns an error, callers cannot know whether a match
    /// exists or not.
    ///
    /// # Notes for implementors
    ///
    /// Implementors of this trait are not required to implement any particular
    /// match semantics (such as leftmost-first), which are instead manifest in
    /// the DFA's transitions. But this search routine should behave as a
    /// general "leftmost" search.
    ///
    /// In particular, this method must continue searching even after it enters
    /// a match state. The search should only terminate once it has reached
    /// the end of the input or when it has entered a dead or quit state. Upon
    /// termination, the position of the last byte seen while still in a match
    /// state is returned.
    ///
    /// Since this trait provides an implementation for this method by default,
    /// it's unlikely that one will need to implement this.
    ///
    /// # Example
    ///
    /// This example shows how to use this method with a
    /// [`dense::DFA`](crate::dfa::dense::DFA).
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense}, HalfMatch, Input};
    ///
    /// let dfa = dense::DFA::new("foo[0-9]+")?;
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new(b"foo12345"))?);
    ///
    /// // Even though a match is found after reading the first byte (`a`),
    /// // the leftmost first match semantics demand that we find the earliest
    /// // match that prefers earlier parts of the pattern over latter parts.
    /// let dfa = dense::DFA::new("abc|a")?;
    /// let expected = Some(HalfMatch::must(0, 3));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new(b"abc"))?);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: specific pattern search
    ///
    /// This example shows how to build a multi-DFA that permits searching for
    /// specific patterns.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{
    ///     dfa::{Automaton, dense},
    ///     Anchored, HalfMatch, PatternID, Input,
    /// };
    ///
    /// let dfa = dense::Builder::new()
    ///     .configure(dense::Config::new().starts_for_each_pattern(true))
    ///     .build_many(&["[a-z0-9]{6}", "[a-z][a-z0-9]{5}"])?;
    /// let haystack = "foo123".as_bytes();
    ///
    /// // Since we are using the default leftmost-first match and both
    /// // patterns match at the same starting position, only the first pattern
    /// // will be returned in this case when doing a search for any of the
    /// // patterns.
    /// let expected = Some(HalfMatch::must(0, 6));
    /// let got = dfa.try_search_fwd(&Input::new(haystack))?;
    /// assert_eq!(expected, got);
    ///
    /// // But if we want to check whether some other pattern matches, then we
    /// // can provide its pattern ID.
    /// let input = Input::new(haystack)
    ///     .anchored(Anchored::Pattern(PatternID::must(1)));
    /// let expected = Some(HalfMatch::must(1, 6));
    /// let got = dfa.try_search_fwd(&input)?;
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: specifying the bounds of a search
    ///
    /// This example shows how providing the bounds of a search can produce
    /// different results than simply sub-slicing the haystack.
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, dense}, HalfMatch, Input};
    ///
    /// // N.B. We disable Unicode here so that we use a simple ASCII word
    /// // boundary. Alternatively, we could enable heuristic support for
    /// // Unicode word boundaries.
    /// let dfa = dense::DFA::new(r"(?-u)\b[0-9]{3}\b")?;
    /// let haystack = "foo123bar".as_bytes();
    ///
    /// // Since we sub-slice the haystack, the search doesn't know about the
    /// // larger context and assumes that `123` is surrounded by word
    /// // boundaries. And of course, the match position is reported relative
    /// // to the sub-slice as well, which means we get `3` instead of `6`.
    /// let input = Input::new(&haystack[3..6]);
    /// let expected = Some(HalfMatch::must(0, 3));
    /// let got = dfa.try_search_fwd(&input)?;
    /// assert_eq!(expected, got);
    ///
    /// // But if we provide the bounds of the search within the context of the
    /// // entire haystack, then the search can take the surrounding context
    /// // into account. (And if we did find a match, it would be reported
    /// // as a valid offset into `haystack` instead of its sub-slice.)
    /// let input = Input::new(haystack).range(3..6);
    /// let expected = None;
    /// let got = dfa.try_search_fwd(&input)?;
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    fn try_search_fwd(
        &self,
        input: &Input<'_>,
    ) -> Result<Option<HalfMatch>, MatchError> {
        let utf8empty = self.has_empty() && self.is_utf8();
        let hm = match search::find_fwd(&self, input)? {
            None => return Ok(None),
            Some(hm) if !utf8empty => return Ok(Some(hm)),
            Some(hm) => hm,
        };
        // We get to this point when we know our DFA can match the empty string
        // AND when UTF-8 mode is enabled. In this case, we skip any matches
        // whose offset splits a codepoint. Such a match is necessarily a
        // zero-width match, because UTF-8 mode requires the underlying NFA
        // to be built such that all non-empty matches span valid UTF-8.
        // Therefore, any match that ends in the middle of a codepoint cannot
        // be part of a span of valid UTF-8 and thus must be an empty match.
        // In such cases, we skip it, so as not to report matches that split a
        // codepoint.
        //
        // Note that this is not a checked assumption. Callers *can* provide an
        // NFA with UTF-8 mode enabled but produces non-empty matches that span
        // invalid UTF-8. But doing so is documented to result in unspecified
        // behavior.
        empty::skip_splits_fwd(input, hm, hm.offset(), |input| {
            let got = search::find_fwd(&self, input)?;
            Ok(got.map(|hm| (hm, hm.offset())))
        })
    }

    /// Executes a reverse search and returns the start of the position of the
    /// leftmost match that is found. If no match exists, then `None` is
    /// returned.
    ///
    /// # Errors
    ///
    /// This routine errors if the search could not complete. This can occur
    /// in a number of circumstances:
    ///
    /// * The configuration of the DFA may permit it to "quit" the search.
    /// For example, setting quit bytes or enabling heuristic support for
    /// Unicode word boundaries. The default configuration does not enable any
    /// option that could result in the DFA quitting.
    /// * When the provided `Input` configuration is not supported. For
    /// example, by providing an unsupported anchor mode.
    ///
    /// When a search returns an error, callers cannot know whether a match
    /// exists or not.
    ///
    /// # Example
    ///
    /// This example shows how to use this method with a
    /// [`dense::DFA`](crate::dfa::dense::DFA). In particular, this
    /// routine is principally useful when used in conjunction with the
    /// [`nfa::thompson::Config::reverse`](crate::nfa::thompson::Config::reverse)
    /// configuration. In general, it's unlikely to be correct to use
    /// both `try_search_fwd` and `try_search_rev` with the same DFA since
    /// any particular DFA will only support searching in one direction with
    /// respect to the pattern.
    ///
    /// ```
    /// use regex_automata::{
    ///     nfa::thompson,
    ///     dfa::{Automaton, dense},
    ///     HalfMatch, Input,
    /// };
    ///
    /// let dfa = dense::Builder::new()
    ///     .thompson(thompson::Config::new().reverse(true))
    ///     .build("foo[0-9]+")?;
    /// let expected = Some(HalfMatch::must(0, 0));
    /// assert_eq!(expected, dfa.try_search_rev(&Input::new(b"foo12345"))?);
    ///
    /// // Even though a match is found after reading the last byte (`c`),
    /// // the leftmost first match semantics demand that we find the earliest
    /// // match that prefers earlier parts of the pattern over latter parts.
    /// let dfa = dense::Builder::new()
    ///     .thompson(thompson::Config::new().reverse(true))
    ///     .build("abc|c")?;
    /// let expected = Some(HalfMatch::must(0, 0));
    /// assert_eq!(expected, dfa.try_search_rev(&Input::new(b"abc"))?);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: UTF-8 mode
    ///
    /// This examples demonstrates that UTF-8 mode applies to reverse
    /// DFAs. When UTF-8 mode is enabled in the underlying NFA, then all
    /// matches reported must correspond to valid UTF-8 spans. This includes
    /// prohibiting zero-width matches that split a codepoint.
    ///
    /// UTF-8 mode is enabled by default. Notice below how the only zero-width
    /// matches reported are those at UTF-8 boundaries:
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{dense::DFA, Automaton},
    ///     nfa::thompson,
    ///     HalfMatch, Input, MatchKind,
    /// };
    ///
    /// let dfa = DFA::builder()
    ///     .thompson(thompson::Config::new().reverse(true))
    ///     .build(r"")?;
    ///
    /// // Run the reverse DFA to collect all matches.
    /// let mut input = Input::new("☃");
    /// let mut matches = vec![];
    /// loop {
    ///     match dfa.try_search_rev(&input)? {
    ///         None => break,
    ///         Some(hm) => {
    ///             matches.push(hm);
    ///             if hm.offset() == 0 || input.end() == 0 {
    ///                 break;
    ///             } else if hm.offset() < input.end() {
    ///                 input.set_end(hm.offset());
    ///             } else {
    ///                 // This is only necessary to handle zero-width
    ///                 // matches, which of course occur in this example.
    ///                 // Without this, the search would never advance
    ///                 // backwards beyond the initial match.
    ///                 input.set_end(input.end() - 1);
    ///             }
    ///         }
    ///     }
    /// }
    ///
    /// // No matches split a codepoint.
    /// let expected = vec![
    ///     HalfMatch::must(0, 3),
    ///     HalfMatch::must(0, 0),
    /// ];
    /// assert_eq!(expected, matches);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Now let's look at the same example, but with UTF-8 mode on the
    /// original NFA disabled (which results in disabling UTF-8 mode on the
    /// DFA):
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{dense::DFA, Automaton},
    ///     nfa::thompson,
    ///     HalfMatch, Input, MatchKind,
    /// };
    ///
    /// let dfa = DFA::builder()
    ///     .thompson(thompson::Config::new().reverse(true).utf8(false))
    ///     .build(r"")?;
    ///
    /// // Run the reverse DFA to collect all matches.
    /// let mut input = Input::new("☃");
    /// let mut matches = vec![];
    /// loop {
    ///     match dfa.try_search_rev(&input)? {
    ///         None => break,
    ///         Some(hm) => {
    ///             matches.push(hm);
    ///             if hm.offset() == 0 || input.end() == 0 {
    ///                 break;
    ///             } else if hm.offset() < input.end() {
    ///                 input.set_end(hm.offset());
    ///             } else {
    ///                 // This is only necessary to handle zero-width
    ///                 // matches, which of course occur in this example.
    ///                 // Without this, the search would never advance
    ///                 // backwards beyond the initial match.
    ///                 input.set_end(input.end() - 1);
    ///             }
    ///         }
    ///     }
    /// }
    ///
    /// // No matches split a codepoint.
    /// let expected = vec![
    ///     HalfMatch::must(0, 3),
    ///     HalfMatch::must(0, 2),
    ///     HalfMatch::must(0, 1),
    ///     HalfMatch::must(0, 0),
    /// ];
    /// assert_eq!(expected, matches);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    fn try_search_rev(
        &self,
        input: &Input<'_>,
    ) -> Result<Option<HalfMatch>, MatchError> {
        let utf8empty = self.has_empty() && self.is_utf8();
        let hm = match search::find_rev(self, input)? {
            None => return Ok(None),
            Some(hm) if !utf8empty => return Ok(Some(hm)),
            Some(hm) => hm,
        };
        empty::skip_splits_rev(input, hm, hm.offset(), |input| {
            let got = search::find_rev(self, input)?;
            Ok(got.map(|hm| (hm, hm.offset())))
        })
    }

    /// Executes an overlapping forward search. Matches, if one exists, can be
    /// obtained via the [`OverlappingState::get_match`] method.
    ///
    /// This routine is principally only useful when searching for multiple
    /// patterns on inputs where multiple patterns may match the same regions
    /// of text. In particular, callers must preserve the automaton's search
    /// state from prior calls so that the implementation knows where the last
    /// match occurred.
    ///
    /// When using this routine to implement an iterator of overlapping
    /// matches, the `start` of the search should always be set to the end
    /// of the last match. If more patterns match at the previous location,
    /// then they will be immediately returned. (This is tracked by the given
    /// overlapping state.) Otherwise, the search continues at the starting
    /// position given.
    ///
    /// If for some reason you want the search to forget about its previous
    /// state and restart the search at a particular position, then setting the
    /// state to [`OverlappingState::start`] will accomplish that.
    ///
    /// # Errors
    ///
    /// This routine errors if the search could not complete. This can occur
    /// in a number of circumstances:
    ///
    /// * The configuration of the DFA may permit it to "quit" the search.
    /// For example, setting quit bytes or enabling heuristic support for
    /// Unicode word boundaries. The default configuration does not enable any
    /// option that could result in the DFA quitting.
    /// * When the provided `Input` configuration is not supported. For
    /// example, by providing an unsupported anchor mode.
    ///
    /// When a search returns an error, callers cannot know whether a match
    /// exists or not.
    ///
    /// # Example
    ///
    /// This example shows how to run a basic overlapping search with a
    /// [`dense::DFA`](crate::dfa::dense::DFA). Notice that we build the
    /// automaton with a `MatchKind::All` configuration. Overlapping searches
    /// are unlikely to work as one would expect when using the default
    /// `MatchKind::LeftmostFirst` match semantics, since leftmost-first
    /// matching is fundamentally incompatible with overlapping searches.
    /// Namely, overlapping searches need to report matches as they are seen,
    /// where as leftmost-first searches will continue searching even after a
    /// match has been observed in order to find the conventional end position
    /// of the match. More concretely, leftmost-first searches use dead states
    /// to terminate a search after a specific match can no longer be extended.
    /// Overlapping searches instead do the opposite by continuing the search
    /// to find totally new matches (potentially of other patterns).
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{
    ///     dfa::{Automaton, OverlappingState, dense},
    ///     HalfMatch, Input, MatchKind,
    /// };
    ///
    /// let dfa = dense::Builder::new()
    ///     .configure(dense::Config::new().match_kind(MatchKind::All))
    ///     .build_many(&[r"[[:word:]]+$", r"[[:^space:]]+$"])?;
    /// let haystack = "@foo";
    /// let mut state = OverlappingState::start();
    ///
    /// let expected = Some(HalfMatch::must(1, 4));
    /// dfa.try_search_overlapping_fwd(&Input::new(haystack), &mut state)?;
    /// assert_eq!(expected, state.get_match());
    ///
    /// // The first pattern also matches at the same position, so re-running
    /// // the search will yield another match. Notice also that the first
    /// // pattern is returned after the second. This is because the second
    /// // pattern begins its match before the first, is therefore an earlier
    /// // match and is thus reported first.
    /// let expected = Some(HalfMatch::must(0, 4));
    /// dfa.try_search_overlapping_fwd(&Input::new(haystack), &mut state)?;
    /// assert_eq!(expected, state.get_match());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    fn try_search_overlapping_fwd(
        &self,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        let utf8empty = self.has_empty() && self.is_utf8();
        search::find_overlapping_fwd(self, input, state)?;
        match state.get_match() {
            None => Ok(()),
            Some(_) if !utf8empty => Ok(()),
            Some(_) => skip_empty_utf8_splits_overlapping(
                input,
                state,
                |input, state| {
                    search::find_overlapping_fwd(self, input, state)
                },
            ),
        }
    }

    /// Executes a reverse overlapping forward search. Matches, if one exists,
    /// can be obtained via the [`OverlappingState::get_match`] method.
    ///
    /// When using this routine to implement an iterator of overlapping
    /// matches, the `start` of the search should remain invariant throughout
    /// iteration. The `OverlappingState` given to the search will keep track
    /// of the current position of the search. (This is because multiple
    /// matches may be reported at the same position, so only the search
    /// implementation itself knows when to advance the position.)
    ///
    /// If for some reason you want the search to forget about its previous
    /// state and restart the search at a particular position, then setting the
    /// state to [`OverlappingState::start`] will accomplish that.
    ///
    /// # Errors
    ///
    /// This routine errors if the search could not complete. This can occur
    /// in a number of circumstances:
    ///
    /// * The configuration of the DFA may permit it to "quit" the search.
    /// For example, setting quit bytes or enabling heuristic support for
    /// Unicode word boundaries. The default configuration does not enable any
    /// option that could result in the DFA quitting.
    /// * When the provided `Input` configuration is not supported. For
    /// example, by providing an unsupported anchor mode.
    ///
    /// When a search returns an error, callers cannot know whether a match
    /// exists or not.
    ///
    /// # Example: UTF-8 mode
    ///
    /// This examples demonstrates that UTF-8 mode applies to reverse
    /// DFAs. When UTF-8 mode is enabled in the underlying NFA, then all
    /// matches reported must correspond to valid UTF-8 spans. This includes
    /// prohibiting zero-width matches that split a codepoint.
    ///
    /// UTF-8 mode is enabled by default. Notice below how the only zero-width
    /// matches reported are those at UTF-8 boundaries:
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{dense::DFA, Automaton, OverlappingState},
    ///     nfa::thompson,
    ///     HalfMatch, Input, MatchKind,
    /// };
    ///
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().match_kind(MatchKind::All))
    ///     .thompson(thompson::Config::new().reverse(true))
    ///     .build_many(&[r"", r"☃"])?;
    ///
    /// // Run the reverse DFA to collect all matches.
    /// let input = Input::new("☃");
    /// let mut state = OverlappingState::start();
    /// let mut matches = vec![];
    /// loop {
    ///     dfa.try_search_overlapping_rev(&input, &mut state)?;
    ///     match state.get_match() {
    ///         None => break,
    ///         Some(hm) => matches.push(hm),
    ///     }
    /// }
    ///
    /// // No matches split a codepoint.
    /// let expected = vec![
    ///     HalfMatch::must(0, 3),
    ///     HalfMatch::must(1, 0),
    ///     HalfMatch::must(0, 0),
    /// ];
    /// assert_eq!(expected, matches);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Now let's look at the same example, but with UTF-8 mode on the
    /// original NFA disabled (which results in disabling UTF-8 mode on the
    /// DFA):
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{dense::DFA, Automaton, OverlappingState},
    ///     nfa::thompson,
    ///     HalfMatch, Input, MatchKind,
    /// };
    ///
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().match_kind(MatchKind::All))
    ///     .thompson(thompson::Config::new().reverse(true).utf8(false))
    ///     .build_many(&[r"", r"☃"])?;
    ///
    /// // Run the reverse DFA to collect all matches.
    /// let input = Input::new("☃");
    /// let mut state = OverlappingState::start();
    /// let mut matches = vec![];
    /// loop {
    ///     dfa.try_search_overlapping_rev(&input, &mut state)?;
    ///     match state.get_match() {
    ///         None => break,
    ///         Some(hm) => matches.push(hm),
    ///     }
    /// }
    ///
    /// // Now *all* positions match, even within a codepoint,
    /// // because we lifted the requirement that matches
    /// // correspond to valid UTF-8 spans.
    /// let expected = vec![
    ///     HalfMatch::must(0, 3),
    ///     HalfMatch::must(0, 2),
    ///     HalfMatch::must(0, 1),
    ///     HalfMatch::must(1, 0),
    ///     HalfMatch::must(0, 0),
    /// ];
    /// assert_eq!(expected, matches);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    fn try_search_overlapping_rev(
        &self,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        let utf8empty = self.has_empty() && self.is_utf8();
        search::find_overlapping_rev(self, input, state)?;
        match state.get_match() {
            None => Ok(()),
            Some(_) if !utf8empty => Ok(()),
            Some(_) => skip_empty_utf8_splits_overlapping(
                input,
                state,
                |input, state| {
                    search::find_overlapping_rev(self, input, state)
                },
            ),
        }
    }

    /// Writes the set of patterns that match anywhere in the given search
    /// configuration to `patset`. If multiple patterns match at the same
    /// position and the underlying DFA supports overlapping matches, then all
    /// matching patterns are written to the given set.
    ///
    /// Unless all of the patterns in this DFA are anchored, then generally
    /// speaking, this will visit every byte in the haystack.
    ///
    /// This search routine *does not* clear the pattern set. This gives some
    /// flexibility to the caller (e.g., running multiple searches with the
    /// same pattern set), but does make the API bug-prone if you're reusing
    /// the same pattern set for multiple searches but intended them to be
    /// independent.
    ///
    /// If a pattern ID matched but the given `PatternSet` does not have
    /// sufficient capacity to store it, then it is not inserted and silently
    /// dropped.
    ///
    /// # Errors
    ///
    /// This routine errors if the search could not complete. This can occur
    /// in a number of circumstances:
    ///
    /// * The configuration of the DFA may permit it to "quit" the search.
    /// For example, setting quit bytes or enabling heuristic support for
    /// Unicode word boundaries. The default configuration does not enable any
    /// option that could result in the DFA quitting.
    /// * When the provided `Input` configuration is not supported. For
    /// example, by providing an unsupported anchor mode.
    ///
    /// When a search returns an error, callers cannot know whether a match
    /// exists or not.
    ///
    /// # Example
    ///
    /// This example shows how to find all matching patterns in a haystack,
    /// even when some patterns match at the same position as other patterns.
    ///
    /// ```
    /// # if cfg!(miri) { return Ok(()); } // miri takes too long
    /// use regex_automata::{
    ///     dfa::{Automaton, dense::DFA},
    ///     Input, MatchKind, PatternSet,
    /// };
    ///
    /// let patterns = &[
    ///     r"[[:word:]]+",
    ///     r"[0-9]+",
    ///     r"[[:alpha:]]+",
    ///     r"foo",
    ///     r"bar",
    ///     r"barfoo",
    ///     r"foobar",
    /// ];
    /// let dfa = DFA::builder()
    ///     .configure(DFA::config().match_kind(MatchKind::All))
    ///     .build_many(patterns)?;
    ///
    /// let input = Input::new("foobar");
    /// let mut patset = PatternSet::new(dfa.pattern_len());
    /// dfa.try_which_overlapping_matches(&input, &mut patset)?;
    /// let expected = vec![0, 2, 3, 4, 6];
    /// let got: Vec<usize> = patset.iter().map(|p| p.as_usize()).collect();
    /// assert_eq!(expected, got);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "alloc")]
    #[inline]
    fn try_which_overlapping_matches(
        &self,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) -> Result<(), MatchError> {
        let mut state = OverlappingState::start();
        while let Some(m) = {
            self.try_search_overlapping_fwd(input, &mut state)?;
            state.get_match()
        } {
            let _ = patset.insert(m.pattern());
            // There's nothing left to find, so we can stop. Or the caller
            // asked us to.
            if patset.is_full() || input.get_earliest() {
                break;
            }
        }
        Ok(())
    }
}

unsafe impl<'a, A: Automaton + ?Sized> Automaton for &'a A {
    #[inline]
    fn next_state(&self, current: StateID, input: u8) -> StateID {
        (**self).next_state(current, input)
    }

    #[inline]
    unsafe fn next_state_unchecked(
        &self,
        current: StateID,
        input: u8,
    ) -> StateID {
        (**self).next_state_unchecked(current, input)
    }

    #[inline]
    fn next_eoi_state(&self, current: StateID) -> StateID {
        (**self).next_eoi_state(current)
    }

    #[inline]
    fn start_state(
        &self,
        config: &start::Config,
    ) -> Result<StateID, StartError> {
        (**self).start_state(config)
    }

    #[inline]
    fn start_state_forward(
        &self,
        input: &Input<'_>,
    ) -> Result<StateID, MatchError> {
        (**self).start_state_forward(input)
    }

    #[inline]
    fn start_state_reverse(
        &self,
        input: &Input<'_>,
    ) -> Result<StateID, MatchError> {
        (**self).start_state_reverse(input)
    }

    #[inline]
    fn universal_start_state(&self, mode: Anchored) -> Option<StateID> {
        (**self).universal_start_state(mode)
    }

    #[inline]
    fn is_special_state(&self, id: StateID) -> bool {
        (**self).is_special_state(id)
    }

    #[inline]
    fn is_dead_state(&self, id: StateID) -> bool {
        (**self).is_dead_state(id)
    }

    #[inline]
    fn is_quit_state(&self, id: StateID) -> bool {
        (**self).is_quit_state(id)
    }

    #[inline]
    fn is_match_state(&self, id: StateID) -> bool {
        (**self).is_match_state(id)
    }

    #[inline]
    fn is_start_state(&self, id: StateID) -> bool {
        (**self).is_start_state(id)
    }

    #[inline]
    fn is_accel_state(&self, id: StateID) -> bool {
        (**self).is_accel_state(id)
    }

    #[inline]
    fn pattern_len(&self) -> usize {
        (**self).pattern_len()
    }

    #[inline]
    fn match_len(&self, id: StateID) -> usize {
        (**self).match_len(id)
    }

    #[inline]
    fn match_pattern(&self, id: StateID, index: usize) -> PatternID {
        (**self).match_pattern(id, index)
    }

    #[inline]
    fn has_empty(&self) -> bool {
        (**self).has_empty()
    }

    #[inline]
    fn is_utf8(&self) -> bool {
        (**self).is_utf8()
    }

    #[inline]
    fn is_always_start_anchored(&self) -> bool {
        (**self).is_always_start_anchored()
    }

    #[inline]
    fn accelerator(&self, id: StateID) -> &[u8] {
        (**self).accelerator(id)
    }

    #[inline]
    fn get_prefilter(&self) -> Option<&Prefilter> {
        (**self).get_prefilter()
    }

    #[inline]
    fn try_search_fwd(
        &self,
        input: &Input<'_>,
    ) -> Result<Option<HalfMatch>, MatchError> {
        (**self).try_search_fwd(input)
    }

    #[inline]
    fn try_search_rev(
        &self,
        input: &Input<'_>,
    ) -> Result<Option<HalfMatch>, MatchError> {
        (**self).try_search_rev(input)
    }

    #[inline]
    fn try_search_overlapping_fwd(
        &self,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        (**self).try_search_overlapping_fwd(input, state)
    }

    #[inline]
    fn try_search_overlapping_rev(
        &self,
        input: &Input<'_>,
        state: &mut OverlappingState,
    ) -> Result<(), MatchError> {
        (**self).try_search_overlapping_rev(input, state)
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn try_which_overlapping_matches(
        &self,
        input: &Input<'_>,
        patset: &mut PatternSet,
    ) -> Result<(), MatchError> {
        (**self).try_which_overlapping_matches(input, patset)
    }
}

/// Represents the current state of an overlapping search.
///
/// This is used for overlapping searches since they need to know something
/// about the previous search. For example, when multiple patterns match at the
/// same position, this state tracks the last reported pattern so that the next
/// search knows whether to report another matching pattern or continue with
/// the search at the next position. Additionally, it also tracks which state
/// the last search call terminated in.
///
/// This type provides little introspection capabilities. The only thing a
/// caller can do is construct it and pass it around to permit search routines
/// to use it to track state, and also ask whether a match has been found.
///
/// Callers should always provide a fresh state constructed via
/// [`OverlappingState::start`] when starting a new search. Reusing state from
/// a previous search may result in incorrect results.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlappingState {
    /// The match reported by the most recent overlapping search to use this
    /// state.
    ///
    /// If a search does not find any matches, then it is expected to clear
    /// this value.
    pub(crate) mat: Option<HalfMatch>,
    /// The state ID of the state at which the search was in when the call
    /// terminated. When this is a match state, `last_match` must be set to a
    /// non-None value.
    ///
    /// A `None` value indicates the start state of the corresponding
    /// automaton. We cannot use the actual ID, since any one automaton may
    /// have many start states, and which one is in use depends on several
    /// search-time factors.
    pub(crate) id: Option<StateID>,
    /// The position of the search.
    ///
    /// When `id` is None (i.e., we are starting a search), this is set to
    /// the beginning of the search as given by the caller regardless of its
    /// current value. Subsequent calls to an overlapping search pick up at
    /// this offset.
    pub(crate) at: usize,
    /// The index into the matching patterns of the next match to report if the
    /// current state is a match state. Note that this may be 1 greater than
    /// the total number of matches to report for the current match state. (In
    /// which case, no more matches should be reported at the current position
    /// and the search should advance to the next position.)
    pub(crate) next_match_index: Option<usize>,
    /// This is set to true when a reverse overlapping search has entered its
    /// EOI transitions.
    ///
    /// This isn't used in a forward search because it knows to stop once the
    /// position exceeds the end of the search range. In a reverse search,
    /// since we use unsigned offsets, we don't "know" once we've gone past
    /// `0`. So the only way to detect it is with this extra flag. The reverse
    /// overlapping search knows to terminate specifically after it has
    /// reported all matches after following the EOI transition.
    pub(crate) rev_eoi: bool,
}

impl OverlappingState {
    /// Create a new overlapping state that begins at the start state of any
    /// automaton.
    pub fn start() -> OverlappingState {
        OverlappingState {
            mat: None,
            id: None,
            at: 0,
            next_match_index: None,
            rev_eoi: false,
        }
    }

    /// Return the match result of the most recent search to execute with this
    /// state.
    ///
    /// A searches will clear this result automatically, such that if no
    /// match is found, this will correctly report `None`.
    pub fn get_match(&self) -> Option<HalfMatch> {
        self.mat
    }
}

/// An error that can occur when computing the start state for a search.
///
/// Computing a start state can fail for a few reasons, either based on
/// incorrect configuration or even based on whether the look-behind byte
/// triggers a quit state. Typically one does not need to handle this error
/// if you're using [`Automaton::start_state_forward`] (or its reverse
/// counterpart), as that routine automatically converts `StartError` to a
/// [`MatchError`] for you.
///
/// This error may be returned by the [`Automaton::start_state`] routine.
///
/// This error implements the `std::error::Error` trait when the `std` feature
/// is enabled.
///
/// This error is marked as non-exhaustive. New variants may be added in a
/// semver compatible release.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum StartError {
    /// An error that occurs when a starting configuration's look-behind byte
    /// is in this DFA's quit set.
    Quit {
        /// The quit byte that was found.
        byte: u8,
    },
    /// An error that occurs when the caller requests an anchored mode that
    /// isn't supported by the DFA.
    UnsupportedAnchored {
        /// The anchored mode given that is unsupported.
        mode: Anchored,
    },
}

impl StartError {
    pub(crate) fn quit(byte: u8) -> StartError {
        StartError::Quit { byte }
    }

    pub(crate) fn unsupported_anchored(mode: Anchored) -> StartError {
        StartError::UnsupportedAnchored { mode }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for StartError {}

impl core::fmt::Display for StartError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            StartError::Quit { byte } => write!(
                f,
                "error computing start state because the look-behind byte \
                 {:?} triggered a quit state",
                crate::util::escape::DebugByte(byte),
            ),
            StartError::UnsupportedAnchored { mode: Anchored::Yes } => {
                write!(
                    f,
                    "error computing start state because \
                     anchored searches are not supported or enabled"
                )
            }
            StartError::UnsupportedAnchored { mode: Anchored::No } => {
                write!(
                    f,
                    "error computing start state because \
                     unanchored searches are not supported or enabled"
                )
            }
            StartError::UnsupportedAnchored {
                mode: Anchored::Pattern(pid),
            } => {
                write!(
                    f,
                    "error computing start state because \
                     anchored searches for a specific pattern ({}) \
                     are not supported or enabled",
                    pid.as_usize(),
                )
            }
        }
    }
}

/// Runs the given overlapping `search` function (forwards or backwards) until
/// a match is found whose offset does not split a codepoint.
///
/// This is *not* always correct to call. It should only be called when the DFA
/// has UTF-8 mode enabled *and* it can produce zero-width matches. Calling
/// this when both of those things aren't true might result in legitimate
/// matches getting skipped.
#[cold]
#[inline(never)]
fn skip_empty_utf8_splits_overlapping<F>(
    input: &Input<'_>,
    state: &mut OverlappingState,
    mut search: F,
) -> Result<(), MatchError>
where
    F: FnMut(&Input<'_>, &mut OverlappingState) -> Result<(), MatchError>,
{
    // Note that this routine works for forwards and reverse searches
    // even though there's no code here to handle those cases. That's
    // because overlapping searches drive themselves to completion via
    // `OverlappingState`. So all we have to do is push it until no matches are
    // found.

    let mut hm = match state.get_match() {
        None => return Ok(()),
        Some(hm) => hm,
    };
    if input.get_anchored().is_anchored() {
        if !input.is_char_boundary(hm.offset()) {
            state.mat = None;
        }
        return Ok(());
    }
    while !input.is_char_boundary(hm.offset()) {
        search(input, state)?;
        hm = match state.get_match() {
            None => return Ok(()),
            Some(hm) => hm,
        };
    }
    Ok(())
}

/// Write a prefix "state" indicator for fmt::Debug impls.
///
/// Specifically, this tries to succinctly distinguish the different types of
/// states: dead states, quit states, accelerated states, start states and
/// match states. It even accounts for the possible overlappings of different
/// state types.
pub(crate) fn fmt_state_indicator<A: Automaton>(
    f: &mut core::fmt::Formatter<'_>,
    dfa: A,
    id: StateID,
) -> core::fmt::Result {
    if dfa.is_dead_state(id) {
        write!(f, "D")?;
        if dfa.is_start_state(id) {
            write!(f, ">")?;
        } else {
            write!(f, " ")?;
        }
    } else if dfa.is_quit_state(id) {
        write!(f, "Q ")?;
    } else if dfa.is_start_state(id) {
        if dfa.is_accel_state(id) {
            write!(f, "A>")?;
        } else {
            write!(f, " >")?;
        }
    } else if dfa.is_match_state(id) {
        if dfa.is_accel_state(id) {
            write!(f, "A*")?;
        } else {
            write!(f, " *")?;
        }
    } else if dfa.is_accel_state(id) {
        write!(f, "A ")?;
    } else {
        write!(f, "  ")?;
    }
    Ok(())
}

#[cfg(all(test, feature = "syntax", feature = "dfa-build"))]
mod tests {
    // A basic test ensuring that our Automaton trait is object safe. (This is
    // the main reason why we don't define the search routines as generic over
    // Into<Input>.)
    #[test]
    fn object_safe() {
        use crate::{
            dfa::{dense, Automaton},
            HalfMatch, Input,
        };

        let dfa = dense::DFA::new("abc").unwrap();
        let dfa: &dyn Automaton = &dfa;
        assert_eq!(
            Ok(Some(HalfMatch::must(0, 6))),
            dfa.try_search_fwd(&Input::new(b"xyzabcxyz")),
        );
    }
}
