use super::size_hint;

use alloc::collections::VecDeque;
use alloc::rc::Rc;
use std::cell::RefCell;

/// Common buffer object for the two tee halves
#[derive(Debug)]
struct TeeBuffer<A, I> {
    backlog: VecDeque<A>,
    iter: I,
    /// The owner field indicates which id should read from the backlog
    owner: bool,
}

/// One half of an iterator pair where both return the same elements.
///
/// See [`.tee()`](crate::Itertools::tee) for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Debug)]
pub struct Tee<I>
where
    I: Iterator,
{
    rcbuffer: Rc<RefCell<TeeBuffer<I::Item, I>>>,
    id: bool,
}

pub fn new<I>(iter: I) -> (Tee<I>, Tee<I>)
where
    I: Iterator,
{
    let buffer = TeeBuffer {
        backlog: VecDeque::new(),
        iter,
        owner: false,
    };
    let t1 = Tee {
        rcbuffer: Rc::new(RefCell::new(buffer)),
        id: true,
    };
    let t2 = Tee {
        rcbuffer: t1.rcbuffer.clone(),
        id: false,
    };
    (t1, t2)
}

impl<I> Iterator for Tee<I>
where
    I: Iterator,
    I::Item: Clone,
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        // .borrow_mut may fail here -- but only if the user has tied some kind of weird
        // knot where the iterator refers back to itself.
        let mut buffer = self.rcbuffer.borrow_mut();
        if buffer.owner == self.id {
            match buffer.backlog.pop_front() {
                None => {}
                some_elt => return some_elt,
            }
        }
        match buffer.iter.next() {
            None => None,
            Some(elt) => {
                buffer.backlog.push_back(elt.clone());
                buffer.owner = !self.id;
                Some(elt)
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let buffer = self.rcbuffer.borrow();
        let sh = buffer.iter.size_hint();

        if buffer.owner == self.id {
            let log_len = buffer.backlog.len();
            size_hint::add_scalar(sh, log_len)
        } else {
            sh
        }
    }
}

impl<I> ExactSizeIterator for Tee<I>
where
    I: ExactSizeIterator,
    I::Item: Clone,
{
}
