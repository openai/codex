use std::{iter::FusedIterator, slice};

use crate::{cfg, page, shard};

/// An exclusive fused iterator over the items in a [`Slab`](crate::Slab).
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Debug)]
pub struct UniqueIter<'a, T, C: cfg::Config> {
    pub(super) shards: shard::IterMut<'a, Option<T>, C>,
    pub(super) pages: slice::Iter<'a, page::Shared<Option<T>, C>>,
    pub(super) slots: Option<page::Iter<'a, T, C>>,
}

impl<'a, T, C: cfg::Config> Iterator for UniqueIter<'a, T, C> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        test_println!("UniqueIter::next");
        loop {
            test_println!("-> try next slot");
            if let Some(item) = self.slots.as_mut().and_then(|slots| slots.next()) {
                test_println!("-> found an item!");
                return Some(item);
            }

            test_println!("-> try next page");
            if let Some(page) = self.pages.next() {
                test_println!("-> found another page");
                self.slots = page.iter();
                continue;
            }

            test_println!("-> try next shard");
            if let Some(shard) = self.shards.next() {
                test_println!("-> found another shard");
                self.pages = shard.iter();
            } else {
                test_println!("-> all done!");
                return None;
            }
        }
    }
}

impl<T, C: cfg::Config> FusedIterator for UniqueIter<'_, T, C> {}
