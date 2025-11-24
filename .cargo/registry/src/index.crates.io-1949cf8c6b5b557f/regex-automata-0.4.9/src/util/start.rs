/*!
Provides helpers for dealing with start state configurations in DFAs.
*/

use crate::util::{
    look::LookMatcher,
    search::{Anchored, Input},
    wire::{self, DeserializeError, SerializeError},
};

/// The configuration used to determine a DFA's start state for a search.
///
/// A DFA has a single starting state in the typical textbook description. That
/// is, it corresponds to the set of all starting states for the NFA that built
/// it, along with their espsilon closures. In this crate, however, DFAs have
/// many possible start states due to a few factors:
///
/// * DFAs support the ability to run either anchored or unanchored searches.
/// Each type of search needs its own start state. For example, an unanchored
/// search requires starting at a state corresponding to a regex with a
/// `(?s-u:.)*?` prefix, which will match through anything.
/// * DFAs also optionally support starting an anchored search for any one
/// specific pattern. Each such pattern requires its own start state.
/// * If a look-behind assertion like `^` or `\b` is used in the regex, then
/// the DFA will need to inspect a single byte immediately before the start of
/// the search to choose the correct start state.
///
/// Indeed, this configuration precisely encapsulates all of the above factors.
/// The [`Config::anchored`] method sets which kind of anchored search to
/// perform while the [`Config::look_behind`] method provides a way to set
/// the byte that occurs immediately before the start of the search.
///
/// Generally speaking, this type is only useful when you want to run searches
/// without using an [`Input`]. In particular, an `Input` wants a haystack
/// slice, but callers may not have a contiguous sequence of bytes as a
/// haystack in all cases. This type provides a lower level of control such
/// that callers can provide their own anchored configuration and look-behind
/// byte explicitly.
///
/// # Example
///
/// This shows basic usage that permits running a search with a DFA without
/// using the `Input` abstraction.
///
/// ```
/// use regex_automata::{
///     dfa::{Automaton, dense},
///     util::start,
///     Anchored,
/// };
///
/// let dfa = dense::DFA::new(r"(?-u)\b\w+\b")?;
/// let haystack = "quartz";
///
/// let config = start::Config::new().anchored(Anchored::Yes);
/// let mut state = dfa.start_state(&config)?;
/// for &b in haystack.as_bytes().iter() {
///     state = dfa.next_state(state, b);
/// }
/// state = dfa.next_eoi_state(state);
/// assert!(dfa.is_match_state(state));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// This example shows how to correctly run a search that doesn't begin at
/// the start of a haystack. Notice how we set the look-behind byte, and as
/// a result, the `\b` assertion does not match.
///
/// ```
/// use regex_automata::{
///     dfa::{Automaton, dense},
///     util::start,
///     Anchored,
/// };
///
/// let dfa = dense::DFA::new(r"(?-u)\b\w+\b")?;
/// let haystack = "quartz";
///
/// let config = start::Config::new()
///     .anchored(Anchored::Yes)
///     .look_behind(Some(b'q'));
/// let mut state = dfa.start_state(&config)?;
/// for &b in haystack.as_bytes().iter().skip(1) {
///     state = dfa.next_state(state, b);
/// }
/// state = dfa.next_eoi_state(state);
/// // No match!
/// assert!(!dfa.is_match_state(state));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// If we had instead not set a look-behind byte, then the DFA would assume
/// that it was starting at the beginning of the haystack, and thus `\b` should
/// match. This in turn would result in erroneously reporting a match:
///
/// ```
/// use regex_automata::{
///     dfa::{Automaton, dense},
///     util::start,
///     Anchored,
/// };
///
/// let dfa = dense::DFA::new(r"(?-u)\b\w+\b")?;
/// let haystack = "quartz";
///
/// // Whoops, forgot the look-behind byte...
/// let config = start::Config::new().anchored(Anchored::Yes);
/// let mut state = dfa.start_state(&config)?;
/// for &b in haystack.as_bytes().iter().skip(1) {
///     state = dfa.next_state(state, b);
/// }
/// state = dfa.next_eoi_state(state);
/// // And now we get a match unexpectedly.
/// assert!(dfa.is_match_state(state));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct Config {
    look_behind: Option<u8>,
    anchored: Anchored,
}

