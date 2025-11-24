use regex_automata::{dfa::Automaton, Anchored, Input};

use crate::{
    ext_slice::ByteSlice,
    unicode::fsm::{
        grapheme_break_fwd::GRAPHEME_BREAK_FWD,
        grapheme_break_rev::GRAPHEME_BREAK_REV,
        regional_indicator_rev::REGIONAL_INDICATOR_REV,
    },
    utf8,
};

/// An iterator over grapheme clusters in a byte string.
///
/// This iterator is typically constructed by
/// [`ByteSlice::graphemes`](trait.ByteSlice.html#method.graphemes).
///
/// Unicode defines a grapheme cluster as an *approximation* to a single user
/// visible character. A grapheme cluster, or just "grapheme," is made up of
/// one or more codepoints. For end user oriented tasks, one should generally
/// prefer using graphemes instead of [`Chars`](struct.Chars.html), which
/// always yields one codepoint at a time.
///
/// Since graphemes are made up of one or more codepoints, this iterator yields
/// `&str` elements. When invalid UTF-8 is encountered, replacement codepoints
/// are [substituted](index.html#handling-of-invalid-utf-8).
///
/// This iterator can be used in reverse. When reversed, exactly the same
/// set of grapheme clusters are yielded, but in reverse order.
///
/// This iterator only yields *extended* grapheme clusters, in accordance with
/// [UAX #29](https://www.unicode.org/reports/tr29/tr29-33.html#Grapheme_Cluster_Boundaries).
#[derive(Clone, Debug)]
pub struct Graphemes<'a> {
    bs: &'a [u8],
}

impl<'a> Graphemes<'a> {
    pub(crate) fn new(bs: &'a [u8]) -> Graphemes<'a> {
        Graphemes { bs }
    }

    /// View the underlying data as a subslice of the original data.
    ///
    /// The slice returned has the same lifetime as the original slice, and so
    /// the iterator can continue to be used while this exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use bstr::ByteSlice;
    ///
    /// let mut it = b"abc".graphemes();
    ///
    /// assert_eq!(b"abc", it.as_bytes());
    /// it.next();
    /// assert_eq!(b"bc", it.as_bytes());
    /// it.next();
    /// it.next();
    /// assert_eq!(b"", it.as_bytes());
    /// ```
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bs
    }
}

impl<'a> Iterator for Graphemes<'a> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        let (grapheme, size) = decode_grapheme(self.bs);
        if size == 0 {
            return None;
        }
        self.bs = &self.bs[size..];
        Some(grapheme)
    }
}

impl<'a> DoubleEndedIterator for Graphemes<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a str> {
        let (grapheme, size) = decode_last_grapheme(self.bs);
        if size == 0 {
            return None;
        }
        self.bs = &self.bs[..self.bs.len() - size];
        Some(grapheme)
    }
}

/// An iterator over grapheme clusters in a byte string and their byte index
/// positions.
///
/// This iterator is typically constructed by
/// [`ByteSlice::grapheme_indices`](trait.ByteSlice.html#method.grapheme_indices).
///
/// Unicode defines a grapheme cluster as an *approximation* to a single user
/// visible character. A grapheme cluster, or just "grapheme," is made up of
/// one or more codepoints. For end user oriented tasks, one should generally
/// prefer using graphemes instead of [`Chars`](struct.Chars.html), which
/// always yields one codepoint at a time.
///
/// Since graphemes are made up of one or more codepoints, this iterator
/// yields `&str` elements (along with their start and end byte offsets).
/// When invalid UTF-8 is encountered, replacement codepoints are
/// [substituted](index.html#handling-of-invalid-utf-8). Because of this, the
/// indices yielded by this iterator may not correspond to the length of the
/// grapheme cluster yielded with those indices. For example, when this
/// iterator encounters `\xFF` in the byte string, then it will yield a pair
/// of indices ranging over a single byte, but will provide an `&str`
/// equivalent to `"\u{FFFD}"`, which is three bytes in length. However, when
/// given only valid UTF-8, then all indices are in exact correspondence with
/// their paired grapheme cluster.
///
/// This iterator can be used in reverse. When reversed, exactly the same
/// set of grapheme clusters are yielded, but in reverse order.
///
/// This iterator only yields *extended* grapheme clusters, in accordance with
/// [UAX #29](https://www.unicode.org/reports/tr29/tr29-33.html#Grapheme_Cluster_Boundaries).
#[derive(Clone, Debug)]
pub struct GraphemeIndices<'a> {
    bs: &'a [u8],
    forward_index: usize,
    reverse_index: usize,
}

