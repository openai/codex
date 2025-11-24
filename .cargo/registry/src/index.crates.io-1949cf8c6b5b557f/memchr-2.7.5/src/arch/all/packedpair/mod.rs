/*!
Provides an architecture independent implementation of the "packed pair"
algorithm.

The "packed pair" algorithm is based on the [generic SIMD] algorithm. The main
difference is that it (by default) uses a background distribution of byte
frequencies to heuristically select the pair of bytes to search for. Note that
this module provides an architecture independent version that doesn't do as
good of a job keeping the search for candidates inside a SIMD hot path. It
however can be good enough in many circumstances.

[generic SIMD]: http://0x80.pl/articles/simd-strfind.html#first-and-last
*/

use crate::memchr;

mod default_rank;

/// An architecture independent "packed pair" finder.
///
/// This finder picks two bytes that it believes have high predictive power for
/// indicating an overall match of a needle. At search time, it reports offsets
/// where the needle could match based on whether the pair of bytes it chose
/// match.
///
/// This is architecture independent because it utilizes `memchr` to find the
/// occurrence of one of the bytes in the pair, and then checks whether the
/// second byte matches. If it does, in the case of [`Finder::find_prefilter`],
/// the location at which the needle could match is returned.
///
/// It is generally preferred to use architecture specific routines for a
/// "packed pair" prefilter, but this can be a useful fallback when the
/// architecture independent routines are unavailable.
#[derive(Clone, Copy, Debug)]
pub struct Finder {
    pair: Pair,
    byte1: u8,
    byte2: u8,
}

impl Finder {
    /// Create a new prefilter that reports possible locations where the given
    /// needle matches.
    #[inline]
    pub fn new(needle: &[u8]) -> Option<Finder> {
        Finder::with_pair(needle, Pair::new(needle)?)
    }

    /// Create a new prefilter using the pair given.
    ///
    /// If the prefilter could not be constructed, then `None` is returned.
    ///
    /// This constructor permits callers to control precisely which pair of
    /// bytes is used as a predicate.
    #[inline]
    pub fn with_pair(needle: &[u8], pair: Pair) -> Option<Finder> {
        let byte1 = needle[usize::from(pair.index1())];
        let byte2 = needle[usize::from(pair.index2())];
        // Currently this can never fail so we could just return a Finder,
        // but it's conceivable this could change.
        Some(Finder { pair, byte1, byte2 })
    }

    /// Run this finder on the given haystack as a prefilter.
    ///
    /// If a candidate match is found, then an offset where the needle *could*
    /// begin in the haystack is returned.
    #[inline]
    pub fn find_prefilter(&self, haystack: &[u8]) -> Option<usize> {
        let mut i = 0;
        let index1 = usize::from(self.pair.index1());
        let index2 = usize::from(self.pair.index2());
        loop {
            // Use a fast vectorized implementation to skip to the next
            // occurrence of the rarest byte (heuristically chosen) in the
            // needle.
            i += memchr(self.byte1, &haystack[i..])?;
            let found = i;
            i += 1;

            // If we can't align our first byte match with the haystack, then a
            // match is impossible.
            let aligned1 = match found.checked_sub(index1) {
                None => continue,
                Some(aligned1) => aligned1,
            };

            // Now align the second byte match with the haystack. A mismatch
            // means that a match is impossible.
            let aligned2 = match aligned1.checked_add(index2) {
                None => continue,
                Some(aligned_index2) => aligned_index2,
            };
            if haystack.get(aligned2).map_or(true, |&b| b != self.byte2) {
                continue;
            }

            // We've done what we can. There might be a match here.
            return Some(aligned1);
        }
    }

    /// Returns the pair of offsets (into the needle) used to check as a
    /// predicate before confirming whether a needle exists at a particular
    /// position.
    #[inline]
    pub fn pair(&self) -> &Pair {
        &self.pair
    }
}

/// A pair of byte offsets into a needle to use as a predicate.
///
/// This pair is used as a predicate to quickly filter out positions in a
/// haystack in which a needle cannot match. In some cases, this pair can even
/// be used in vector algorithms such that the vector algorithm only switches
/// over to scalar code once this pair has been found.
///
/// A pair of offsets can be used in both substring search implementations and
/// in prefilters. The former will report matches of a needle in a haystack
/// where as the latter will only report possible matches of a needle.
///
/// The offsets are limited each to a maximum of 255 to keep memory usage low.
/// Moreover, it's rarely advantageous to create a predicate using offsets
/// greater than 255 anyway.
///
/// The only guarantee enforced on the pair of offsets is that they are not
/// equivalent. It is not necessarily the case that `index1 < index2` for
/// example. By convention, `index1` corresponds to the byte in the needle
/// that is believed to be most the predictive. Note also that because of the
/// requirement that the indices be both valid for the needle used to build
/// the pair and not equal, it follows that a pair can only be constructed for
/// needles with length at least 2.
#[derive(Clone, Copy, Debug)]
pub struct Pair {
    index1: u8,
    index2: u8,
}

