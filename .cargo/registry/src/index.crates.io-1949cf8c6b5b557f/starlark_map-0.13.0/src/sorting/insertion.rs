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

//! Generic insertion sort (sort arbitrary collections, not just slices).

use std::ptr;

/// Find the insertion point for an element.
///
/// The array is sorted up to `next_unsorted`,
/// and the element to insert is at `next_unsorted`.
fn find_insertion_point<T: ?Sized>(
    array: &mut T,
    next_unsorted: usize,
    mut less: impl FnMut(&mut T, usize, usize) -> bool,
) -> usize {
    let mut i = next_unsorted;
    while i > 0 && less(array, next_unsorted, i - 1) {
        i -= 1;
    }
    i
}

/// `swap_shift` operation for generic `insertion_sort` implemented for slices.
///
/// Index `a` is strictly less than index `b`, and `b` is less than slice length.
/// Perform two operations simultaneously:
/// * Move the element at `b` to `a`
/// * Shift all elements in the range `a..b` one position to the right
pub(crate) fn slice_swap_shift<T>(slice: &mut [T], a: usize, b: usize) {
    // Assert parameters are valid because this function performs unsafe operations.
    assert!(a < b);
    assert!(b < slice.len());

    unsafe {
        let tmp = ptr::read(slice.as_mut_ptr().add(b));
        ptr::copy(slice.as_ptr().add(a), slice.as_mut_ptr().add(a + 1), b - a);
        ptr::write(slice.as_mut_ptr().add(a), tmp);
    }
}

/// Insertion sort for generic collections (not just slices).
pub(crate) fn insertion_sort<A: ?Sized>(
    array: &mut A,
    len: usize,
    mut less: impl FnMut(&mut A, usize, usize) -> bool,
    // Function takes two indices, `a` and `b`. `a` is strictly less than `b`.
    // Function moves the element at `b` to the position at `a`,
    // and simultaneously shifts all elements in the range `a..b` one position to the right.
    mut swap_shift: impl FnMut(&mut A, usize, usize),
) {
    for i in 1..len {
        // At this point, the array is sorted up to `i`.
        let insertion_point = find_insertion_point(array, i, &mut less);
        debug_assert!(insertion_point <= i);
        if insertion_point != i {
            // Move the element at `i` to the insertion point.
            swap_shift(array, insertion_point, i);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::sorting::insertion::find_insertion_point;
    use crate::sorting::insertion::insertion_sort;
    use crate::sorting::insertion::slice_swap_shift;

    #[test]
    fn test_find_insertion_point() {
        fn find_insertion_point_ints(slice: &mut [u32]) -> usize {
            find_insertion_point(slice, slice.len() - 1, |slice, i, j| slice[i] < slice[j])
        }

        assert_eq!(0, find_insertion_point_ints(&mut [2, 4, 6, 0]));
        assert_eq!(0, find_insertion_point_ints(&mut [2, 4, 6, 1]));
        assert_eq!(1, find_insertion_point_ints(&mut [2, 4, 6, 2]));
        assert_eq!(1, find_insertion_point_ints(&mut [2, 4, 6, 3]));
        assert_eq!(2, find_insertion_point_ints(&mut [2, 4, 6, 4]));
        assert_eq!(2, find_insertion_point_ints(&mut [2, 4, 6, 5]));
        assert_eq!(3, find_insertion_point_ints(&mut [2, 4, 6, 6]));
    }

    #[test]
    fn test_insertion_sort() {
        fn insertion_sort_ints(slice: &mut [u32]) {
            insertion_sort(
                slice,
                slice.len(),
                |slice, i, j| {
                    // Compare / 100 to test stability.
                    slice[i] / 100 < slice[j] / 100
                },
                slice_swap_shift,
            );
        }

        let mut slice = [200, 400, 600];
        insertion_sort_ints(&mut slice);
        assert_eq!([200, 400, 600], slice);

        let mut slice = [600, 200, 400];
        insertion_sort_ints(&mut slice);
        assert_eq!([200, 400, 600], slice);

        // Now test sorting is stable.
        let mut slice = [202, 402, 602, 201, 401, 601];
        insertion_sort_ints(&mut slice);
        assert_eq!([202, 201, 402, 401, 602, 601], slice);
    }
}
