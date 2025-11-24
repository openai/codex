use crate::{
    dfa::DEAD,
    util::{
        primitives::StateID,
        wire::{self, DeserializeError, Endian, SerializeError},
    },
};

macro_rules! err {
    ($msg:expr) => {
        return Err(DeserializeError::generic($msg));
    };
}

// Special represents the identifiers in a DFA that correspond to "special"
// states. If a state is one or more of the following, then it is considered
// special:
//
// * dead - A non-matching state where all outgoing transitions lead back to
//   itself. There is only one of these, regardless of whether minimization
//   has run. The dead state always has an ID of 0. i.e., It is always the
//   first state in a DFA.
// * quit - A state that is entered whenever a byte is seen that should cause
//   a DFA to give up and stop searching. This results in a MatchError::quit
//   error being returned at search time. The default configuration for a DFA
//   has no quit bytes, which means this state is unreachable by default,
//   although it is always present for reasons of implementation simplicity.
//   This state is only reachable when the caller configures the DFA to quit
//   on certain bytes. There is always exactly one of these states and it
//   is always the second state. (Its actual ID depends on the size of the
//   alphabet in dense DFAs, since state IDs are premultiplied in order to
//   allow them to be used directly as indices into the transition table.)
// * match - An accepting state, i.e., indicative of a match. There may be
//   zero or more of these states.
// * accelerated - A state where all of its outgoing transitions, except a
//   few, loop back to itself. These states are candidates for acceleration
//   via memchr during search. There may be zero or more of these states.
// * start - A non-matching state that indicates where the automaton should
//   start during a search. There is always at least one starting state and
//   all are guaranteed to be non-match states. (A start state cannot be a
//   match state because the DFAs in this crate delay all matches by one byte.
//   So every search that finds a match must move through one transition to
//   some other match state, even when searching an empty string.)
//
// These are not mutually exclusive categories. Namely, the following
// overlappings can occur:
//
// * {dead, start} - If a DFA can never lead to a match and it is minimized,
//   then it will typically compile to something where all starting IDs point
//   to the DFA's dead state.
// * {match, accelerated} - It is possible for a match state to have the
//   majority of its transitions loop back to itself, which means it's
//   possible for a match state to be accelerated.
// * {start, accelerated} - Similarly, it is possible for a start state to be
//   accelerated. Note that it is possible for an accelerated state to be
//   neither a match or a start state. Also note that just because both match
//   and start states overlap with accelerated states does not mean that
//   match and start states overlap with each other. In fact, they are
//   guaranteed not to overlap.
//
// As a special mention, every DFA always has a dead and a quit state, even
// though from the perspective of the DFA, they are equivalent. (Indeed,
// minimization special cases them to ensure they don't get merged.) The
// purpose of keeping them distinct is to use the quit state as a sentinel to
// distguish between whether a search finished successfully without finding
// anything or whether it gave up before finishing.
//
// So the main problem we want to solve here is the *fast* detection of whether
// a state is special or not. And we also want to do this while storing as
// little extra data as possible. AND we want to be able to quickly determine
// which categories a state falls into above if it is special.
//
// We achieve this by essentially shuffling all special states to the beginning
// of a DFA. That is, all special states appear before every other non-special
// state. By representing special states this way, we can determine whether a
// state is special or not by a single comparison, where special.max is the
// identifier of the last special state in the DFA:
//
//     if current_state <= special.max:
//         ... do something with special state
//
// The only thing left to do is to determine what kind of special state
// it is. Because what we do next depends on that. Since special states
// are typically rare, we can afford to do a bit more extra work, but we'd
// still like this to be as fast as possible. The trick we employ here is to
// continue shuffling states even within the special state range. Such that
// one contiguous region corresponds to match states, another for start states
// and then an overlapping range for accelerated states. At a high level, our
// special state detection might look like this (for leftmost searching, where
// we continue searching even after seeing a match):
//
//     byte = input[offset]
//     current_state = next_state(current_state, byte)
//     offset += 1
//     if current_state <= special.max:
//         if current_state == 0:
//             # We can never leave a dead state, so this always marks the
//             # end of our search.
//             return last_match
//         if current_state == special.quit_id:
//             # A quit state means we give up. If he DFA has no quit state,
//             # then special.quit_id == 0 == dead, which is handled by the
//             # conditional above.
//             return Err(MatchError::quit { byte, offset: offset - 1 })
//         if special.min_match <= current_state <= special.max_match:
//             last_match = Some(offset)
//             if special.min_accel <= current_state <= special.max_accel:
//                 offset = accelerate(input, offset)
//                 last_match = Some(offset)
//         elif special.min_start <= current_state <= special.max_start:
//             offset = prefilter.find(input, offset)
//             if special.min_accel <= current_state <= special.max_accel:
//                 offset = accelerate(input, offset)
//         elif special.min_accel <= current_state <= special.max_accel:
//             offset = accelerate(input, offset)
//
// There are some small details left out of the logic above. For example,
// in order to accelerate a state, we need to know which bytes to search for.
// This in turn implies some extra data we need to store in the DFA. To keep
// things compact, we would ideally only store
//
//     N = special.max_accel - special.min_accel + 1
//
// items. But state IDs are premultiplied, which means they are not contiguous.
// So in order to take a state ID and index an array of accelerated structures,
// we need to do:
//
//     i = (state_id - special.min_accel) / stride
//
// (N.B. 'stride' is always a power of 2, so the above can be implemented via
// '(state_id - special.min_accel) >> stride2', where 'stride2' is x in
// 2^x=stride.)
//
// Moreover, some of these specialty categories may be empty. For example,
// DFAs are not required to have any match states or any accelerated states.
// In that case, the lower and upper bounds are both set to 0 (the dead state
// ID) and the first `current_state == 0` check subsumes cases where the
// ranges are empty.
//
// Loop unrolling, if applicable, has also been left out of the logic above.
//
// Graphically, the ranges look like this, where asterisks indicate ranges
// that can be empty. Each 'x' is a state.
//
//      quit
//  dead|
//     ||
//     xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
//     | |             |    | start |                       |
//     | |-------------|    |-------|                       |
//     |   match*   |          |    |                       |
//     |            |          |    |                       |
//     |            |----------|    |                       |
//     |                accel*      |                       |
//     |                            |                       |
//     |                            |                       |
//     |----------------------------|------------------------
//              special                   non-special*
#[derive(Clone, Copy, Debug)]
pub(crate) struct Special {
    /// The identifier of the last special state in a DFA. A state is special
    /// if and only if its identifier is less than or equal to `max`.
    pub(crate) max: StateID,
    /// The identifier of the quit state in a DFA. (There is no analogous field
    /// for the dead state since the dead state's ID is always zero, regardless
    /// of state ID size.)
    pub(crate) quit_id: StateID,
    /// The identifier of the first match state.
    pub(crate) min_match: StateID,
    /// The identifier of the last match state.
    pub(crate) max_match: StateID,
    /// The identifier of the first accelerated state.
    pub(crate) min_accel: StateID,
    /// The identifier of the last accelerated state.
    pub(crate) max_accel: StateID,
    /// The identifier of the first start state.
    pub(crate) min_start: StateID,
    /// The identifier of the last start state.
    pub(crate) max_start: StateID,
}

