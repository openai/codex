use alloc::collections::BinaryHeap;
use core::cmp::Ord;

pub(crate) fn k_smallest<T: Ord, I: Iterator<Item = T>>(mut iter: I, k: usize) -> BinaryHeap<T> {
    if k == 0 { return BinaryHeap::new(); }

    let mut heap = iter.by_ref().take(k).collect::<BinaryHeap<_>>();

    iter.for_each(|i| {
        debug_assert_eq!(heap.len(), k);
        // Equivalent to heap.push(min(i, heap.pop())) but more efficient.
        // This should be done with a single `.peek_mut().unwrap()` but
        //  `PeekMut` sifts-down unconditionally on Rust 1.46.0 and prior.
        if *heap.peek().unwrap() > i {
            *heap.peek_mut().unwrap() = i;
        }
    });

    heap
}