impl<'a> GraphemeIndices<'a> {
    pub(crate) fn new(bs: &'a [u8]) -> GraphemeIndices<'a> {
        GraphemeIndices { bs, forward_index: 0, reverse_index: bs.len() }
    }

    /// View the underlying data as a subslice of the original data.
    ///
    /// The slice returned has the same lifetime as the original slice, and so
    /// the iterator can continue to be used while this exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use bstr::ByteSlice;
    ///
    /// let mut it = b"abc".grapheme_indices();
    ///
    /// assert_eq!(b"abc", it.as_bytes());
    /// it.next();
    /// assert_eq!(b"bc", it.as_bytes());
    /// it.next();
    /// it.next();
    /// assert_eq!(b"", it.as_bytes());
    /// ```
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bs
    }
}

impl<'a> Iterator for GraphemeIndices<'a> {
    type Item = (usize, usize, &'a str);

    #[inline]
    fn next(&mut self) -> Option<(usize, usize, &'a str)> {
        let index = self.forward_index;
        let (grapheme, size) = decode_grapheme(self.bs);
        if size == 0 {
            return None;
        }
        self.bs = &self.bs[size..];
        self.forward_index += size;
        Some((index, index + size, grapheme))
    }
}

impl<'a> DoubleEndedIterator for GraphemeIndices<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<(usize, usize, &'a str)> {
        let (grapheme, size) = decode_last_grapheme(self.bs);
        if size == 0 {
            return None;
        }
        self.bs = &self.bs[..self.bs.len() - size];
        self.reverse_index -= size;
        Some((self.reverse_index, self.reverse_index + size, grapheme))
    }
}

/// Decode a grapheme from the given byte string.
///
/// This returns the resulting grapheme (which may be a Unicode replacement
/// codepoint if invalid UTF-8 was found), along with the number of bytes
/// decoded in the byte string. The number of bytes decoded may not be the
/// same as the length of grapheme in the case where invalid UTF-8 is found.
pub fn decode_grapheme(bs: &[u8]) -> (&str, usize) {
    if bs.is_empty() {
        ("", 0)
    } else if bs.len() >= 2
        && bs[0].is_ascii()
        && bs[1].is_ascii()
        && !bs[0].is_ascii_whitespace()
    {
        // FIXME: It is somewhat sad that we have to special case this, but it
        // leads to a significant speed up in predominantly ASCII text. The
        // issue here is that the DFA has a bit of overhead, and running it for
        // every byte in mostly ASCII text results in a bit slowdown. We should
        // re-litigate this once regex-automata 0.3 is out, but it might be
        // hard to avoid the special case. A DFA is always going to at least
        // require some memory access.

        // Safe because all ASCII bytes are valid UTF-8.
        let grapheme = unsafe { bs[..1].to_str_unchecked() };
        (grapheme, 1)
    } else if let Some(hm) = {
        let input = Input::new(bs).anchored(Anchored::Yes);
        GRAPHEME_BREAK_FWD.try_search_fwd(&input).unwrap()
    } {
        // Safe because a match can only occur for valid UTF-8.
        let grapheme = unsafe { bs[..hm.offset()].to_str_unchecked() };
        (grapheme, grapheme.len())
    } else {
        const INVALID: &str = "\u{FFFD}";
        // No match on non-empty bytes implies we found invalid UTF-8.
        let (_, size) = utf8::decode_lossy(bs);
        (INVALID, size)
    }
}

fn decode_last_grapheme(bs: &[u8]) -> (&str, usize) {
    if bs.is_empty() {
        ("", 0)
    } else if let Some(hm) = {
        let input = Input::new(bs).anchored(Anchored::Yes);
        GRAPHEME_BREAK_REV.try_search_rev(&input).unwrap()
    } {
        let start = adjust_rev_for_regional_indicator(bs, hm.offset());
        // Safe because a match can only occur for valid UTF-8.
        let grapheme = unsafe { bs[start..].to_str_unchecked() };
        (grapheme, grapheme.len())
    } else {
        const INVALID: &str = "\u{FFFD}";
        // No match on non-empty bytes implies we found invalid UTF-8.
        let (_, size) = utf8::decode_last_lossy(bs);
        (INVALID, size)
    }
}

