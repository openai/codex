// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! # ZeroTrie Builder
//!
//! There are two implementations of the ZeroTrie Builder:
//!
//! - [konst::ZeroTrieBuilderConst] allows for human-readable const construction
//! - [nonconst::ZeroTrieBuilder] has the full feaure set but requires `alloc`
//!
//! The two builders follow the same algorithm but have different capabilities.
//!
//! ## Builder Algorithm Overview
//!
//! The tries are built backwards, from the last node to the first node. The key step of the
//! algorithm is **determining what is the next node to prepend.**
//!
//! In the simple case of [`ZeroTrieSimpleAscii`], all nodes are binary-search, so if the input
//! strings are provided in lexicographic order, there is a simple, deterministic method for
//! identifying the next node. This insight is what enables us to make the const builder.
//!
//! The builder works with the following intermediate state variables:
//!
//! - `prefix_len` indicates the byte index we are currently processing.
//! - `i` and `j` bracket a window of strings in the input that share the same prefix.
//! - `current_len` is the length in bytes of the current self-contained trie.
//! - `lengths_stack` contains metadata for branch nodes.
//!
//! What follows is a verbal explanation of the build steps for a trie containing:
//!
//! - "" → 11
//! - "ad" → 22
//! - "adef" → 33
//! - "adghk" → 44
//!
//! When a node is prepended, it is shown in **boldface**.
//!
//! 1. Initialize the builder by setting `i=3`, `j=4`, `prefix_len=5` (the last string),
//!    `current_len=0`, and `lengths_stack` empty. Start the main loop.
//! 2. Top of loop. The string at `i` is equal in length to `prefix_len`, so we prepend
//!    our first node: a **value node 44**, which requires a 2-byte varint. Increase
//!    `current_len` to 2.
//! 3. Reduce `prefix_len` to 4, read our `key_ascii="k"`, and recalculate `i` and `j`
//!    _(this calculation is a long chunk of code in the builder impls)_. Since there is no
//!    other string with the prefix "adgh", `i` and `j` stay the same, we prepend an
//!    **ASCII node "k"**, increase `current_len` to 3, and continue the main loop.
//! 4. Top of loop. The string at `i` is of length 5, but `prefix_len` is 4, so there is
//!    no value node to prepend.
//! 5. Reduce `prefix_len` to 3, read our `key_ascii="h"`, and recalculate `i` and `j`.
//!    There are no other strings sharing the prefix "abg", so we prepend an
//!    **ASCII node "h"**, increase `current_len` to 4, and continue the main loop.
//! 6. Top of loop. There is still no value node to prepend.
//! 7. Reduce `prefix_len` to 2, read our `key_ascii="g"`, and recalculate `i` and `j`.
//!    We find that `i=1` and `j=4`, the range of strings sharing the prefix "ad". Since
//!    `i` or `j` changed, proceed to evaluate the branch node.
//! 8. The last branch byte `ascii_j` for this prefix is "g", which is the same as `key_ascii`,
//!    so we are the _last_ target of a branch node. Push an entry onto `lengths_stack`:
//!    `BranchMeta { ascii: "g", cumulative_length: 4, local_length: 4, count: 1 }`.
//! 9. The first branch byte `ascii_i` for this prefix is "e", which is NOT equal to `key_ascii`,
//!    so we are _not the first_ target of a branch node. We therefore start evaluating the
//!    string preceding where we were at the top of the current loop. We set `i=2`, `j=3`,
//!    `prefix_len=4` (length of the string at `i`), and continue the main loop.
//! 10. Top of loop. Since the string at `i` is equal in length to `prefix_len`, we prepend a
//!     **value node 33** (which requires a 2-byte varint) and increase `current_len` to 2.
//! 11. Reduce `prefix_len` to 3, read our `key_ascii="f"`, and recalculate `i` and `j`.
//!     They stay the same, so we prepend an **ASCII node "f"**, increase `current_len` to 3,
//!     and continue the main loop.
//! 12. Top of loop. No value node this time.
//! 13. Reduce `prefix_len` to 2, read our `key_ascii="e"`, and recalculate `i` and `j`.
//!     They go back to `i=1` and `j=4`.
//! 14. The last branch byte `ascii_j` for this prefix is "g", which is NOT equal to `key_ascii`,
//!     so we are _not the last_ target of a branch node. We peek at the entry at the front of
//!     the lengths stack and use it to push another entry onto the stack:
//!     `BranchMeta { ascii: "e", cumulative_length: 7, local_length: 3, count: 2 }`
//! 15. The first branch byte `ascii_i` for this prefix is "e", which is the same as `key_ascii`,
//!     wo we are the _first_ target of a branch node. We can therefore proceed to prepend the
//!     metadata for the branch node. We peek at the top of the stack and find that there are 2
//!     tries reachable from this branch and they have a total byte length of 5. We then pull off
//!     2 entries from the stack into a local variable `branch_metas`. From here, we write out
//!     the **offset table**, **lookup table**, and **branch head node**, which are determined
//!     from the metadata entries. We set `current_len` to the length of the two tries plus the
//!     metadata, which happens to be 11. Then we return to the top of the main loop.
//! 16. Top of loop. The string at `i` is length 2, which is the same as `prefix_len`, so we
//!     prepend a **value node 22** (2-byte varint) and increase `current_len` to 13.
//! 17. Reduce `prefix_len` to 1, read our `key_ascii="d"`, and recalculate `i` and `j`.
//!     They stay the same, so we prepend an **ASCII node "d"**, increase `current_len` to 14,
//!     and continue the main loop.
//! 18. Top of loop. No value node this time.
//! 19. Reduce `prefix_len` to 0, read our `key_ascii="a"`, and recalculate `i` and `j`.
//!     They change to `i=0` and `j=4`, since all strings have the empty string as a prefix.
//!     However, `ascii_i` and `ascii_j` both equal `key_ascii`, so we prepend **ASCII node "a"**,
//!     increase `current_len` to 15, and continue the main loop.
//! 16. Top of loop. The string at `i` is length 0, which is the same as `prefix_len`, so we
//!     prepend a **value node 11** and increase `current_len` to 16.
//! 17. We can no longer reduce `prefix_len`, so our trie is complete.
//!
//! ## Perfect Hash Reordering
//!
//! When the PHF is added to the mix, the main change is that the strings are no longer in sorted
//! order when they are in the trie. To resolve this issue, when adding a branch node, the target
//! tries are rearranged in-place in the buffer to be in the correct order for the PHF.
//!
//! ## Example
//!
//! Here is the output of the trie described above.
//!
//! ```
//! use zerotrie::ZeroTrieSimpleAscii;
//!
//! const DATA: [(&str, usize); 4] =
//!     [("", 11), ("ad", 22), ("adef", 33), ("adghk", 44)];
//!
//! // As demonstrated above, the required capacity for this trie is 16 bytes
//! const TRIE: ZeroTrieSimpleAscii<[u8; 16]> =
//!     ZeroTrieSimpleAscii::from_sorted_str_tuples(&DATA);
//!
//! assert_eq!(
//!     TRIE.as_bytes(),
//!     &[
//!         0x8B, // value node 11
//!         b'a', // ASCII node 'a'
//!         b'd', // ASCII node 'd'
//!         0x90, // value node 22 lead byte
//!         0x06, // value node 22 trail byte
//!         0xC2, // branch node 2
//!         b'e', // first target of branch
//!         b'g', // second target of branch
//!         3,    // offset
//!         b'f', // ASCII node 'f'
//!         0x90, // value node 33 lead byte
//!         0x11, // value node 33 trail byte
//!         b'h', // ASCII node 'h'
//!         b'k', // ASCII node 'k'
//!         0x90, // value node 44 lead byte
//!         0x1C, // value node 44 trail byte
//!     ]
//! );
//!
//! assert_eq!(TRIE.get(b""), Some(11));
//! assert_eq!(TRIE.get(b"ad"), Some(22));
//! assert_eq!(TRIE.get(b"adef"), Some(33));
//! assert_eq!(TRIE.get(b"adghk"), Some(44));
//! assert_eq!(TRIE.get(b"unknown"), None);
//! ```

