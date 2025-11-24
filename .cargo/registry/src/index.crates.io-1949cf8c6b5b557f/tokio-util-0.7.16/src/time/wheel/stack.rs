use std::borrow::Borrow;
use std::cmp::Eq;
use std::hash::Hash;

/// Abstracts the stack operations needed to track timeouts.
pub(crate) trait Stack: Default {
    /// Type of the item stored in the stack
    type Owned: Borrow<Self::Borrowed>;

    /// Borrowed item
    type Borrowed: Eq + Hash;

    /// Item storage, this allows a slab to be used instead of just the heap
    type Store;

    /// Returns `true` if the stack is empty
    fn is_empty(&self) -> bool;

    /// Push an item onto the stack
    fn push(&mut self, item: Self::Owned, store: &mut Self::Store);

    /// Pop an item from the stack
    fn pop(&mut self, store: &mut Self::Store) -> Option<Self::Owned>;

    /// Peek into the stack.
    fn peek(&self) -> Option<Self::Owned>;

    fn remove(&mut self, item: &Self::Borrowed, store: &mut Self::Store);

    fn when(item: &Self::Borrowed, store: &Self::Store) -> u64;
}
