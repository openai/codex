use std::fmt;
use std::iter::{Fuse, FusedIterator, Peekable};

/// An iterator adaptor that wraps each element in an [`Position`].
///
/// Iterator element type is `(Position, I::Item)`.
///
/// See [`.with_position()`](crate::Itertools::with_position) for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct WithPosition<I>
where
    I: Iterator,
{
    handled_first: bool,
    peekable: Peekable<Fuse<I>>,
}

impl<I> fmt::Debug for WithPosition<I>
where
    I: Iterator,
    Peekable<Fuse<I>>: fmt::Debug,
{
    debug_fmt_fields!(WithPosition, handled_first, peekable);
}

impl<I> Clone for WithPosition<I>
where
    I: Clone + Iterator,
    I::Item: Clone,
{
    clone_fields!(handled_first, peekable);
}

/// Create a new `WithPosition` iterator.
pub fn with_position<I>(iter: I) -> WithPosition<I>
where
    I: Iterator,
{
    WithPosition {
        handled_first: false,
        peekable: iter.fuse().peekable(),
    }
}

/// The first component of the value yielded by `WithPosition`.
/// Indicates the position of this element in the iterator results.
///
/// See [`.with_position()`](crate::Itertools::with_position) for more information.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Position {
    /// This is the first element.
    First,
    /// This is neither the first nor the last element.
    Middle,
    /// This is the last element.
    Last,
    /// This is the only element.
    Only,
}

impl<I: Iterator> Iterator for WithPosition<I> {
    type Item = (Position, I::Item);

    fn next(&mut self) -> Option<Self::Item> {
        match self.peekable.next() {
            Some(item) => {
                if !self.handled_first {
                    // Haven't seen the first item yet, and there is one to give.
                    self.handled_first = true;
                    // Peek to see if this is also the last item,
                    // in which case tag it as `Only`.
                    match self.peekable.peek() {
                        Some(_) => Some((Position::First, item)),
                        None => Some((Position::Only, item)),
                    }
                } else {
                    // Have seen the first item, and there's something left.
                    // Peek to see if this is the last item.
                    match self.peekable.peek() {
                        Some(_) => Some((Position::Middle, item)),
                        None => Some((Position::Last, item)),
                    }
                }
            }
            // Iterator is finished.
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.peekable.size_hint()
    }

    fn fold<B, F>(mut self, mut init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        if let Some(mut head) = self.peekable.next() {
            if !self.handled_first {
                // The current head is `First` or `Only`,
                // it depends if there is another item or not.
                match self.peekable.next() {
                    Some(second) => {
                        let first = std::mem::replace(&mut head, second);
                        init = f(init, (Position::First, first));
                    }
                    None => return f(init, (Position::Only, head)),
                }
            }
            // Have seen the first item, and there's something left.
            init = self.peekable.fold(init, |acc, mut item| {
                std::mem::swap(&mut head, &mut item);
                f(acc, (Position::Middle, item))
            });
            // The "head" is now the last item.
            init = f(init, (Position::Last, head));
        }
        init
    }
}

impl<I> ExactSizeIterator for WithPosition<I> where I: ExactSizeIterator {}

impl<I: Iterator> FusedIterator for WithPosition<I> {}
