use core::{
    cmp,
    fmt::Debug,
    panic::{RefUnwindSafe, UnwindSafe},
    u8,
};

use alloc::{sync::Arc, vec, vec::Vec};

use crate::{
    packed,
    util::{
        alphabet::ByteSet,
        search::{Match, MatchKind, Span},
    },
};

/// A prefilter for accelerating a search.
///
/// This crate uses prefilters in the core search implementations to accelerate
/// common cases. They typically only apply to cases where there are a small
/// number of patterns (less than 100 or so), but when they do, thoughput can
/// be boosted considerably, perhaps by an order of magnitude. When a prefilter
/// is active, it is used whenever a search enters an automaton's start state.
///
/// Currently, prefilters cannot be constructed by
/// callers. A `Prefilter` can only be accessed via the
/// [`Automaton::prefilter`](crate::automaton::Automaton::prefilter)
/// method and used to execute a search. In other words, a prefilter can be
/// used to optimize your own search implementation if necessary, but cannot do
/// much else. If you have a use case for more APIs, please submit an issue.
#[derive(Clone, Debug)]
pub struct Prefilter {
    finder: Arc<dyn PrefilterI>,
    memory_usage: usize,
}

impl Prefilter {
    /// Execute a search in the haystack within the span given. If a match or
    /// a possible match is returned, then it is guaranteed to occur within
    /// the bounds of the span.
    ///
    /// If the span provided is invalid for the given haystack, then behavior
    /// is unspecified.
    #[inline]
    pub fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        self.finder.find_in(haystack, span)
    }

    #[inline]
    pub(crate) fn memory_usage(&self) -> usize {
        self.memory_usage
    }
}

/// A candidate is the result of running a prefilter on a haystack at a
/// particular position.
///
/// The result is either no match, a confirmed match or a possible match.
///
/// When no match is returned, the prefilter is guaranteeing that no possible
/// match can be found in the haystack, and the caller may trust this. That is,
/// all correct prefilters must never report false negatives.
///
/// In some cases, a prefilter can confirm a match very quickly, in which case,
/// the caller may use this to stop what it's doing and report the match. In
/// this case, prefilter implementations must never report a false positive.
/// In other cases, the prefilter can only report a potential match, in which
/// case the callers must attempt to confirm the match. In this case, prefilter
/// implementations are permitted to return false positives.
#[derive(Clone, Debug)]
pub enum Candidate {
    /// No match was found. Since false negatives are not possible, this means
    /// the search can quit as it is guaranteed not to find another match.
    None,
    /// A confirmed match was found. Callers do not need to confirm it.
    Match(Match),
    /// The start of a possible match was found. Callers must confirm it before
    /// reporting it as a match.
    PossibleStartOfMatch(usize),
}

impl Candidate {
    /// Convert this candidate into an option. This is useful when callers
    /// do not distinguish between true positives and false positives (i.e.,
    /// the caller must always confirm the match).
    pub fn into_option(self) -> Option<usize> {
        match self {
            Candidate::None => None,
            Candidate::Match(ref m) => Some(m.start()),
            Candidate::PossibleStartOfMatch(start) => Some(start),
        }
    }
}

/// A prefilter describes the behavior of fast literal scanners for quickly
/// skipping past bytes in the haystack that we know cannot possibly
/// participate in a match.
trait PrefilterI:
    Send + Sync + RefUnwindSafe + UnwindSafe + Debug + 'static
{
    /// Returns the next possible match candidate. This may yield false
    /// positives, so callers must confirm a match starting at the position
    /// returned. This, however, must never produce false negatives. That is,
    /// this must, at minimum, return the starting position of the next match
    /// in the given haystack after or at the given position.
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate;
}

impl<P: PrefilterI + ?Sized> PrefilterI for Arc<P> {
    #[inline(always)]
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        (**self).find_in(haystack, span)
    }
}

