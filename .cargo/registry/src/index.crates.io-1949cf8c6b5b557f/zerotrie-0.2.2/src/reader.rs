// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! # Internal layout of ZeroTrie
//!
//! A ZeroTrie is composed of a series of nodes stored in sequence in a byte slice.
//!
//! There are 4 types of nodes:
//!
//! 1. ASCII (`0xxxxxxx`): matches a literal ASCII byte.
//! 2. Span (`101xxxxx`): matches a span of non-ASCII bytes.
//! 3. Value (`100xxxxx`): associates a value with a string
//! 4. Branch (`11xxxxxx`): matches one of a set of bytes.
//!
//! Span, Value, and Branch nodes contain a varint, which has different semantics for each:
//!
//! - Span varint: length of the span
//! - Value varint: value associated with the string
//! - Branch varint: number of edges in the branch and width of the offset table
//!
//! If reading an ASCII, Span, or Branch node, one or more bytes are consumed from the input
//! string. If the next byte(s) in the input string do not match the node, we return `None`.
//! If reading a Value node, if the string is empty, return `Some(value)`; otherwise, we skip
//! the Value node and continue on to the next node.
//!
//! When a node is consumed, a shorter, well-formed ZeroTrie remains.
//!
//! ### Basic Example
//!
//! Here is an example ZeroTrie without branch nodes:
//!
//! ```
//! use zerotrie::ZeroTriePerfectHash;
//!
//! let bytes = [
//!     b'a',       // ASCII literal
//!     0b10001010, // value 10
//!     b'b',       // ASCII literal
//!     0b10100011, // span of 3
//!     0x81,       // first byte in span
//!     0x91,       // second byte in span
//!     0xA1,       // third and final byte in span
//!     0b10000100, // value 4
//! ];
//!
//! let trie = ZeroTriePerfectHash::from_bytes(&bytes);
//!
//! // First value: "a" → 10
//! assert_eq!(trie.get(b"a"), Some(10));
//!
//! // Second value: "ab\x81\x91\xA1" → 4
//! assert_eq!(trie.get(b"ab\x81\x91\xA1"), Some(4));
//!
//! // A few examples of strings that do NOT have values in the trie:
//! assert_eq!(trie.get(b"ab"), None);
//! assert_eq!(trie.get(b"b"), None);
//! assert_eq!(trie.get(b"b\x81\x91\xA1"), None);
//! ```
//!
//! ## Branch Nodes
//!
//! There are two types of branch nodes: binary search and perfect hash. `ZeroTrieSimpleAscii`
//! contains only binary search nodes, whereas `ZeroTriePerfectHash` can contain either.
//!
//! The head node of the branch has a varint that encodes two things:
//!
//! - Bottom 8 bits: number of edges in the branch (`N`); if N = 0, set N to 256
//! - Bits 9 and 10: width of the offset table (`W`)
//!
//! Note that N is always in the range [1, 256]. There can't be more than 256 edges because
//! there are only 256 unique u8 values.
//!
//! A few examples of the head node of the branch:
//!
//! - `0b11000000`: varint bits `0`: N = 0 which means N = 256; W = 0
//! - `0b11000110`: varint bits `110`: N = 6; W = 0
//! - `0b11100000 0b00000101`: varint bits `1000101`: N = 69; W = 0
//! - `0b11100010 0b00000000`: varint bits `101000000`: N = 64; W = 1
//!
//! In `ZeroTriePerfectHash`, if N <= 15, the branch is assumed to be a binary search, and if
//! N > 15, the branch is assumed to be a perfect hash.
//!
//! ### Binary Search Branch Nodes
//!
//! A binary search branch node is used when:
//!
//! 1. The trie is a `ZeroTrieSimpleAscii`, OR
//! 2. There are 15 or fewer items in the branch.
//!
//! The head branch node is followed by N sorted bytes. When evaluating a branch node, one byte
//! is consumed from the input. If it is one of the N sorted bytes (scanned using binary search),
//! the index `i` of the byte within the list is used to index into the offset table (described
//! below). If the byte is not in the list, the string is not in the trie, so return `None`.
//!
//! ### Perfect Hash Branch Nodes
//!
//! A perfect hash branch node is used when:
//!
//! 1. The trie is NOT a `ZeroTrieSimpleAscii`, AND
//! 2. There are 16 or more items in the branch.
//!
//! The head branch node is followed by 1 byte containing parameter `p`, N bytes containing
//! parameters `q`, and N bytes containing the bytes to match. From these parameters, either an
//! index within the hash table `i` is resolved and used as input to index into the offset
//! table (described below), or the value is determined to not be present and `None` is
//! returned. For more detail on resolving the perfect hash function, see [`crate::byte_phf`].
//!
//! ### Offset Tables
//!
//! The _offset table_ encodes the range of the remaining buffer containing the trie reachable
//! from the byte matched in the branch node. Both types of branch nodes include an offset
//! table followig the key lookup. Given the index `i` from the first step, the range
//! `[s_i, s_(i+1))` brackets the next step in the trie.
//!
//! Offset tables utilize the `W` parameter stored in the branch head node. The special case
//! when `W == 0`, with `N - 1` bytes, is easiest to understand:
//!
//! **Offset table, W = 0:** `[s_1, s_2, ..., s_(N-1)]`
//!
//! Note that `s_0` is always 0 and `s_N` is always the length of the remaining slice, so those
//! values are not explicitly included in the offset table.
//!
//! When W > 0, the high and low bits of the offsets are in separate bytes, arranged as follows:
//!
//! **Generalized offset table:** `[a_1, a_2, ..., a_(N-1), b_1, b_2, ..., b_(N-1), c_1, ...]`
//!
//! where `s_i = (a_i << 8 + b_i) << 8 + c_i ...` (high bits first, low bits last)
//!
//! ### Advanced Example
//!
//! The following trie encodes the following map. It has multiple varints and branch nodes, which
//! are all binary search with W = 0. Note that there is a value for the empty string.
//!
//! - "" → 0
//! - "axb" → 100
//! - "ayc" → 2
//! - "azd" → 3
//! - "bxe" → 4
//! - "bxefg" → 500
//! - "bxefh" → 6
//! - "bxei" → 7
//! - "bxeikl" → 8
//!
//! ```
//! use zerotrie::ZeroTrieSimpleAscii;
//!
//! let bytes = [
//!     0b10000000, // value 0
//!     0b11000010, // branch of 2
//!     b'a',       //
//!     b'b',       //
//!     13,         //
//!     0b11000011, // start of 'a' subtree: branch of 3
//!     b'x',       //
//!     b'y',       //
//!     b'z',       //
//!     3,          //
//!     5,          //
//!     b'b',       //
//!     0b10010000, // value 100 (lead)
//!     0x54,       // value 100 (trail)
//!     b'c',       //
//!     0b10000010, // value 2
//!     b'd',       //
//!     0b10000011, // value 3
//!     b'x',       // start of 'b' subtree
//!     b'e',       //
//!     0b10000100, // value 4
//!     0b11000010, // branch of 2
//!     b'f',       //
//!     b'i',       //
//!     7,          //
//!     0b11000010, // branch of 2
//!     b'g',       //
//!     b'h',       //
//!     2,          //
//!     0b10010011, // value 500 (lead)
//!     0x64,       // value 500 (trail)
//!     0b10000110, // value 6
//!     0b10000111, // value 7
//!     b'k',       //
//!     b'l',       //
//!     0b10001000, // value 8
//! ];
//!
//! let trie = ZeroTrieSimpleAscii::from_bytes(&bytes);
//!
//! // Assert that the specified items are in the map
//! assert_eq!(trie.get(b""), Some(0));
//! assert_eq!(trie.get(b"axb"), Some(100));
//! assert_eq!(trie.get(b"ayc"), Some(2));
//! assert_eq!(trie.get(b"azd"), Some(3));
//! assert_eq!(trie.get(b"bxe"), Some(4));
//! assert_eq!(trie.get(b"bxefg"), Some(500));
//! assert_eq!(trie.get(b"bxefh"), Some(6));
//! assert_eq!(trie.get(b"bxei"), Some(7));
//! assert_eq!(trie.get(b"bxeikl"), Some(8));
//!
//! // Assert that some other items are not in the map
//! assert_eq!(trie.get(b"a"), None);
//! assert_eq!(trie.get(b"bx"), None);
//! assert_eq!(trie.get(b"xba"), None);
//! ```

