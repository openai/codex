//! This module defines an `IdleNotifiedSet`, which is a collection of elements.
//! Each element is intended to correspond to a task, and the collection will
//! keep track of which tasks have had their waker notified, and which have not.
//!
//! Each entry in the set holds some user-specified value. The value's type is
//! specified using the `T` parameter. It will usually be a `JoinHandle` or
//! similar.

use std::marker::PhantomPinned;
use std::mem::ManuallyDrop;
use std::ptr::NonNull;
use std::task::{Context, Waker};

use crate::loom::cell::UnsafeCell;
use crate::loom::sync::{Arc, Mutex};
use crate::util::linked_list::{self, Link};
use crate::util::{waker_ref, Wake};

type LinkedList<T> =
    linked_list::LinkedList<ListEntry<T>, <ListEntry<T> as linked_list::Link>::Target>;

/// This is the main handle to the collection.
pub(crate) struct IdleNotifiedSet<T> {
    lists: Arc<Lists<T>>,
    length: usize,
}

/// A handle to an entry that is guaranteed to be stored in the idle or notified
/// list of its `IdleNotifiedSet`. This value borrows the `IdleNotifiedSet`
/// mutably to prevent the entry from being moved to the `Neither` list, which
/// only the `IdleNotifiedSet` may do.
///
/// The main consequence of being stored in one of the lists is that the `value`
/// field has not yet been consumed.
///
/// Note: This entry can be moved from the idle to the notified list while this
/// object exists by waking its waker.
pub(crate) struct EntryInOneOfTheLists<'a, T> {
    entry: Arc<ListEntry<T>>,
    set: &'a mut IdleNotifiedSet<T>,
}

type Lists<T> = Mutex<ListsInner<T>>;

/// The linked lists hold strong references to the `ListEntry` items, and the
/// `ListEntry` items also hold a strong reference back to the Lists object, but
/// the destructor of the `IdleNotifiedSet` will clear the two lists, so once
/// that object is destroyed, no ref-cycles will remain.
struct ListsInner<T> {
    notified: LinkedList<T>,
    idle: LinkedList<T>,
    /// Whenever an element in the `notified` list is woken, this waker will be
    /// notified and consumed, if it exists.
    waker: Option<Waker>,
}

/// Which of the two lists in the shared Lists object is this entry stored in?
///
/// If the value is `Idle`, then an entry's waker may move it to the notified
/// list. Otherwise, only the `IdleNotifiedSet` may move it.
///
/// If the value is `Neither`, then it is still possible that the entry is in
/// some third external list (this happens in `drain`).
#[derive(Copy, Clone, Eq, PartialEq)]
enum List {
    Notified,
    Idle,
    Neither,
}

/// An entry in the list.
///
/// # Safety
///
/// The `my_list` field must only be accessed while holding the mutex in
/// `parent`. It is an invariant that the value of `my_list` corresponds to
/// which linked list in the `parent` holds this entry. Once this field takes
/// the value `Neither`, then it may never be modified again.
///
/// If the value of `my_list` is `Notified` or `Idle`, then the `pointers` field
/// must only be accessed while holding the mutex. If the value of `my_list` is
/// `Neither`, then the `pointers` field may be accessed by the
/// `IdleNotifiedSet` (this happens inside `drain`).
///
/// The `value` field is owned by the `IdleNotifiedSet` and may only be accessed
/// by the `IdleNotifiedSet`. The operation that sets the value of `my_list` to
/// `Neither` assumes ownership of the `value`, and it must either drop it or
/// move it out from this entry to prevent it from getting leaked. (Since the
/// two linked lists are emptied in the destructor of `IdleNotifiedSet`, the
/// value should not be leaked.)
struct ListEntry<T> {
    /// The linked list pointers of the list this entry is in.
    pointers: linked_list::Pointers<ListEntry<T>>,
    /// Pointer to the shared `Lists` struct.
    parent: Arc<Lists<T>>,
    /// The value stored in this entry.
    value: UnsafeCell<ManuallyDrop<T>>,
    /// Used to remember which list this entry is in.
    my_list: UnsafeCell<List>,
    /// Required by the `linked_list::Pointers` field.
    _pin: PhantomPinned,
}

