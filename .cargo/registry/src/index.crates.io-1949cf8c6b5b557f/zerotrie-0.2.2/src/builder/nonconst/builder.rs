// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use core::cmp::Ordering;

use super::super::branch_meta::BranchMeta;
use super::store::NonConstLengthsStack;
use super::store::TrieBuilderStore;
use crate::builder::bytestr::ByteStr;
use crate::byte_phf::PerfectByteHashMapCacheOwned;
use crate::error::ZeroTrieBuildError;
use crate::options::*;
use crate::varint;
use alloc::borrow::Cow;
use alloc::vec::Vec;

/// A low-level builder for ZeroTrie. Supports all options.
pub(crate) struct ZeroTrieBuilder<S> {
    data: S,
    phf_cache: PerfectByteHashMapCacheOwned,
    options: ZeroTrieBuilderOptions,
}

impl<S: TrieBuilderStore> ZeroTrieBuilder<S> {
    /// Returns the trie data as a `Vec<u8>`.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.data.atbs_to_bytes()
    }

    /// Prepends a byte value to the front of the builder. If it is ASCII, an ASCII
    /// node is prepended. If it is non-ASCII, if there is already a span node at
    /// the front, we modify the span node to add the new byte; otherwise, we create
    /// a new span node. Returns the delta in length, which is either 1 or 2.
    fn prepend_ascii(&mut self, ascii: u8) -> Result<usize, ZeroTrieBuildError> {
        if ascii <= 127 {
            self.data.atbs_push_front(ascii);
            Ok(1)
        } else if matches!(self.options.ascii_mode, AsciiMode::BinarySpans) {
            if let Some(old_front) = self.data.atbs_pop_front() {
                let old_byte_len = self.data.atbs_len() + 1;
                if old_front & 0b11100000 == 0b10100000 {
                    // Extend an existing span
                    // Unwrap OK: there is a varint at this location in the buffer
                    #[allow(clippy::unwrap_used)]
                    let old_span_size =
                        varint::try_read_varint_meta3_from_tstore(old_front, &mut self.data)
                            .unwrap();
                    self.data.atbs_push_front(ascii);
                    let varint_array = varint::write_varint_meta3(old_span_size + 1);
                    self.data.atbs_extend_front(varint_array.as_slice());
                    self.data.atbs_bitor_assign(0, 0b10100000);
                    let new_byte_len = self.data.atbs_len();
                    return Ok(new_byte_len - old_byte_len);
                } else {
                    self.data.atbs_push_front(old_front);
                }
            }
            // Create a new span
            self.data.atbs_push_front(ascii);
            self.data.atbs_push_front(0b10100001);
            Ok(2)
        } else {
            Err(ZeroTrieBuildError::NonAsciiError)
        }
    }

    /// Prepends a value node to the front of the builder. Returns the
    /// delta in length, which depends on the size of the varint.
    #[must_use]
    fn prepend_value(&mut self, value: usize) -> usize {
        let varint_array = varint::write_varint_meta3(value);
        self.data.atbs_extend_front(varint_array.as_slice());
        self.data.atbs_bitor_assign(0, 0b10000000);
        varint_array.len()
    }

    /// Prepends a branch node to the front of the builder. Returns the
    /// delta in length, which depends on the size of the varint.
    #[must_use]
    fn prepend_branch(&mut self, value: usize) -> usize {
        let varint_array = varint::write_varint_meta2(value);
        self.data.atbs_extend_front(varint_array.as_slice());
        self.data.atbs_bitor_assign(0, 0b11000000);
        varint_array.len()
    }

    /// Prepends multiple arbitrary bytes to the front of the builder. Returns the
    /// delta in length, which is the length of the slice.
    #[must_use]
    fn prepend_slice(&mut self, s: &[u8]) -> usize {
        self.data.atbs_extend_front(s);
        s.len()
    }

    /// Builds a ZeroTrie from an iterator of bytes. It first collects and sorts the iterator.
    pub fn from_bytes_iter<K: AsRef<[u8]>, I: IntoIterator<Item = (K, usize)>>(
        iter: I,
        options: ZeroTrieBuilderOptions,
    ) -> Result<Self, ZeroTrieBuildError> {
        let items = Vec::<(K, usize)>::from_iter(iter);
        let mut items = items
            .iter()
            .map(|(k, v)| (k.as_ref(), *v))
            .collect::<Vec<(&[u8], usize)>>();
        items.sort_by(|a, b| cmp_keys_values(options, *a, *b));
        let ascii_str_slice = items.as_slice();
        let byte_str_slice = ByteStr::from_byte_slice_with_value(ascii_str_slice);
        Self::from_sorted_tuple_slice_impl(byte_str_slice, options)
    }

    /// Builds a ZeroTrie with the given items and options. Assumes that the items are sorted,
    /// except for a case-insensitive trie where the items are re-sorted.
    ///
    /// # Panics
    ///
    /// May panic if the items are not sorted.
    pub fn from_sorted_tuple_slice(
        items: &[(&ByteStr, usize)],
        options: ZeroTrieBuilderOptions,
    ) -> Result<Self, ZeroTrieBuildError> {
        let mut items = Cow::Borrowed(items);
        if matches!(options.case_sensitivity, CaseSensitivity::IgnoreCase) {
            // We need to re-sort the items with our custom comparator.
            items.to_mut().sort_by(|a, b| {
                cmp_keys_values(options, (a.0.as_bytes(), a.1), (b.0.as_bytes(), b.1))
            });
        }
        Self::from_sorted_tuple_slice_impl(&items, options)
    }

    /// Internal constructor that does not re-sort the items.
    fn from_sorted_tuple_slice_impl(
        items: &[(&ByteStr, usize)],
        options: ZeroTrieBuilderOptions,
    ) -> Result<Self, ZeroTrieBuildError> {
        for ab in items.windows(2) {
            debug_assert!(cmp_keys_values(
                options,
                (ab[0].0.as_bytes(), ab[0].1),
                (ab[1].0.as_bytes(), ab[1].1)
            )
            .is_lt());
        }
        let mut result = Self {
            data: S::atbs_new_empty(),
            phf_cache: PerfectByteHashMapCacheOwned::new_empty(),
            options,
        };
        let total_size = result.create(items)?;
        debug_assert!(total_size == result.data.atbs_len());
        Ok(result)
    }

    /// The actual builder algorithm. For an explanation, see [`crate::builder`].
    #[allow(clippy::unwrap_used)] // lots of indexing, but all indexes should be in range
    fn create(&mut self, all_items: &[(&ByteStr, usize)]) -> Result<usize, ZeroTrieBuildError> {
        let mut prefix_len = match all_items.last() {
            Some(x) => x.0.len(),
            // Empty slice:
            None => return Ok(0),
        };
        // Initialize the main loop to point at the last string.
        let mut lengths_stack = NonConstLengthsStack::new();
        let mut i = all_items.len() - 1;
        let mut j = all_items.len();
        let mut current_len = 0;
        // Start the main loop.
        loop {
            let item_i = all_items.get(i).unwrap();
            let item_j = all_items.get(j - 1).unwrap();
            debug_assert!(item_i.0.prefix_eq(item_j.0, prefix_len));
            // Check if we need to add a value node here.
            if item_i.0.len() == prefix_len {
                let len = self.prepend_value(item_i.1);
                current_len += len;
            }
            if prefix_len == 0 {
                // All done! Leave the main loop.
                break;
            }
            // Reduce the prefix length by 1 and recalculate i and j.
            prefix_len -= 1;
            let mut new_i = i;
            let mut new_j = j;
            let mut ascii_i = item_i.0.byte_at_or_panic(prefix_len);
            let mut ascii_j = item_j.0.byte_at_or_panic(prefix_len);
            debug_assert_eq!(ascii_i, ascii_j);
            let key_ascii = ascii_i;
            loop {
                if new_i == 0 {
                    break;
                }
                let candidate = all_items.get(new_i - 1).unwrap().0;
                if candidate.len() < prefix_len {
                    // Too short
                    break;
                }
                if item_i.0.prefix_eq(candidate, prefix_len) {
                    new_i -= 1;
                } else {
                    break;
                }
                if candidate.len() == prefix_len {
                    // A string that equals the prefix does not take part in the branch node.
                    break;
                }
                let candidate = candidate.byte_at_or_panic(prefix_len);
                if candidate != ascii_i {
                    ascii_i = candidate;
                }
            }
            loop {
                if new_j == all_items.len() {
                    break;
                }
                let candidate = all_items.get(new_j).unwrap().0;
                if candidate.len() < prefix_len {
                    // Too short
                    break;
                }
                if item_j.0.prefix_eq(candidate, prefix_len) {
                    new_j += 1;
                } else {
                    break;
                }
                if candidate.len() == prefix_len {
                    panic!("A shorter string should be earlier in the sequence");
                }
                let candidate = candidate.byte_at_or_panic(prefix_len);
                if candidate != ascii_j {
                    ascii_j = candidate;
                }
            }
            // If there are no different bytes at this prefix level, we can add an ASCII or Span
            // node and then continue to the next iteration of the main loop.
            if ascii_i == key_ascii && ascii_j == key_ascii {
                let len = self.prepend_ascii(key_ascii)?;
                current_len += len;
                if matches!(self.options.case_sensitivity, CaseSensitivity::IgnoreCase)
                    && i == new_i + 2
                {
                    // This can happen if two strings were picked up, each with a different case
                    return Err(ZeroTrieBuildError::MixedCase);
                }
                debug_assert!(
                    i == new_i || i == new_i + 1,
                    "only the exact prefix string can be picked up at this level: {}",
                    key_ascii
                );
                i = new_i;
                debug_assert_eq!(j, new_j);
                continue;
            }
            // If i and j changed, we are a target of a branch node.
            if ascii_j == key_ascii {
                // We are the _last_ target of a branch node.
                lengths_stack.push(BranchMeta {
                    ascii: key_ascii,
                    cumulative_length: current_len,
                    local_length: current_len,
                    count: 1,
                });
            } else {
                // We are the _not the last_ target of a branch node.
                let BranchMeta {
                    cumulative_length,
                    count,
                    ..
                } = lengths_stack.peek_or_panic();
                lengths_stack.push(BranchMeta {
                    ascii: key_ascii,
                    cumulative_length: cumulative_length + current_len,
                    local_length: current_len,
                    count: count + 1,
                });
            }
            if ascii_i != key_ascii {
                // We are _not the first_ target of a branch node.
                // Set the cursor to the previous string and continue the loop.
                j = i;
                i -= 1;
                prefix_len = all_items.get(i).unwrap().0.len();
                current_len = 0;
                continue;
            }
            // Branch (first)
            // std::println!("lengths_stack: {lengths_stack:?}");
            let (total_length, total_count) = {
                let BranchMeta {
                    cumulative_length,
                    count,
                    ..
                } = lengths_stack.peek_or_panic();
                (cumulative_length, count)
            };
            let mut branch_metas = lengths_stack.pop_many_or_panic(total_count);
            let original_keys = branch_metas.map_to_ascii_bytes();
            if matches!(self.options.case_sensitivity, CaseSensitivity::IgnoreCase) {
                // Check to see if we have the same letter in two different cases
                let mut seen_ascii_alpha = [false; 26];
                for c in original_keys.as_const_slice().as_slice() {
                    if c.is_ascii_alphabetic() {
                        let i = (c.to_ascii_lowercase() - b'a') as usize;
                        if seen_ascii_alpha[i] {
                            return Err(ZeroTrieBuildError::MixedCase);
                        } else {
                            seen_ascii_alpha[i] = true;
                        }
                    }
                }
            }
            let use_phf = matches!(self.options.phf_mode, PhfMode::UsePhf);
            let opt_phf_vec = if total_count > 15 && use_phf {
                let phf_vec = self
                    .phf_cache
                    .try_get_or_insert(original_keys.as_const_slice().as_slice().to_vec())?;
                // Put everything in order via bubble sort
                // Note: branch_metas is stored in reverse order (0 = last element)
                loop {
                    let mut l = total_count - 1;
                    let mut changes = 0;
                    let mut start = 0;
                    while l > 0 {
                        let a = *branch_metas.as_const_slice().get_or_panic(l);
                        let b = *branch_metas.as_const_slice().get_or_panic(l - 1);
                        let a_idx = phf_vec.keys().iter().position(|x| x == &a.ascii).unwrap();
                        let b_idx = phf_vec.keys().iter().position(|x| x == &b.ascii).unwrap();
                        if a_idx > b_idx {
                            // std::println!("{a:?} <=> {b:?} ({phf_vec:?})");
                            self.data.atbs_swap_ranges(
                                start,
                                start + a.local_length,
                                start + a.local_length + b.local_length,
                            );
                            branch_metas = branch_metas.swap_or_panic(l - 1, l);
                            start += b.local_length;
                            changes += 1;
                            // FIXME: fix the `length` field
                        } else {
                            start += a.local_length;
                        }
                        l -= 1;
                    }
                    if changes == 0 {
                        break;
                    }
                }
                Some(phf_vec)
            } else {
                None
            };
            // Write out the offset table
            current_len = total_length;
            const USIZE_BITS: usize = core::mem::size_of::<usize>() * 8;
            let w = (USIZE_BITS - (total_length.leading_zeros() as usize) - 1) / 8;
            if w > 3 && matches!(self.options.capacity_mode, CapacityMode::Normal) {
                return Err(ZeroTrieBuildError::CapacityExceeded);
            }
            let mut k = 0;
            while k <= w {
                self.data.atbs_prepend_n_zeros(total_count - 1);
                current_len += total_count - 1;
                let mut l = 0;
                let mut length_to_write = 0;
                while l < total_count {
                    let BranchMeta { local_length, .. } = *branch_metas
                        .as_const_slice()
                        .get_or_panic(total_count - l - 1);
                    let mut adjusted_length = length_to_write;
                    let mut m = 0;
                    while m < k {
                        adjusted_length >>= 8;
                        m += 1;
                    }
                    if l > 0 {
                        self.data.atbs_bitor_assign(l - 1, adjusted_length as u8);
                    }
                    l += 1;
                    length_to_write += local_length;
                }
                k += 1;
            }
            // Write out the lookup table
            assert!(0 < total_count && total_count <= 256);
            let branch_value = (w << 8) + (total_count & 0xff);
            if let Some(phf_vec) = opt_phf_vec {
                self.data.atbs_extend_front(phf_vec.as_bytes());
                let phf_len = phf_vec.as_bytes().len();
                let branch_len = self.prepend_branch(branch_value);
                current_len += phf_len + branch_len;
            } else {
                let search_len = self.prepend_slice(original_keys.as_slice());
                let branch_len = self.prepend_branch(branch_value);
                current_len += search_len + branch_len;
            }
            i = new_i;
            j = new_j;
        }
        assert!(lengths_stack.is_empty());
        Ok(current_len)
    }
}

fn cmp_keys_values(
    options: ZeroTrieBuilderOptions,
    a: (&[u8], usize),
    b: (&[u8], usize),
) -> Ordering {
    if matches!(options.case_sensitivity, CaseSensitivity::Sensitive) {
        a.0.cmp(b.0)
    } else {
        let a_iter = a.0.iter().map(|x| x.to_ascii_lowercase());
        let b_iter = b.0.iter().map(|x| x.to_ascii_lowercase());
        Iterator::cmp(a_iter, b_iter)
    }
    .then_with(|| a.1.cmp(&b.1))
}