/// A builder for constructing the best possible prefilter. When constructed,
/// this builder will heuristically select the best prefilter it can build,
/// if any, and discard the rest.
#[derive(Debug)]
pub(crate) struct Builder {
    count: usize,
    ascii_case_insensitive: bool,
    start_bytes: StartBytesBuilder,
    rare_bytes: RareBytesBuilder,
    memmem: MemmemBuilder,
    packed: Option<packed::Builder>,
    // If we run across a condition that suggests we shouldn't use a prefilter
    // at all (like an empty pattern), then disable prefilters entirely.
    enabled: bool,
}

impl Builder {
    /// Create a new builder for constructing the best possible prefilter.
    pub(crate) fn new(kind: MatchKind) -> Builder {
        let pbuilder = kind
            .as_packed()
            .map(|kind| packed::Config::new().match_kind(kind).builder());
        Builder {
            count: 0,
            ascii_case_insensitive: false,
            start_bytes: StartBytesBuilder::new(),
            rare_bytes: RareBytesBuilder::new(),
            memmem: MemmemBuilder::default(),
            packed: pbuilder,
            enabled: true,
        }
    }

    /// Enable ASCII case insensitivity. When set, byte strings added to this
    /// builder will be interpreted without respect to ASCII case.
    pub(crate) fn ascii_case_insensitive(mut self, yes: bool) -> Builder {
        self.ascii_case_insensitive = yes;
        self.start_bytes = self.start_bytes.ascii_case_insensitive(yes);
        self.rare_bytes = self.rare_bytes.ascii_case_insensitive(yes);
        self
    }

