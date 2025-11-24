//! [![github]](https://github.com/dtolnay/unicode-ident)&ensp;[![crates-io]](https://crates.io/crates/unicode-ident)&ensp;[![docs-rs]](https://docs.rs/unicode-ident)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! <br>
//!
//! Implementation of [Unicode Standard Annex #31][tr31] for determining which
//! `char` values are valid in programming language identifiers.
//!
//! [tr31]: https://www.unicode.org/reports/tr31/
//!
//! This crate is a better optimized implementation of the older `unicode-xid`
//! crate. This crate uses less static storage, and is able to classify both
//! ASCII and non-ASCII codepoints with better performance, 2&ndash;6&times;
//! faster than `unicode-xid`.
//!
//! <br>
//!
//! ## Comparison of performance
//!
//! The following table shows a comparison between five Unicode identifier
//! implementations.
//!
//! - `unicode-ident` is this crate;
//! - [`unicode-xid`] is a widely used crate run by the "unicode-rs" org;
//! - `ucd-trie` and `fst` are two data structures supported by the
//!   [`ucd-generate`] tool;
//! - [`roaring`] is a Rust implementation of Roaring bitmap.
//!
//! The *static storage* column shows the total size of `static` tables that the
//! crate bakes into your binary, measured in 1000s of bytes.
//!
//! The remaining columns show the **cost per call** to evaluate whether a
//! single `char` has the XID\_Start or XID\_Continue Unicode property,
//! comparing across different ratios of ASCII to non-ASCII codepoints in the
//! input data.
//!
//! [`unicode-xid`]: https://github.com/unicode-rs/unicode-xid
//! [`ucd-generate`]: https://github.com/BurntSushi/ucd-generate
//! [`roaring`]: https://github.com/RoaringBitmap/roaring-rs
//!
//! | | static storage | 0% nonascii | 1% | 10% | 100% nonascii |
//! |---|---|---|---|---|---|
//! | **`unicode-ident`** | 10.4 K | 1.03 ns | 1.02 ns | 1.11 ns | 1.66 ns |
//! | **`unicode-xid`** | 11.8 K | 2.57 ns | 2.74 ns | 3.20 ns | 9.35 ns |
//! | **`ucd-trie`** | 10.3 K | 1.27 ns | 1.27 ns | 1.41 ns | 2.53 ns |
//! | **`fst`** | 144 K | 49.3 ns | 49.1 ns | 47.1 ns | 27.9 ns |
//! | **`roaring`** | 66.1 K | 4.10 ns | 4.05 ns | 4.02 ns | 5.12 ns |
//!
//! Source code for the benchmark is provided in the *bench* directory of this
//! repo and may be repeated by running `cargo criterion`.
//!
//! <br>
//!
//! ## Comparison of data structures
//!
//! #### unicode-xid
//!
//! They use a sorted array of character ranges, and do a binary search to look
//! up whether a given character lands inside one of those ranges.
//!
//! ```rust
//! # const _: &str = stringify! {
//! static XID_Continue_table: [(char, char); 763] = [
//!     ('\u{30}', '\u{39}'),  // 0-9
//!     ('\u{41}', '\u{5a}'),  // A-Z
//! # "
//!     …
//! # "
//!     ('\u{e0100}', '\u{e01ef}'),
//! ];
//! # };
//! ```
//!
//! The static storage used by this data structure scales with the number of
//! contiguous ranges of identifier codepoints in Unicode. Every table entry
//! consumes 8 bytes, because it consists of a pair of 32-bit `char` values.
//!
//! In some ranges of the Unicode codepoint space, this is quite a sparse
//! representation &ndash; there are some ranges where tens of thousands of
//! adjacent codepoints are all valid identifier characters. In other places,
//! the representation is quite inefficient. A characater like `µ` (U+00B5)
//! which is surrounded by non-identifier codepoints consumes 64 bits in the
//! table, while it would be just 1 bit in a dense bitmap.
//!
//! On a system with 64-byte cache lines, binary searching the table touches 7
//! cache lines on average. Each cache line fits only 8 table entries.
//! Additionally, the branching performed during the binary search is probably
//! mostly unpredictable to the branch predictor.
//!
//! Overall, the crate ends up being about 6&times; slower on non-ASCII input
//! compared to the fastest crate.
//!
//! A potential improvement would be to pack the table entries more compactly.
//! Rust's `char` type is a 21-bit integer padded to 32 bits, which means every
//! table entry is holding 22 bits of wasted space, adding up to 3.9 K. They
//! could instead fit every table entry into 6 bytes, leaving out some of the
//! padding, for a 25% improvement in space used. With some cleverness it may be
//! possible to fit in 5 bytes or even 4 bytes by storing a low char and an
//! extent, instead of low char and high char. I don't expect that performance
//! would improve much but this could be the most efficient for space across all
//! the libraries, needing only about 7 K to store.
//!
//! #### ucd-trie
//!
//! Their data structure is a compressed trie set specifically tailored for
//! Unicode codepoints. The design is credited to Raph Levien in
//! [rust-lang/rust#33098].
//!
//! [rust-lang/rust#33098]: https://github.com/rust-lang/rust/pull/33098
//!
//! ```rust
//! pub struct TrieSet {
//!     tree1_level1: &'static [u64; 32],
//!     tree2_level1: &'static [u8; 992],
//!     tree2_level2: &'static [u64],
//!     tree3_level1: &'static [u8; 256],
//!     tree3_level2: &'static [u8],
//!     tree3_level3: &'static [u64],
//! }
//! ```
//!
//! It represents codepoint sets using a trie to achieve prefix compression. The
//! final states of the trie are embedded in leaves or "chunks", where each
//! chunk is a 64-bit integer. Each bit position of the integer corresponds to
//! whether a particular codepoint is in the set or not. These chunks are not
//! just a compact representation of the final states of the trie, but are also
//! a form of suffix compression. In particular, if multiple ranges of 64
//! contiguous codepoints have the same Unicode properties, then they all map to
//! the same chunk in the final level of the trie.
//!
//! Being tailored for Unicode codepoints, this trie is partitioned into three
//! disjoint sets: tree1, tree2, tree3. The first set corresponds to codepoints
//! \[0, 0x800), the second \[0x800, 0x10000) and the third \[0x10000,
//! 0x110000). These partitions conveniently correspond to the space of 1 or 2
//! byte UTF-8 encoded codepoints, 3 byte UTF-8 encoded codepoints and 4 byte
//! UTF-8 encoded codepoints, respectively.
//!
//! Lookups in this data structure are significantly more efficient than binary
//! search. A lookup touches either 1, 2, or 3 cache lines based on which of the
//! trie partitions is being accessed.
//!
//! One possible performance improvement would be for this crate to expose a way
//! to query based on a UTF-8 encoded string, returning the Unicode property
//! corresponding to the first character in the string. Without such an API, the
//! caller is required to tokenize their UTF-8 encoded input data into `char`,
//! hand the `char` into `ucd-trie`, only for `ucd-trie` to undo that work by
//! converting back into the variable-length representation for trie traversal.
//!
//! #### fst
//!
//! Uses a [finite state transducer][fst]. This representation is built into
//! [ucd-generate] but I am not aware of any advantage over the `ucd-trie`
//! representation. In particular `ucd-trie` is optimized for storing Unicode
//! properties while `fst` is not.
//!
//! [fst]: https://github.com/BurntSushi/fst
//! [ucd-generate]: https://github.com/BurntSushi/ucd-generate
//!
//! As far as I can tell, the main thing that causes `fst` to have large size
//! and slow lookups for this use case relative to `ucd-trie` is that it does
//! not specialize for the fact that only 21 of the 32 bits in a `char` are
//! meaningful. There are some dense arrays in the structure with large ranges
//! that could never possibly be used.
//!
//! #### roaring
//!
//! This crate is a pure-Rust implementation of [Roaring Bitmap], a data
//! structure designed for storing sets of 32-bit unsigned integers.
//!
//! [Roaring Bitmap]: https://roaringbitmap.org/about/
//!
//! Roaring bitmaps are compressed bitmaps which tend to outperform conventional
//! compressed bitmaps such as WAH, EWAH or Concise. In some instances, they can
//! be hundreds of times faster and they often offer significantly better
//! compression.
//!
//! In this use case the performance was reasonably competitive but still
//! substantially slower than the Unicode-optimized crates. Meanwhile the
//! compression was significantly worse, requiring 6&times; as much storage for
//! the data structure.
//!
//! I also benchmarked the [`croaring`] crate which is an FFI wrapper around the
//! C reference implementation of Roaring Bitmap. This crate was consistently
//! about 15% slower than pure-Rust `roaring`, which could just be FFI overhead.
//! I did not investigate further.
//!
//! [`croaring`]: https://crates.io/crates/croaring
//!
//! #### unicode-ident
//!
//! This crate is most similar to the `ucd-trie` library, in that it's based on
//! bitmaps stored in the leafs of a trie representation, achieving both prefix
//! compression and suffix compression.
//!
//! The key differences are:
//!
//! - Uses a single 2-level trie, rather than 3 disjoint partitions of different
//!   depth each.
//! - Uses significantly larger chunks: 512 bits rather than 64 bits.
//! - Compresses the XID\_Start and XID\_Continue properties together
//!   simultaneously, rather than duplicating identical trie leaf chunks across
//!   the two.
//!
//! The following diagram show the XID\_Start and XID\_Continue Unicode boolean
//! properties in uncompressed form, in row-major order:
//!
//! <table>
//! <tr><th>XID_Start</th><th>XID_Continue</th></tr>
//! <tr>
//! <td><img alt="XID_Start bitmap" width="256" src="https://user-images.githubusercontent.com/1940490/168647353-c6eeb922-afec-49b2-9ef5-c03e9d1e0760.png"></td>
//! <td><img alt="XID_Continue bitmap" width="256" src="https://user-images.githubusercontent.com/1940490/168647367-f447cca7-2362-4d7d-8cd7-d21c011d329b.png"></td>
//! </tr>
//! </table>
//!
//! Uncompressed, these would take 140 K to store, which is beyond what would be
//! reasonable. However, as you can see there is a large degree of similarity
//! between the two bitmaps and across the rows, which lends well to
//! compression.
//!
//! This crate stores one 512-bit "row" of the above bitmaps in the leaf level
//! of a trie, and a single additional level to index into the leafs. It turns
//! out there are 124 unique 512-bit chunks across the two bitmaps so 7 bits are
//! sufficient to index them.
//!
//! The chunk size of 512 bits is selected as the size that minimizes the total
//! size of the data structure. A smaller chunk, like 256 or 128 bits, would
//! achieve better deduplication but require a larger index. A larger chunk
//! would increase redundancy in the leaf bitmaps. 512 bit chunks are the
//! optimum for total size of the index plus leaf bitmaps.
//!
//! In fact since there are only 124 unique chunks, we can use an 8-bit index
//! with a spare bit to index at the half-chunk level. This achieves an
//! additional 8.5% compression by eliminating redundancies between the second
//! half of any chunk and the first half of any other chunk. Note that this is
//! not the same as using chunks which are half the size, because it does not
//! necessitate raising the size of the trie's first level.
//!
//! In contrast to binary search or the `ucd-trie` crate, performing lookups in
//! this data structure is straight-line code with no need for branching.

