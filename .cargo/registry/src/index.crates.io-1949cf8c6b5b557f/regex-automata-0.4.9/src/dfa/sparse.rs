/*!
Types and routines specific to sparse DFAs.

This module is the home of [`sparse::DFA`](DFA).

Unlike the [`dense`] module, this module does not contain a builder or
configuration specific for sparse DFAs. Instead, the intended way to build a
sparse DFA is either by using a default configuration with its constructor
[`sparse::DFA::new`](DFA::new), or by first configuring the construction of a
dense DFA with [`dense::Builder`] and then calling [`dense::DFA::to_sparse`].
For example, this configures a sparse DFA to do an overlapping search:

```
use regex_automata::{
    dfa::{Automaton, OverlappingState, dense},
    HalfMatch, Input, MatchKind,
};

let dense_re = dense::Builder::new()
    .configure(dense::Config::new().match_kind(MatchKind::All))
    .build(r"Samwise|Sam")?;
let sparse_re = dense_re.to_sparse()?;

// Setup our haystack and initial start state.
let input = Input::new("Samwise");
let mut state = OverlappingState::start();

// First, 'Sam' will match.
sparse_re.try_search_overlapping_fwd(&input, &mut state)?;
assert_eq!(Some(HalfMatch::must(0, 3)), state.get_match());

// And now 'Samwise' will match.
sparse_re.try_search_overlapping_fwd(&input, &mut state)?;
assert_eq!(Some(HalfMatch::must(0, 7)), state.get_match());
# Ok::<(), Box<dyn std::error::Error>>(())
```
*/

#[cfg(feature = "dfa-build")]
use core::iter;
use core::{fmt, mem::size_of};

#[cfg(feature = "dfa-build")]
use alloc::{vec, vec::Vec};

#[cfg(feature = "dfa-build")]
use crate::dfa::dense::{self, BuildError};
use crate::{
    dfa::{
        automaton::{fmt_state_indicator, Automaton, StartError},
        dense::Flags,
        special::Special,
        StartKind, DEAD,
    },
    util::{
        alphabet::{ByteClasses, ByteSet},
        escape::DebugByte,
        int::{Pointer, Usize, U16, U32},
        prefilter::Prefilter,
        primitives::{PatternID, StateID},
        search::Anchored,
        start::{self, Start, StartByteMap},
        wire::{self, DeserializeError, Endian, SerializeError},
    },
};

const LABEL: &str = "rust-regex-automata-dfa-sparse";
const VERSION: u32 = 2;

/// A sparse deterministic finite automaton (DFA) with variable sized states.
///
/// In contrast to a [dense::DFA], a sparse DFA uses a more space efficient
/// representation for its transitions. Consequently, sparse DFAs may use much
/// less memory than dense DFAs, but this comes at a price. In particular,
/// reading the more space efficient transitions takes more work, and
/// consequently, searching using a sparse DFA is typically slower than a dense
/// DFA.
///
/// A sparse DFA can be built using the default configuration via the
/// [`DFA::new`] constructor. Otherwise, one can configure various aspects of a
/// dense DFA via [`dense::Builder`], and then convert a dense DFA to a sparse
/// DFA using [`dense::DFA::to_sparse`].
///
/// In general, a sparse DFA supports all the same search operations as a dense
/// DFA.
///
/// Making the choice between a dense and sparse DFA depends on your specific
/// work load. If you can sacrifice a bit of search time performance, then a
/// sparse DFA might be the best choice. In particular, while sparse DFAs are
/// probably always slower than dense DFAs, you may find that they are easily
/// fast enough for your purposes!
///
/// # Type parameters
///
/// A `DFA` has one type parameter, `T`, which is used to represent the parts
/// of a sparse DFA. `T` is typically a `Vec<u8>` or a `&[u8]`.
///
/// # The `Automaton` trait
///
/// This type implements the [`Automaton`] trait, which means it can be used
/// for searching. For example:
///
/// ```
/// use regex_automata::{dfa::{Automaton, sparse::DFA}, HalfMatch, Input};
///
/// let dfa = DFA::new("foo[0-9]+")?;
/// let expected = Some(HalfMatch::must(0, 8));
/// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct DFA<T> {
    // When compared to a dense DFA, a sparse DFA *looks* a lot simpler
    // representation-wise. In reality, it is perhaps more complicated. Namely,
    // in a dense DFA, all information needs to be very cheaply accessible
    // using only state IDs. In a sparse DFA however, each state uses a
    // variable amount of space because each state encodes more information
    // than just its transitions. Each state also includes an accelerator if
    // one exists, along with the matching pattern IDs if the state is a match
    // state.
    //
    // That is, a lot of the complexity is pushed down into how each state
    // itself is represented.
    tt: Transitions<T>,
    st: StartTable<T>,
    special: Special,
    pre: Option<Prefilter>,
    quitset: ByteSet,
    flags: Flags,
}

#[cfg(feature = "dfa-build")]
impl DFA<Vec<u8>> {
    /// Parse the given regular expression using a default configuration and
    /// return the corresponding sparse DFA.
    ///
    /// If you want a non-default configuration, then use the
    /// [`dense::Builder`] to set your own configuration, and then call
    /// [`dense::DFA::to_sparse`] to create a sparse DFA.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, sparse}, HalfMatch, Input};
    ///
    /// let dfa = sparse::DFA::new("foo[0-9]+bar")?;
    ///
    /// let expected = Some(HalfMatch::must(0, 11));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345bar"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "syntax")]
    pub fn new(pattern: &str) -> Result<DFA<Vec<u8>>, BuildError> {
        dense::Builder::new()
            .build(pattern)
            .and_then(|dense| dense.to_sparse())
    }

    /// Parse the given regular expressions using a default configuration and
    /// return the corresponding multi-DFA.
    ///
    /// If you want a non-default configuration, then use the
    /// [`dense::Builder`] to set your own configuration, and then call
    /// [`dense::DFA::to_sparse`] to create a sparse DFA.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, sparse}, HalfMatch, Input};
    ///
    /// let dfa = sparse::DFA::new_many(&["[0-9]+", "[a-z]+"])?;
    /// let expected = Some(HalfMatch::must(1, 3));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345bar"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "syntax")]
    pub fn new_many<P: AsRef<str>>(
        patterns: &[P],
    ) -> Result<DFA<Vec<u8>>, BuildError> {
        dense::Builder::new()
            .build_many(patterns)
            .and_then(|dense| dense.to_sparse())
    }
}

#[cfg(feature = "dfa-build")]
impl DFA<Vec<u8>> {
    /// Create a new DFA that matches every input.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{
    ///     dfa::{Automaton, sparse},
    ///     HalfMatch, Input,
    /// };
    ///
    /// let dfa = sparse::DFA::always_match()?;
    ///
    /// let expected = Some(HalfMatch::must(0, 0));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new(""))?);
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn always_match() -> Result<DFA<Vec<u8>>, BuildError> {
        dense::DFA::always_match()?.to_sparse()
    }

    /// Create a new sparse DFA that never matches any input.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, sparse}, Input};
    ///
    /// let dfa = sparse::DFA::never_match()?;
    /// assert_eq!(None, dfa.try_search_fwd(&Input::new(""))?);
    /// assert_eq!(None, dfa.try_search_fwd(&Input::new("foo"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn never_match() -> Result<DFA<Vec<u8>>, BuildError> {
        dense::DFA::never_match()?.to_sparse()
    }

    /// The implementation for constructing a sparse DFA from a dense DFA.
    pub(crate) fn from_dense<T: AsRef<[u32]>>(
        dfa: &dense::DFA<T>,
    ) -> Result<DFA<Vec<u8>>, BuildError> {
        // In order to build the transition table, we need to be able to write
        // state identifiers for each of the "next" transitions in each state.
        // Our state identifiers correspond to the byte offset in the
        // transition table at which the state is encoded. Therefore, we do not
        // actually know what the state identifiers are until we've allocated
        // exactly as much space as we need for each state. Thus, construction
        // of the transition table happens in two passes.
        //
        // In the first pass, we fill out the shell of each state, which
        // includes the transition length, the input byte ranges and
        // zero-filled space for the transitions and accelerators, if present.
        // In this first pass, we also build up a map from the state identifier
        // index of the dense DFA to the state identifier in this sparse DFA.
        //
        // In the second pass, we fill in the transitions based on the map
        // built in the first pass.

        // The capacity given here reflects a minimum. (Well, the true minimum
        // is likely even bigger, but hopefully this saves a few reallocs.)
        let mut sparse = Vec::with_capacity(StateID::SIZE * dfa.state_len());
        // This maps state indices from the dense DFA to StateIDs in the sparse
        // DFA. We build out this map on the first pass, and then use it in the
        // second pass to back-fill our transitions.
        let mut remap: Vec<StateID> = vec![DEAD; dfa.state_len()];
        for state in dfa.states() {
            let pos = sparse.len();

            remap[dfa.to_index(state.id())] = StateID::new(pos)
                .map_err(|_| BuildError::too_many_states())?;
            // zero-filled space for the transition length
            sparse.push(0);
            sparse.push(0);

            let mut transition_len = 0;
            for (unit1, unit2, _) in state.sparse_transitions() {
                match (unit1.as_u8(), unit2.as_u8()) {
                    (Some(b1), Some(b2)) => {
                        transition_len += 1;
                        sparse.push(b1);
                        sparse.push(b2);
                    }
                    (None, None) => {}
                    (Some(_), None) | (None, Some(_)) => {
                        // can never occur because sparse_transitions never
                        // groups EOI with any other transition.
                        unreachable!()
                    }
                }
            }
            // Add dummy EOI transition. This is never actually read while
            // searching, but having space equivalent to the total number
            // of transitions is convenient. Otherwise, we'd need to track
            // a different number of transitions for the byte ranges as for
            // the 'next' states.
            //
            // N.B. The loop above is not guaranteed to yield the EOI
            // transition, since it may point to a DEAD state. By putting
            // it here, we always write the EOI transition, and thus
            // guarantee that our transition length is >0. Why do we always
            // need the EOI transition? Because in order to implement
            // Automaton::next_eoi_state, this lets us just ask for the last
            // transition. There are probably other/better ways to do this.
            transition_len += 1;
            sparse.push(0);
            sparse.push(0);

            // Check some assumptions about transition length.
            assert_ne!(
                transition_len, 0,
                "transition length should be non-zero",
            );
            assert!(
                transition_len <= 257,
                "expected transition length {} to be <= 257",
                transition_len,
            );

            // Fill in the transition length.
            // Since transition length is always <= 257, we use the most
            // significant bit to indicate whether this is a match state or
            // not.
            let ntrans = if dfa.is_match_state(state.id()) {
                transition_len | (1 << 15)
            } else {
                transition_len
            };
            wire::NE::write_u16(ntrans, &mut sparse[pos..]);

            // zero-fill the actual transitions.
            // Unwraps are OK since transition_length <= 257 and our minimum
            // support usize size is 16-bits.
            let zeros = usize::try_from(transition_len)
                .unwrap()
                .checked_mul(StateID::SIZE)
                .unwrap();
            sparse.extend(iter::repeat(0).take(zeros));

            // If this is a match state, write the pattern IDs matched by this
            // state.
            if dfa.is_match_state(state.id()) {
                let plen = dfa.match_pattern_len(state.id());
                // Write the actual pattern IDs with a u32 length prefix.
                // First, zero-fill space.
                let mut pos = sparse.len();
                // Unwraps are OK since it's guaranteed that plen <=
                // PatternID::LIMIT, which is in turn guaranteed to fit into a
                // u32.
                let zeros = size_of::<u32>()
                    .checked_mul(plen)
                    .unwrap()
                    .checked_add(size_of::<u32>())
                    .unwrap();
                sparse.extend(iter::repeat(0).take(zeros));

                // Now write the length prefix.
                wire::NE::write_u32(
                    // Will never fail since u32::MAX is invalid pattern ID.
                    // Thus, the number of pattern IDs is representable by a
                    // u32.
                    plen.try_into().expect("pattern ID length fits in u32"),
                    &mut sparse[pos..],
                );
                pos += size_of::<u32>();

                // Now write the pattern IDs.
                for &pid in dfa.pattern_id_slice(state.id()) {
                    pos += wire::write_pattern_id::<wire::NE>(
                        pid,
                        &mut sparse[pos..],
                    );
                }
            }

            // And now add the accelerator, if one exists. An accelerator is
            // at most 4 bytes and at least 1 byte. The first byte is the
            // length, N. N bytes follow the length. The set of bytes that
            // follow correspond (exhaustively) to the bytes that must be seen
            // to leave this state.
            let accel = dfa.accelerator(state.id());
            sparse.push(accel.len().try_into().unwrap());
            sparse.extend_from_slice(accel);
        }

        let mut new = DFA {
            tt: Transitions {
                sparse,
                classes: dfa.byte_classes().clone(),
                state_len: dfa.state_len(),
                pattern_len: dfa.pattern_len(),
            },
            st: StartTable::from_dense_dfa(dfa, &remap)?,
            special: dfa.special().remap(|id| remap[dfa.to_index(id)]),
            pre: dfa.get_prefilter().map(|p| p.clone()),
            quitset: dfa.quitset().clone(),
            flags: dfa.flags().clone(),
        };
        // And here's our second pass. Iterate over all of the dense states
        // again, and update the transitions in each of the states in the
        // sparse DFA.
        for old_state in dfa.states() {
            let new_id = remap[dfa.to_index(old_state.id())];
            let mut new_state = new.tt.state_mut(new_id);
            let sparse = old_state.sparse_transitions();
            for (i, (_, _, next)) in sparse.enumerate() {
                let next = remap[dfa.to_index(next)];
                new_state.set_next_at(i, next);
            }
        }
        debug!(
            "created sparse DFA, memory usage: {} (dense memory usage: {})",
            new.memory_usage(),
            dfa.memory_usage(),
        );
        Ok(new)
    }
}