impl Special {
    /// Creates a new set of special ranges for a DFA. All ranges are initially
    /// set to only contain the dead state. This is interpreted as an empty
    /// range.
    #[cfg(feature = "dfa-build")]
    pub(crate) fn new() -> Special {
        Special {
            max: DEAD,
            quit_id: DEAD,
            min_match: DEAD,
            max_match: DEAD,
            min_accel: DEAD,
            max_accel: DEAD,
            min_start: DEAD,
            max_start: DEAD,
        }
    }

    /// Remaps all of the special state identifiers using the function given.
    #[cfg(feature = "dfa-build")]
    pub(crate) fn remap(&self, map: impl Fn(StateID) -> StateID) -> Special {
        Special {
            max: map(self.max),
            quit_id: map(self.quit_id),
            min_match: map(self.min_match),
            max_match: map(self.max_match),
            min_accel: map(self.min_accel),
            max_accel: map(self.max_accel),
            min_start: map(self.min_start),
            max_start: map(self.max_start),
        }
    }

    /// Deserialize the given bytes into special state ranges. If the slice
    /// given is not big enough, then this returns an error. Similarly, if
    /// any of the expected invariants around special state ranges aren't
    /// upheld, an error is returned. Note that this does not guarantee that
    /// the information returned is correct.
    ///
    /// Upon success, this returns the number of bytes read in addition to the
    /// special state IDs themselves.
    pub(crate) fn from_bytes(
        mut slice: &[u8],
    ) -> Result<(Special, usize), DeserializeError> {
        wire::check_slice_len(slice, 8 * StateID::SIZE, "special states")?;

        let mut nread = 0;
        let mut read_id = |what| -> Result<StateID, DeserializeError> {
            let (id, nr) = wire::try_read_state_id(slice, what)?;
            nread += nr;
            slice = &slice[StateID::SIZE..];
            Ok(id)
        };

        let max = read_id("special max id")?;
        let quit_id = read_id("special quit id")?;
        let min_match = read_id("special min match id")?;
        let max_match = read_id("special max match id")?;
        let min_accel = read_id("special min accel id")?;
        let max_accel = read_id("special max accel id")?;
        let min_start = read_id("special min start id")?;
        let max_start = read_id("special max start id")?;

        let special = Special {
            max,
            quit_id,
            min_match,
            max_match,
            min_accel,
            max_accel,
            min_start,
            max_start,
        };
        special.validate()?;
        assert_eq!(nread, special.write_to_len());
        Ok((special, nread))
    }