    /// Return a prefilter suitable for quickly finding potential matches.
    ///
    /// All patterns added to an Aho-Corasick automaton should be added to this
    /// builder before attempting to construct the prefilter.
    pub(crate) fn build(&self) -> Option<Prefilter> {
        if !self.enabled {
            debug!("prefilter not enabled, skipping");
            return None;
        }
        // If we only have one pattern, then deferring to memmem is always
        // the best choice. This is kind of a weird case, because, well, why
        // use Aho-Corasick if you only have one pattern? But maybe you don't
        // know exactly how many patterns you'll get up front, and you need to
        // support the option of multiple patterns. So instead of relying on
        // the caller to branch and use memmem explicitly, we just do it for
        // them.
        if !self.ascii_case_insensitive {
            if let Some(pre) = self.memmem.build() {
                debug!("using memmem prefilter");
                return Some(pre);
            }
        }
        let (packed, patlen, minlen) = if self.ascii_case_insensitive {
            (None, usize::MAX, 0)
        } else {
            let patlen = self.packed.as_ref().map_or(usize::MAX, |p| p.len());
            let minlen = self.packed.as_ref().map_or(0, |p| p.minimum_len());
            let packed =
                self.packed.as_ref().and_then(|b| b.build()).map(|s| {
                    let memory_usage = s.memory_usage();
                    debug!(
                        "built packed prefilter (len: {}, \
                         minimum pattern len: {}, memory usage: {}) \
                         for consideration",
                        patlen, minlen, memory_usage,
                    );
                    Prefilter { finder: Arc::new(Packed(s)), memory_usage }
                });
            (packed, patlen, minlen)
        };
        match (self.start_bytes.build(), self.rare_bytes.build()) {
            // If we could build both start and rare prefilters, then there are
            // a few cases in which we'd want to use the start-byte prefilter
            // over the rare-byte prefilter, since the former has lower
            // overhead.
            (prestart @ Some(_), prerare @ Some(_)) => {
                debug!(
                    "both start (len={}, rank={}) and \
                     rare (len={}, rank={}) byte prefilters \
                     are available",
                    self.start_bytes.count,
                    self.start_bytes.rank_sum,
                    self.rare_bytes.count,
                    self.rare_bytes.rank_sum,
                );
                if patlen <= 16
                    && minlen >= 2
                    && self.start_bytes.count >= 3
                    && self.rare_bytes.count >= 3
                {
                    debug!(
                        "start and rare byte prefilters available, but \
                             they're probably slower than packed so using \
                             packed"
                    );
                    return packed;
                }
                // If the start-byte prefilter can scan for a smaller number
                // of bytes than the rare-byte prefilter, then it's probably
                // faster.
                let has_fewer_bytes =
                    self.start_bytes.count < self.rare_bytes.count;
                // Otherwise, if the combined frequency rank of the detected
                // bytes in the start-byte prefilter is "close" to the combined
                // frequency rank of the rare-byte prefilter, then we pick
                // the start-byte prefilter even if the rare-byte prefilter
                // heuristically searches for rare bytes. This is because the
                // rare-byte prefilter has higher constant costs, so we tend to
                // prefer the start-byte prefilter when we can.
                let has_rarer_bytes =
                    self.start_bytes.rank_sum <= self.rare_bytes.rank_sum + 50;
                if has_fewer_bytes {
                    debug!(
                        "using start byte prefilter because it has fewer
                         bytes to search for than the rare byte prefilter",
                    );
                    prestart
                } else if has_rarer_bytes {
                    debug!(
                        "using start byte prefilter because its byte \
                         frequency rank was determined to be \
                         \"good enough\" relative to the rare byte prefilter \
                         byte frequency rank",
                    );
                    prestart
                } else {
                    debug!("using rare byte prefilter");
                    prerare
                }
            }
            (prestart @ Some(_), None) => {
                if patlen <= 16 && minlen >= 2 && self.start_bytes.count >= 3 {
                    debug!(
                        "start byte prefilter available, but \
                         it's probably slower than packed so using \
                         packed"
                    );
                    return packed;
                }
                debug!(
                    "have start byte prefilter but not rare byte prefilter, \
                     so using start byte prefilter",
                );
                prestart
            }
            (None, prerare @ Some(_)) => {
                if patlen <= 16 && minlen >= 2 && self.rare_bytes.count >= 3 {
                    debug!(
                        "rare byte prefilter available, but \
                         it's probably slower than packed so using \
                         packed"
                    );
                    return packed;
                }
                debug!(
                    "have rare byte prefilter but not start byte prefilter, \
                     so using rare byte prefilter",
                );
                prerare
            }
            (None, None) if self.ascii_case_insensitive => {
                debug!(
                    "no start or rare byte prefilter and ASCII case \
                     insensitivity was enabled, so skipping prefilter",
                );
                None
            }
            (None, None) => {
                if packed.is_some() {
                    debug!("falling back to packed prefilter");
                } else {
                    debug!("no prefilter available");
                }
                packed
            }
        }
    }

    /// Add a literal string to this prefilter builder.
    pub(crate) fn add(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            self.enabled = false;
        }
        if !self.enabled {
            return;
        }
        self.count += 1;
        self.start_bytes.add(bytes);
        self.rare_bytes.add(bytes);
        self.memmem.add(bytes);
        if let Some(ref mut pbuilder) = self.packed {
            pbuilder.add(bytes);
        }
    }
}

/// A type that wraps a packed searcher and implements the `Prefilter`
/// interface.
#[derive(Clone, Debug)]
struct Packed(packed::Searcher);

impl PrefilterI for Packed {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        self.0
            .find_in(&haystack, span)
            .map_or(Candidate::None, Candidate::Match)
    }
}

/// A builder for constructing a prefilter that uses memmem.
#[derive(Debug, Default)]
struct MemmemBuilder {
    /// The number of patterns that have been added.
    count: usize,
    /// The singular pattern to search for. This is only set when count==1.
    one: Option<Vec<u8>>,
}