use crate::byte_phf::PerfectByteHashMap;
use crate::cursor::AsciiProbeResult;
use crate::helpers::*;
use crate::options::*;
use crate::varint::read_varint_meta2;
use crate::varint::read_varint_meta3;

#[cfg(feature = "alloc")]
use alloc::string::String;

/// Given a slice starting with an offset table, returns the trie for the given index.
///
/// Arguments:
/// - `trie` = a trie pointing at an offset table (after the branch node and search table)
/// - `i` = the desired index within the offset table
/// - `n` = the number of items in the offset table
/// - `w` = the width of the offset table items minus one
#[inline]
fn get_branch(mut trie: &[u8], i: usize, n: usize, mut w: usize) -> &[u8] {
    let mut p = 0usize;
    let mut q = 0usize;
    loop {
        let indices;
        (indices, trie) = trie.debug_split_at(n - 1);
        p = (p << 8)
            + if i == 0 {
                0
            } else {
                *indices.get(i - 1).debug_unwrap_or(&0) as usize
            };
        q = match indices.get(i) {
            Some(x) => (q << 8) + *x as usize,
            None => trie.len(),
        };
        if w == 0 {
            break;
        }
        w -= 1;
    }
    trie.get(p..q).debug_unwrap_or(&[])
}

/// Version of [`get_branch()`] specialized for the case `w == 0` for performance
#[inline]
fn get_branch_w0(mut trie: &[u8], i: usize, n: usize) -> &[u8] {
    let indices;
    (indices, trie) = trie.debug_split_at(n - 1);
    let p = if i == 0 {
        0
    } else {
        *indices.get(i - 1).debug_unwrap_or(&0) as usize
    };
    let q = match indices.get(i) {
        Some(x) => *x as usize,
        None => trie.len(),
    };
    trie.get(p..q).debug_unwrap_or(&[])
}