    /// Validate that the information describing special states satisfies
    /// all known invariants.
    pub(crate) fn validate(&self) -> Result<(), DeserializeError> {
        // Check that both ends of the range are DEAD or neither are.
        if self.min_match == DEAD && self.max_match != DEAD {
            err!("min_match is DEAD, but max_match is not");
        }
        if self.min_match != DEAD && self.max_match == DEAD {
            err!("max_match is DEAD, but min_match is not");
        }
        if self.min_accel == DEAD && self.max_accel != DEAD {
            err!("min_accel is DEAD, but max_accel is not");
        }
        if self.min_accel != DEAD && self.max_accel == DEAD {
            err!("max_accel is DEAD, but min_accel is not");
        }
        if self.min_start == DEAD && self.max_start != DEAD {
            err!("min_start is DEAD, but max_start is not");
        }
        if self.min_start != DEAD && self.max_start == DEAD {
            err!("max_start is DEAD, but min_start is not");
        }

        // Check that ranges are well formed.
        if self.min_match > self.max_match {
            err!("min_match should not be greater than max_match");
        }
        if self.min_accel > self.max_accel {
            err!("min_accel should not be greater than max_accel");
        }
        if self.min_start > self.max_start {
            err!("min_start should not be greater than max_start");
        }

        // Check that ranges are ordered with respect to one another.
        if self.matches() && self.quit_id >= self.min_match {
            err!("quit_id should not be greater than min_match");
        }
        if self.accels() && self.quit_id >= self.min_accel {
            err!("quit_id should not be greater than min_accel");
        }
        if self.starts() && self.quit_id >= self.min_start {
            err!("quit_id should not be greater than min_start");
        }
        if self.matches() && self.accels() && self.min_accel < self.min_match {
            err!("min_match should not be greater than min_accel");
        }
        if self.matches() && self.starts() && self.min_start < self.min_match {
            err!("min_match should not be greater than min_start");
        }
        if self.accels() && self.starts() && self.min_start < self.min_accel {
            err!("min_accel should not be greater than min_start");
        }

        // Check that max is at least as big as everything else.
        if self.max < self.quit_id {
            err!("quit_id should not be greater than max");
        }
        if self.max < self.max_match {
            err!("max_match should not be greater than max");
        }
        if self.max < self.max_accel {
            err!("max_accel should not be greater than max");
        }
        if self.max < self.max_start {
            err!("max_start should not be greater than max");
        }

        Ok(())
    }

    /// Validate that the special state information is compatible with the
    /// given state len.
    pub(crate) fn validate_state_len(
        &self,
        len: usize,
        stride2: usize,
    ) -> Result<(), DeserializeError> {
        // We assume that 'validate' has already passed, so we know that 'max'
        // is truly the max. So all we need to check is that the max state ID
        // is less than the state ID len. The max legal value here is len-1,
        // which occurs when there are no non-special states.
        if (self.max.as_usize() >> stride2) >= len {
            err!("max should not be greater than or equal to state length");
        }
        Ok(())
    }

