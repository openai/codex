//! Pre-allocated storage with constant-time LRU tracking

#![warn(missing_docs)]
#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use core::{fmt, iter::FusedIterator, marker::PhantomData, ptr::addr_of_mut};

/// A random-access table that maintains an LRU list in constant time
#[derive(Clone)]
pub struct LruSlab<T> {
    slots: Box<[Slot<T>]>,
    /// Most recently used
    head: u32,
    /// Least recently used
    tail: u32,
    /// First unused
    free: u32,
    /// Number of occupied slots
    len: u32,
}

impl<T> LruSlab<T> {
    /// Create an empty [`LruSlab`]
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Create an [`LruSlab`] that can store at least `capacity` elements without reallocating
    pub fn with_capacity(capacity: u32) -> Self {
        assert!(capacity != u32::MAX, "capacity too large");
        Self {
            slots: (0..capacity)
                .map(|n| Slot {
                    value: None,
                    prev: NONE,
                    next: if n + 1 == capacity { NONE } else { n + 1 },
                })
                .collect(),
            head: NONE,
            tail: NONE,
            free: if capacity == 0 { NONE } else { 0 },
            len: 0,
        }
    }

    /// Whether no elements are stored
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Number of elements stored
    pub fn len(&self) -> u32 {
        self.len
    }

    /// Number of elements that can be stored without reallocating
    pub fn capacity(&self) -> u32 {
        self.slots.len() as u32
    }

    /// The slot that will be returned by the next call to `insert`, unless `remove` is called first
    pub fn vacant_key(&self) -> u32 {
        match self.free {
            NONE => self.capacity(),
            _ => self.free,
        }
    }

    /// Insert a value, returning the slot it was stored in
    ///
    /// The returned slot is marked as the most recently used.
    pub fn insert(&mut self, value: T) -> u32 {
        let id = match self.alloc() {
            Some(id) => id,
            None => {
                let len = self.capacity();
                // Ensure `NONE` never becomes a real slot index
                let cap = 2u32.saturating_mul(len.max(2)).min(u32::MAX - 1);
                if cap == len {
                    panic!("cannot store more than 2^32-2 elements");
                }
                self.slots = self
                    .slots
                    .iter_mut()
                    .map(|x| Slot {
                        value: x.value.take(),
                        next: x.next,
                        prev: x.prev,
                    })
                    .chain((len..cap).map(|n| Slot {
                        value: None,
                        prev: NONE,
                        next: if n + 1 == cap { NONE } else { n + 1 },
                    }))
                    .collect();
                self.free = len + 1;
                len
            }
        };
        let idx = id as usize;

        debug_assert!(self.slots[idx].value.is_none(), "corrupt free list");
        self.slots[idx].value = Some(value);
        self.link_at_head(id);
        self.len += 1;

        id
    }

    /// Get the least recently used slot, if any
    pub fn lru(&self) -> Option<u32> {
        if self.tail == NONE {
            debug_assert_eq!(self.head, NONE);
            None
        } else {
            Some(self.tail)
        }
    }

    /// Remove the element stored in `slot`, returning it
    pub fn remove(&mut self, slot: u32) -> T {
        self.unlink(slot);
        self.slots[slot as usize].next = self.free;
        self.slots[slot as usize].prev = NONE;
        self.free = slot;
        self.len -= 1;
        self.slots[slot as usize]
            .value
            .take()
            .expect("removing empty slot")
    }

    /// Mark `slot` as the most recently used and access it uniquely
    pub fn get_mut(&mut self, slot: u32) -> &mut T {
        self.freshen(slot);
        self.peek_mut(slot)
    }

    /// Access `slot` without marking it as most recently used
    pub fn peek(&self, slot: u32) -> &T {
        self.slots[slot as usize].value.as_ref().unwrap()
    }

    /// Access `slot` uniquely without marking it as most recently used
    pub fn peek_mut(&mut self, slot: u32) -> &mut T {
        self.slots[slot as usize].value.as_mut().unwrap()
    }

