use crate::size_hint;
use std::iter::{Fuse, FusedIterator};

/// An iterator adaptor that pads a sequence to a minimum length by filling
/// missing elements using a closure.
///
/// Iterator element type is `I::Item`.
///
/// See [`.pad_using()`](crate::Itertools::pad_using) for more information.
#[derive(Clone)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct PadUsing<I, F> {
    iter: Fuse<I>,
    min: usize,
    pos: usize,
    filler: F,
}

impl<I, F> std::fmt::Debug for PadUsing<I, F>
where
    I: std::fmt::Debug,
{
    debug_fmt_fields!(PadUsing, iter, min, pos);
}

/// Create a new `PadUsing` iterator.
pub fn pad_using<I, F>(iter: I, min: usize, filler: F) -> PadUsing<I, F>
where
    I: Iterator,
    F: FnMut(usize) -> I::Item,
{
    PadUsing {
        iter: iter.fuse(),
        min,
        pos: 0,
        filler,
    }
}

impl<I, F> Iterator for PadUsing<I, F>
where
    I: Iterator,
    F: FnMut(usize) -> I::Item,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            None => {
                if self.pos < self.min {
                    let e = Some((self.filler)(self.pos));
                    self.pos += 1;
                    e
                } else {
                    None
                }
            }
            e => {
                self.pos += 1;
                e
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let tail = self.min.saturating_sub(self.pos);
        size_hint::max(self.iter.size_hint(), (tail, Some(tail)))
    }

    fn fold<B, G>(self, mut init: B, mut f: G) -> B
    where
        G: FnMut(B, Self::Item) -> B,
    {
        let mut pos = self.pos;
        init = self.iter.fold(init, |acc, item| {
            pos += 1;
            f(acc, item)
        });
        (pos..self.min).map(self.filler).fold(init, f)
    }
}

impl<I, F> DoubleEndedIterator for PadUsing<I, F>
where
    I: DoubleEndedIterator + ExactSizeIterator,
    F: FnMut(usize) -> I::Item,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.min == 0 {
            self.iter.next_back()
        } else if self.iter.len() >= self.min {
            self.min -= 1;
            self.iter.next_back()
        } else {
            self.min -= 1;
            Some((self.filler)(self.min))
        }
    }

    fn rfold<B, G>(self, mut init: B, mut f: G) -> B
    where
        G: FnMut(B, Self::Item) -> B,
    {
        init = (self.iter.len()..self.min)
            .map(self.filler)
            .rfold(init, &mut f);
        self.iter.rfold(init, f)
    }
}

impl<I, F> ExactSizeIterator for PadUsing<I, F>
where
    I: ExactSizeIterator,
    F: FnMut(usize) -> I::Item,
{
}

impl<I, F> FusedIterator for PadUsing<I, F>
where
    I: FusedIterator,
    F: FnMut(usize) -> I::Item,
{
}