impl<T: AsRef<[u8]>> DFA<T> {
    /// Cheaply return a borrowed version of this sparse DFA. Specifically, the
    /// DFA returned always uses `&[u8]` for its transitions.
    pub fn as_ref<'a>(&'a self) -> DFA<&'a [u8]> {
        DFA {
            tt: self.tt.as_ref(),
            st: self.st.as_ref(),
            special: self.special,
            pre: self.pre.clone(),
            quitset: self.quitset,
            flags: self.flags,
        }
    }

    /// Return an owned version of this sparse DFA. Specifically, the DFA
    /// returned always uses `Vec<u8>` for its transitions.
    ///
    /// Effectively, this returns a sparse DFA whose transitions live on the
    /// heap.
    #[cfg(feature = "alloc")]
    pub fn to_owned(&self) -> DFA<alloc::vec::Vec<u8>> {
        DFA {
            tt: self.tt.to_owned(),
            st: self.st.to_owned(),
            special: self.special,
            pre: self.pre.clone(),
            quitset: self.quitset,
            flags: self.flags,
        }
    }

    /// Returns the starting state configuration for this DFA.
    ///
    /// The default is [`StartKind::Both`], which means the DFA supports both
    /// unanchored and anchored searches. However, this can generally lead to
    /// bigger DFAs. Therefore, a DFA might be compiled with support for just
    /// unanchored or anchored searches. In that case, running a search with
    /// an unsupported configuration will panic.
    pub fn start_kind(&self) -> StartKind {
        self.st.kind
    }

    /// Returns true only if this DFA has starting states for each pattern.
    ///
    /// When a DFA has starting states for each pattern, then a search with the
    /// DFA can be configured to only look for anchored matches of a specific
    /// pattern. Specifically, APIs like [`Automaton::try_search_fwd`] can
    /// accept a [`Anchored::Pattern`] if and only if this method returns true.
    /// Otherwise, an error will be returned.
    ///
    /// Note that if the DFA is empty, this always returns false.
    pub fn starts_for_each_pattern(&self) -> bool {
        self.st.pattern_len.is_some()
    }

    /// Returns the equivalence classes that make up the alphabet for this DFA.
    ///
    /// Unless [`dense::Config::byte_classes`] was disabled, it is possible
    /// that multiple distinct bytes are grouped into the same equivalence
    /// class if it is impossible for them to discriminate between a match and
    /// a non-match. This has the effect of reducing the overall alphabet size
    /// and in turn potentially substantially reducing the size of the DFA's
    /// transition table.
    ///
    /// The downside of using equivalence classes like this is that every state
    /// transition will automatically use this map to convert an arbitrary
    /// byte to its corresponding equivalence class. In practice this has a
    /// negligible impact on performance.
    pub fn byte_classes(&self) -> &ByteClasses {
        &self.tt.classes
    }

    /// Returns the memory usage, in bytes, of this DFA.
    ///
    /// The memory usage is computed based on the number of bytes used to
    /// represent this DFA.
    ///
    /// This does **not** include the stack size used up by this DFA. To
    /// compute that, use `std::mem::size_of::<sparse::DFA>()`.
    pub fn memory_usage(&self) -> usize {
        self.tt.memory_usage() + self.st.memory_usage()
    }
}

