// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use alloc::vec;
use alloc::vec::Vec;
use core::hash::{Hash, Hasher};
use twox_hash::XxHash64;

// Const seed to be used with [`XxHash64::with_seed`].
const SEED: u64 = 0xaabbccdd;

/// Split the 64bit `hash` into (g, f0, f1).
///
/// g denotes the highest 16bits of the hash modulo `m`, and is referred to as first level hash.
/// (f0, f1) denotes the middle, and lower 24bits of the hash respectively.
/// (f0, f1) are used to distribute the keys with same g, into distinct slots.
///
/// # Arguments
///
/// * `hash` - The hash to split.
/// * `m` - The modulo used to split the hash.
pub const fn split_hash64(hash: u64, m: usize) -> (usize, u32, u32) {
    (
        ((hash >> 48) as usize % m),
        ((hash >> 24) as u32 & 0xffffff),
        ((hash & 0xffffff) as u32),
    )
}

/// Compute hash using [`XxHash64`].
pub fn compute_hash<K: Hash + ?Sized>(key: &K) -> u64 {
    let mut hasher = XxHash64::with_seed(SEED);
    key.hash(&mut hasher);
    hasher.finish()
}

/// Calculate the index using (f0, f1), (d0, d1) in modulo m.
/// Returns [`None`] if d is (0, 0) or modulo is 0
/// else returns the index computed using (f0 + f1 * d0 + d1) mod m.
pub fn compute_index(f: (u32, u32), d: (u32, u32), m: u32) -> Option<usize> {
    if d == (0, 0) || m == 0 {
        None
    } else {
        Some((f.1.wrapping_mul(d.0).wrapping_add(f.0).wrapping_add(d.1) % m) as usize)
    }
}

/// Compute displacements for the given `key_hashes`, which split the keys into distinct slots by a
/// two-level hashing schema.
///
/// Returns a tuple of where the first item is the displacement array and the second item is the
/// reverse mapping used to permute keys, values into their slots.
///
/// 1. Split the hashes into (g, f0, f1).
/// 2. Bucket and sort the split hash on g in descending order.
/// 3. In decreasing order of bucket size, try until a (d0, d1) is found that splits the keys
///    in the bucket into distinct slots.
/// 4. Mark the slots for current bucket as occupied and store the reverse mapping.
/// 5. Repeat untill all the keys have been assigned distinct slots.
///
/// # Arguments
///
/// * `key_hashes` - [`ExactSizeIterator`] over the hashed key values
#[expect(clippy::indexing_slicing, clippy::unwrap_used)]
pub fn compute_displacements(
    key_hashes: impl ExactSizeIterator<Item = u64>,
) -> (Vec<(u32, u32)>, Vec<usize>) {
    let len = key_hashes.len();

    // A vector to track the size of buckets for sorting.
    let mut bucket_sizes = vec![0; len];

    // A flattened representation of items in the buckets after applying first level hash function
    let mut bucket_flatten = Vec::with_capacity(len);

    // Compute initial displacement and bucket sizes

    key_hashes.into_iter().enumerate().for_each(|(i, kh)| {
        let h = split_hash64(kh, len);
        bucket_sizes[h.0] += 1;
        bucket_flatten.push((h, i))
    });

    // Sort by decreasing order of bucket_sizes.
    bucket_flatten.sort_by(|&(ha, _), &(hb, _)| {
        // ha.0, hb.0 are always within bounds of `bucket_sizes`
        (bucket_sizes[hb.0], hb).cmp(&(bucket_sizes[ha.0], ha))
    });

    // Generation count while iterating buckets.
    // Each trial of ((d0, d1), bucket chain) is a new generation.
    // We use this to track which all slots are assigned for the current bucket chain.
    let mut generation = 0;

    // Whether a slot has been occupied by previous buckets with a different first level hash (different
    // bucket chain).
    let mut occupied = vec![false; len];

    // Track generation count for the slots.
    // A slot is empty if either it is unoccupied by the previous bucket chains and the
    // assignment is not equal to generation.
    let mut assignments = vec![0; len];

    // Vec to store the displacements (saves us a recomputation of hash while assigning slots).
    let mut current_displacements = Vec::with_capacity(16);

    // (d0, d1) which splits the bucket into different slots
    let mut displacements = vec![(0, 0); len];

    // Vec to store mapping to the original order of keys.
    // This is a permutation which will be applied to keys, values at the end.
    let mut reverse_mapping = vec![0; len];

    let mut start = 0;
    while start < len {
        // Bucket span with the same first level hash
        // start is always within bounds of `bucket_flatten`
        let g = bucket_flatten[start].0 .0;
        // g is always within bounds of `bucket_sizes`
        let end = start + bucket_sizes[g];
        // start, end - 1 are always within bounds of `bucket_sizes`
        let buckets = &bucket_flatten[start..end];

        'd0: for d0 in 0..len as u32 {
            'd1: for d1 in 0..len as u32 {
                if (d0, d1) == (0, 0) {
                    continue;
                }
                current_displacements.clear();
                generation += 1;

                for ((_, f0, f1), _) in buckets {
                    let displacement_idx = compute_index((*f0, *f1), (d0, d1), len as u32).unwrap();

                    // displacement_idx is always within bounds
                    if occupied[displacement_idx] || assignments[displacement_idx] == generation {
                        continue 'd1;
                    }
                    assignments[displacement_idx] = generation;
                    current_displacements.push(displacement_idx);
                }

                // Successfully found a (d0, d1), store it as index g.
                // g < displacements.len() due to modulo operation
                displacements[g] = (d0, d1);

                for (i, displacement_idx) in current_displacements.iter().enumerate() {
                    // `current_displacements` has same size as `buckets`
                    let (_, idx) = &buckets[i];

                    // displacement_idx is always within bounds
                    occupied[*displacement_idx] = true;
                    reverse_mapping[*displacement_idx] = *idx;
                }
                break 'd0;
            }
        }

        start = end;
    }

    (displacements, reverse_mapping)
}
