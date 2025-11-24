/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#[inline(always)]
pub(crate) fn find_hash_in_array_without_simd(array: &[u32], hash: u32) -> Option<usize> {
    let mut i = 0;
    while i < array.len() {
        if array[i] == hash {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[inline]
pub(crate) fn find_hash_in_array(array: &[u32], hash: u32) -> Option<usize> {
    #[cfg(rust_nightly)]
    unsafe {
        // TODO(yurysamkevich): remove conditional compilation after updating rust toolchain
        #[cfg(version("1.76"))]
        use std::simd::cmp::SimdPartialEq;
        use std::simd::*;

        // 128-bit SIMD is available on x86_64 and aarch64.
        // Also shorter SIMD works better for shorter arrays.
        type T = Simd<u32, 4>;

        #[cfg(version("1.76"))]
        const LANES: usize = T::LEN;
        #[cfg(not(version("1.76")))]
        const LANES: usize = T::LANES;

        if array.len() < LANES {
            find_hash_in_array_without_simd(array, hash)
        } else {
            let mut i = 0;
            let hash = T::splat(hash);

            // Process 4 elements at a time except last <= 4 elements.
            while i + LANES < array.len() {
                let next_hashes = T::from_slice(array.get_unchecked(i..i + LANES));
                let eq = next_hashes.simd_eq(hash);
                if eq.any() {
                    return Some(i + eq.to_bitmask().trailing_zeros() as usize);
                }
                i += LANES;
            }

            // Process last <= 4 elements.
            debug_assert!(i >= array.len() - LANES);
            debug_assert!(i < array.len());
            let next_hashes = T::from_slice(array.get_unchecked(array.len() - LANES..));
            let eq = next_hashes.simd_eq(hash);
            if eq.any() {
                Some(array.len() - LANES + eq.to_bitmask().trailing_zeros() as usize)
            } else {
                None
            }
        }
    }
    #[cfg(not(rust_nightly))]
    {
        find_hash_in_array_without_simd(array, hash)
    }
}

#[cfg(test)]
mod tests {
    use crate::vec_map::simd::find_hash_in_array;

    #[test]
    fn test_find_hash_in_array() {
        assert_eq!(None, find_hash_in_array(&[], 77));

        for len in 1..20 {
            let array: Vec<u32> = (0..len).map(|_| 88).collect();
            assert_eq!(None, find_hash_in_array(&array, 77));

            for i in 0..len {
                let array: Vec<u32> = (0..len).map(|j| if j == i { 77 } else { 88 }).collect();
                assert_eq!(Some(i as usize), find_hash_in_array(&array, 77));
            }

            for i in 0..len {
                // Test first index is returned if there are multiple matches.
                let array: Vec<u32> = (0..len).map(|j| if j < i { 88 } else { 77 }).collect();
                assert_eq!(Some(i as usize), find_hash_in_array(&array, 77));
            }
        }
    }
}