/// Routines for converting a sparse DFA to other representations, such as raw
/// bytes suitable for persistent storage.
impl<T: AsRef<[u8]>> DFA<T> {
    /// Serialize this DFA as raw bytes to a `Vec<u8>` in little endian
    /// format.
    ///
    /// The written bytes are guaranteed to be deserialized correctly and
    /// without errors in a semver compatible release of this crate by a
    /// `DFA`'s deserialization APIs (assuming all other criteria for the
    /// deserialization APIs has been satisfied):
    ///
    /// * [`DFA::from_bytes`]
    /// * [`DFA::from_bytes_unchecked`]
    ///
    /// Note that unlike a [`dense::DFA`]'s serialization methods, this does
    /// not add any initial padding to the returned bytes. Padding isn't
    /// required for sparse DFAs since they have no alignment requirements.
    ///
    /// # Example
    ///
    /// This example shows how to serialize and deserialize a DFA:
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, sparse::DFA}, HalfMatch, Input};
    ///
    /// // Compile our original DFA.
    /// let original_dfa = DFA::new("foo[0-9]+")?;
    ///
    /// // N.B. We use native endianness here to make the example work, but
    /// // using to_bytes_little_endian would work on a little endian target.
    /// let buf = original_dfa.to_bytes_native_endian();
    /// // Even if buf has initial padding, DFA::from_bytes will automatically
    /// // ignore it.
    /// let dfa: DFA<&[u8]> = DFA::from_bytes(&buf)?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "dfa-build")]
    pub fn to_bytes_little_endian(&self) -> Vec<u8> {
        self.to_bytes::<wire::LE>()
    }

    /// Serialize this DFA as raw bytes to a `Vec<u8>` in big endian
    /// format.
    ///
    /// The written bytes are guaranteed to be deserialized correctly and
    /// without errors in a semver compatible release of this crate by a
    /// `DFA`'s deserialization APIs (assuming all other criteria for the
    /// deserialization APIs has been satisfied):
    ///
    /// * [`DFA::from_bytes`]
    /// * [`DFA::from_bytes_unchecked`]
    ///
    /// Note that unlike a [`dense::DFA`]'s serialization methods, this does
    /// not add any initial padding to the returned bytes. Padding isn't
    /// required for sparse DFAs since they have no alignment requirements.
    ///
    /// # Example
    ///
    /// This example shows how to serialize and deserialize a DFA:
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, sparse::DFA}, HalfMatch, Input};
    ///
    /// // Compile our original DFA.
    /// let original_dfa = DFA::new("foo[0-9]+")?;
    ///
    /// // N.B. We use native endianness here to make the example work, but
    /// // using to_bytes_big_endian would work on a big endian target.
    /// let buf = original_dfa.to_bytes_native_endian();
    /// // Even if buf has initial padding, DFA::from_bytes will automatically
    /// // ignore it.
    /// let dfa: DFA<&[u8]> = DFA::from_bytes(&buf)?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "dfa-build")]
    pub fn to_bytes_big_endian(&self) -> Vec<u8> {
        self.to_bytes::<wire::BE>()
    }

    /// Serialize this DFA as raw bytes to a `Vec<u8>` in native endian
    /// format.
    ///
    /// The written bytes are guaranteed to be deserialized correctly and
    /// without errors in a semver compatible release of this crate by a
    /// `DFA`'s deserialization APIs (assuming all other criteria for the
    /// deserialization APIs has been satisfied):
    ///
    /// * [`DFA::from_bytes`]
    /// * [`DFA::from_bytes_unchecked`]
    ///
    /// Note that unlike a [`dense::DFA`]'s serialization methods, this does
    /// not add any initial padding to the returned bytes. Padding isn't
    /// required for sparse DFAs since they have no alignment requirements.
    ///
    /// Generally speaking, native endian format should only be used when
    /// you know that the target you're compiling the DFA for matches the
    /// endianness of the target on which you're compiling DFA. For example,
    /// if serialization and deserialization happen in the same process or on
    /// the same machine. Otherwise, when serializing a DFA for use in a
    /// portable environment, you'll almost certainly want to serialize _both_
    /// a little endian and a big endian version and then load the correct one
    /// based on the target's configuration.
    ///
    /// # Example
    ///
    /// This example shows how to serialize and deserialize a DFA:
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, sparse::DFA}, HalfMatch, Input};
    ///
    /// // Compile our original DFA.
    /// let original_dfa = DFA::new("foo[0-9]+")?;
    ///
    /// let buf = original_dfa.to_bytes_native_endian();
    /// // Even if buf has initial padding, DFA::from_bytes will automatically
    /// // ignore it.
    /// let dfa: DFA<&[u8]> = DFA::from_bytes(&buf)?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "dfa-build")]
    pub fn to_bytes_native_endian(&self) -> Vec<u8> {
        self.to_bytes::<wire::NE>()
    }

    /// The implementation of the public `to_bytes` serialization methods,
    /// which is generic over endianness.
    #[cfg(feature = "dfa-build")]
    fn to_bytes<E: Endian>(&self) -> Vec<u8> {
        let mut buf = vec![0; self.write_to_len()];
        // This should always succeed since the only possible serialization
        // error is providing a buffer that's too small, but we've ensured that
        // `buf` is big enough here.
        self.write_to::<E>(&mut buf).unwrap();
        buf
    }

    /// Serialize this DFA as raw bytes to the given slice, in little endian
    /// format. Upon success, the total number of bytes written to `dst` is
    /// returned.
    ///
    /// The written bytes are guaranteed to be deserialized correctly and
    /// without errors in a semver compatible release of this crate by a
    /// `DFA`'s deserialization APIs (assuming all other criteria for the
    /// deserialization APIs has been satisfied):
    ///
    /// * [`DFA::from_bytes`]
    /// * [`DFA::from_bytes_unchecked`]
    ///
    /// # Errors
    ///
    /// This returns an error if the given destination slice is not big enough
    /// to contain the full serialized DFA. If an error occurs, then nothing
    /// is written to `dst`.
    ///
    /// # Example
    ///
    /// This example shows how to serialize and deserialize a DFA without
    /// dynamic memory allocation.
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, sparse::DFA}, HalfMatch, Input};
    ///
    /// // Compile our original DFA.
    /// let original_dfa = DFA::new("foo[0-9]+")?;
    ///
    /// // Create a 4KB buffer on the stack to store our serialized DFA.
    /// let mut buf = [0u8; 4 * (1<<10)];
    /// // N.B. We use native endianness here to make the example work, but
    /// // using write_to_little_endian would work on a little endian target.
    /// let written = original_dfa.write_to_native_endian(&mut buf)?;
    /// let dfa: DFA<&[u8]> = DFA::from_bytes(&buf[..written])?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn write_to_little_endian(
        &self,
        dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        self.write_to::<wire::LE>(dst)
    }

    /// Serialize this DFA as raw bytes to the given slice, in big endian
    /// format. Upon success, the total number of bytes written to `dst` is
    /// returned.
    ///
    /// The written bytes are guaranteed to be deserialized correctly and
    /// without errors in a semver compatible release of this crate by a
    /// `DFA`'s deserialization APIs (assuming all other criteria for the
    /// deserialization APIs has been satisfied):
    ///
    /// * [`DFA::from_bytes`]
    /// * [`DFA::from_bytes_unchecked`]
    ///
    /// # Errors
    ///
    /// This returns an error if the given destination slice is not big enough
    /// to contain the full serialized DFA. If an error occurs, then nothing
    /// is written to `dst`.
    ///
    /// # Example
    ///
    /// This example shows how to serialize and deserialize a DFA without
    /// dynamic memory allocation.
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, sparse::DFA}, HalfMatch, Input};
    ///
    /// // Compile our original DFA.
    /// let original_dfa = DFA::new("foo[0-9]+")?;
    ///
    /// // Create a 4KB buffer on the stack to store our serialized DFA.
    /// let mut buf = [0u8; 4 * (1<<10)];
    /// // N.B. We use native endianness here to make the example work, but
    /// // using write_to_big_endian would work on a big endian target.
    /// let written = original_dfa.write_to_native_endian(&mut buf)?;
    /// let dfa: DFA<&[u8]> = DFA::from_bytes(&buf[..written])?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn write_to_big_endian(
        &self,
        dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        self.write_to::<wire::BE>(dst)
    }

    /// Serialize this DFA as raw bytes to the given slice, in native endian
    /// format. Upon success, the total number of bytes written to `dst` is
    /// returned.
    ///
    /// The written bytes are guaranteed to be deserialized correctly and
    /// without errors in a semver compatible release of this crate by a
    /// `DFA`'s deserialization APIs (assuming all other criteria for the
    /// deserialization APIs has been satisfied):
    ///
    /// * [`DFA::from_bytes`]
    /// * [`DFA::from_bytes_unchecked`]
    ///
    /// Generally speaking, native endian format should only be used when
    /// you know that the target you're compiling the DFA for matches the
    /// endianness of the target on which you're compiling DFA. For example,
    /// if serialization and deserialization happen in the same process or on
    /// the same machine. Otherwise, when serializing a DFA for use in a
    /// portable environment, you'll almost certainly want to serialize _both_
    /// a little endian and a big endian version and then load the correct one
    /// based on the target's configuration.
    ///
    /// # Errors
    ///
    /// This returns an error if the given destination slice is not big enough
    /// to contain the full serialized DFA. If an error occurs, then nothing
    /// is written to `dst`.
    ///
    /// # Example
    ///
    /// This example shows how to serialize and deserialize a DFA without
    /// dynamic memory allocation.
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, sparse::DFA}, HalfMatch, Input};
    ///
    /// // Compile our original DFA.
    /// let original_dfa = DFA::new("foo[0-9]+")?;
    ///
    /// // Create a 4KB buffer on the stack to store our serialized DFA.
    /// let mut buf = [0u8; 4 * (1<<10)];
    /// let written = original_dfa.write_to_native_endian(&mut buf)?;
    /// let dfa: DFA<&[u8]> = DFA::from_bytes(&buf[..written])?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn write_to_native_endian(
        &self,
        dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        self.write_to::<wire::NE>(dst)
    }

    /// The implementation of the public `write_to` serialization methods,
    /// which is generic over endianness.
    fn write_to<E: Endian>(
        &self,
        dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        let mut nw = 0;
        nw += wire::write_label(LABEL, &mut dst[nw..])?;
        nw += wire::write_endianness_check::<E>(&mut dst[nw..])?;
        nw += wire::write_version::<E>(VERSION, &mut dst[nw..])?;
        nw += {
            // Currently unused, intended for future flexibility
            E::write_u32(0, &mut dst[nw..]);
            size_of::<u32>()
        };
        nw += self.flags.write_to::<E>(&mut dst[nw..])?;
        nw += self.tt.write_to::<E>(&mut dst[nw..])?;
        nw += self.st.write_to::<E>(&mut dst[nw..])?;
        nw += self.special.write_to::<E>(&mut dst[nw..])?;
        nw += self.quitset.write_to::<E>(&mut dst[nw..])?;
        Ok(nw)
    }

    /// Return the total number of bytes required to serialize this DFA.
    ///
    /// This is useful for determining the size of the buffer required to pass
    /// to one of the serialization routines:
    ///
    /// * [`DFA::write_to_little_endian`]
    /// * [`DFA::write_to_big_endian`]
    /// * [`DFA::write_to_native_endian`]
    ///
    /// Passing a buffer smaller than the size returned by this method will
    /// result in a serialization error.
    ///
    /// # Example
    ///
    /// This example shows how to dynamically allocate enough room to serialize
    /// a sparse DFA.
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, sparse::DFA}, HalfMatch, Input};
    ///
    /// // Compile our original DFA.
    /// let original_dfa = DFA::new("foo[0-9]+")?;
    ///
    /// let mut buf = vec![0; original_dfa.write_to_len()];
    /// let written = original_dfa.write_to_native_endian(&mut buf)?;
    /// let dfa: DFA<&[u8]> = DFA::from_bytes(&buf[..written])?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn write_to_len(&self) -> usize {
        wire::write_label_len(LABEL)
        + wire::write_endianness_check_len()
        + wire::write_version_len()
        + size_of::<u32>() // unused, intended for future flexibility
        + self.flags.write_to_len()
        + self.tt.write_to_len()
        + self.st.write_to_len()
        + self.special.write_to_len()
        + self.quitset.write_to_len()
    }
}