impl MemmemBuilder {
    fn build(&self) -> Option<Prefilter> {
        #[cfg(all(feature = "std", feature = "perf-literal"))]
        fn imp(builder: &MemmemBuilder) -> Option<Prefilter> {
            let pattern = builder.one.as_ref()?;
            assert_eq!(1, builder.count);
            let finder = Arc::new(Memmem(
                memchr::memmem::Finder::new(pattern).into_owned(),
            ));
            let memory_usage = pattern.len();
            Some(Prefilter { finder, memory_usage })
        }

        #[cfg(not(all(feature = "std", feature = "perf-literal")))]
        fn imp(_: &MemmemBuilder) -> Option<Prefilter> {
            None
        }

        imp(self)
    }

    fn add(&mut self, bytes: &[u8]) {
        self.count += 1;
        if self.count == 1 {
            self.one = Some(bytes.to_vec());
        } else {
            self.one = None;
        }
    }
}

/// A type that wraps a SIMD accelerated single substring search from the
/// `memchr` crate for use as a prefilter.
///
/// Currently, this prefilter is only active for Aho-Corasick searchers with
/// a single pattern. In theory, this could be extended to support searchers
/// that have a common prefix of more than one byte (for one byte, we would use
/// memchr), but it's not clear if it's worth it or not.
///
/// Also, unfortunately, this currently also requires the 'std' feature to
/// be enabled. That's because memchr doesn't have a no-std-but-with-alloc
/// mode, and so APIs like Finder::into_owned aren't available when 'std' is
/// disabled. But there should be an 'alloc' feature that brings in APIs like
/// Finder::into_owned but doesn't use std-only features like runtime CPU
/// feature detection.
#[cfg(all(feature = "std", feature = "perf-literal"))]
#[derive(Clone, Debug)]
struct Memmem(memchr::memmem::Finder<'static>);

#[cfg(all(feature = "std", feature = "perf-literal"))]
impl PrefilterI for Memmem {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        use crate::util::primitives::PatternID;

        self.0.find(&haystack[span]).map_or(Candidate::None, |i| {
            let start = span.start + i;
            let end = start + self.0.needle().len();
            // N.B. We can declare a match and use a fixed pattern ID here
            // because a Memmem prefilter is only ever created for searchers
            // with exactly one pattern. Thus, every match is always a match
            // and it is always for the first and only pattern.
            Candidate::Match(Match::new(PatternID::ZERO, start..end))
        })
    }
}

/// A builder for constructing a rare byte prefilter.
///
/// A rare byte prefilter attempts to pick out a small set of rare bytes that
/// occurr in the patterns, and then quickly scan to matches of those rare
/// bytes.
#[derive(Clone, Debug)]
struct RareBytesBuilder {
    /// Whether this prefilter should account for ASCII case insensitivity or
    /// not.
    ascii_case_insensitive: bool,
    /// A set of rare bytes, indexed by byte value.
    rare_set: ByteSet,
    /// A set of byte offsets associated with bytes in a pattern. An entry
    /// corresponds to a particular bytes (its index) and is only non-zero if
    /// the byte occurred at an offset greater than 0 in at least one pattern.
    ///
    /// If a byte's offset is not representable in 8 bits, then the rare bytes
    /// prefilter becomes inert.
    byte_offsets: RareByteOffsets,
    /// Whether this is available as a prefilter or not. This can be set to
    /// false during construction if a condition is seen that invalidates the
    /// use of the rare-byte prefilter.
    available: bool,
    /// The number of bytes set to an active value in `byte_offsets`.
    count: usize,
    /// The sum of frequency ranks for the rare bytes detected. This is
    /// intended to give a heuristic notion of how rare the bytes are.
    rank_sum: u16,
}

/// A set of byte offsets, keyed by byte.
#[derive(Clone, Copy)]
struct RareByteOffsets {
    /// Each entry corresponds to the maximum offset of the corresponding
    /// byte across all patterns seen.
    set: [RareByteOffset; 256],
}

