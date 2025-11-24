use crate::size_hint;
use crate::Itertools;

use alloc::vec::Vec;
use std::fmt;
use std::iter::FusedIterator;
use std::mem::replace;

/// Head element and Tail iterator pair
///
/// `PartialEq`, `Eq`, `PartialOrd` and `Ord` are implemented by comparing sequences based on
/// first items (which are guaranteed to exist).
///
/// The meanings of `PartialOrd` and `Ord` are reversed so as to turn the heap used in
/// `KMerge` into a min-heap.
#[derive(Debug)]
struct HeadTail<I>
where
    I: Iterator,
{
    head: I::Item,
    tail: I,
}

impl<I> HeadTail<I>
where
    I: Iterator,
{
    /// Constructs a `HeadTail` from an `Iterator`. Returns `None` if the `Iterator` is empty.
    fn new(mut it: I) -> Option<Self> {
        let head = it.next();
        head.map(|h| Self { head: h, tail: it })
    }

    /// Get the next element and update `head`, returning the old head in `Some`.
    ///
    /// Returns `None` when the tail is exhausted (only `head` then remains).
    fn next(&mut self) -> Option<I::Item> {
        if let Some(next) = self.tail.next() {
            Some(replace(&mut self.head, next))
        } else {
            None
        }
    }

    /// Hints at the size of the sequence, same as the `Iterator` method.
    fn size_hint(&self) -> (usize, Option<usize>) {
        size_hint::add_scalar(self.tail.size_hint(), 1)
    }
}

impl<I> Clone for HeadTail<I>
where
    I: Iterator + Clone,
    I::Item: Clone,
{
    clone_fields!(head, tail);
}

/// Make `data` a heap (min-heap w.r.t the sorting).
fn heapify<T, S>(data: &mut [T], mut less_than: S)
where
    S: FnMut(&T, &T) -> bool,
{
    for i in (0..data.len() / 2).rev() {
        sift_down(data, i, &mut less_than);
    }
}

/// Sift down element at `index` (`heap` is a min-heap wrt the ordering)
fn sift_down<T, S>(heap: &mut [T], index: usize, mut less_than: S)
where
    S: FnMut(&T, &T) -> bool,
{
    debug_assert!(index <= heap.len());
    let mut pos = index;
    let mut child = 2 * pos + 1;
    // Require the right child to be present
    // This allows to find the index of the smallest child without a branch
    // that wouldn't be predicted if present
    while child + 1 < heap.len() {
        // pick the smaller of the two children
        // use arithmetic to avoid an unpredictable branch
        child += less_than(&heap[child + 1], &heap[child]) as usize;

        // sift down is done if we are already in order
        if !less_than(&heap[child], &heap[pos]) {
            return;
        }
        heap.swap(pos, child);
        pos = child;
        child = 2 * pos + 1;
    }
    // Check if the last (left) child was an only child
    // if it is then it has to be compared with the parent
    if child + 1 == heap.len() && less_than(&heap[child], &heap[pos]) {
        heap.swap(pos, child);
    }
}

/// An iterator adaptor that merges an abitrary number of base iterators in ascending order.
/// If all base iterators are sorted (ascending), the result is sorted.
///
/// Iterator element type is `I::Item`.
///
/// See [`.kmerge()`](crate::Itertools::kmerge) for more information.
pub type KMerge<I> = KMergeBy<I, KMergeByLt>;

pub trait KMergePredicate<T> {
    fn kmerge_pred(&mut self, a: &T, b: &T) -> bool;
}

#[derive(Clone, Debug)]
pub struct KMergeByLt;

impl<T: PartialOrd> KMergePredicate<T> for KMergeByLt {
    fn kmerge_pred(&mut self, a: &T, b: &T) -> bool {
        a < b
    }
}

impl<T, F: FnMut(&T, &T) -> bool> KMergePredicate<T> for F {
    fn kmerge_pred(&mut self, a: &T, b: &T) -> bool {
        self(a, b)
    }
}

/// Create an iterator that merges elements of the contained iterators using
/// the ordering function.
///
/// [`IntoIterator`] enabled version of [`Itertools::kmerge`].
///
/// ```
/// use itertools::kmerge;
///
/// for elt in kmerge(vec![vec![0, 2, 4], vec![1, 3, 5], vec![6, 7]]) {
///     /* loop body */
/// }
/// ```
pub fn kmerge<I>(iterable: I) -> KMerge<<I::Item as IntoIterator>::IntoIter>
where
    I: IntoIterator,
    I::Item: IntoIterator,
    <<I as IntoIterator>::Item as IntoIterator>::Item: PartialOrd,
{
    kmerge_by(iterable, KMergeByLt)
}

/// An iterator adaptor that merges an abitrary number of base iterators
/// according to an ordering function.
///
/// Iterator element type is `I::Item`.
///
/// See [`.kmerge_by()`](crate::Itertools::kmerge_by) for more
/// information.
#[must_use = "this iterator adaptor is not lazy but does nearly nothing unless consumed"]
pub struct KMergeBy<I, F>
where
    I: Iterator,
{
    heap: Vec<HeadTail<I>>,
    less_than: F,
}

impl<I, F> fmt::Debug for KMergeBy<I, F>
where
    I: Iterator + fmt::Debug,
    I::Item: fmt::Debug,
{
    debug_fmt_fields!(KMergeBy, heap);
}

/// Create an iterator that merges elements of the contained iterators.
///
/// [`IntoIterator`] enabled version of [`Itertools::kmerge_by`].
pub fn kmerge_by<I, F>(
    iterable: I,
    mut less_than: F,
) -> KMergeBy<<I::Item as IntoIterator>::IntoIter, F>
where
    I: IntoIterator,
    I::Item: IntoIterator,
    F: KMergePredicate<<<I as IntoIterator>::Item as IntoIterator>::Item>,
{
    let iter = iterable.into_iter();
    let (lower, _) = iter.size_hint();
    let mut heap: Vec<_> = Vec::with_capacity(lower);
    heap.extend(iter.filter_map(|it| HeadTail::new(it.into_iter())));
    heapify(&mut heap, |a, b| less_than.kmerge_pred(&a.head, &b.head));
    KMergeBy { heap, less_than }
}

impl<I, F> Clone for KMergeBy<I, F>
where
    I: Iterator + Clone,
    I::Item: Clone,
    F: Clone,
{
    clone_fields!(heap, less_than);
}

impl<I, F> Iterator for KMergeBy<I, F>
where
    I: Iterator,
    F: KMergePredicate<I::Item>,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if self.heap.is_empty() {
            return None;
        }
        let result = if let Some(next) = self.heap[0].next() {
            next
        } else {
            self.heap.swap_remove(0).head
        };
        let less_than = &mut self.less_than;
        sift_down(&mut self.heap, 0, |a, b| {
            less_than.kmerge_pred(&a.head, &b.head)
        });
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        #[allow(deprecated)] //TODO: once msrv hits 1.51. replace `fold1` with `reduce`
        self.heap
            .iter()
            .map(|i| i.size_hint())
            .fold1(size_hint::add)
            .unwrap_or((0, Some(0)))
    }
}

impl<I, F> FusedIterator for KMergeBy<I, F>
where
    I: Iterator,
    F: KMergePredicate<I::Item>,
{
}