/// Return the correct offset for the next grapheme decoded at the end of the
/// given byte string, where `i` is the initial guess. In particular,
/// `&bs[i..]` represents the candidate grapheme.
///
/// `i` is returned by this function in all cases except when `&bs[i..]` is
/// a pair of regional indicator codepoints. In that case, if an odd number of
/// additional regional indicator codepoints precedes `i`, then `i` is
/// adjusted such that it points to only a single regional indicator.
///
/// This "fixing" is necessary to handle the requirement that a break cannot
/// occur between regional indicators where it would cause an odd number of
/// regional indicators to exist before the break from the *start* of the
/// string. A reverse regex cannot detect this case easily without look-around.
fn adjust_rev_for_regional_indicator(mut bs: &[u8], i: usize) -> usize {
    // All regional indicators use a 4 byte encoding, and we only care about
    // the case where we found a pair of regional indicators.
    if bs.len() - i != 8 {
        return i;
    }
    // Count all contiguous occurrences of regional indicators. If there's an
    // even number of them, then we can accept the pair we found. Otherwise,
    // we can only take one of them.
    //
    // FIXME: This is quadratic in the worst case, e.g., a string of just
    // regional indicator codepoints. A fix probably requires refactoring this
    // code a bit such that we don't rescan regional indicators.
    let mut count = 0;
    while let Some(hm) = {
        let input = Input::new(bs).anchored(Anchored::Yes);
        REGIONAL_INDICATOR_REV.try_search_rev(&input).unwrap()
    } {
        bs = &bs[..hm.offset()];
        count += 1;
    }
    if count % 2 == 0 {
        i
    } else {
        i + 4
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use alloc::{
        string::{String, ToString},
        vec,
        vec::Vec,
    };

    #[cfg(not(miri))]
    use ucd_parse::GraphemeClusterBreakTest;

    use crate::tests::LOSSY_TESTS;

    use super::*;

    #[test]
    #[cfg(not(miri))]
    fn forward_ucd() {
        for (i, test) in ucdtests().into_iter().enumerate() {
            let given = test.grapheme_clusters.concat();
            let got: Vec<String> = Graphemes::new(given.as_bytes())
                .map(|cluster| cluster.to_string())
                .collect();
            assert_eq!(
                test.grapheme_clusters,
                got,
                "\ngrapheme forward break test {} failed:\n\
                 given:    {:?}\n\
                 expected: {:?}\n\
                 got:      {:?}\n",
                i,
                uniescape(&given),
                uniescape_vec(&test.grapheme_clusters),
                uniescape_vec(&got),
            );
        }
    }

    #[test]
    #[cfg(not(miri))]
    fn reverse_ucd() {
        for (i, test) in ucdtests().into_iter().enumerate() {
            let given = test.grapheme_clusters.concat();
            let mut got: Vec<String> = Graphemes::new(given.as_bytes())
                .rev()
                .map(|cluster| cluster.to_string())
                .collect();
            got.reverse();
            assert_eq!(
                test.grapheme_clusters,
                got,
                "\n\ngrapheme reverse break test {} failed:\n\
                 given:    {:?}\n\
                 expected: {:?}\n\
                 got:      {:?}\n",
                i,
                uniescape(&given),
                uniescape_vec(&test.grapheme_clusters),
                uniescape_vec(&got),
            );
        }
    }

    #[test]
    fn forward_lossy() {
        for &(expected, input) in LOSSY_TESTS {
            let got = Graphemes::new(input.as_bytes()).collect::<String>();
            assert_eq!(expected, got);
        }
    }

    #[test]
    fn reverse_lossy() {
        for &(expected, input) in LOSSY_TESTS {
            let expected: String = expected.chars().rev().collect();
            let got =
                Graphemes::new(input.as_bytes()).rev().collect::<String>();
            assert_eq!(expected, got);
        }
    }

    #[cfg(not(miri))]
    fn uniescape(s: &str) -> String {
        s.chars().flat_map(|c| c.escape_unicode()).collect::<String>()
    }

    #[cfg(not(miri))]
    fn uniescape_vec(strs: &[String]) -> Vec<String> {
        strs.iter().map(|s| uniescape(s)).collect()
    }

    /// Return all of the UCD for grapheme breaks.
    #[cfg(not(miri))]
    fn ucdtests() -> Vec<GraphemeClusterBreakTest> {
        const TESTDATA: &str = include_str!("data/GraphemeBreakTest.txt");

        let mut tests = vec![];
        for mut line in TESTDATA.lines() {
            line = line.trim();
            if line.starts_with("#") || line.contains("surrogate") {
                continue;
            }
            tests.push(line.parse().unwrap());
        }
        tests
    }
}