    /// Walk the container from most to least recently used
    pub fn iter(&self) -> Iter<'_, T> {
        let state = IterState::new(self);
        Iter {
            slots: &self.slots[..],
            state,
        }
    }

    /// Walk the container from most to least recently used
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        let state = IterState::new(self);
        IterMut {
            slots: self.slots[..].as_mut_ptr().cast(),
            state,
            _marker: PhantomData,
        }
    }

    /// Remove a slot from the freelist
    fn alloc(&mut self) -> Option<u32> {
        if self.free == NONE {
            return None;
        }
        let slot = self.free;
        self.free = self.slots[slot as usize].next;
        Some(slot)
    }

    /// Mark `slot` as the most recently used
    fn freshen(&mut self, slot: u32) {
        if self.slots[slot as usize].prev == NONE {
            // This is already the freshest slot, so we don't need to do anything
            debug_assert_eq!(self.head, slot, "corrupt LRU list");
            return;
        }

        self.unlink(slot);
        self.link_at_head(slot);
    }

    /// Add a link to the head of the list
    fn link_at_head(&mut self, slot: u32) {
        let idx = slot as usize;
        if self.head == NONE {
            // List was empty
            self.slots[idx].next = NONE;
            self.tail = slot;
        } else {
            self.slots[idx].next = self.head;
            self.slots[self.head as usize].prev = slot;
        }
        self.slots[idx].prev = NONE;
        self.head = slot;
    }

    /// Remove a link from anywhere in the list
    fn unlink(&mut self, slot: u32) {
        let idx = slot as usize;
        if self.slots[idx].prev != NONE {
            self.slots[self.slots[idx].prev as usize].next = self.slots[idx].next;
        } else {
            self.head = self.slots[idx].next;
        }
        if self.slots[idx].next != NONE {
            self.slots[self.slots[idx].next as usize].prev = self.slots[idx].prev;
        } else {
            // This was the tail
            self.tail = self.slots[idx].prev;
        }
    }
}

impl<T> Default for LruSlab<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> FromIterator<T> for LruSlab<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let mut slab = LruSlab::with_capacity(u32::try_from(iter.size_hint().0).unwrap());
        for x in iter {
            slab.insert(x);
        }
        slab
    }
}

impl<'a, T> IntoIterator for &'a LruSlab<T> {
    type Item = (u32, &'a T);

    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut LruSlab<T> {
    type Item = (u32, &'a mut T);

    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: fmt::Debug> fmt::Debug for LruSlab<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self).finish()
    }
}

#[derive(Clone)]
struct Slot<T> {
    value: Option<T>,
    /// Next slot in the LRU or free list
    next: u32,
    /// Previous slot in the LRU list; NONE when free
    prev: u32,
}

const NONE: u32 = u32::MAX;

/// Iterator over elements of an [`LruSlab`], from most to least recently used
pub struct Iter<'a, T> {
    slots: &'a [Slot<T>],
    state: IterState,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (u32, &'a T);
    fn next(&mut self) -> Option<(u32, &'a T)> {
        let idx = self.state.next(|i| self.slots[i as usize].next)?;
        let result = self.slots[idx as usize]
            .value
            .as_ref()
            .expect("corrupt LRU list");
        Some((idx, result))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.state.len as usize, Some(self.state.len as usize))
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<(u32, &'a T)> {
        let idx = self.state.next_back(|i| self.slots[i as usize].prev)?;
        let result = self.slots[idx as usize]
            .value
            .as_ref()
            .expect("corrupt LRU list");
        Some((idx, result))
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {
    fn len(&self) -> usize {
        self.state.len as usize
    }
}

impl<T> FusedIterator for Iter<'_, T> {}

/// Iterator over mutable elements of an [`LruSlab`], from most to least recently used
pub struct IterMut<'a, T> {
    slots: *mut Slot<T>,
    state: IterState,
    _marker: PhantomData<&'a mut [Slot<T>]>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (u32, &'a mut T);
    fn next(&mut self) -> Option<(u32, &'a mut T)> {
        // Safety: `next` returns unique in-bounds indices, and no live references overlap with any
        // `next` field
        unsafe {
            let idx = self
                .state
                .next(|i| *addr_of_mut!((*self.slots.add(i as usize)).next))?;
            let result = (*addr_of_mut!((*self.slots.add(idx as usize)).value))
                .as_mut()
                .expect("corrupt LRU list");
            Some((idx, result))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.state.len as usize, Some(self.state.len as usize))
    }
}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<(u32, &'a mut T)> {
        // Safety: `next_back` returns unique in-bounds indices, and no live references overlap with
        // any `prev` field
        unsafe {
            let idx = self
                .state
                .next_back(|i| *addr_of_mut!((*self.slots.add(i as usize)).prev))?;
            let result = (*addr_of_mut!((*self.slots.add(idx as usize)).value))
                .as_mut()
                .expect("corrupt LRU list");
            Some((idx, result))
        }
    }
}