impl<'a> DFA<&'a [u8]> {
    /// Safely deserialize a sparse DFA with a specific state identifier
    /// representation. Upon success, this returns both the deserialized DFA
    /// and the number of bytes read from the given slice. Namely, the contents
    /// of the slice beyond the DFA are not read.
    ///
    /// Deserializing a DFA using this routine will never allocate heap memory.
    /// For safety purposes, the DFA's transitions will be verified such that
    /// every transition points to a valid state. If this verification is too
    /// costly, then a [`DFA::from_bytes_unchecked`] API is provided, which
    /// will always execute in constant time.
    ///
    /// The bytes given must be generated by one of the serialization APIs
    /// of a `DFA` using a semver compatible release of this crate. Those
    /// include:
    ///
    /// * [`DFA::to_bytes_little_endian`]
    /// * [`DFA::to_bytes_big_endian`]
    /// * [`DFA::to_bytes_native_endian`]
    /// * [`DFA::write_to_little_endian`]
    /// * [`DFA::write_to_big_endian`]
    /// * [`DFA::write_to_native_endian`]
    ///
    /// The `to_bytes` methods allocate and return a `Vec<u8>` for you. The
    /// `write_to` methods do not allocate and write to an existing slice
    /// (which may be on the stack). Since deserialization always uses the
    /// native endianness of the target platform, the serialization API you use
    /// should match the endianness of the target platform. (It's often a good
    /// idea to generate serialized DFAs for both forms of endianness and then
    /// load the correct one based on endianness.)
    ///
    /// # Errors
    ///
    /// Generally speaking, it's easier to state the conditions in which an
    /// error is _not_ returned. All of the following must be true:
    ///
    /// * The bytes given must be produced by one of the serialization APIs
    ///   on this DFA, as mentioned above.
    /// * The endianness of the target platform matches the endianness used to
    ///   serialized the provided DFA.
    ///
    /// If any of the above are not true, then an error will be returned.
    ///
    /// Note that unlike deserializing a [`dense::DFA`], deserializing a sparse
    /// DFA has no alignment requirements. That is, an alignment of `1` is
    /// valid.
    ///
    /// # Panics
    ///
    /// This routine will never panic for any input.
    ///
    /// # Example
    ///
    /// This example shows how to serialize a DFA to raw bytes, deserialize it
    /// and then use it for searching.
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, sparse::DFA}, HalfMatch, Input};
    ///
    /// let initial = DFA::new("foo[0-9]+")?;
    /// let bytes = initial.to_bytes_native_endian();
    /// let dfa: DFA<&[u8]> = DFA::from_bytes(&bytes)?.0;
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Example: loading a DFA from static memory
    ///
    /// One use case this library supports is the ability to serialize a
    /// DFA to disk and then use `include_bytes!` to store it in a compiled
    /// Rust program. Those bytes can then be cheaply deserialized into a
    /// `DFA` structure at runtime and used for searching without having to
    /// re-compile the DFA (which can be quite costly).
    ///
    /// We can show this in two parts. The first part is serializing the DFA to
    /// a file:
    ///
    /// ```no_run
    /// use regex_automata::dfa::sparse::DFA;
    ///
    /// let dfa = DFA::new("foo[0-9]+")?;
    ///
    /// // Write a big endian serialized version of this DFA to a file.
    /// let bytes = dfa.to_bytes_big_endian();
    /// std::fs::write("foo.bigendian.dfa", &bytes)?;
    ///
    /// // Do it again, but this time for little endian.
    /// let bytes = dfa.to_bytes_little_endian();
    /// std::fs::write("foo.littleendian.dfa", &bytes)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// And now the second part is embedding the DFA into the compiled program
    /// and deserializing it at runtime on first use. We use conditional
    /// compilation to choose the correct endianness. We do not need to employ
    /// any special tricks to ensure a proper alignment, since a sparse DFA has
    /// no alignment requirements.
    ///
    /// ```no_run
    /// use regex_automata::{
    ///     dfa::{Automaton, sparse::DFA},
    ///     util::lazy::Lazy,
    ///     HalfMatch, Input,
    /// };
    ///
    /// // This crate provides its own "lazy" type, kind of like
    /// // lazy_static! or once_cell::sync::Lazy. But it works in no-alloc
    /// // no-std environments and let's us write this using completely
    /// // safe code.
    /// static RE: Lazy<DFA<&'static [u8]>> = Lazy::new(|| {
    ///     # const _: &str = stringify! {
    ///     #[cfg(target_endian = "big")]
    ///     static BYTES: &[u8] = include_bytes!("foo.bigendian.dfa");
    ///     #[cfg(target_endian = "little")]
    ///     static BYTES: &[u8] = include_bytes!("foo.littleendian.dfa");
    ///     # };
    ///     # static BYTES: &[u8] = b"";
    ///
    ///     let (dfa, _) = DFA::from_bytes(BYTES)
    ///         .expect("serialized DFA should be valid");
    ///     dfa
    /// });
    ///
    /// let expected = Ok(Some(HalfMatch::must(0, 8)));
    /// assert_eq!(expected, RE.try_search_fwd(&Input::new("foo12345")));
    /// ```
    ///
    /// Alternatively, consider using
    /// [`lazy_static`](https://crates.io/crates/lazy_static)
    /// or
    /// [`once_cell`](https://crates.io/crates/once_cell),
    /// which will guarantee safety for you.
    pub fn from_bytes(
        slice: &'a [u8],
    ) -> Result<(DFA<&'a [u8]>, usize), DeserializeError> {
        // SAFETY: This is safe because we validate both the sparse transitions
        // (by trying to decode every state) and start state ID list below. If
        // either validation fails, then we return an error.
        let (dfa, nread) = unsafe { DFA::from_bytes_unchecked(slice)? };
        let seen = dfa.tt.validate(&dfa.special)?;
        dfa.st.validate(&dfa.special, &seen)?;
        // N.B. dfa.special doesn't have a way to do unchecked deserialization,
        // so it has already been validated.
        Ok((dfa, nread))
    }

    /// Deserialize a DFA with a specific state identifier representation in
    /// constant time by omitting the verification of the validity of the
    /// sparse transitions.
    ///
    /// This is just like [`DFA::from_bytes`], except it can potentially return
    /// a DFA that exhibits undefined behavior if its transitions contains
    /// invalid state identifiers.
    ///
    /// This routine is useful if you need to deserialize a DFA cheaply and
    /// cannot afford the transition validation performed by `from_bytes`.
    ///
    /// # Safety
    ///
    /// This routine is not safe because it permits callers to provide
    /// arbitrary transitions with possibly incorrect state identifiers. While
    /// the various serialization routines will never return an incorrect
    /// DFA, there is no guarantee that the bytes provided here are correct.
    /// While `from_bytes_unchecked` will still do several forms of basic
    /// validation, this routine does not check that the transitions themselves
    /// are correct. Given an incorrect transition table, it is possible for
    /// the search routines to access out-of-bounds memory because of explicit
    /// bounds check elision.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{dfa::{Automaton, sparse::DFA}, HalfMatch, Input};
    ///
    /// let initial = DFA::new("foo[0-9]+")?;
    /// let bytes = initial.to_bytes_native_endian();
    /// // SAFETY: This is guaranteed to be safe since the bytes given come
    /// // directly from a compatible serialization routine.
    /// let dfa: DFA<&[u8]> = unsafe { DFA::from_bytes_unchecked(&bytes)?.0 };
    ///
    /// let expected = Some(HalfMatch::must(0, 8));
    /// assert_eq!(expected, dfa.try_search_fwd(&Input::new("foo12345"))?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub unsafe fn from_bytes_unchecked(
        slice: &'a [u8],
    ) -> Result<(DFA<&'a [u8]>, usize), DeserializeError> {
        let mut nr = 0;

        nr += wire::read_label(&slice[nr..], LABEL)?;
        nr += wire::read_endianness_check(&slice[nr..])?;
        nr += wire::read_version(&slice[nr..], VERSION)?;

        let _unused = wire::try_read_u32(&slice[nr..], "unused space")?;
        nr += size_of::<u32>();

        let (flags, nread) = Flags::from_bytes(&slice[nr..])?;
        nr += nread;

        let (tt, nread) = Transitions::from_bytes_unchecked(&slice[nr..])?;
        nr += nread;

        let (st, nread) = StartTable::from_bytes_unchecked(&slice[nr..])?;
        nr += nread;

        let (special, nread) = Special::from_bytes(&slice[nr..])?;
        nr += nread;
        if special.max.as_usize() >= tt.sparse().len() {
            return Err(DeserializeError::generic(
                "max should not be greater than or equal to sparse bytes",
            ));
        }

        let (quitset, nread) = ByteSet::from_bytes(&slice[nr..])?;
        nr += nread;

        // Prefilters don't support serialization, so they're always absent.
        let pre = None;
        Ok((DFA { tt, st, special, pre, quitset, flags }, nr))
    }
}

impl<T: AsRef<[u8]>> fmt::Debug for DFA<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "sparse::DFA(")?;
        for state in self.tt.states() {
            fmt_state_indicator(f, self, state.id())?;
            writeln!(f, "{:06?}: {:?}", state.id().as_usize(), state)?;
        }
        writeln!(f, "")?;
        for (i, (start_id, anchored, sty)) in self.st.iter().enumerate() {
            if i % self.st.stride == 0 {
                match anchored {
                    Anchored::No => writeln!(f, "START-GROUP(unanchored)")?,
                    Anchored::Yes => writeln!(f, "START-GROUP(anchored)")?,
                    Anchored::Pattern(pid) => writeln!(
                        f,
                        "START_GROUP(pattern: {:?})",
                        pid.as_usize()
                    )?,
                }
            }
            writeln!(f, "  {:?} => {:06?}", sty, start_id.as_usize())?;
        }
        writeln!(f, "state length: {:?}", self.tt.state_len)?;
        writeln!(f, "pattern length: {:?}", self.pattern_len())?;
        writeln!(f, "flags: {:?}", self.flags)?;
        writeln!(f, ")")?;
        Ok(())
    }
}

// SAFETY: We assert that our implementation of each method is correct.
unsafe impl<T: AsRef<[u8]>> Automaton for DFA<T> {
    #[inline]
    fn is_special_state(&self, id: StateID) -> bool {
        self.special.is_special_state(id)
    }

    #[inline]
    fn is_dead_state(&self, id: StateID) -> bool {
        self.special.is_dead_state(id)
    }

    #[inline]
    fn is_quit_state(&self, id: StateID) -> bool {
        self.special.is_quit_state(id)
    }

    #[inline]
    fn is_match_state(&self, id: StateID) -> bool {
        self.special.is_match_state(id)
    }

    #[inline]
    fn is_start_state(&self, id: StateID) -> bool {
        self.special.is_start_state(id)
    }

    #[inline]
    fn is_accel_state(&self, id: StateID) -> bool {
        self.special.is_accel_state(id)
    }

    // This is marked as inline to help dramatically boost sparse searching,
    // which decodes each state it enters to follow the next transition.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn next_state(&self, current: StateID, input: u8) -> StateID {
        let input = self.tt.classes.get(input);
        self.tt.state(current).next(input)
    }

    #[inline]
    unsafe fn next_state_unchecked(
        &self,
        current: StateID,
        input: u8,
    ) -> StateID {
        self.next_state(current, input)
    }

    #[inline]
    fn next_eoi_state(&self, current: StateID) -> StateID {
        self.tt.state(current).next_eoi()
    }

    #[inline]
    fn pattern_len(&self) -> usize {
        self.tt.pattern_len
    }

    #[inline]
    fn match_len(&self, id: StateID) -> usize {
        self.tt.state(id).pattern_len()
    }

    #[inline]
    fn match_pattern(&self, id: StateID, match_index: usize) -> PatternID {
        // This is an optimization for the very common case of a DFA with a
        // single pattern. This conditional avoids a somewhat more costly path
        // that finds the pattern ID from the state machine, which requires
        // a bit of slicing/pointer-chasing. This optimization tends to only
        // matter when matches are frequent.
        if self.tt.pattern_len == 1 {
            return PatternID::ZERO;
        }
        self.tt.state(id).pattern_id(match_index)
    }

    #[inline]
    fn has_empty(&self) -> bool {
        self.flags.has_empty
    }

    #[inline]
    fn is_utf8(&self) -> bool {
        self.flags.is_utf8
    }

    #[inline]
    fn is_always_start_anchored(&self) -> bool {
        self.flags.is_always_start_anchored
    }

    #[inline]
    fn start_state(
        &self,
        config: &start::Config,
    ) -> Result<StateID, StartError> {
        let anchored = config.get_anchored();
        let start = match config.get_look_behind() {
            None => Start::Text,
            Some(byte) => {
                if !self.quitset.is_empty() && self.quitset.contains(byte) {
                    return Err(StartError::quit(byte));
                }
                self.st.start_map.get(byte)
            }
        };
        self.st.start(anchored, start)
    }

    #[inline]
    fn universal_start_state(&self, mode: Anchored) -> Option<StateID> {
        match mode {
            Anchored::No => self.st.universal_start_unanchored,
            Anchored::Yes => self.st.universal_start_anchored,
            Anchored::Pattern(_) => None,
        }
    }

    #[inline]
    fn accelerator(&self, id: StateID) -> &[u8] {
        self.tt.state(id).accelerator()
    }

    #[inline]
    fn get_prefilter(&self) -> Option<&Prefilter> {
        self.pre.as_ref()
    }
}

/// The transition table portion of a sparse DFA.
///
/// The transition table is the core part of the DFA in that it describes how
/// to move from one state to another based on the input sequence observed.
///
/// Unlike a typical dense table based DFA, states in a sparse transition
/// table have variable size. That is, states with more transitions use more
/// space than states with fewer transitions. This means that finding the next
/// transition takes more work than with a dense DFA, but also typically uses
/// much less space.
#[derive(Clone)]
struct Transitions<T> {
    /// The raw encoding of each state in this DFA.
    ///
    /// Each state has the following information:
    ///
    /// * A set of transitions to subsequent states. Transitions to the dead
    ///   state are omitted.
    /// * If the state can be accelerated, then any additional accelerator
    ///   information.
    /// * If the state is a match state, then the state contains all pattern
    ///   IDs that match when in that state.
    ///
    /// To decode a state, use Transitions::state.
    ///
    /// In practice, T is either Vec<u8> or &[u8].
    sparse: T,
    /// A set of equivalence classes, where a single equivalence class
    /// represents a set of bytes that never discriminate between a match
    /// and a non-match in the DFA. Each equivalence class corresponds to a
    /// single character in this DFA's alphabet, where the maximum number of
    /// characters is 257 (each possible value of a byte plus the special
    /// EOI transition). Consequently, the number of equivalence classes
    /// corresponds to the number of transitions for each DFA state. Note
    /// though that the *space* used by each DFA state in the transition table
    /// may be larger. The total space used by each DFA state is known as the
    /// stride and is documented above.
    ///
    /// The only time the number of equivalence classes is fewer than 257 is
    /// if the DFA's kind uses byte classes which is the default. Equivalence
    /// classes should generally only be disabled when debugging, so that
    /// the transitions themselves aren't obscured. Disabling them has no
    /// other benefit, since the equivalence class map is always used while
    /// searching. In the vast majority of cases, the number of equivalence
    /// classes is substantially smaller than 257, particularly when large
    /// Unicode classes aren't used.
    ///
    /// N.B. Equivalence classes aren't particularly useful in a sparse DFA
    /// in the current implementation, since equivalence classes generally tend
    /// to correspond to continuous ranges of bytes that map to the same
    /// transition. So in a sparse DFA, equivalence classes don't really lead
    /// to a space savings. In the future, it would be good to try and remove
    /// them from sparse DFAs entirely, but requires a bit of work since sparse
    /// DFAs are built from dense DFAs, which are in turn built on top of
    /// equivalence classes.
    classes: ByteClasses,
    /// The total number of states in this DFA. Note that a DFA always has at
    /// least one state---the dead state---even the empty DFA. In particular,
    /// the dead state always has ID 0 and is correspondingly always the first
    /// state. The dead state is never a match state.
    state_len: usize,
    /// The total number of unique patterns represented by these match states.
    pattern_len: usize,
}