impl RareByteOffsets {
    /// Create a new empty set of rare byte offsets.
    pub(crate) fn empty() -> RareByteOffsets {
        RareByteOffsets { set: [RareByteOffset::default(); 256] }
    }

    /// Add the given offset for the given byte to this set. If the offset is
    /// greater than the existing offset, then it overwrites the previous
    /// value and returns false. If there is no previous value set, then this
    /// sets it and returns true.
    pub(crate) fn set(&mut self, byte: u8, off: RareByteOffset) {
        self.set[byte as usize].max =
            cmp::max(self.set[byte as usize].max, off.max);
    }
}

impl core::fmt::Debug for RareByteOffsets {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut offsets = vec![];
        for off in self.set.iter() {
            if off.max > 0 {
                offsets.push(off);
            }
        }
        f.debug_struct("RareByteOffsets").field("set", &offsets).finish()
    }
}

/// Offsets associated with an occurrence of a "rare" byte in any of the
/// patterns used to construct a single Aho-Corasick automaton.
#[derive(Clone, Copy, Debug)]
struct RareByteOffset {
    /// The maximum offset at which a particular byte occurs from the start
    /// of any pattern. This is used as a shift amount. That is, when an
    /// occurrence of this byte is found, the candidate position reported by
    /// the prefilter is `position_of_byte - max`, such that the automaton
    /// will begin its search at a position that is guaranteed to observe a
    /// match.
    ///
    /// To avoid accidentally quadratic behavior, a prefilter is considered
    /// ineffective when it is asked to start scanning from a position that it
    /// has already scanned past.
    ///
    /// Using a `u8` here means that if we ever see a pattern that's longer
    /// than 255 bytes, then the entire rare byte prefilter is disabled.
    max: u8,
}

impl Default for RareByteOffset {
    fn default() -> RareByteOffset {
        RareByteOffset { max: 0 }
    }
}

impl RareByteOffset {
    /// Create a new rare byte offset. If the given offset is too big, then
    /// None is returned. In that case, callers should render the rare bytes
    /// prefilter inert.
    fn new(max: usize) -> Option<RareByteOffset> {
        if max > u8::MAX as usize {
            None
        } else {
            Some(RareByteOffset { max: max as u8 })
        }
    }
}

impl RareBytesBuilder {
    /// Create a new builder for constructing a rare byte prefilter.
    fn new() -> RareBytesBuilder {
        RareBytesBuilder {
            ascii_case_insensitive: false,
            rare_set: ByteSet::empty(),
            byte_offsets: RareByteOffsets::empty(),
            available: true,
            count: 0,
            rank_sum: 0,
        }
    }

    /// Enable ASCII case insensitivity. When set, byte strings added to this
    /// builder will be interpreted without respect to ASCII case.
    fn ascii_case_insensitive(mut self, yes: bool) -> RareBytesBuilder {
        self.ascii_case_insensitive = yes;
        self
    }

    /// Build the rare bytes prefilter.
    ///
    /// If there are more than 3 distinct rare bytes found, or if heuristics
    /// otherwise determine that this prefilter should not be used, then `None`
    /// is returned.
    fn build(&self) -> Option<Prefilter> {
        #[cfg(feature = "perf-literal")]
        fn imp(builder: &RareBytesBuilder) -> Option<Prefilter> {
            if !builder.available || builder.count > 3 {
                return None;
            }
            let (mut bytes, mut len) = ([0; 3], 0);
            for b in 0..=255 {
                if builder.rare_set.contains(b) {
                    bytes[len] = b as u8;
                    len += 1;
                }
            }
            let finder: Arc<dyn PrefilterI> = match len {
                0 => return None,
                1 => Arc::new(RareBytesOne {
                    byte1: bytes[0],
                    offset: builder.byte_offsets.set[bytes[0] as usize],
                }),
                2 => Arc::new(RareBytesTwo {
                    offsets: builder.byte_offsets,
                    byte1: bytes[0],
                    byte2: bytes[1],
                }),
                3 => Arc::new(RareBytesThree {
                    offsets: builder.byte_offsets,
                    byte1: bytes[0],
                    byte2: bytes[1],
                    byte3: bytes[2],
                }),
                _ => unreachable!(),
            };
            Some(Prefilter { finder, memory_usage: 0 })
        }

        #[cfg(not(feature = "perf-literal"))]
        fn imp(_: &RareBytesBuilder) -> Option<Prefilter> {
            None
        }

        imp(self)
    }