impl Config {
    /// Create a new default start configuration.
    ///
    /// The default is an unanchored search that starts at the beginning of the
    /// haystack.
    pub fn new() -> Config {
        Config { anchored: Anchored::No, look_behind: None }
    }

    /// A convenience routine for building a start configuration from an
    /// [`Input`] for a forward search.
    ///
    /// This automatically sets the look-behind byte to the byte immediately
    /// preceding the start of the search. If the start of the search is at
    /// offset `0`, then no look-behind byte is set.
    pub fn from_input_forward(input: &Input<'_>) -> Config {
        let look_behind = input
            .start()
            .checked_sub(1)
            .and_then(|i| input.haystack().get(i).copied());
        Config { look_behind, anchored: input.get_anchored() }
    }

    /// A convenience routine for building a start configuration from an
    /// [`Input`] for a reverse search.
    ///
    /// This automatically sets the look-behind byte to the byte immediately
    /// following the end of the search. If the end of the search is at
    /// offset `haystack.len()`, then no look-behind byte is set.
    pub fn from_input_reverse(input: &Input<'_>) -> Config {
        let look_behind = input.haystack().get(input.end()).copied();
        Config { look_behind, anchored: input.get_anchored() }
    }

    /// Set the look-behind byte at the start of a search.
    ///
    /// Unless the search is intended to logically start at the beginning of a
    /// haystack, this should _always_ be set to the byte immediately preceding
    /// the start of the search. If no look-behind byte is set, then the start
    /// configuration will assume it is at the beginning of the haystack. For
    /// example, the anchor `^` will match.
    ///
    /// The default is that no look-behind byte is set.
    pub fn look_behind(mut self, byte: Option<u8>) -> Config {
        self.look_behind = byte;
        self
    }

    /// Set the anchored mode of a search.
    ///
    /// The default is an unanchored search.
    pub fn anchored(mut self, mode: Anchored) -> Config {
        self.anchored = mode;
        self
    }

    /// Return the look-behind byte in this configuration, if one exists.
    pub fn get_look_behind(&self) -> Option<u8> {
        self.look_behind
    }

    /// Return the anchored mode in this configuration.
    pub fn get_anchored(&self) -> Anchored {
        self.anchored
    }
}

/// A map from every possible byte value to its corresponding starting
/// configuration.
///
/// This map is used in order to lookup the start configuration for a particular
/// position in a haystack. This start configuration is then used in
/// combination with things like the anchored mode and pattern ID to fully
/// determine the start state.
///
/// Generally speaking, this map is only used for fully compiled DFAs and lazy
/// DFAs. For NFAs (including the one-pass DFA), the start state is generally
/// selected by virtue of traversing the NFA state graph. DFAs do the same
/// thing, but at build time and not search time. (Well, technically the lazy
/// DFA does it at search time, but it does enough work to cache the full
/// result of the epsilon closure that the NFA engines tend to need to do.)
#[derive(Clone)]
pub(crate) struct StartByteMap {
    map: [Start; 256],
}

impl StartByteMap {
    /// Create a new map from byte values to their corresponding starting
    /// configurations. The map is determined, in part, by how look-around
    /// assertions are matched via the matcher given.
    pub(crate) fn new(lookm: &LookMatcher) -> StartByteMap {
        let mut map = [Start::NonWordByte; 256];
        map[usize::from(b'\n')] = Start::LineLF;
        map[usize::from(b'\r')] = Start::LineCR;
        map[usize::from(b'_')] = Start::WordByte;

        let mut byte = b'0';
        while byte <= b'9' {
            map[usize::from(byte)] = Start::WordByte;
            byte += 1;
        }
        byte = b'A';
        while byte <= b'Z' {
            map[usize::from(byte)] = Start::WordByte;
            byte += 1;
        }
        byte = b'a';
        while byte <= b'z' {
            map[usize::from(byte)] = Start::WordByte;
            byte += 1;
        }

        let lineterm = lookm.get_line_terminator();
        // If our line terminator is normal, then it is already handled by
        // the LineLF and LineCR configurations. But if it's weird, then we
        // overwrite whatever was there before for that terminator with a
        // special configuration. The trick here is that if the terminator
        // is, say, a word byte like `a`, then callers seeing this start
        // configuration need to account for that and build their DFA state as
        // if it *also* came from a word byte.
        if lineterm != b'\r' && lineterm != b'\n' {
            map[usize::from(lineterm)] = Start::CustomLineTerminator;
        }
        StartByteMap { map }
    }