impl<'a> Transitions<&'a [u8]> {
    unsafe fn from_bytes_unchecked(
        mut slice: &'a [u8],
    ) -> Result<(Transitions<&'a [u8]>, usize), DeserializeError> {
        let slice_start = slice.as_ptr().as_usize();

        let (state_len, nr) =
            wire::try_read_u32_as_usize(&slice, "state length")?;
        slice = &slice[nr..];

        let (pattern_len, nr) =
            wire::try_read_u32_as_usize(&slice, "pattern length")?;
        slice = &slice[nr..];

        let (classes, nr) = ByteClasses::from_bytes(&slice)?;
        slice = &slice[nr..];

        let (len, nr) =
            wire::try_read_u32_as_usize(&slice, "sparse transitions length")?;
        slice = &slice[nr..];

        wire::check_slice_len(slice, len, "sparse states byte length")?;
        let sparse = &slice[..len];
        slice = &slice[len..];

        let trans = Transitions { sparse, classes, state_len, pattern_len };
        Ok((trans, slice.as_ptr().as_usize() - slice_start))
    }
}

impl<T: AsRef<[u8]>> Transitions<T> {
    /// Writes a serialized form of this transition table to the buffer given.
    /// If the buffer is too small, then an error is returned. To determine
    /// how big the buffer must be, use `write_to_len`.
    fn write_to<E: Endian>(
        &self,
        mut dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small(
                "sparse transition table",
            ));
        }
        dst = &mut dst[..nwrite];

        // write state length
        E::write_u32(u32::try_from(self.state_len).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];

        // write pattern length
        E::write_u32(u32::try_from(self.pattern_len).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];

        // write byte class map
        let n = self.classes.write_to(dst)?;
        dst = &mut dst[n..];

        // write number of bytes in sparse transitions
        E::write_u32(u32::try_from(self.sparse().len()).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];

        // write actual transitions
        let mut id = DEAD;
        while id.as_usize() < self.sparse().len() {
            let state = self.state(id);
            let n = state.write_to::<E>(&mut dst)?;
            dst = &mut dst[n..];
            // The next ID is the offset immediately following `state`.
            id = StateID::new(id.as_usize() + state.write_to_len()).unwrap();
        }
        Ok(nwrite)
    }

    /// Returns the number of bytes the serialized form of this transition
    /// table will use.
    fn write_to_len(&self) -> usize {
        size_of::<u32>()   // state length
        + size_of::<u32>() // pattern length
        + self.classes.write_to_len()
        + size_of::<u32>() // sparse transitions length
        + self.sparse().len()
    }

    /// Validates that every state ID in this transition table is valid.
    ///
    /// That is, every state ID can be used to correctly index a state in this
    /// table.
    fn validate(&self, sp: &Special) -> Result<Seen, DeserializeError> {
        let mut verified = Seen::new();
        // We need to make sure that we decode the correct number of states.
        // Otherwise, an empty set of transitions would validate even if the
        // recorded state length is non-empty.
        let mut len = 0;
        // We can't use the self.states() iterator because it assumes the state
        // encodings are valid. It could panic if they aren't.
        let mut id = DEAD;
        while id.as_usize() < self.sparse().len() {
            // Before we even decode the state, we check that the ID itself
            // is well formed. That is, if it's a special state then it must
            // actually be a quit, dead, accel, match or start state.
            if sp.is_special_state(id) {
                let is_actually_special = sp.is_dead_state(id)
                    || sp.is_quit_state(id)
                    || sp.is_match_state(id)
                    || sp.is_start_state(id)
                    || sp.is_accel_state(id);
                if !is_actually_special {
                    // This is kind of a cryptic error message...
                    return Err(DeserializeError::generic(
                        "found sparse state tagged as special but \
                         wasn't actually special",
                    ));
                }
            }
            let state = self.try_state(sp, id)?;
            verified.insert(id);
            // The next ID should be the offset immediately following `state`.
            id = StateID::new(wire::add(
                id.as_usize(),
                state.write_to_len(),
                "next state ID offset",
            )?)
            .map_err(|err| {
                DeserializeError::state_id_error(err, "next state ID offset")
            })?;
            len += 1;
        }
        // Now that we've checked that all top-level states are correct and
        // importantly, collected a set of valid state IDs, we have all the
        // information we need to check that all transitions are correct too.
        //
        // Note that we can't use `valid_ids` to iterate because it will
        // be empty in no-std no-alloc contexts. (And yes, that means our
        // verification isn't quite as good.) We can use `self.states()`
        // though at least, since we know that all states can at least be
        // decoded and traversed correctly.
        for state in self.states() {
            // Check that all transitions in this state are correct.
            for i in 0..state.ntrans {
                let to = state.next_at(i);
                // For no-alloc, we just check that the state can decode. It is
                // technically possible that the state ID could still point to
                // a non-existent state even if it decodes (fuzzing proved this
                // to be true), but it shouldn't result in any memory unsafety
                // or panics in non-debug mode.
                #[cfg(not(feature = "alloc"))]
                {
                    let _ = self.try_state(sp, to)?;
                }
                #[cfg(feature = "alloc")]
                {
                    if !verified.contains(&to) {
                        return Err(DeserializeError::generic(
                            "found transition that points to a \
                             non-existent state",
                        ));
                    }
                }
            }
        }
        if len != self.state_len {
            return Err(DeserializeError::generic(
                "mismatching sparse state length",
            ));
        }
        Ok(verified)
    }

    /// Converts these transitions to a borrowed value.
    fn as_ref(&self) -> Transitions<&'_ [u8]> {
        Transitions {
            sparse: self.sparse(),
            classes: self.classes.clone(),
            state_len: self.state_len,
            pattern_len: self.pattern_len,
        }
    }

    /// Converts these transitions to an owned value.
    #[cfg(feature = "alloc")]
    fn to_owned(&self) -> Transitions<alloc::vec::Vec<u8>> {
        Transitions {
            sparse: self.sparse().to_vec(),
            classes: self.classes.clone(),
            state_len: self.state_len,
            pattern_len: self.pattern_len,
        }
    }

    /// Return a convenient representation of the given state.
    ///
    /// This panics if the state is invalid.
    ///
    /// This is marked as inline to help dramatically boost sparse searching,
    /// which decodes each state it enters to follow the next transition. Other
    /// functions involved are also inlined, which should hopefully eliminate
    /// a lot of the extraneous decoding that is never needed just to follow
    /// the next transition.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn state(&self, id: StateID) -> State<'_> {
        let mut state = &self.sparse()[id.as_usize()..];
        let mut ntrans = wire::read_u16(&state).as_usize();
        let is_match = (1 << 15) & ntrans != 0;
        ntrans &= !(1 << 15);
        state = &state[2..];

        let (input_ranges, state) = state.split_at(ntrans * 2);
        let (next, state) = state.split_at(ntrans * StateID::SIZE);
        let (pattern_ids, state) = if is_match {
            let npats = wire::read_u32(&state).as_usize();
            state[4..].split_at(npats * 4)
        } else {
            (&[][..], state)
        };

        let accel_len = usize::from(state[0]);
        let accel = &state[1..accel_len + 1];
        State { id, is_match, ntrans, input_ranges, next, pattern_ids, accel }
    }

    /// Like `state`, but will return an error if the state encoding is
    /// invalid. This is useful for verifying states after deserialization,
    /// which is required for a safe deserialization API.
    ///
    /// Note that this only verifies that this state is decodable and that
    /// all of its data is consistent. It does not verify that its state ID
    /// transitions point to valid states themselves, nor does it verify that
    /// every pattern ID is valid.
    fn try_state(
        &self,
        sp: &Special,
        id: StateID,
    ) -> Result<State<'_>, DeserializeError> {
        if id.as_usize() > self.sparse().len() {
            return Err(DeserializeError::generic(
                "invalid caller provided sparse state ID",
            ));
        }
        let mut state = &self.sparse()[id.as_usize()..];
        // Encoding format starts with a u16 that stores the total number of
        // transitions in this state.
        let (mut ntrans, _) =
            wire::try_read_u16_as_usize(state, "state transition length")?;
        let is_match = ((1 << 15) & ntrans) != 0;
        ntrans &= !(1 << 15);
        state = &state[2..];
        if ntrans > 257 || ntrans == 0 {
            return Err(DeserializeError::generic(
                "invalid transition length",
            ));
        }
        if is_match && !sp.is_match_state(id) {
            return Err(DeserializeError::generic(
                "state marked as match but not in match ID range",
            ));
        } else if !is_match && sp.is_match_state(id) {
            return Err(DeserializeError::generic(
                "state in match ID range but not marked as match state",
            ));
        }

        // Each transition has two pieces: an inclusive range of bytes on which
        // it is defined, and the state ID that those bytes transition to. The
        // pairs come first, followed by a corresponding sequence of state IDs.
        let input_ranges_len = ntrans.checked_mul(2).unwrap();
        wire::check_slice_len(state, input_ranges_len, "sparse byte pairs")?;
        let (input_ranges, state) = state.split_at(input_ranges_len);
        // Every range should be of the form A-B, where A<=B.
        for pair in input_ranges.chunks(2) {
            let (start, end) = (pair[0], pair[1]);
            if start > end {
                return Err(DeserializeError::generic("invalid input range"));
            }
        }

        // And now extract the corresponding sequence of state IDs. We leave
        // this sequence as a &[u8] instead of a &[S] because sparse DFAs do
        // not have any alignment requirements.
        let next_len = ntrans
            .checked_mul(self.id_len())
            .expect("state size * #trans should always fit in a usize");
        wire::check_slice_len(state, next_len, "sparse trans state IDs")?;
        let (next, state) = state.split_at(next_len);
        // We can at least verify that every state ID is in bounds.
        for idbytes in next.chunks(self.id_len()) {
            let (id, _) =
                wire::read_state_id(idbytes, "sparse state ID in try_state")?;
            wire::check_slice_len(
                self.sparse(),
                id.as_usize(),
                "invalid sparse state ID",
            )?;
        }

        // If this is a match state, then read the pattern IDs for this state.
        // Pattern IDs is a u32-length prefixed sequence of native endian
        // encoded 32-bit integers.
        let (pattern_ids, state) = if is_match {
            let (npats, nr) =
                wire::try_read_u32_as_usize(state, "pattern ID length")?;
            let state = &state[nr..];
            if npats == 0 {
                return Err(DeserializeError::generic(
                    "state marked as a match, but pattern length is zero",
                ));
            }

            let pattern_ids_len =
                wire::mul(npats, 4, "sparse pattern ID byte length")?;
            wire::check_slice_len(
                state,
                pattern_ids_len,
                "sparse pattern IDs",
            )?;
            let (pattern_ids, state) = state.split_at(pattern_ids_len);
            for patbytes in pattern_ids.chunks(PatternID::SIZE) {
                wire::read_pattern_id(
                    patbytes,
                    "sparse pattern ID in try_state",
                )?;
            }
            (pattern_ids, state)
        } else {
            (&[][..], state)
        };
        if is_match && pattern_ids.is_empty() {
            return Err(DeserializeError::generic(
                "state marked as a match, but has no pattern IDs",
            ));
        }
        if sp.is_match_state(id) && pattern_ids.is_empty() {
            return Err(DeserializeError::generic(
                "state marked special as a match, but has no pattern IDs",
            ));
        }
        if sp.is_match_state(id) != is_match {
            return Err(DeserializeError::generic(
                "whether state is a match or not is inconsistent",
            ));
        }

        // Now read this state's accelerator info. The first byte is the length
        // of the accelerator, which is typically 0 (for no acceleration) but
        // is no bigger than 3. The length indicates the number of bytes that
        // follow, where each byte corresponds to a transition out of this
        // state.
        if state.is_empty() {
            return Err(DeserializeError::generic("no accelerator length"));
        }
        let (accel_len, state) = (usize::from(state[0]), &state[1..]);

        if accel_len > 3 {
            return Err(DeserializeError::generic(
                "sparse invalid accelerator length",
            ));
        } else if accel_len == 0 && sp.is_accel_state(id) {
            return Err(DeserializeError::generic(
                "got no accelerators in state, but in accelerator ID range",
            ));
        } else if accel_len > 0 && !sp.is_accel_state(id) {
            return Err(DeserializeError::generic(
                "state in accelerator ID range, but has no accelerators",
            ));
        }

        wire::check_slice_len(
            state,
            accel_len,
            "sparse corrupt accelerator length",
        )?;
        let (accel, _) = (&state[..accel_len], &state[accel_len..]);

        let state = State {
            id,
            is_match,
            ntrans,
            input_ranges,
            next,
            pattern_ids,
            accel,
        };
        if sp.is_quit_state(state.next_at(state.ntrans - 1)) {
            return Err(DeserializeError::generic(
                "state with EOI transition to quit state is illegal",
            ));
        }
        Ok(state)
    }

    /// Return an iterator over all of the states in this DFA.
    ///
    /// The iterator returned yields tuples, where the first element is the
    /// state ID and the second element is the state itself.
    fn states(&self) -> StateIter<'_, T> {
        StateIter { trans: self, id: DEAD.as_usize() }
    }

    /// Returns the sparse transitions as raw bytes.
    fn sparse(&self) -> &[u8] {
        self.sparse.as_ref()
    }

    /// Returns the number of bytes represented by a single state ID.
    fn id_len(&self) -> usize {
        StateID::SIZE
    }

    /// Return the memory usage, in bytes, of these transitions.
    ///
    /// This does not include the size of a `Transitions` value itself.
    fn memory_usage(&self) -> usize {
        self.sparse().len()
    }
}