    /// Add a byte string to this builder.
    ///
    /// All patterns added to an Aho-Corasick automaton should be added to this
    /// builder before attempting to construct the prefilter.
    fn add(&mut self, bytes: &[u8]) {
        // If we've already given up, then do nothing.
        if !self.available {
            return;
        }
        // If we've already blown our budget, then don't waste time looking
        // for more rare bytes.
        if self.count > 3 {
            self.available = false;
            return;
        }
        // If the pattern is too long, then our offset table is bunk, so
        // give up.
        if bytes.len() >= 256 {
            self.available = false;
            return;
        }
        let mut rarest = match bytes.get(0) {
            None => return,
            Some(&b) => (b, freq_rank(b)),
        };
        // The idea here is to look for the rarest byte in each pattern, and
        // add that to our set. As a special exception, if we see a byte that
        // we've already added, then we immediately stop and choose that byte,
        // even if there's another rare byte in the pattern. This helps us
        // apply the rare byte optimization in more cases by attempting to pick
        // bytes that are in common between patterns. So for example, if we
        // were searching for `Sherlock` and `lockjaw`, then this would pick
        // `k` for both patterns, resulting in the use of `memchr` instead of
        // `memchr2` for `k` and `j`.
        let mut found = false;
        for (pos, &b) in bytes.iter().enumerate() {
            self.set_offset(pos, b);
            if found {
                continue;
            }
            if self.rare_set.contains(b) {
                found = true;
                continue;
            }
            let rank = freq_rank(b);
            if rank < rarest.1 {
                rarest = (b, rank);
            }
        }
        if !found {
            self.add_rare_byte(rarest.0);
        }
    }

    fn set_offset(&mut self, pos: usize, byte: u8) {
        // This unwrap is OK because pos is never bigger than our max.
        let offset = RareByteOffset::new(pos).unwrap();
        self.byte_offsets.set(byte, offset);
        if self.ascii_case_insensitive {
            self.byte_offsets.set(opposite_ascii_case(byte), offset);
        }
    }

    fn add_rare_byte(&mut self, byte: u8) {
        self.add_one_rare_byte(byte);
        if self.ascii_case_insensitive {
            self.add_one_rare_byte(opposite_ascii_case(byte));
        }
    }

    fn add_one_rare_byte(&mut self, byte: u8) {
        if !self.rare_set.contains(byte) {
            self.rare_set.add(byte);
            self.count += 1;
            self.rank_sum += freq_rank(byte) as u16;
        }
    }
}

/// A prefilter for scanning for a single "rare" byte.
#[cfg(feature = "perf-literal")]
#[derive(Clone, Debug)]
struct RareBytesOne {
    byte1: u8,
    offset: RareByteOffset,
}

#[cfg(feature = "perf-literal")]
impl PrefilterI for RareBytesOne {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        memchr::memchr(self.byte1, &haystack[span])
            .map(|i| {
                let pos = span.start + i;
                cmp::max(
                    span.start,
                    pos.saturating_sub(usize::from(self.offset.max)),
                )
            })
            .map_or(Candidate::None, Candidate::PossibleStartOfMatch)
    }
}

/// A prefilter for scanning for two "rare" bytes.
#[cfg(feature = "perf-literal")]
#[derive(Clone, Debug)]
struct RareBytesTwo {
    offsets: RareByteOffsets,
    byte1: u8,
    byte2: u8,
}

