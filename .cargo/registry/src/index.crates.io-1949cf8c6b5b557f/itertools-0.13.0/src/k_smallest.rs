use alloc::vec::Vec;
use core::cmp::Ordering;

/// Consumes a given iterator, returning the minimum elements in **ascending** order.
pub(crate) fn k_smallest_general<I, F>(iter: I, k: usize, mut comparator: F) -> Vec<I::Item>
where
    I: Iterator,
    F: FnMut(&I::Item, &I::Item) -> Ordering,
{
    /// Sift the element currently at `origin` away from the root until it is properly ordered.
    ///
    /// This will leave **larger** elements closer to the root of the heap.
    fn sift_down<T, F>(heap: &mut [T], is_less_than: &mut F, mut origin: usize)
    where
        F: FnMut(&T, &T) -> bool,
    {
        #[inline]
        fn children_of(n: usize) -> (usize, usize) {
            (2 * n + 1, 2 * n + 2)
        }

        while origin < heap.len() {
            let (left_idx, right_idx) = children_of(origin);
            if left_idx >= heap.len() {
                return;
            }

            let replacement_idx =
                if right_idx < heap.len() && is_less_than(&heap[left_idx], &heap[right_idx]) {
                    right_idx
                } else {
                    left_idx
                };

            if is_less_than(&heap[origin], &heap[replacement_idx]) {
                heap.swap(origin, replacement_idx);
                origin = replacement_idx;
            } else {
                return;
            }
        }
    }

    if k == 0 {
        iter.last();
        return Vec::new();
    }
    if k == 1 {
        return iter.min_by(comparator).into_iter().collect();
    }
    let mut iter = iter.fuse();
    let mut storage: Vec<I::Item> = iter.by_ref().take(k).collect();

    let mut is_less_than = move |a: &_, b: &_| comparator(a, b) == Ordering::Less;

    // Rearrange the storage into a valid heap by reordering from the second-bottom-most layer up to the root.
    // Slightly faster than ordering on each insert, but only by a factor of lg(k).
    // The resulting heap has the **largest** item on top.
    for i in (0..=(storage.len() / 2)).rev() {
        sift_down(&mut storage, &mut is_less_than, i);
    }

    iter.for_each(|val| {
        debug_assert_eq!(storage.len(), k);
        if is_less_than(&val, &storage[0]) {
            // Treating this as an push-and-pop saves having to write a sift-up implementation.
            // https://en.wikipedia.org/wiki/Binary_heap#Insert_then_extract
            storage[0] = val;
            // We retain the smallest items we've seen so far, but ordered largest first so we can drop the largest efficiently.
            sift_down(&mut storage, &mut is_less_than, 0);
        }
    });

    // Ultimately the items need to be in least-first, strict order, but the heap is currently largest-first.
    // To achieve this, repeatedly,
    // 1) "pop" the largest item off the heap into the tail slot of the underlying storage,
    // 2) shrink the logical size of the heap by 1,
    // 3) restore the heap property over the remaining items.
    let mut heap = &mut storage[..];
    while heap.len() > 1 {
        let last_idx = heap.len() - 1;
        heap.swap(0, last_idx);
        // Sifting over a truncated slice means that the sifting will not disturb already popped elements.
        heap = &mut heap[..last_idx];
        sift_down(heap, &mut is_less_than, 0);
    }

    storage
}

#[inline]
pub(crate) fn key_to_cmp<T, K, F>(mut key: F) -> impl FnMut(&T, &T) -> Ordering
where
    F: FnMut(&T) -> K,
    K: Ord,
{
    move |a, b| key(a).cmp(&key(b))
}