#[cfg(feature = "dfa-build")]
impl<T: AsMut<[u8]>> Transitions<T> {
    /// Return a convenient mutable representation of the given state.
    /// This panics if the state is invalid.
    fn state_mut(&mut self, id: StateID) -> StateMut<'_> {
        let mut state = &mut self.sparse_mut()[id.as_usize()..];
        let mut ntrans = wire::read_u16(&state).as_usize();
        let is_match = (1 << 15) & ntrans != 0;
        ntrans &= !(1 << 15);
        state = &mut state[2..];

        let (input_ranges, state) = state.split_at_mut(ntrans * 2);
        let (next, state) = state.split_at_mut(ntrans * StateID::SIZE);
        let (pattern_ids, state) = if is_match {
            let npats = wire::read_u32(&state).as_usize();
            state[4..].split_at_mut(npats * 4)
        } else {
            (&mut [][..], state)
        };

        let accel_len = usize::from(state[0]);
        let accel = &mut state[1..accel_len + 1];
        StateMut {
            id,
            is_match,
            ntrans,
            input_ranges,
            next,
            pattern_ids,
            accel,
        }
    }

    /// Returns the sparse transitions as raw mutable bytes.
    fn sparse_mut(&mut self) -> &mut [u8] {
        self.sparse.as_mut()
    }
}

/// The set of all possible starting states in a DFA.
///
/// See the eponymous type in the `dense` module for more details. This type
/// is very similar to `dense::StartTable`, except that its underlying
/// representation is `&[u8]` instead of `&[S]`. (The latter would require
/// sparse DFAs to be aligned, which is explicitly something we do not require
/// because we don't really need it.)
#[derive(Clone)]
struct StartTable<T> {
    /// The initial start state IDs as a contiguous table of native endian
    /// encoded integers, represented by `S`.
    ///
    /// In practice, T is either Vec<u8> or &[u8] and has no alignment
    /// requirements.
    ///
    /// The first `2 * stride` (currently always 8) entries always correspond
    /// to the starts states for the entire DFA, with the first 4 entries being
    /// for unanchored searches and the second 4 entries being for anchored
    /// searches. To keep things simple, we always use 8 entries even if the
    /// `StartKind` is not both.
    ///
    /// After that, there are `stride * patterns` state IDs, where `patterns`
    /// may be zero in the case of a DFA with no patterns or in the case where
    /// the DFA was built without enabling starting states for each pattern.
    table: T,
    /// The starting state configuration supported. When 'both', both
    /// unanchored and anchored searches work. When 'unanchored', anchored
    /// searches panic. When 'anchored', unanchored searches panic.
    kind: StartKind,
    /// The start state configuration for every possible byte.
    start_map: StartByteMap,
    /// The number of starting state IDs per pattern.
    stride: usize,
    /// The total number of patterns for which starting states are encoded.
    /// This is `None` for DFAs that were built without start states for each
    /// pattern. Thus, one cannot use this field to say how many patterns
    /// are in the DFA in all cases. It is specific to how many patterns are
    /// represented in this start table.
    pattern_len: Option<usize>,
    /// The universal starting state for unanchored searches. This is only
    /// present when the DFA supports unanchored searches and when all starting
    /// state IDs for an unanchored search are equivalent.
    universal_start_unanchored: Option<StateID>,
    /// The universal starting state for anchored searches. This is only
    /// present when the DFA supports anchored searches and when all starting
    /// state IDs for an anchored search are equivalent.
    universal_start_anchored: Option<StateID>,
}

#[cfg(feature = "dfa-build")]
impl StartTable<Vec<u8>> {
    fn new<T: AsRef<[u32]>>(
        dfa: &dense::DFA<T>,
        pattern_len: Option<usize>,
    ) -> StartTable<Vec<u8>> {
        let stride = Start::len();
        // This is OK since the only way we're here is if a dense DFA could be
        // constructed successfully, which uses the same space.
        let len = stride
            .checked_mul(pattern_len.unwrap_or(0))
            .unwrap()
            .checked_add(stride.checked_mul(2).unwrap())
            .unwrap()
            .checked_mul(StateID::SIZE)
            .unwrap();
        StartTable {
            table: vec![0; len],
            kind: dfa.start_kind(),
            start_map: dfa.start_map().clone(),
            stride,
            pattern_len,
            universal_start_unanchored: dfa
                .universal_start_state(Anchored::No),
            universal_start_anchored: dfa.universal_start_state(Anchored::Yes),
        }
    }

    fn from_dense_dfa<T: AsRef<[u32]>>(
        dfa: &dense::DFA<T>,
        remap: &[StateID],
    ) -> Result<StartTable<Vec<u8>>, BuildError> {
        // Unless the DFA has start states compiled for each pattern, then
        // as far as the starting state table is concerned, there are zero
        // patterns to account for. It will instead only store starting states
        // for the entire DFA.
        let start_pattern_len = if dfa.starts_for_each_pattern() {
            Some(dfa.pattern_len())
        } else {
            None
        };
        let mut sl = StartTable::new(dfa, start_pattern_len);
        for (old_start_id, anchored, sty) in dfa.starts() {
            let new_start_id = remap[dfa.to_index(old_start_id)];
            sl.set_start(anchored, sty, new_start_id);
        }
        Ok(sl)
    }
}

impl<'a> StartTable<&'a [u8]> {
    unsafe fn from_bytes_unchecked(
        mut slice: &'a [u8],
    ) -> Result<(StartTable<&'a [u8]>, usize), DeserializeError> {
        let slice_start = slice.as_ptr().as_usize();

        let (kind, nr) = StartKind::from_bytes(slice)?;
        slice = &slice[nr..];

        let (start_map, nr) = StartByteMap::from_bytes(slice)?;
        slice = &slice[nr..];

        let (stride, nr) =
            wire::try_read_u32_as_usize(slice, "sparse start table stride")?;
        slice = &slice[nr..];
        if stride != Start::len() {
            return Err(DeserializeError::generic(
                "invalid sparse starting table stride",
            ));
        }

        let (maybe_pattern_len, nr) =
            wire::try_read_u32_as_usize(slice, "sparse start table patterns")?;
        slice = &slice[nr..];
        let pattern_len = if maybe_pattern_len.as_u32() == u32::MAX {
            None
        } else {
            Some(maybe_pattern_len)
        };
        if pattern_len.map_or(false, |len| len > PatternID::LIMIT) {
            return Err(DeserializeError::generic(
                "sparse invalid number of patterns",
            ));
        }

        let (universal_unanchored, nr) =
            wire::try_read_u32(slice, "universal unanchored start")?;
        slice = &slice[nr..];
        let universal_start_unanchored = if universal_unanchored == u32::MAX {
            None
        } else {
            Some(StateID::try_from(universal_unanchored).map_err(|e| {
                DeserializeError::state_id_error(
                    e,
                    "universal unanchored start",
                )
            })?)
        };

        let (universal_anchored, nr) =
            wire::try_read_u32(slice, "universal anchored start")?;
        slice = &slice[nr..];
        let universal_start_anchored = if universal_anchored == u32::MAX {
            None
        } else {
            Some(StateID::try_from(universal_anchored).map_err(|e| {
                DeserializeError::state_id_error(e, "universal anchored start")
            })?)
        };

        let pattern_table_size = wire::mul(
            stride,
            pattern_len.unwrap_or(0),
            "sparse invalid pattern length",
        )?;
        // Our start states always start with a single stride of start states
        // for the entire automaton which permit it to match any pattern. What
        // follows it are an optional set of start states for each pattern.
        let start_state_len = wire::add(
            wire::mul(2, stride, "start state stride too big")?,
            pattern_table_size,
            "sparse invalid 'any' pattern starts size",
        )?;
        let table_bytes_len = wire::mul(
            start_state_len,
            StateID::SIZE,
            "sparse pattern table bytes length",
        )?;
        wire::check_slice_len(
            slice,
            table_bytes_len,
            "sparse start ID table",
        )?;
        let table = &slice[..table_bytes_len];
        slice = &slice[table_bytes_len..];

        let sl = StartTable {
            table,
            kind,
            start_map,
            stride,
            pattern_len,
            universal_start_unanchored,
            universal_start_anchored,
        };
        Ok((sl, slice.as_ptr().as_usize() - slice_start))
    }
}