#[cfg(feature = "perf-literal")]
impl PrefilterI for RareBytesTwo {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        memchr::memchr2(self.byte1, self.byte2, &haystack[span])
            .map(|i| {
                let pos = span.start + i;
                let offset = self.offsets.set[usize::from(haystack[pos])].max;
                cmp::max(span.start, pos.saturating_sub(usize::from(offset)))
            })
            .map_or(Candidate::None, Candidate::PossibleStartOfMatch)
    }
}

/// A prefilter for scanning for three "rare" bytes.
#[cfg(feature = "perf-literal")]
#[derive(Clone, Debug)]
struct RareBytesThree {
    offsets: RareByteOffsets,
    byte1: u8,
    byte2: u8,
    byte3: u8,
}

#[cfg(feature = "perf-literal")]
impl PrefilterI for RareBytesThree {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        memchr::memchr3(self.byte1, self.byte2, self.byte3, &haystack[span])
            .map(|i| {
                let pos = span.start + i;
                let offset = self.offsets.set[usize::from(haystack[pos])].max;
                cmp::max(span.start, pos.saturating_sub(usize::from(offset)))
            })
            .map_or(Candidate::None, Candidate::PossibleStartOfMatch)
    }
}

/// A builder for constructing a starting byte prefilter.
///
/// A starting byte prefilter is a simplistic prefilter that looks for possible
/// matches by reporting all positions corresponding to a particular byte. This
/// generally only takes affect when there are at most 3 distinct possible
/// starting bytes. e.g., the patterns `foo`, `bar`, and `baz` have two
/// distinct starting bytes (`f` and `b`), and this prefilter returns all
/// occurrences of either `f` or `b`.
///
/// In some cases, a heuristic frequency analysis may determine that it would
/// be better not to use this prefilter even when there are 3 or fewer distinct
/// starting bytes.
#[derive(Clone, Debug)]
struct StartBytesBuilder {
    /// Whether this prefilter should account for ASCII case insensitivity or
    /// not.
    ascii_case_insensitive: bool,
    /// The set of starting bytes observed.
    byteset: Vec<bool>,
    /// The number of bytes set to true in `byteset`.
    count: usize,
    /// The sum of frequency ranks for the rare bytes detected. This is
    /// intended to give a heuristic notion of how rare the bytes are.
    rank_sum: u16,
}

impl StartBytesBuilder {
    /// Create a new builder for constructing a start byte prefilter.
    fn new() -> StartBytesBuilder {
        StartBytesBuilder {
            ascii_case_insensitive: false,
            byteset: vec![false; 256],
            count: 0,
            rank_sum: 0,
        }
    }

    /// Enable ASCII case insensitivity. When set, byte strings added to this
    /// builder will be interpreted without respect to ASCII case.
    fn ascii_case_insensitive(mut self, yes: bool) -> StartBytesBuilder {
        self.ascii_case_insensitive = yes;
        self
    }

    /// Build the starting bytes prefilter.
    ///
    /// If there are more than 3 distinct starting bytes, or if heuristics
    /// otherwise determine that this prefilter should not be used, then `None`
    /// is returned.
    fn build(&self) -> Option<Prefilter> {
        #[cfg(feature = "perf-literal")]
        fn imp(builder: &StartBytesBuilder) -> Option<Prefilter> {
            if builder.count > 3 {
                return None;
            }
            let (mut bytes, mut len) = ([0; 3], 0);
            for b in 0..256 {
                if !builder.byteset[b] {
                    continue;
                }
                // We don't handle non-ASCII bytes for now. Getting non-ASCII
                // bytes right is trickier, since we generally don't want to put
                // a leading UTF-8 code unit into a prefilter that isn't ASCII,
                // since they can frequently. Instead, it would be better to use a
                // continuation byte, but this requires more sophisticated analysis
                // of the automaton and a richer prefilter API.
                if b > 0x7F {
                    return None;
                }
                bytes[len] = b as u8;
                len += 1;
            }
            let finder: Arc<dyn PrefilterI> = match len {
                0 => return None,
                1 => Arc::new(StartBytesOne { byte1: bytes[0] }),
                2 => Arc::new(StartBytesTwo {
                    byte1: bytes[0],
                    byte2: bytes[1],
                }),
                3 => Arc::new(StartBytesThree {
                    byte1: bytes[0],
                    byte2: bytes[1],
                    byte3: bytes[2],
                }),
                _ => unreachable!(),
            };
            Some(Prefilter { finder, memory_usage: 0 })
        }

        #[cfg(not(feature = "perf-literal"))]
        fn imp(_: &StartBytesBuilder) -> Option<Prefilter> {
            None
        }

        imp(self)
    }