generate_addr_of_methods! {
    impl<T> ListEntry<T> {
        unsafe fn addr_of_pointers(self: NonNull<Self>) -> NonNull<linked_list::Pointers<ListEntry<T>>> {
            &self.pointers
        }
    }
}

// With mutable access to the `IdleNotifiedSet`, you can get mutable access to
// the values.
unsafe impl<T: Send> Send for IdleNotifiedSet<T> {}
// With the current API we strictly speaking don't even need `T: Sync`, but we
// require it anyway to support adding &self APIs that access the values in the
// future.
unsafe impl<T: Sync> Sync for IdleNotifiedSet<T> {}

// These impls control when it is safe to create a Waker. Since the waker does
// not allow access to the value in any way (including its destructor), it is
// not necessary for `T` to be Send or Sync.
unsafe impl<T> Send for ListEntry<T> {}
unsafe impl<T> Sync for ListEntry<T> {}

impl<T> IdleNotifiedSet<T> {
    /// Create a new `IdleNotifiedSet`.
    pub(crate) fn new() -> Self {
        let lists = Mutex::new(ListsInner {
            notified: LinkedList::new(),
            idle: LinkedList::new(),
            waker: None,
        });

        IdleNotifiedSet {
            lists: Arc::new(lists),
            length: 0,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.length
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Insert the given value into the `idle` list.
    pub(crate) fn insert_idle(&mut self, value: T) -> EntryInOneOfTheLists<'_, T> {
        self.length += 1;

        let entry = Arc::new(ListEntry {
            parent: self.lists.clone(),
            value: UnsafeCell::new(ManuallyDrop::new(value)),
            my_list: UnsafeCell::new(List::Idle),
            pointers: linked_list::Pointers::new(),
            _pin: PhantomPinned,
        });

        {
            let mut lock = self.lists.lock();
            lock.idle.push_front(entry.clone());
        }

        // Safety: We just put the entry in the idle list, so it is in one of the lists.
        EntryInOneOfTheLists { entry, set: self }
    }

    /// Pop an entry from the notified list to poll it. The entry is moved to
    /// the idle list atomically.
    pub(crate) fn pop_notified(&mut self, waker: &Waker) -> Option<EntryInOneOfTheLists<'_, T>> {
        // We don't decrement the length because this call moves the entry to
        // the idle list rather than removing it.
        if self.length == 0 {
            // Fast path.
            return None;
        }

        let mut lock = self.lists.lock();

        let should_update_waker = match lock.waker.as_mut() {
            Some(cur_waker) => !waker.will_wake(cur_waker),
            None => true,
        };
        if should_update_waker {
            lock.waker = Some(waker.clone());
        }

        // Pop the entry, returning None if empty.
        let entry = lock.notified.pop_back()?;

        lock.idle.push_front(entry.clone());

        // Safety: We are holding the lock.
        entry.my_list.with_mut(|ptr| unsafe {
            *ptr = List::Idle;
        });

        drop(lock);

        // Safety: We just put the entry in the idle list, so it is in one of the lists.
        Some(EntryInOneOfTheLists { entry, set: self })
    }

    /// Tries to pop an entry from the notified list to poll it. The entry is moved to
    /// the idle list atomically.
    pub(crate) fn try_pop_notified(&mut self) -> Option<EntryInOneOfTheLists<'_, T>> {
        // We don't decrement the length because this call moves the entry to
        // the idle list rather than removing it.
        if self.length == 0 {
            // Fast path.
            return None;
        }

        let mut lock = self.lists.lock();

        // Pop the entry, returning None if empty.
        let entry = lock.notified.pop_back()?;

        lock.idle.push_front(entry.clone());

        // Safety: We are holding the lock.
        entry.my_list.with_mut(|ptr| unsafe {
            *ptr = List::Idle;
        });

        drop(lock);