impl<T: AsRef<[u8]>> StartTable<T> {
    fn write_to<E: Endian>(
        &self,
        mut dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small(
                "sparse starting table ids",
            ));
        }
        dst = &mut dst[..nwrite];

        // write start kind
        let nw = self.kind.write_to::<E>(dst)?;
        dst = &mut dst[nw..];
        // write start byte map
        let nw = self.start_map.write_to(dst)?;
        dst = &mut dst[nw..];
        // write stride
        E::write_u32(u32::try_from(self.stride).unwrap(), dst);
        dst = &mut dst[size_of::<u32>()..];
        // write pattern length
        E::write_u32(
            u32::try_from(self.pattern_len.unwrap_or(0xFFFF_FFFF)).unwrap(),
            dst,
        );
        dst = &mut dst[size_of::<u32>()..];
        // write universal start unanchored state id, u32::MAX if absent
        E::write_u32(
            self.universal_start_unanchored
                .map_or(u32::MAX, |sid| sid.as_u32()),
            dst,
        );
        dst = &mut dst[size_of::<u32>()..];
        // write universal start anchored state id, u32::MAX if absent
        E::write_u32(
            self.universal_start_anchored.map_or(u32::MAX, |sid| sid.as_u32()),
            dst,
        );
        dst = &mut dst[size_of::<u32>()..];
        // write start IDs
        for (sid, _, _) in self.iter() {
            E::write_u32(sid.as_u32(), dst);
            dst = &mut dst[StateID::SIZE..];
        }
        Ok(nwrite)
    }

    /// Returns the number of bytes the serialized form of this transition
    /// table will use.
    fn write_to_len(&self) -> usize {
        self.kind.write_to_len()
        + self.start_map.write_to_len()
        + size_of::<u32>() // stride
        + size_of::<u32>() // # patterns
        + size_of::<u32>() // universal unanchored start
        + size_of::<u32>() // universal anchored start
        + self.table().len()
    }

    /// Validates that every starting state ID in this table is valid.
    ///
    /// That is, every starting state ID can be used to correctly decode a
    /// state in the DFA's sparse transitions.
    fn validate(
        &self,
        sp: &Special,
        seen: &Seen,
    ) -> Result<(), DeserializeError> {
        for (id, _, _) in self.iter() {
            if !seen.contains(&id) {
                return Err(DeserializeError::generic(
                    "found invalid start state ID",
                ));
            }
            if sp.is_match_state(id) {
                return Err(DeserializeError::generic(
                    "start states cannot be match states",
                ));
            }
        }
        Ok(())
    }

    /// Converts this start list to a borrowed value.
    fn as_ref(&self) -> StartTable<&'_ [u8]> {
        StartTable {
            table: self.table(),
            kind: self.kind,
            start_map: self.start_map.clone(),
            stride: self.stride,
            pattern_len: self.pattern_len,
            universal_start_unanchored: self.universal_start_unanchored,
            universal_start_anchored: self.universal_start_anchored,
        }
    }

    /// Converts this start list to an owned value.
    #[cfg(feature = "alloc")]
    fn to_owned(&self) -> StartTable<alloc::vec::Vec<u8>> {
        StartTable {
            table: self.table().to_vec(),
            kind: self.kind,
            start_map: self.start_map.clone(),
            stride: self.stride,
            pattern_len: self.pattern_len,
            universal_start_unanchored: self.universal_start_unanchored,
            universal_start_anchored: self.universal_start_anchored,
        }
    }

    /// Return the start state for the given index and pattern ID. If the
    /// pattern ID is None, then the corresponding start state for the entire
    /// DFA is returned. If the pattern ID is not None, then the corresponding
    /// starting state for the given pattern is returned. If this start table
    /// does not have individual starting states for each pattern, then this
    /// panics.
    fn start(
        &self,
        anchored: Anchored,
        start: Start,
    ) -> Result<StateID, StartError> {
        let start_index = start.as_usize();
        let index = match anchored {
            Anchored::No => {
                if !self.kind.has_unanchored() {
                    return Err(StartError::unsupported_anchored(anchored));
                }
                start_index
            }
            Anchored::Yes => {
                if !self.kind.has_anchored() {
                    return Err(StartError::unsupported_anchored(anchored));
                }
                self.stride + start_index
            }
            Anchored::Pattern(pid) => {
                let len = match self.pattern_len {
                    None => {
                        return Err(StartError::unsupported_anchored(anchored))
                    }
                    Some(len) => len,
                };
                if pid.as_usize() >= len {
                    return Ok(DEAD);
                }
                (2 * self.stride)
                    + (self.stride * pid.as_usize())
                    + start_index
            }
        };
        let start = index * StateID::SIZE;
        // This OK since we're allowed to assume that the start table contains
        // valid StateIDs.
        Ok(wire::read_state_id_unchecked(&self.table()[start..]).0)
    }

    /// Return an iterator over all start IDs in this table.
    fn iter(&self) -> StartStateIter<'_, T> {
        StartStateIter { st: self, i: 0 }
    }

    /// Returns the total number of start state IDs in this table.
    fn len(&self) -> usize {
        self.table().len() / StateID::SIZE
    }

    /// Returns the table as a raw slice of bytes.
    fn table(&self) -> &[u8] {
        self.table.as_ref()
    }

    /// Return the memory usage, in bytes, of this start list.
    ///
    /// This does not include the size of a `StartTable` value itself.
    fn memory_usage(&self) -> usize {
        self.table().len()
    }
}

#[cfg(feature = "dfa-build")]
impl<T: AsMut<[u8]>> StartTable<T> {
    /// Set the start state for the given index and pattern.
    ///
    /// If the pattern ID or state ID are not valid, then this will panic.
    fn set_start(&mut self, anchored: Anchored, start: Start, id: StateID) {
        let start_index = start.as_usize();
        let index = match anchored {
            Anchored::No => start_index,
            Anchored::Yes => self.stride + start_index,
            Anchored::Pattern(pid) => {
                let pid = pid.as_usize();
                let len = self
                    .pattern_len
                    .expect("start states for each pattern enabled");
                assert!(pid < len, "invalid pattern ID {:?}", pid);
                self.stride
                    .checked_mul(pid)
                    .unwrap()
                    .checked_add(self.stride.checked_mul(2).unwrap())
                    .unwrap()
                    .checked_add(start_index)
                    .unwrap()
            }
        };
        let start = index * StateID::SIZE;
        let end = start + StateID::SIZE;
        wire::write_state_id::<wire::NE>(
            id,
            &mut self.table.as_mut()[start..end],
        );
    }
}

/// An iterator over all state state IDs in a sparse DFA.
struct StartStateIter<'a, T> {
    st: &'a StartTable<T>,
    i: usize,
}

impl<'a, T: AsRef<[u8]>> Iterator for StartStateIter<'a, T> {
    type Item = (StateID, Anchored, Start);

    fn next(&mut self) -> Option<(StateID, Anchored, Start)> {
        let i = self.i;
        if i >= self.st.len() {
            return None;
        }
        self.i += 1;

        // This unwrap is okay since the stride of any DFA must always match
        // the number of start state types.
        let start_type = Start::from_usize(i % self.st.stride).unwrap();
        let anchored = if i < self.st.stride {
            Anchored::No
        } else if i < (2 * self.st.stride) {
            Anchored::Yes
        } else {
            let pid = (i - (2 * self.st.stride)) / self.st.stride;
            Anchored::Pattern(PatternID::new(pid).unwrap())
        };
        let start = i * StateID::SIZE;
        let end = start + StateID::SIZE;
        let bytes = self.st.table()[start..end].try_into().unwrap();
        // This is OK since we're allowed to assume that any IDs in this start
        // table are correct and valid for this DFA.
        let id = StateID::from_ne_bytes_unchecked(bytes);
        Some((id, anchored, start_type))
    }
}

impl<'a, T> fmt::Debug for StartStateIter<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("StartStateIter").field("i", &self.i).finish()
    }
}

/// An iterator over all states in a sparse DFA.
///
/// This iterator yields tuples, where the first element is the state ID and
/// the second element is the state itself.
struct StateIter<'a, T> {
    trans: &'a Transitions<T>,
    id: usize,
}

impl<'a, T: AsRef<[u8]>> Iterator for StateIter<'a, T> {
    type Item = State<'a>;

    fn next(&mut self) -> Option<State<'a>> {
        if self.id >= self.trans.sparse().len() {
            return None;
        }
        let state = self.trans.state(StateID::new_unchecked(self.id));
        self.id = self.id + state.write_to_len();
        Some(state)
    }
}

impl<'a, T> fmt::Debug for StateIter<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("StateIter").field("id", &self.id).finish()
    }
}

/// A representation of a sparse DFA state that can be cheaply materialized
/// from a state identifier.
#[derive(Clone)]
struct State<'a> {
    /// The identifier of this state.
    id: StateID,
    /// Whether this is a match state or not.
    is_match: bool,
    /// The number of transitions in this state.
    ntrans: usize,
    /// Pairs of input ranges, where there is one pair for each transition.
    /// Each pair specifies an inclusive start and end byte range for the
    /// corresponding transition.
    input_ranges: &'a [u8],
    /// Transitions to the next state. This slice contains native endian
    /// encoded state identifiers, with `S` as the representation. Thus, there
    /// are `ntrans * size_of::<S>()` bytes in this slice.
    next: &'a [u8],
    /// If this is a match state, then this contains the pattern IDs that match
    /// when the DFA is in this state.
    ///
    /// This is a contiguous sequence of 32-bit native endian encoded integers.
    pattern_ids: &'a [u8],
    /// An accelerator for this state, if present. If this state has no
    /// accelerator, then this is an empty slice. When non-empty, this slice
    /// has length at most 3 and corresponds to the exhaustive set of bytes
    /// that must be seen in order to transition out of this state.
    accel: &'a [u8],
}