    /// Return the starting configuration for the given look-behind byte.
    ///
    /// If no look-behind exists, callers should use `Start::Text`.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn get(&self, byte: u8) -> Start {
        self.map[usize::from(byte)]
    }

    /// Deserializes a byte class map from the given slice. If the slice is of
    /// insufficient length or otherwise contains an impossible mapping, then
    /// an error is returned. Upon success, the number of bytes read along with
    /// the map are returned. The number of bytes read is always a multiple of
    /// 8.
    pub(crate) fn from_bytes(
        slice: &[u8],
    ) -> Result<(StartByteMap, usize), DeserializeError> {
        wire::check_slice_len(slice, 256, "start byte map")?;
        let mut map = [Start::NonWordByte; 256];
        for (i, &repr) in slice[..256].iter().enumerate() {
            map[i] = match Start::from_usize(usize::from(repr)) {
                Some(start) => start,
                None => {
                    return Err(DeserializeError::generic(
                        "found invalid starting configuration",
                    ))
                }
            };
        }
        Ok((StartByteMap { map }, 256))
    }

    /// Writes this map to the given byte buffer. if the given buffer is too
    /// small, then an error is returned. Upon success, the total number of
    /// bytes written is returned. The number of bytes written is guaranteed to
    /// be a multiple of 8.
    pub(crate) fn write_to(
        &self,
        dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("start byte map"));
        }
        for (i, &start) in self.map.iter().enumerate() {
            dst[i] = start.as_u8();
        }
        Ok(nwrite)
    }

    /// Returns the total number of bytes written by `write_to`.
    pub(crate) fn write_to_len(&self) -> usize {
        256
    }
}

impl core::fmt::Debug for StartByteMap {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::util::escape::DebugByte;

        write!(f, "StartByteMap{{")?;
        for byte in 0..=255 {
            if byte > 0 {
                write!(f, ", ")?;
            }
            let start = self.map[usize::from(byte)];
            write!(f, "{:?} => {:?}", DebugByte(byte), start)?;
        }
        write!(f, "}}")?;
        Ok(())
    }
}

/// Represents the six possible starting configurations of a DFA search.
///
/// The starting configuration is determined by inspecting the beginning
/// of the haystack (up to 1 byte). Ultimately, this along with a pattern ID
/// (if specified) and the type of search (anchored or not) is what selects the
/// start state to use in a DFA.
///
/// As one example, if a DFA only supports unanchored searches and does not
/// support anchored searches for each pattern, then it will have at most 6
/// distinct start states. (Some start states may be reused if determinization
/// can determine that they will be equivalent.) If the DFA supports both
/// anchored and unanchored searches, then it will have a maximum of 12
/// distinct start states. Finally, if the DFA also supports anchored searches
/// for each pattern, then it can have up to `12 + (N * 6)` start states, where
/// `N` is the number of patterns.
///
/// Handling each of these starting configurations in the context of DFA
/// determinization can be *quite* tricky and subtle. But the code is small
/// and can be found at `crate::util::determinize::set_lookbehind_from_start`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Start {
    /// This occurs when the starting position is not any of the ones below.
    NonWordByte = 0,
    /// This occurs when the byte immediately preceding the start of the search
    /// is an ASCII word byte.
    WordByte = 1,
    /// This occurs when the starting position of the search corresponds to the
    /// beginning of the haystack.
    Text = 2,
    /// This occurs when the byte immediately preceding the start of the search
    /// is a line terminator. Specifically, `\n`.
    LineLF = 3,
    /// This occurs when the byte immediately preceding the start of the search
    /// is a line terminator. Specifically, `\r`.
    LineCR = 4,
    /// This occurs when a custom line terminator has been set via a
    /// `LookMatcher`, and when that line terminator is neither a `\r` or a
    /// `\n`.
    ///
    /// If the custom line terminator is a word byte, then this start
    /// configuration is still selected. DFAs that implement word boundary
    /// assertions will likely need to check whether the custom line terminator
    /// is a word byte, in which case, it should behave as if the byte
    /// satisfies `\b` in addition to multi-line anchors.
    CustomLineTerminator = 5,
}