        // Safety: We just put the entry in the idle list, so it is in one of the lists.
        Some(EntryInOneOfTheLists { entry, set: self })
    }

    /// Call a function on every element in this list.
    pub(crate) fn for_each<F: FnMut(&mut T)>(&mut self, mut func: F) {
        fn get_ptrs<T>(list: &mut LinkedList<T>, ptrs: &mut Vec<*mut T>) {
            let mut node = list.last();

            while let Some(entry) = node {
                ptrs.push(entry.value.with_mut(|ptr| {
                    let ptr: *mut ManuallyDrop<T> = ptr;
                    let ptr: *mut T = ptr.cast();
                    ptr
                }));

                let prev = entry.pointers.get_prev();
                node = prev.map(|prev| unsafe { &*prev.as_ptr() });
            }
        }

        // Atomically get a raw pointer to the value of every entry.
        //
        // Since this only locks the mutex once, it is not possible for a value
        // to get moved from the idle list to the notified list during the
        // operation, which would otherwise result in some value being listed
        // twice.
        let mut ptrs = Vec::with_capacity(self.len());
        {
            let mut lock = self.lists.lock();

            get_ptrs(&mut lock.idle, &mut ptrs);
            get_ptrs(&mut lock.notified, &mut ptrs);
        }
        debug_assert_eq!(ptrs.len(), ptrs.capacity());

        for ptr in ptrs {
            // Safety: When we grabbed the pointers, the entries were in one of
            // the two lists. This means that their value was valid at the time,
            // and it must still be valid because we are the IdleNotifiedSet,
            // and only we can remove an entry from the two lists. (It's
            // possible that an entry is moved from one list to the other during
            // this loop, but that is ok.)
            func(unsafe { &mut *ptr });
        }
    }

    /// Remove all entries in both lists, applying some function to each element.
    ///
    /// The closure is called on all elements even if it panics. Having it panic
    /// twice is a double-panic, and will abort the application.
    pub(crate) fn drain<F: FnMut(T)>(&mut self, func: F) {
        if self.length == 0 {
            // Fast path.
            return;
        }
        self.length = 0;

        // The LinkedList is not cleared on panic, so we use a bomb to clear it.
        //
        // This value has the invariant that any entry in its `all_entries` list
        // has `my_list` set to `Neither` and that the value has not yet been
        // dropped.
        struct AllEntries<T, F: FnMut(T)> {
            all_entries: LinkedList<T>,
            func: F,
        }

        impl<T, F: FnMut(T)> AllEntries<T, F> {
            fn pop_next(&mut self) -> bool {
                if let Some(entry) = self.all_entries.pop_back() {
                    // Safety: We just took this value from the list, so we can
                    // destroy the value in the entry.
                    entry
                        .value
                        .with_mut(|ptr| unsafe { (self.func)(ManuallyDrop::take(&mut *ptr)) });
                    true
                } else {
                    false
                }
            }
        }

        impl<T, F: FnMut(T)> Drop for AllEntries<T, F> {
            fn drop(&mut self) {
                while self.pop_next() {}
            }
        }

        let mut all_entries = AllEntries {
            all_entries: LinkedList::new(),
            func,
        };

        // Atomically move all entries to the new linked list in the AllEntries
        // object.
        {
            let mut lock = self.lists.lock();
            unsafe {
                // Safety: We are holding the lock and `all_entries` is a new
                // LinkedList.
                move_to_new_list(&mut lock.idle, &mut all_entries.all_entries);
                move_to_new_list(&mut lock.notified, &mut all_entries.all_entries);
            }
        }

        // Keep destroying entries in the list until it is empty.
        //
        // If the closure panics, then the destructor of the `AllEntries` bomb
        // ensures that we keep running the destructor on the remaining values.
        // A second panic will abort the program.
        while all_entries.pop_next() {}
    }
}

/// # Safety
///
/// The mutex for the entries must be held, and the target list must be such
/// that setting `my_list` to `Neither` is ok.
unsafe fn move_to_new_list<T>(from: &mut LinkedList<T>, to: &mut LinkedList<T>) {
    while let Some(entry) = from.pop_back() {
        entry.my_list.with_mut(|ptr| {
            *ptr = List::Neither;
        });
        to.push_front(entry);
    }
}