mod branch_meta;
pub(crate) mod bytestr;
pub(crate) mod konst;
#[cfg(feature = "litemap")]
mod litemap;
#[cfg(feature = "alloc")]
pub(crate) mod nonconst;

use bytestr::ByteStr;

use super::ZeroTrieSimpleAscii;

impl<const N: usize> ZeroTrieSimpleAscii<[u8; N]> {
    /// **Const Constructor:** Creates an [`ZeroTrieSimpleAscii`] from a sorted slice of keys and values.
    ///
    /// This function needs to know the exact length of the resulting trie at compile time. To
    /// figure out `N`, first set `N` to be too large (say 0xFFFF), then look at the resulting
    /// compile error which will tell you how to set `N`, like this:
    ///
    /// > the evaluated program panicked at 'Buffer too large. Size needed: 17'
    ///
    /// That error message says you need to set `N` to 17.
    ///
    /// Also see [`Self::from_sorted_str_tuples`].
    ///
    /// # Panics
    ///
    /// Panics if `items` is not sorted or if `N` is not correct.
    ///
    /// # Examples
    ///
    /// Create a `const` ZeroTrieSimpleAscii at compile time:
    ///
    /// ```
    /// use zerotrie::ZeroTrieSimpleAscii;
    ///
    /// // The required capacity for this trie happens to be 17 bytes
    /// const TRIE: ZeroTrieSimpleAscii<[u8; 17]> =
    ///     ZeroTrieSimpleAscii::from_sorted_u8_tuples(&[
    ///         (b"bar", 2),
    ///         (b"bazzoo", 3),
    ///         (b"foo", 1),
    ///     ]);
    ///
    /// assert_eq!(TRIE.get(b"foo"), Some(1));
    /// assert_eq!(TRIE.get(b"bar"), Some(2));
    /// assert_eq!(TRIE.get(b"bazzoo"), Some(3));
    /// assert_eq!(TRIE.get(b"unknown"), None);
    /// ```
    ///
    /// Panics if strings are not sorted:
    ///
    /// ```compile_fail
    /// # use zerotrie::ZeroTrieSimpleAscii;
    /// const TRIE: ZeroTrieSimpleAscii<[u8; 17]> = ZeroTrieSimpleAscii::from_sorted_u8_tuples(&[
    ///     (b"foo", 1),
    ///     (b"bar", 2),
    ///     (b"bazzoo", 3),
    /// ]);
    /// ```
    ///
    /// Panics if capacity is too small:
    ///
    /// ```compile_fail
    /// # use zerotrie::ZeroTrieSimpleAscii;
    /// const TRIE: ZeroTrieSimpleAscii<[u8; 15]> = ZeroTrieSimpleAscii::from_sorted_u8_tuples(&[
    ///     (b"bar", 2),
    ///     (b"bazzoo", 3),
    ///     (b"foo", 1),
    /// ]);
    /// ```
    ///
    /// Panics if capacity is too large:
    ///
    /// ```compile_fail
    /// # use zerotrie::ZeroTrieSimpleAscii;
    /// const TRIE: ZeroTrieSimpleAscii<[u8; 20]> = ZeroTrieSimpleAscii::from_sorted_u8_tuples(&[
    ///     (b"bar", 2),
    ///     (b"bazzoo", 3),
    ///     (b"foo", 1),
    /// ]);
    /// ```
    pub const fn from_sorted_u8_tuples(tuples: &[(&[u8], usize)]) -> Self {
        use konst::*;
        let byte_str_slice = ByteStr::from_byte_slice_with_value(tuples);
        let result = ZeroTrieBuilderConst::<N>::from_tuple_slice::<100>(byte_str_slice);
        match result {
            Ok(s) => Self::from_store(s.build_or_panic()),
            Err(_) => panic!("Failed to build ZeroTrie"),
        }
    }