/// The node type. See the module-level docs for more explanation of the four node types.
enum NodeType {
    /// An ASCII node. Contains a single literal ASCII byte and no varint.
    Ascii,
    /// A span node. Contains a varint indicating how big the span is.
    Span,
    /// A value node. Contains a varint representing the value.
    Value,
    /// A branch node. Contains a varint of the number of output nodes, plus W in the high bits.
    Branch,
}

impl core::fmt::Debug for NodeType {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use NodeType::*;
        f.write_str(match *self {
            Ascii => "a",
            Span => "s",
            Value => "v",
            Branch => "m",
        })
    }
}

#[inline]
fn byte_type(b: u8) -> NodeType {
    match b & 0b11100000 {
        0b10000000 => NodeType::Value,
        0b10100000 => NodeType::Span,
        0b11000000 => NodeType::Branch,
        0b11100000 => NodeType::Branch,
        _ => NodeType::Ascii,
    }
}

#[inline]
pub(crate) fn get_parameterized<T: ZeroTrieWithOptions + ?Sized>(
    mut trie: &[u8],
    mut ascii: &[u8],
) -> Option<usize> {
    loop {
        let (b, x, i, search);
        (b, trie) = trie.split_first()?;
        let byte_type = byte_type(*b);
        (x, trie) = match byte_type {
            NodeType::Ascii => (0, trie),
            NodeType::Span => {
                if matches!(T::OPTIONS.ascii_mode, AsciiMode::BinarySpans) {
                    read_varint_meta3(*b, trie)
                } else {
                    debug_assert!(false, "Span node found in ASCII trie!");
                    return None;
                }
            }
            NodeType::Value => read_varint_meta3(*b, trie),
            NodeType::Branch => read_varint_meta2(*b, trie),
        };
        if let Some((c, temp)) = ascii.split_first() {
            if matches!(byte_type, NodeType::Ascii) {
                let is_match = if matches!(T::OPTIONS.case_sensitivity, CaseSensitivity::IgnoreCase)
                {
                    b.eq_ignore_ascii_case(c)
                } else {
                    b == c
                };
                if is_match {
                    // Matched a byte
                    ascii = temp;
                    continue;
                } else {
                    // Byte that doesn't match
                    return None;
                }
            }
            if matches!(byte_type, NodeType::Value) {
                // Value node, but not at end of string
                continue;
            }
            if matches!(T::OPTIONS.ascii_mode, AsciiMode::BinarySpans)
                && matches!(byte_type, NodeType::Span)
            {
                let (trie_span, ascii_span);
                (trie_span, trie) = trie.debug_split_at(x);
                (ascii_span, ascii) = ascii.split_at_checked(x)?;
                if trie_span == ascii_span {
                    // Matched a byte span
                    continue;
                } else {
                    // Byte span that doesn't match
                    return None;
                }
            }
            // Branch node
            let (x, w) = if x >= 256 { (x & 0xff, x >> 8) } else { (x, 0) };
            let w = if matches!(T::OPTIONS.capacity_mode, CapacityMode::Extended) {
                w
            } else {
                // See the table below regarding this assertion
                debug_assert!(w <= 3, "get: w > 3 but we assume w <= 3");
                w & 0x3
            };
            let x = if x == 0 { 256 } else { x };
            if matches!(T::OPTIONS.phf_mode, PhfMode::BinaryOnly) || x < 16 {
                // binary search
                (search, trie) = trie.debug_split_at(x);
                let bsearch_result =
                    if matches!(T::OPTIONS.case_sensitivity, CaseSensitivity::IgnoreCase) {
                        search.binary_search_by_key(&c.to_ascii_lowercase(), |x| {
                            x.to_ascii_lowercase()
                        })
                    } else {
                        search.binary_search(c)
                    };
                i = bsearch_result.ok()?;
            } else {
                // phf
                (search, trie) = trie.debug_split_at(x * 2 + 1);
                i = PerfectByteHashMap::from_store(search).get(*c)?;
            }
            trie = if w == 0 {
                get_branch_w0(trie, i, x)
            } else {
                get_branch(trie, i, x, w)
            };
            ascii = temp;
            continue;
        } else {
            if matches!(byte_type, NodeType::Value) {
                // Value node at end of string
                return Some(x);
            }
            return None;
        }
    }
}