#![no_std]
#![doc(html_root_url = "https://docs.rs/unicode-ident/1.0.18")]
#![allow(clippy::doc_markdown, clippy::must_use_candidate)]

#[rustfmt::skip]
mod tables;

use crate::tables::{ASCII_CONTINUE, ASCII_START, CHUNK, LEAF, TRIE_CONTINUE, TRIE_START};

/// Whether the character has the Unicode property XID\_Start.
pub fn is_xid_start(ch: char) -> bool {
    if ch.is_ascii() {
        return ASCII_START.0[ch as usize];
    }
    let chunk = *TRIE_START.0.get(ch as usize / 8 / CHUNK).unwrap_or(&0);
    let offset = chunk as usize * CHUNK / 2 + ch as usize / 8 % CHUNK;
    unsafe { LEAF.0.get_unchecked(offset) }.wrapping_shr(ch as u32 % 8) & 1 != 0
}

/// Whether the character has the Unicode property XID\_Continue.
pub fn is_xid_continue(ch: char) -> bool {
    if ch.is_ascii() {
        return ASCII_CONTINUE.0[ch as usize];
    }
    let chunk = *TRIE_CONTINUE.0.get(ch as usize / 8 / CHUNK).unwrap_or(&0);
    let offset = chunk as usize * CHUNK / 2 + ch as usize / 8 % CHUNK;
    unsafe { LEAF.0.get_unchecked(offset) }.wrapping_shr(ch as u32 % 8) & 1 != 0
}