impl<'a> State<'a> {
    /// Searches for the next transition given an input byte. If no such
    /// transition could be found, then a dead state is returned.
    ///
    /// This is marked as inline to help dramatically boost sparse searching,
    /// which decodes each state it enters to follow the next transition.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn next(&self, input: u8) -> StateID {
        // This straight linear search was observed to be much better than
        // binary search on ASCII haystacks, likely because a binary search
        // visits the ASCII case last but a linear search sees it first. A
        // binary search does do a little better on non-ASCII haystacks, but
        // not by much. There might be a better trade off lurking here.
        for i in 0..(self.ntrans - 1) {
            let (start, end) = self.range(i);
            if start <= input && input <= end {
                return self.next_at(i);
            }
            // We could bail early with an extra branch: if input < b1, then
            // we know we'll never find a matching transition. Interestingly,
            // this extra branch seems to not help performance, or will even
            // hurt it. It's likely very dependent on the DFA itself and what
            // is being searched.
        }
        DEAD
    }

    /// Returns the next state ID for the special EOI transition.
    fn next_eoi(&self) -> StateID {
        self.next_at(self.ntrans - 1)
    }

    /// Returns the identifier for this state.
    fn id(&self) -> StateID {
        self.id
    }

    /// Returns the inclusive input byte range for the ith transition in this
    /// state.
    fn range(&self, i: usize) -> (u8, u8) {
        (self.input_ranges[i * 2], self.input_ranges[i * 2 + 1])
    }

    /// Returns the next state for the ith transition in this state.
    fn next_at(&self, i: usize) -> StateID {
        let start = i * StateID::SIZE;
        let end = start + StateID::SIZE;
        let bytes = self.next[start..end].try_into().unwrap();
        StateID::from_ne_bytes_unchecked(bytes)
    }

    /// Returns the pattern ID for the given match index. If the match index
    /// is invalid, then this panics.
    fn pattern_id(&self, match_index: usize) -> PatternID {
        let start = match_index * PatternID::SIZE;
        wire::read_pattern_id_unchecked(&self.pattern_ids[start..]).0
    }

    /// Returns the total number of pattern IDs for this state. This is always
    /// zero when `is_match` is false.
    fn pattern_len(&self) -> usize {
        assert_eq!(0, self.pattern_ids.len() % 4);
        self.pattern_ids.len() / 4
    }

    /// Return an accelerator for this state.
    fn accelerator(&self) -> &'a [u8] {
        self.accel
    }

    /// Write the raw representation of this state to the given buffer using
    /// the given endianness.
    fn write_to<E: Endian>(
        &self,
        mut dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small(
                "sparse state transitions",
            ));
        }

        let ntrans =
            if self.is_match { self.ntrans | (1 << 15) } else { self.ntrans };
        E::write_u16(u16::try_from(ntrans).unwrap(), dst);
        dst = &mut dst[size_of::<u16>()..];

        dst[..self.input_ranges.len()].copy_from_slice(self.input_ranges);
        dst = &mut dst[self.input_ranges.len()..];

        for i in 0..self.ntrans {
            E::write_u32(self.next_at(i).as_u32(), dst);
            dst = &mut dst[StateID::SIZE..];
        }

        if self.is_match {
            E::write_u32(u32::try_from(self.pattern_len()).unwrap(), dst);
            dst = &mut dst[size_of::<u32>()..];
            for i in 0..self.pattern_len() {
                let pid = self.pattern_id(i);
                E::write_u32(pid.as_u32(), dst);
                dst = &mut dst[PatternID::SIZE..];
            }
        }

        dst[0] = u8::try_from(self.accel.len()).unwrap();
        dst[1..][..self.accel.len()].copy_from_slice(self.accel);

        Ok(nwrite)
    }

    /// Return the total number of bytes that this state consumes in its
    /// encoded form.
    fn write_to_len(&self) -> usize {
        let mut len = 2
            + (self.ntrans * 2)
            + (self.ntrans * StateID::SIZE)
            + (1 + self.accel.len());
        if self.is_match {
            len += size_of::<u32>() + self.pattern_ids.len();
        }
        len
    }
}

impl<'a> fmt::Debug for State<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut printed = false;
        for i in 0..(self.ntrans - 1) {
            let next = self.next_at(i);
            if next == DEAD {
                continue;
            }

            if printed {
                write!(f, ", ")?;
            }
            let (start, end) = self.range(i);
            if start == end {
                write!(f, "{:?} => {:?}", DebugByte(start), next.as_usize())?;
            } else {
                write!(
                    f,
                    "{:?}-{:?} => {:?}",
                    DebugByte(start),
                    DebugByte(end),
                    next.as_usize(),
                )?;
            }
            printed = true;
        }
        let eoi = self.next_at(self.ntrans - 1);
        if eoi != DEAD {
            if printed {
                write!(f, ", ")?;
            }
            write!(f, "EOI => {:?}", eoi.as_usize())?;
        }
        Ok(())
    }
}

/// A representation of a mutable sparse DFA state that can be cheaply
/// materialized from a state identifier.
#[cfg(feature = "dfa-build")]
struct StateMut<'a> {
    /// The identifier of this state.
    id: StateID,
    /// Whether this is a match state or not.
    is_match: bool,
    /// The number of transitions in this state.
    ntrans: usize,
    /// Pairs of input ranges, where there is one pair for each transition.
    /// Each pair specifies an inclusive start and end byte range for the
    /// corresponding transition.
    input_ranges: &'a mut [u8],
    /// Transitions to the next state. This slice contains native endian
    /// encoded state identifiers, with `S` as the representation. Thus, there
    /// are `ntrans * size_of::<S>()` bytes in this slice.
    next: &'a mut [u8],
    /// If this is a match state, then this contains the pattern IDs that match
    /// when the DFA is in this state.
    ///
    /// This is a contiguous sequence of 32-bit native endian encoded integers.
    pattern_ids: &'a [u8],
    /// An accelerator for this state, if present. If this state has no
    /// accelerator, then this is an empty slice. When non-empty, this slice
    /// has length at most 3 and corresponds to the exhaustive set of bytes
    /// that must be seen in order to transition out of this state.
    accel: &'a mut [u8],
}

#[cfg(feature = "dfa-build")]
impl<'a> StateMut<'a> {
    /// Sets the ith transition to the given state.
    fn set_next_at(&mut self, i: usize, next: StateID) {
        let start = i * StateID::SIZE;
        let end = start + StateID::SIZE;
        wire::write_state_id::<wire::NE>(next, &mut self.next[start..end]);
    }
}

#[cfg(feature = "dfa-build")]
impl<'a> fmt::Debug for StateMut<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = State {
            id: self.id,
            is_match: self.is_match,
            ntrans: self.ntrans,
            input_ranges: self.input_ranges,
            next: self.next,
            pattern_ids: self.pattern_ids,
            accel: self.accel,
        };
        fmt::Debug::fmt(&state, f)
    }
}

// In order to validate everything, we not only need to make sure we
// can decode every state, but that every transition in every state
// points to a valid state. There are many duplicative transitions, so
// we record state IDs that we've verified so that we don't redo the
// decoding work.
//
// Except, when in no_std mode, we don't have dynamic memory allocation
// available to us, so we skip this optimization. It's not clear
// whether doing something more clever is worth it just yet. If you're
// profiling this code and need it to run faster, please file an issue.
//
// OK, so we also use this to record the set of valid state IDs. Since
// it is possible for a transition to point to an invalid state ID that
// still (somehow) deserializes to a valid state. So we need to make
// sure our transitions are limited to actually correct state IDs.
// The problem is, I'm not sure how to do this verification step in
// no-std no-alloc mode. I think we'd *have* to store the set of valid
// state IDs in the DFA itself. For now, we don't do this verification
// in no-std no-alloc mode. The worst thing that can happen is an
// incorrect result. But no panics or memory safety problems should
// result. Because we still do validate that the state itself is
// "valid" in the sense that everything it points to actually exists.
//
// ---AG
#[derive(Debug)]
struct Seen {
    #[cfg(feature = "alloc")]
    set: alloc::collections::BTreeSet<StateID>,
    #[cfg(not(feature = "alloc"))]
    set: core::marker::PhantomData<StateID>,
}

#[cfg(feature = "alloc")]
impl Seen {
    fn new() -> Seen {
        Seen { set: alloc::collections::BTreeSet::new() }
    }
    fn insert(&mut self, id: StateID) {
        self.set.insert(id);
    }
    fn contains(&self, id: &StateID) -> bool {
        self.set.contains(id)
    }
}

#[cfg(not(feature = "alloc"))]
impl Seen {
    fn new() -> Seen {
        Seen { set: core::marker::PhantomData }
    }
    fn insert(&mut self, _id: StateID) {}
    fn contains(&self, _id: &StateID) -> bool {
        true
    }
}

/*
/// A binary search routine specialized specifically to a sparse DFA state's
/// transitions. Specifically, the transitions are defined as a set of pairs
/// of input bytes that delineate an inclusive range of bytes. If the input
/// byte is in the range, then the corresponding transition is a match.
///
/// This binary search accepts a slice of these pairs and returns the position
/// of the matching pair (the ith transition), or None if no matching pair
/// could be found.
///
/// Note that this routine is not currently used since it was observed to
/// either decrease performance when searching ASCII, or did not provide enough
/// of a boost on non-ASCII haystacks to be worth it. However, we leave it here
/// for posterity in case we can find a way to use it.
///
/// In theory, we could use the standard library's search routine if we could
/// cast a `&[u8]` to a `&[(u8, u8)]`, but I don't believe this is currently
/// guaranteed to be safe and is thus UB (since I don't think the in-memory
/// representation of `(u8, u8)` has been nailed down). One could define a
/// repr(C) type, but the casting doesn't seem justified.
#[cfg_attr(feature = "perf-inline", inline(always))]
fn binary_search_ranges(ranges: &[u8], needle: u8) -> Option<usize> {
    debug_assert!(ranges.len() % 2 == 0, "ranges must have even length");
    debug_assert!(ranges.len() <= 512, "ranges should be short");

    let (mut left, mut right) = (0, ranges.len() / 2);
    while left < right {
        let mid = (left + right) / 2;
        let (b1, b2) = (ranges[mid * 2], ranges[mid * 2 + 1]);
        if needle < b1 {
            right = mid;
        } else if needle > b2 {
            left = mid + 1;
        } else {
            return Some(mid);
        }
    }
    None
}
*/

#[cfg(all(test, feature = "syntax", feature = "dfa-build"))]
mod tests {
    use crate::{
        dfa::{dense::DFA, Automaton},
        nfa::thompson,
        Input, MatchError,
    };

    // See the analogous test in src/hybrid/dfa.rs and src/dfa/dense.rs.
    #[test]
    fn heuristic_unicode_forward() {
        let dfa = DFA::builder()
            .configure(DFA::config().unicode_word_boundary(true))
            .thompson(thompson::Config::new().reverse(true))
            .build(r"\b[0-9]+\b")
            .unwrap()
            .to_sparse()
            .unwrap();

        let input = Input::new("β123").range(2..);
        let expected = MatchError::quit(0xB2, 1);
        let got = dfa.try_search_fwd(&input);
        assert_eq!(Err(expected), got);

        let input = Input::new("123β").range(..3);
        let expected = MatchError::quit(0xCE, 3);
        let got = dfa.try_search_fwd(&input);
        assert_eq!(Err(expected), got);
    }

    // See the analogous test in src/hybrid/dfa.rs and src/dfa/dense.rs.
    #[test]
    fn heuristic_unicode_reverse() {
        let dfa = DFA::builder()
            .configure(DFA::config().unicode_word_boundary(true))
            .thompson(thompson::Config::new().reverse(true))
            .build(r"\b[0-9]+\b")
            .unwrap()
            .to_sparse()
            .unwrap();

        let input = Input::new("β123").range(2..);
        let expected = MatchError::quit(0xB2, 1);
        let got = dfa.try_search_rev(&input);
        assert_eq!(Err(expected), got);

        let input = Input::new("123β").range(..3);
        let expected = MatchError::quit(0xCE, 3);
        let got = dfa.try_search_rev(&input);
        assert_eq!(Err(expected), got);
    }
}