// DISCUSS: This function is 7% faster *on aarch64* if we assert a max on w.
//
// | Bench         | No Assert, x86_64 | No Assert, aarch64 | Assertion, x86_64 | Assertion, aarch64 |
// |---------------|-------------------|--------------------|-------------------|--------------------|
// | basic         | ~187.51 ns        | ~97.586 ns         | ~199.11 ns        | ~99.236 ns         |
// | subtags_10pct | ~9.5557 µs        | ~4.8696 µs         | ~9.5779 µs        | ~4.5649 µs         |
// | subtags_full  | ~137.75 µs        | ~76.016 µs         | ~142.02 µs        | ~70.254 µs         |

/// Steps one node into the trie assuming all branch nodes are binary search and that
/// there are no span nodes.
///
/// The input-output argument `trie` starts at the original trie and ends pointing to
/// the sub-trie reachable by `c`.
#[inline]
pub(crate) fn step_parameterized<T: ZeroTrieWithOptions + ?Sized>(
    trie: &mut &[u8],
    c: u8,
) -> Option<u8> {
    // Currently, the only option `step_parameterized` supports is `CaseSensitivity::IgnoreCase`.
    // `AsciiMode::BinarySpans` is tricky because the state can no longer be simply a trie.
    // If a span node is encountered, `None` is returned later in this function.
    debug_assert!(
        matches!(T::OPTIONS.ascii_mode, AsciiMode::AsciiOnly),
        "Spans not yet implemented in step function"
    );
    // PHF can be easily implemented but the code is not yet reachable
    debug_assert!(
        matches!(T::OPTIONS.phf_mode, PhfMode::BinaryOnly),
        "PHF not yet implemented in step function"
    );
    // Extended Capacity can be easily implemented but the code is not yet reachable
    debug_assert!(
        matches!(T::OPTIONS.capacity_mode, CapacityMode::Normal),
        "Extended capacity not yet implemented in step function"
    );
    let (mut b, x, search);
    loop {
        (b, *trie) = match trie.split_first() {
            Some(v) => v,
            None => {
                // Empty trie or only a value node
                return None;
            }
        };
        match byte_type(*b) {
            NodeType::Ascii => {
                let is_match = if matches!(T::OPTIONS.case_sensitivity, CaseSensitivity::IgnoreCase)
                {
                    b.eq_ignore_ascii_case(&c)
                } else {
                    *b == c
                };
                if is_match {
                    // Matched a byte
                    return Some(*b);
                } else {
                    // Byte that doesn't match
                    *trie = &[];
                    return None;
                }
            }
            NodeType::Branch => {
                // Proceed to the branch node logic below
                (x, *trie) = read_varint_meta2(*b, trie);
                break;
            }
            NodeType::Span => {
                // Question: Should we put the trie back into a valid state?
                // Currently this code is unreachable so let's not worry about it.
                debug_assert!(false, "Span node found in ASCII trie!");
                return None;
            }
            NodeType::Value => {
                // Skip the value node and go to the next node
                (_, *trie) = read_varint_meta3(*b, trie);
                continue;
            }
        };
    }
    // Branch node
    let (x, w) = if x >= 256 { (x & 0xff, x >> 8) } else { (x, 0) };
    // See comment above regarding this assertion
    debug_assert!(w <= 3, "get: w > 3 but we assume w <= 3");
    let w = w & 0x3;
    let x = if x == 0 { 256 } else { x };
    // Always use binary search
    (search, *trie) = trie.debug_split_at(x);
    let bsearch_result = if matches!(T::OPTIONS.case_sensitivity, CaseSensitivity::IgnoreCase) {
        search.binary_search_by_key(&c.to_ascii_lowercase(), |x| x.to_ascii_lowercase())
    } else {
        search.binary_search(&c)
    };
    match bsearch_result {
        Ok(i) => {
            // Matched a byte
            *trie = if w == 0 {
                get_branch_w0(trie, i, x)
            } else {
                get_branch(trie, i, x, w)
            };
            Some(search[i])
        }
        Err(_) => {
            // Byte that doesn't match
            *trie = &[];
            None
        }
    }
}