impl Start {
    /// Return the starting state corresponding to the given integer. If no
    /// starting state exists for the given integer, then None is returned.
    pub(crate) fn from_usize(n: usize) -> Option<Start> {
        match n {
            0 => Some(Start::NonWordByte),
            1 => Some(Start::WordByte),
            2 => Some(Start::Text),
            3 => Some(Start::LineLF),
            4 => Some(Start::LineCR),
            5 => Some(Start::CustomLineTerminator),
            _ => None,
        }
    }

    /// Returns the total number of starting state configurations.
    pub(crate) fn len() -> usize {
        6
    }

    /// Return this starting configuration as `u8` integer. It is guaranteed to
    /// be less than `Start::len()`.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn as_u8(&self) -> u8 {
        // AFAIK, 'as' is the only way to zero-cost convert an int enum to an
        // actual int.
        *self as u8
    }

    /// Return this starting configuration as a `usize` integer. It is
    /// guaranteed to be less than `Start::len()`.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn as_usize(&self) -> usize {
        usize::from(self.as_u8())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_fwd_done_range() {
        let smap = StartByteMap::new(&LookMatcher::default());
        let input = Input::new("").range(1..0);
        let config = Config::from_input_forward(&input);
        let start =
            config.get_look_behind().map_or(Start::Text, |b| smap.get(b));
        assert_eq!(Start::Text, start);
    }

    #[test]
    fn start_rev_done_range() {
        let smap = StartByteMap::new(&LookMatcher::default());
        let input = Input::new("").range(1..0);
        let config = Config::from_input_reverse(&input);
        let start =
            config.get_look_behind().map_or(Start::Text, |b| smap.get(b));
        assert_eq!(Start::Text, start);
    }

    #[test]
    fn start_fwd() {
        let f = |haystack, start, end| {
            let smap = StartByteMap::new(&LookMatcher::default());
            let input = Input::new(haystack).range(start..end);
            let config = Config::from_input_forward(&input);
            let start =
                config.get_look_behind().map_or(Start::Text, |b| smap.get(b));
            start
        };

        assert_eq!(Start::Text, f("", 0, 0));
        assert_eq!(Start::Text, f("abc", 0, 3));
        assert_eq!(Start::Text, f("\nabc", 0, 3));

        assert_eq!(Start::LineLF, f("\nabc", 1, 3));

        assert_eq!(Start::LineCR, f("\rabc", 1, 3));

        assert_eq!(Start::WordByte, f("abc", 1, 3));

        assert_eq!(Start::NonWordByte, f(" abc", 1, 3));
    }

    #[test]
    fn start_rev() {
        let f = |haystack, start, end| {
            let smap = StartByteMap::new(&LookMatcher::default());
            let input = Input::new(haystack).range(start..end);
            let config = Config::from_input_reverse(&input);
            let start =
                config.get_look_behind().map_or(Start::Text, |b| smap.get(b));
            start
        };

        assert_eq!(Start::Text, f("", 0, 0));
        assert_eq!(Start::Text, f("abc", 0, 3));
        assert_eq!(Start::Text, f("abc\n", 0, 4));

        assert_eq!(Start::LineLF, f("abc\nz", 0, 3));

        assert_eq!(Start::LineCR, f("abc\rz", 0, 3));

        assert_eq!(Start::WordByte, f("abc", 0, 2));

        assert_eq!(Start::NonWordByte, f("abc ", 0, 3));
    }
}