    /// Add a byte string to this builder.
    ///
    /// All patterns added to an Aho-Corasick automaton should be added to this
    /// builder before attempting to construct the prefilter.
    fn add(&mut self, bytes: &[u8]) {
        if self.count > 3 {
            return;
        }
        if let Some(&byte) = bytes.get(0) {
            self.add_one_byte(byte);
            if self.ascii_case_insensitive {
                self.add_one_byte(opposite_ascii_case(byte));
            }
        }
    }

    fn add_one_byte(&mut self, byte: u8) {
        if !self.byteset[byte as usize] {
            self.byteset[byte as usize] = true;
            self.count += 1;
            self.rank_sum += freq_rank(byte) as u16;
        }
    }
}

/// A prefilter for scanning for a single starting byte.
#[cfg(feature = "perf-literal")]
#[derive(Clone, Debug)]
struct StartBytesOne {
    byte1: u8,
}

#[cfg(feature = "perf-literal")]
impl PrefilterI for StartBytesOne {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        memchr::memchr(self.byte1, &haystack[span])
            .map(|i| span.start + i)
            .map_or(Candidate::None, Candidate::PossibleStartOfMatch)
    }
}

/// A prefilter for scanning for two starting bytes.
#[cfg(feature = "perf-literal")]
#[derive(Clone, Debug)]
struct StartBytesTwo {
    byte1: u8,
    byte2: u8,
}

#[cfg(feature = "perf-literal")]
impl PrefilterI for StartBytesTwo {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        memchr::memchr2(self.byte1, self.byte2, &haystack[span])
            .map(|i| span.start + i)
            .map_or(Candidate::None, Candidate::PossibleStartOfMatch)
    }
}

/// A prefilter for scanning for three starting bytes.
#[cfg(feature = "perf-literal")]
#[derive(Clone, Debug)]
struct StartBytesThree {
    byte1: u8,
    byte2: u8,
    byte3: u8,
}

#[cfg(feature = "perf-literal")]
impl PrefilterI for StartBytesThree {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        memchr::memchr3(self.byte1, self.byte2, self.byte3, &haystack[span])
            .map(|i| span.start + i)
            .map_or(Candidate::None, Candidate::PossibleStartOfMatch)
    }
}

/// If the given byte is an ASCII letter, then return it in the opposite case.
/// e.g., Given `b'A'`, this returns `b'a'`, and given `b'a'`, this returns
/// `b'A'`. If a non-ASCII letter is given, then the given byte is returned.
pub(crate) fn opposite_ascii_case(b: u8) -> u8 {
    if b'A' <= b && b <= b'Z' {
        b.to_ascii_lowercase()
    } else if b'a' <= b && b <= b'z' {
        b.to_ascii_uppercase()
    } else {
        b
    }
}

/// Return the frequency rank of the given byte. The higher the rank, the more
/// common the byte (heuristically speaking).
fn freq_rank(b: u8) -> u8 {
    use crate::util::byte_frequencies::BYTE_FREQUENCIES;
    BYTE_FREQUENCIES[b as usize]
}