/// Steps one node into the trie, assuming all branch nodes are binary search and that
/// there are no span nodes, using an index.
///
/// The input-output argument `trie` starts at the original trie and ends pointing to
/// the sub-trie indexed by `index`.
#[inline]
pub(crate) fn probe_parameterized<T: ZeroTrieWithOptions + ?Sized>(
    trie: &mut &[u8],
    index: usize,
) -> Option<AsciiProbeResult> {
    // Currently, the only option `step_parameterized` supports is `CaseSensitivity::IgnoreCase`.
    // `AsciiMode::BinarySpans` is tricky because the state can no longer be simply a trie.
    // If a span node is encountered, `None` is returned later in this function.
    debug_assert!(
        matches!(T::OPTIONS.ascii_mode, AsciiMode::AsciiOnly),
        "Spans not yet implemented in step function"
    );
    // PHF can be easily implemented but the code is not yet reachable
    debug_assert!(
        matches!(T::OPTIONS.phf_mode, PhfMode::BinaryOnly),
        "PHF not yet implemented in step function"
    );
    // Extended Capacity can be easily implemented but the code is not yet reachable
    debug_assert!(
        matches!(T::OPTIONS.capacity_mode, CapacityMode::Normal),
        "Extended capacity not yet implemented in step function"
    );
    let (mut b, x, search);
    loop {
        (b, *trie) = match trie.split_first() {
            Some(v) => v,
            None => {
                // Empty trie or only a value node
                return None;
            }
        };
        match byte_type(*b) {
            NodeType::Ascii => {
                if index > 0 {
                    *trie = &[];
                    return None;
                }
                return Some(AsciiProbeResult {
                    byte: *b,
                    total_siblings: 1,
                });
            }
            NodeType::Branch => {
                // Proceed to the branch node logic below
                (x, *trie) = read_varint_meta2(*b, trie);
                break;
            }
            NodeType::Span => {
                // Question: Should we put the trie back into a valid state?
                // Currently this code is unreachable so let's not worry about it.
                debug_assert!(false, "Span node found in ASCII trie!");
                return None;
            }
            NodeType::Value => {
                // Skip the value node and go to the next node
                (_, *trie) = read_varint_meta3(*b, trie);
                continue;
            }
        };
    }
    // Branch node
    let (x, w) = if x >= 256 { (x & 0xff, x >> 8) } else { (x, 0) };
    debug_assert!(u8::try_from(x).is_ok());
    let total_siblings = x as u8;
    // See comment above regarding this assertion
    debug_assert!(w <= 3, "get: w > 3 but we assume w <= 3");
    let w = w & 0x3;
    let x = if x == 0 { 256 } else { x };
    if index >= x {
        *trie = &[];
        return None;
    }
    (search, *trie) = trie.debug_split_at(x);
    *trie = if w == 0 {
        get_branch_w0(trie, index, x)
    } else {
        get_branch(trie, index, x, w)
    };
    Some(AsciiProbeResult {
        byte: search[index],
        total_siblings,
    })
}