    /// Write the IDs and ranges for special states to the given byte buffer.
    /// The buffer given must have enough room to store all data, otherwise
    /// this will return an error. The number of bytes written is returned
    /// on success. The number of bytes written is guaranteed to be a multiple
    /// of 8.
    pub(crate) fn write_to<E: Endian>(
        &self,
        dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        use crate::util::wire::write_state_id as write;

        if dst.len() < self.write_to_len() {
            return Err(SerializeError::buffer_too_small("special state ids"));
        }

        let mut nwrite = 0;
        nwrite += write::<E>(self.max, &mut dst[nwrite..]);
        nwrite += write::<E>(self.quit_id, &mut dst[nwrite..]);
        nwrite += write::<E>(self.min_match, &mut dst[nwrite..]);
        nwrite += write::<E>(self.max_match, &mut dst[nwrite..]);
        nwrite += write::<E>(self.min_accel, &mut dst[nwrite..]);
        nwrite += write::<E>(self.max_accel, &mut dst[nwrite..]);
        nwrite += write::<E>(self.min_start, &mut dst[nwrite..]);
        nwrite += write::<E>(self.max_start, &mut dst[nwrite..]);

        assert_eq!(
            self.write_to_len(),
            nwrite,
            "expected to write certain number of bytes",
        );
        assert_eq!(
            nwrite % 8,
            0,
            "expected to write multiple of 8 bytes for special states",
        );
        Ok(nwrite)
    }

    /// Returns the total number of bytes written by `write_to`.
    pub(crate) fn write_to_len(&self) -> usize {
        8 * StateID::SIZE
    }

    /// Sets the maximum special state ID based on the current values. This
    /// should be used once all possible state IDs are set.
    #[cfg(feature = "dfa-build")]
    pub(crate) fn set_max(&mut self) {
        use core::cmp::max;
        self.max = max(
            self.quit_id,
            max(self.max_match, max(self.max_accel, self.max_start)),
        );
    }

    /// Sets the maximum special state ID such that starting states are not
    /// considered "special." This also marks the min/max starting states as
    /// DEAD such that 'is_start_state' always returns false, even if the state
    /// is actually a starting state.
    ///
    /// This is useful when there is no prefilter set. It will avoid
    /// ping-ponging between the hot path in the DFA search code and the start
    /// state handling code, which is typically only useful for executing a
    /// prefilter.
    #[cfg(feature = "dfa-build")]
    pub(crate) fn set_no_special_start_states(&mut self) {
        use core::cmp::max;
        self.max = max(self.quit_id, max(self.max_match, self.max_accel));
        self.min_start = DEAD;
        self.max_start = DEAD;
    }

    /// Returns true if and only if the given state ID is a special state.
    #[inline]
    pub(crate) fn is_special_state(&self, id: StateID) -> bool {
        id <= self.max
    }

    /// Returns true if and only if the given state ID is a dead state.
    #[inline]
    pub(crate) fn is_dead_state(&self, id: StateID) -> bool {
        id == DEAD
    }

    /// Returns true if and only if the given state ID is a quit state.
    #[inline]
    pub(crate) fn is_quit_state(&self, id: StateID) -> bool {
        !self.is_dead_state(id) && self.quit_id == id
    }

    /// Returns true if and only if the given state ID is a match state.
    #[inline]
    pub(crate) fn is_match_state(&self, id: StateID) -> bool {
        !self.is_dead_state(id) && self.min_match <= id && id <= self.max_match
    }

    /// Returns true if and only if the given state ID is an accel state.
    #[inline]
    pub(crate) fn is_accel_state(&self, id: StateID) -> bool {
        !self.is_dead_state(id) && self.min_accel <= id && id <= self.max_accel
    }

    /// Returns true if and only if the given state ID is a start state.
    #[inline]
    pub(crate) fn is_start_state(&self, id: StateID) -> bool {
        !self.is_dead_state(id) && self.min_start <= id && id <= self.max_start
    }

    /// Returns the total number of match states for a dense table based DFA.
    #[inline]
    pub(crate) fn match_len(&self, stride: usize) -> usize {
        if self.matches() {
            (self.max_match.as_usize() - self.min_match.as_usize() + stride)
                / stride
        } else {
            0
        }
    }

    /// Returns true if and only if there is at least one match state.
    #[inline]
    pub(crate) fn matches(&self) -> bool {
        self.min_match != DEAD
    }

    /// Returns the total number of accel states.
    #[cfg(feature = "dfa-build")]
    pub(crate) fn accel_len(&self, stride: usize) -> usize {
        if self.accels() {
            (self.max_accel.as_usize() - self.min_accel.as_usize() + stride)
                / stride
        } else {
            0
        }
    }

    /// Returns true if and only if there is at least one accel state.
    #[inline]
    pub(crate) fn accels(&self) -> bool {
        self.min_accel != DEAD
    }

    /// Returns true if and only if there is at least one start state.
    #[inline]
    pub(crate) fn starts(&self) -> bool {
        self.min_start != DEAD
    }
}