impl<'a, T> EntryInOneOfTheLists<'a, T> {
    /// Remove this entry from the list it is in, returning the value associated
    /// with the entry.
    ///
    /// This consumes the value, since it is no longer guaranteed to be in a
    /// list.
    pub(crate) fn remove(self) -> T {
        self.set.length -= 1;

        {
            let mut lock = self.set.lists.lock();

            // Safety: We are holding the lock so there is no race, and we will
            // remove the entry afterwards to uphold invariants.
            let old_my_list = self.entry.my_list.with_mut(|ptr| unsafe {
                let old_my_list = *ptr;
                *ptr = List::Neither;
                old_my_list
            });

            let list = match old_my_list {
                List::Idle => &mut lock.idle,
                List::Notified => &mut lock.notified,
                // An entry in one of the lists is in one of the lists.
                List::Neither => unreachable!(),
            };

            unsafe {
                // Safety: We just checked that the entry is in this particular
                // list.
                list.remove(ListEntry::as_raw(&self.entry)).unwrap();
            }
        }

        // By setting `my_list` to `Neither`, we have taken ownership of the
        // value. We return it to the caller.
        //
        // Safety: We have a mutable reference to the `IdleNotifiedSet` that
        // owns this entry, so we can use its permission to access the value.
        self.entry
            .value
            .with_mut(|ptr| unsafe { ManuallyDrop::take(&mut *ptr) })
    }

    /// Access the value in this entry together with a context for its waker.
    pub(crate) fn with_value_and_context<F, U>(&mut self, func: F) -> U
    where
        F: FnOnce(&mut T, &mut Context<'_>) -> U,
        T: 'static,
    {
        let waker = waker_ref(&self.entry);

        let mut context = Context::from_waker(&waker);

        // Safety: We have a mutable reference to the `IdleNotifiedSet` that
        // owns this entry, so we can use its permission to access the value.
        self.entry
            .value
            .with_mut(|ptr| unsafe { func(&mut *ptr, &mut context) })
    }
}

impl<T> Drop for IdleNotifiedSet<T> {
    fn drop(&mut self) {
        // Clear both lists.
        self.drain(drop);

        #[cfg(debug_assertions)]
        if !std::thread::panicking() {
            let lock = self.lists.lock();
            assert!(lock.idle.is_empty());
            assert!(lock.notified.is_empty());
        }
    }
}

impl<T: 'static> Wake for ListEntry<T> {
    fn wake_by_ref(me: &Arc<Self>) {
        let mut lock = me.parent.lock();

        // Safety: We are holding the lock and we will update the lists to
        // maintain invariants.
        let old_my_list = me.my_list.with_mut(|ptr| unsafe {
            let old_my_list = *ptr;
            if old_my_list == List::Idle {
                *ptr = List::Notified;
            }
            old_my_list
        });

        if old_my_list == List::Idle {
            // We move ourself to the notified list.
            let me = unsafe {
                // Safety: We just checked that we are in this particular list.
                lock.idle.remove(ListEntry::as_raw(me)).unwrap()
            };
            lock.notified.push_front(me);

            if let Some(waker) = lock.waker.take() {
                drop(lock);
                waker.wake();
            }
        }
    }

    fn wake(me: Arc<Self>) {
        Self::wake_by_ref(&me);
    }
}

/// # Safety
///
/// `ListEntry` is forced to be !Unpin.
unsafe impl<T> linked_list::Link for ListEntry<T> {
    type Handle = Arc<ListEntry<T>>;
    type Target = ListEntry<T>;

    fn as_raw(handle: &Self::Handle) -> NonNull<ListEntry<T>> {
        let ptr: *const ListEntry<T> = Arc::as_ptr(handle);
        // Safety: We can't get a null pointer from `Arc::as_ptr`.
        unsafe { NonNull::new_unchecked(ptr as *mut ListEntry<T>) }
    }

    unsafe fn from_raw(ptr: NonNull<ListEntry<T>>) -> Arc<ListEntry<T>> {
        Arc::from_raw(ptr.as_ptr())
    }

    unsafe fn pointers(
        target: NonNull<ListEntry<T>>,
    ) -> NonNull<linked_list::Pointers<ListEntry<T>>> {
        ListEntry::addr_of_pointers(target)
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use crate::runtime::Builder;
    use crate::task::JoinSet;

    // A test that runs under miri.
    //
    // https://github.com/tokio-rs/tokio/pull/5693
    #[test]
    fn join_set_test() {
        let rt = Builder::new_current_thread().build().unwrap();

        let mut set = JoinSet::new();
        set.spawn_on(futures::future::ready(()), rt.handle());

        rt.block_on(set.join_next()).unwrap().unwrap();
    }
}