/// Steps one node into the trie if the head node is a value node, returning the value.
/// If the head node is not a value node, no change is made.
///
/// The input-output argument `trie` starts at the original trie and ends pointing to
/// the sub-trie with the value node removed.
pub(crate) fn take_value(trie: &mut &[u8]) -> Option<usize> {
    let (b, new_trie) = trie.split_first()?;
    match byte_type(*b) {
        NodeType::Ascii | NodeType::Span | NodeType::Branch => None,
        NodeType::Value => {
            let x;
            (x, *trie) = read_varint_meta3(*b, new_trie);
            Some(x)
        }
    }
}

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// Iterator type for walking the byte sequences contained in a ZeroTrie.
#[cfg(feature = "alloc")]
#[derive(Debug)]
pub struct ZeroTrieIterator<'a> {
    /// Whether the PHF is enabled on this trie.
    use_phf: bool,
    /// Intermediate state during iteration:
    /// 1. A trie (usually a slice of the original, bigger trie)
    /// 2. The string that leads to the trie
    /// 3. If the trie's lead node is a branch node, the current index being evaluated
    state: Vec<(&'a [u8], Vec<u8>, usize)>,
}

#[cfg(feature = "alloc")]
impl<'a> ZeroTrieIterator<'a> {
    pub(crate) fn new<S: AsRef<[u8]> + ?Sized>(store: &'a S, use_phf: bool) -> Self {
        ZeroTrieIterator {
            use_phf,
            state: alloc::vec![(store.as_ref(), alloc::vec![], 0)],
        }
    }
}

#[cfg(feature = "alloc")]
impl Iterator for ZeroTrieIterator<'_> {
    type Item = (Vec<u8>, usize);
    fn next(&mut self) -> Option<Self::Item> {
        let (mut trie, mut string, mut branch_idx);
        (trie, string, branch_idx) = self.state.pop()?;
        loop {
            let (b, x, span, search);
            let return_trie = trie;
            (b, trie) = match trie.split_first() {
                Some(tpl) => tpl,
                None => {
                    // At end of current branch; step back to the branch node.
                    // If there are no more branches, we are finished.
                    (trie, string, branch_idx) = self.state.pop()?;
                    continue;
                }
            };
            let byte_type = byte_type(*b);
            if matches!(byte_type, NodeType::Ascii) {
                string.push(*b);
                continue;
            }
            (x, trie) = match byte_type {
                NodeType::Ascii => (0, trie),
                NodeType::Span | NodeType::Value => read_varint_meta3(*b, trie),
                NodeType::Branch => read_varint_meta2(*b, trie),
            };
            if matches!(byte_type, NodeType::Span) {
                (span, trie) = trie.debug_split_at(x);
                string.extend(span);
                continue;
            }
            if matches!(byte_type, NodeType::Value) {
                let retval = string.clone();
                // Return to this position on the next step
                self.state.push((trie, string, 0));
                return Some((retval, x));
            }
            // Match node
            let (x, w) = if x >= 256 { (x & 0xff, x >> 8) } else { (x, 0) };
            let x = if x == 0 { 256 } else { x };
            if branch_idx + 1 < x {
                // Return to this branch node at the next index
                self.state
                    .push((return_trie, string.clone(), branch_idx + 1));
            }
            let byte = if x < 16 || !self.use_phf {
                // binary search
                (search, trie) = trie.debug_split_at(x);
                debug_unwrap!(search.get(branch_idx), return None)
            } else {
                // phf
                (search, trie) = trie.debug_split_at(x * 2 + 1);
                debug_unwrap!(search.get(branch_idx + x + 1), return None)
            };
            string.push(*byte);
            trie = if w == 0 {
                get_branch_w0(trie, branch_idx, x)
            } else {
                get_branch(trie, branch_idx, x, w)
            };
            branch_idx = 0;
        }
    }
}

#[cfg(feature = "alloc")]
pub(crate) fn get_iter_phf<S: AsRef<[u8]> + ?Sized>(store: &S) -> ZeroTrieIterator<'_> {
    ZeroTrieIterator::new(store, true)
}

/// # Panics
/// Panics if the trie contains non-ASCII items.
#[cfg(feature = "alloc")]
#[allow(clippy::type_complexity)]
pub(crate) fn get_iter_ascii_or_panic<S: AsRef<[u8]> + ?Sized>(
    store: &S,
) -> core::iter::Map<ZeroTrieIterator<'_>, fn((Vec<u8>, usize)) -> (String, usize)> {
    ZeroTrieIterator::new(store, false).map(|(k, v)| {
        #[allow(clippy::unwrap_used)] // in signature of function
        let ascii_str = String::from_utf8(k).unwrap();
        (ascii_str, v)
    })
}