impl Pair {
    /// Create a new pair of offsets from the given needle.
    ///
    /// If a pair could not be created (for example, if the needle is too
    /// short), then `None` is returned.
    ///
    /// This chooses the pair in the needle that is believed to be as
    /// predictive of an overall match of the needle as possible.
    #[inline]
    pub fn new(needle: &[u8]) -> Option<Pair> {
        Pair::with_ranker(needle, DefaultFrequencyRank)
    }

    /// Create a new pair of offsets from the given needle and ranker.
    ///
    /// This permits the caller to choose a background frequency distribution
    /// with which bytes are selected. The idea is to select a pair of bytes
    /// that is believed to strongly predict a match in the haystack. This
    /// usually means selecting bytes that occur rarely in a haystack.
    ///
    /// If a pair could not be created (for example, if the needle is too
    /// short), then `None` is returned.
    #[inline]
    pub fn with_ranker<R: HeuristicFrequencyRank>(
        needle: &[u8],
        ranker: R,
    ) -> Option<Pair> {
        if needle.len() <= 1 {
            return None;
        }
        // Find the rarest two bytes. We make them distinct indices by
        // construction. (The actual byte value may be the same in degenerate
        // cases, but that's OK.)
        let (mut rare1, mut index1) = (needle[0], 0);
        let (mut rare2, mut index2) = (needle[1], 1);
        if ranker.rank(rare2) < ranker.rank(rare1) {
            core::mem::swap(&mut rare1, &mut rare2);
            core::mem::swap(&mut index1, &mut index2);
        }
        let max = usize::from(core::u8::MAX);
        for (i, &b) in needle.iter().enumerate().take(max).skip(2) {
            if ranker.rank(b) < ranker.rank(rare1) {
                rare2 = rare1;
                index2 = index1;
                rare1 = b;
                index1 = u8::try_from(i).unwrap();
            } else if b != rare1 && ranker.rank(b) < ranker.rank(rare2) {
                rare2 = b;
                index2 = u8::try_from(i).unwrap();
            }
        }
        // While not strictly required for how a Pair is normally used, we
        // really don't want these to be equivalent. If they were, it would
        // reduce the effectiveness of candidate searching using these rare
        // bytes by increasing the rate of false positives.
        assert_ne!(index1, index2);
        Some(Pair { index1, index2 })
    }

    /// Create a new pair using the offsets given for the needle given.
    ///
    /// This bypasses any sort of heuristic process for choosing the offsets
    /// and permits the caller to choose the offsets themselves.
    ///
    /// Indices are limited to valid `u8` values so that a `Pair` uses less
    /// memory. It is not possible to create a `Pair` with offsets bigger than
    /// `u8::MAX`. It's likely that such a thing is not needed, but if it is,
    /// it's suggested to build your own bespoke algorithm because you're
    /// likely working on a very niche case. (File an issue if this suggestion
    /// does not make sense to you.)
    ///
    /// If a pair could not be created (for example, if the needle is too
    /// short), then `None` is returned.
    #[inline]
    pub fn with_indices(
        needle: &[u8],
        index1: u8,
        index2: u8,
    ) -> Option<Pair> {
        // While not strictly required for how a Pair is normally used, we
        // really don't want these to be equivalent. If they were, it would
        // reduce the effectiveness of candidate searching using these rare
        // bytes by increasing the rate of false positives.
        if index1 == index2 {
            return None;
        }
        // Similarly, invalid indices means the Pair is invalid too.
        if usize::from(index1) >= needle.len() {
            return None;
        }
        if usize::from(index2) >= needle.len() {
            return None;
        }
        Some(Pair { index1, index2 })
    }

    /// Returns the first offset of the pair.
    #[inline]
    pub fn index1(&self) -> u8 {
        self.index1
    }

    /// Returns the second offset of the pair.
    #[inline]
    pub fn index2(&self) -> u8 {
        self.index2
    }
}