impl<T> ExactSizeIterator for IterMut<'_, T> {
    fn len(&self) -> usize {
        self.state.len as usize
    }
}

impl<T> FusedIterator for IterMut<'_, T> {}

struct IterState {
    head: u32,
    tail: u32,
    len: u32,
}

impl IterState {
    fn new<T>(slab: &LruSlab<T>) -> Self {
        Self {
            head: slab.head,
            tail: slab.tail,
            len: slab.len,
        }
    }

    fn next(&mut self, get_next: impl Fn(u32) -> u32) -> Option<u32> {
        if self.len == 0 {
            return None;
        }
        let idx = self.head;
        self.head = get_next(idx);
        self.len -= 1;
        Some(idx)
    }

    fn next_back(&mut self, get_prev: impl Fn(u32) -> u32) -> Option<u32> {
        if self.len == 0 {
            return None;
        }
        let idx = self.tail;
        self.tail = get_prev(idx);
        self.len -= 1;
        Some(idx)
    }
}

#[cfg(test)]
mod tests {
    use alloc::{format, string::String, vec::Vec};

    use super::*;

    #[test]
    fn lru_order() {
        let mut cache = LruSlab::new();
        let b = cache.insert('b');
        assert_eq!(cache.iter().map(|(_, x)| x).collect::<String>(), "b");
        let _a = cache.insert('a');
        assert_eq!(cache.iter().map(|(_, x)| x).collect::<String>(), "ab");
        let d = cache.insert('d');
        assert_eq!(cache.iter().map(|(_, x)| x).collect::<String>(), "dab");
        let c = cache.insert('c');
        assert_eq!(cache.iter().map(|(_, x)| x).collect::<String>(), "cdab");
        let e = cache.insert('e');
        assert_eq!(cache.iter().map(|(_, x)| x).collect::<String>(), "ecdab");

        cache.get_mut(b);
        cache.get_mut(c);
        cache.get_mut(d);
        cache.get_mut(e);

        assert_eq!(cache.remove(cache.lru().unwrap()), 'a');
        assert_eq!(cache.remove(cache.lru().unwrap()), 'b');
        assert_eq!(cache.remove(cache.lru().unwrap()), 'c');
        assert_eq!(cache.remove(cache.lru().unwrap()), 'd');
        assert_eq!(cache.remove(cache.lru().unwrap()), 'e');
        assert!(cache.lru().is_none());
    }

    #[test]
    fn slot_reuse() {
        let mut cache = LruSlab::new();
        let a = cache.insert('a');
        cache.remove(a);
        let a_prime = cache.insert('a');
        assert_eq!(a, a_prime);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn debug() {
        let slab = ['a', 'b'].into_iter().collect::<LruSlab<_>>();
        assert_eq!(format!("{:?}", slab), "{1: 'b', 0: 'a'}");
    }

    #[test]
    fn iter_reverse() {
        let slab = ['a', 'b'].into_iter().collect::<LruSlab<_>>();
        let mut double_reversed = slab.iter().rev().collect::<Vec<_>>();
        double_reversed.reverse();
        assert_eq!(slab.iter().collect::<Vec<_>>(), double_reversed);
    }

    #[test]
    fn vacant_key() {
        let mut slab = LruSlab::new();
        assert_eq!(slab.vacant_key(), 0);
        slab.insert(());
        assert_eq!(slab.vacant_key(), 1);
        slab.remove(0);
        assert_eq!(slab.vacant_key(), 0);
    }
}