    /// **Const Constructor:** Creates an [`ZeroTrieSimpleAscii`] from a sorted slice of keys and values.
    ///
    /// This function needs to know the exact length of the resulting trie at compile time. To
    /// figure out `N`, first set `N` to be too large (say 0xFFFF), then look at the resulting
    /// compile error which will tell you how to set `N`, like this:
    ///
    /// > the evaluated program panicked at 'Buffer too large. Size needed: 17'
    ///
    /// That error message says you need to set `N` to 17.
    ///
    /// Also see [`Self::from_sorted_u8_tuples`].
    ///
    /// # Panics
    ///
    /// Panics if `items` is not sorted, if `N` is not correct, or if any of the strings contain
    /// non-ASCII characters.
    ///
    /// # Examples
    ///
    /// Create a `const` ZeroTrieSimpleAscii at compile time:
    ///
    /// ```
    /// use zerotrie::ZeroTrieSimpleAscii;
    ///
    /// // The required capacity for this trie happens to be 17 bytes
    /// const TRIE: ZeroTrieSimpleAscii<[u8; 17]> =
    ///     ZeroTrieSimpleAscii::from_sorted_str_tuples(&[
    ///         ("bar", 2),
    ///         ("bazzoo", 3),
    ///         ("foo", 1),
    ///     ]);
    ///
    /// assert_eq!(TRIE.get(b"foo"), Some(1));
    /// assert_eq!(TRIE.get(b"bar"), Some(2));
    /// assert_eq!(TRIE.get(b"bazzoo"), Some(3));
    /// assert_eq!(TRIE.get(b"unknown"), None);
    /// ```
    ///
    /// Panics if the strings are not ASCII:
    ///
    /// ```compile_fail
    /// # use zerotrie::ZeroTrieSimpleAscii;
    /// const TRIE: ZeroTrieSimpleAscii<[u8; 100]> = ZeroTrieSimpleAscii::from_sorted_str_tuples(&[
    ///     ("bár", 2),
    ///     ("båzzöo", 3),
    ///     ("foo", 1),
    /// ]);
    /// ```
    pub const fn from_sorted_str_tuples(tuples: &[(&str, usize)]) -> Self {
        use konst::*;
        let byte_str_slice = ByteStr::from_str_slice_with_value(tuples);
        // 100 is the value of `K`, the size of the lengths stack. If compile errors are
        // encountered, this number may need to be increased.
        let result = ZeroTrieBuilderConst::<N>::from_tuple_slice::<100>(byte_str_slice);
        match result {
            Ok(s) => Self::from_store(s.build_or_panic()),
            Err(_) => panic!("Failed to build ZeroTrie"),
        }
    }
}