/// This trait allows the user to customize the heuristic used to determine the
/// relative frequency of a given byte in the dataset being searched.
///
/// The use of this trait can have a dramatic impact on performance depending
/// on the type of data being searched. The details of why are explained in the
/// docs of [`crate::memmem::Prefilter`]. To summarize, the core algorithm uses
/// a prefilter to quickly identify candidate matches that are later verified
/// more slowly. This prefilter is implemented in terms of trying to find
/// `rare` bytes at specific offsets that will occur less frequently in the
/// dataset. While the concept of a `rare` byte is similar for most datasets,
/// there are some specific datasets (like binary executables) that have
/// dramatically different byte distributions. For these datasets customizing
/// the byte frequency heuristic can have a massive impact on performance, and
/// might even need to be done at runtime.
///
/// The default implementation of `HeuristicFrequencyRank` reads from the
/// static frequency table defined in `src/memmem/byte_frequencies.rs`. This
/// is optimal for most inputs, so if you are unsure of the impact of using a
/// custom `HeuristicFrequencyRank` you should probably just use the default.
///
/// # Example
///
/// ```
/// use memchr::{
///     arch::all::packedpair::HeuristicFrequencyRank,
///     memmem::FinderBuilder,
/// };
///
/// /// A byte-frequency table that is good for scanning binary executables.
/// struct Binary;
///
/// impl HeuristicFrequencyRank for Binary {
///     fn rank(&self, byte: u8) -> u8 {
///         const TABLE: [u8; 256] = [
///             255, 128, 61, 43, 50, 41, 27, 28, 57, 15, 21, 13, 24, 17, 17,
///             89, 58, 16, 11, 7, 14, 23, 7, 6, 24, 9, 6, 5, 9, 4, 7, 16,
///             68, 11, 9, 6, 88, 7, 4, 4, 23, 9, 4, 8, 8, 5, 10, 4, 30, 11,
///             9, 24, 11, 5, 5, 5, 19, 11, 6, 17, 9, 9, 6, 8,
///             48, 58, 11, 14, 53, 40, 9, 9, 254, 35, 3, 6, 52, 23, 6, 6, 27,
///             4, 7, 11, 14, 13, 10, 11, 11, 5, 2, 10, 16, 12, 6, 19,
///             19, 20, 5, 14, 16, 31, 19, 7, 14, 20, 4, 4, 19, 8, 18, 20, 24,
///             1, 25, 19, 58, 29, 10, 5, 15, 20, 2, 2, 9, 4, 3, 5,
///             51, 11, 4, 53, 23, 39, 6, 4, 13, 81, 4, 186, 5, 67, 3, 2, 15,
///             0, 0, 1, 3, 2, 0, 0, 5, 0, 0, 0, 2, 0, 0, 0,
///             12, 2, 1, 1, 3, 1, 1, 1, 6, 1, 2, 1, 3, 1, 1, 2, 9, 1, 1, 0,
///             2, 2, 4, 4, 11, 6, 7, 3, 6, 9, 4, 5,
///             46, 18, 8, 18, 17, 3, 8, 20, 16, 10, 3, 7, 175, 4, 6, 7, 13,
///             3, 7, 3, 3, 1, 3, 3, 10, 3, 1, 5, 2, 0, 1, 2,
///             16, 3, 5, 1, 6, 1, 1, 2, 58, 20, 3, 14, 12, 2, 1, 3, 16, 3, 5,
///             8, 3, 1, 8, 6, 17, 6, 5, 3, 8, 6, 13, 175,
///         ];
///         TABLE[byte as usize]
///     }
/// }
/// // Create a new finder with the custom heuristic.
/// let finder = FinderBuilder::new()
///     .build_forward_with_ranker(Binary, b"\x00\x00\xdd\xdd");
/// // Find needle with custom heuristic.
/// assert!(finder.find(b"\x00\x00\x00\xdd\xdd").is_some());
/// ```
pub trait HeuristicFrequencyRank {
    /// Return the heuristic frequency rank of the given byte. A lower rank
    /// means the byte is believed to occur less frequently in the haystack.
    ///
    /// Some uses of this heuristic may treat arbitrary absolute rank values as
    /// significant. For example, an implementation detail in this crate may
    /// determine that heuristic prefilters are inappropriate if every byte in
    /// the needle has a "high" rank.
    fn rank(&self, byte: u8) -> u8;
}

/// The default byte frequency heuristic that is good for most haystacks.
pub(crate) struct DefaultFrequencyRank;

impl HeuristicFrequencyRank for DefaultFrequencyRank {
    fn rank(&self, byte: u8) -> u8 {
        self::default_rank::RANK[usize::from(byte)]
    }
}

/// This permits passing any implementation of `HeuristicFrequencyRank` as a
/// borrowed version of itself.
impl<'a, R> HeuristicFrequencyRank for &'a R
where
    R: HeuristicFrequencyRank,
{
    fn rank(&self, byte: u8) -> u8 {
        (**self).rank(byte)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_packedpair() {
        fn find(
            haystack: &[u8],
            needle: &[u8],
            _index1: u8,
            _index2: u8,
        ) -> Option<Option<usize>> {
            // We ignore the index positions requested since it winds up making
            // this test too slow overall.
            let f = Finder::new(needle)?;
            Some(f.find_prefilter(haystack))
        }
        crate::tests::packedpair::Runner::new().fwd(find).run()
    }
}
