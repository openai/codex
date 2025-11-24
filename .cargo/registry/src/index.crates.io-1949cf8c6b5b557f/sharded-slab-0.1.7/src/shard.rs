use crate::{
    cfg::{self, CfgPrivate},
    clear::Clear,
    page,
    sync::{
        alloc,
        atomic::{
            AtomicPtr, AtomicUsize,
            Ordering::{self, *},
        },
    },
    tid::Tid,
    Pack,
};

use std::{fmt, ptr, slice};

// ┌─────────────┐      ┌────────┐
// │ page 1      │      │        │
// ├─────────────┤ ┌───▶│  next──┼─┐
// │ page 2      │ │    ├────────┤ │
// │             │ │    │XXXXXXXX│ │
// │ local_free──┼─┘    ├────────┤ │
// │ global_free─┼─┐    │        │◀┘
// ├─────────────┤ └───▶│  next──┼─┐
// │   page 3    │      ├────────┤ │
// └─────────────┘      │XXXXXXXX│ │
//       ...            ├────────┤ │
// ┌─────────────┐      │XXXXXXXX│ │
// │ page n      │      ├────────┤ │
// └─────────────┘      │        │◀┘
//                      │  next──┼───▶
//                      ├────────┤
//                      │XXXXXXXX│
//                      └────────┘
//                         ...
pub(crate) struct Shard<T, C: cfg::Config> {
    /// The shard's parent thread ID.
    pub(crate) tid: usize,
    /// The local free list for each page.
    ///
    /// These are only ever accessed from this shard's thread, so they are
    /// stored separately from the shared state for the page that can be
    /// accessed concurrently, to minimize false sharing.
    local: Box<[page::Local]>,
    /// The shared state for each page in this shard.
    ///
    /// This consists of the page's metadata (size, previous size), remote free
    /// list, and a pointer to the actual array backing that page.
    shared: Box<[page::Shared<T, C>]>,
}

pub(crate) struct Array<T, C: cfg::Config> {
    shards: Box<[Ptr<T, C>]>,
    max: AtomicUsize,
}

#[derive(Debug)]
struct Ptr<T, C: cfg::Config>(AtomicPtr<alloc::Track<Shard<T, C>>>);

#[derive(Debug)]
pub(crate) struct IterMut<'a, T: 'a, C: cfg::Config + 'a>(slice::IterMut<'a, Ptr<T, C>>);

// === impl Shard ===

impl<T, C> Shard<T, C>
where
    C: cfg::Config,
{
    #[inline(always)]
    pub(crate) fn with_slot<'a, U>(
        &'a self,
        idx: usize,
        f: impl FnOnce(&'a page::Slot<T, C>) -> Option<U>,
    ) -> Option<U> {
        debug_assert_eq_in_drop!(Tid::<C>::from_packed(idx).as_usize(), self.tid);
        let (addr, page_index) = page::indices::<C>(idx);

        test_println!("-> {:?}", addr);
        if page_index >= self.shared.len() {
            return None;
        }

        self.shared[page_index].with_slot(addr, f)
    }

    pub(crate) fn new(tid: usize) -> Self {
        let mut total_sz = 0;
        let shared = (0..C::MAX_PAGES)
            .map(|page_num| {
                let sz = C::page_size(page_num);
                let prev_sz = total_sz;
                total_sz += sz;
                page::Shared::new(sz, prev_sz)
            })
            .collect();
        let local = (0..C::MAX_PAGES).map(|_| page::Local::new()).collect();
        Self { tid, local, shared }
    }
}

impl<T, C> Shard<Option<T>, C>
where
    C: cfg::Config,
{
    /// Remove an item on the shard's local thread.
    pub(crate) fn take_local(&self, idx: usize) -> Option<T> {
        debug_assert_eq_in_drop!(Tid::<C>::from_packed(idx).as_usize(), self.tid);
        let (addr, page_index) = page::indices::<C>(idx);

        test_println!("-> remove_local {:?}", addr);

        self.shared
            .get(page_index)?
            .take(addr, C::unpack_gen(idx), self.local(page_index))
    }

    /// Remove an item, while on a different thread from the shard's local thread.
    pub(crate) fn take_remote(&self, idx: usize) -> Option<T> {
        debug_assert_eq_in_drop!(Tid::<C>::from_packed(idx).as_usize(), self.tid);
        debug_assert!(Tid::<C>::current().as_usize() != self.tid);

        let (addr, page_index) = page::indices::<C>(idx);

        test_println!("-> take_remote {:?}; page {:?}", addr, page_index);

        let shared = self.shared.get(page_index)?;
        shared.take(addr, C::unpack_gen(idx), shared.free_list())
    }

    pub(crate) fn remove_local(&self, idx: usize) -> bool {
        debug_assert_eq_in_drop!(Tid::<C>::from_packed(idx).as_usize(), self.tid);
        let (addr, page_index) = page::indices::<C>(idx);

        if page_index >= self.shared.len() {
            return false;
        }

        self.shared[page_index].remove(addr, C::unpack_gen(idx), self.local(page_index))
    }

    pub(crate) fn remove_remote(&self, idx: usize) -> bool {
        debug_assert_eq_in_drop!(Tid::<C>::from_packed(idx).as_usize(), self.tid);
        let (addr, page_index) = page::indices::<C>(idx);

        if page_index >= self.shared.len() {
            return false;
        }

        let shared = &self.shared[page_index];
        shared.remove(addr, C::unpack_gen(idx), shared.free_list())
    }

    pub(crate) fn iter(&self) -> std::slice::Iter<'_, page::Shared<Option<T>, C>> {
        self.shared.iter()
    }
}

impl<T, C> Shard<T, C>
where
    T: Clear + Default,
    C: cfg::Config,
{
    pub(crate) fn init_with<U>(
        &self,
        mut init: impl FnMut(usize, &page::Slot<T, C>) -> Option<U>,
    ) -> Option<U> {
        // Can we fit the value into an exist`ing page?
        for (page_idx, page) in self.shared.iter().enumerate() {
            let local = self.local(page_idx);

            test_println!("-> page {}; {:?}; {:?}", page_idx, local, page);

            if let Some(res) = page.init_with(local, &mut init) {
                return Some(res);
            }
        }

        None
    }

    pub(crate) fn mark_clear_local(&self, idx: usize) -> bool {
        debug_assert_eq_in_drop!(Tid::<C>::from_packed(idx).as_usize(), self.tid);
        let (addr, page_index) = page::indices::<C>(idx);

        if page_index >= self.shared.len() {
            return false;
        }

        self.shared[page_index].mark_clear(addr, C::unpack_gen(idx), self.local(page_index))
    }

    pub(crate) fn mark_clear_remote(&self, idx: usize) -> bool {
        debug_assert_eq_in_drop!(Tid::<C>::from_packed(idx).as_usize(), self.tid);
        let (addr, page_index) = page::indices::<C>(idx);

        if page_index >= self.shared.len() {
            return false;
        }

        let shared = &self.shared[page_index];
        shared.mark_clear(addr, C::unpack_gen(idx), shared.free_list())
    }

    pub(crate) fn clear_after_release(&self, idx: usize) {
        crate::sync::atomic::fence(crate::sync::atomic::Ordering::Acquire);
        let tid = Tid::<C>::current().as_usize();
        test_println!(
            "-> clear_after_release; self.tid={:?}; current.tid={:?};",
            tid,
            self.tid
        );
        if tid == self.tid {
            self.clear_local(idx);
        } else {
            self.clear_remote(idx);
        }
    }

    fn clear_local(&self, idx: usize) -> bool {
        debug_assert_eq_in_drop!(Tid::<C>::from_packed(idx).as_usize(), self.tid);
        let (addr, page_index) = page::indices::<C>(idx);

        if page_index >= self.shared.len() {
            return false;
        }

        self.shared[page_index].clear(addr, C::unpack_gen(idx), self.local(page_index))
    }

    fn clear_remote(&self, idx: usize) -> bool {
        debug_assert_eq_in_drop!(Tid::<C>::from_packed(idx).as_usize(), self.tid);
        let (addr, page_index) = page::indices::<C>(idx);

        if page_index >= self.shared.len() {
            return false;
        }

        let shared = &self.shared[page_index];
        shared.clear(addr, C::unpack_gen(idx), shared.free_list())
    }

    #[inline(always)]
    fn local(&self, i: usize) -> &page::Local {
        #[cfg(debug_assertions)]
        debug_assert_eq_in_drop!(
            Tid::<C>::current().as_usize(),
            self.tid,
            "tried to access local data from another thread!"
        );

        &self.local[i]
    }
}

impl<T: fmt::Debug, C: cfg::Config> fmt::Debug for Shard<T, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("Shard");

        #[cfg(debug_assertions)]
        d.field("tid", &self.tid);
        d.field("shared", &self.shared).finish()
    }
}

// === impl Array ===

impl<T, C> Array<T, C>
where
    C: cfg::Config,
{
    pub(crate) fn new() -> Self {
        let mut shards = Vec::with_capacity(C::MAX_SHARDS);
        for _ in 0..C::MAX_SHARDS {
            // XXX(eliza): T_T this could be avoided with maybeuninit or something...
            shards.push(Ptr::null());
        }
        Self {
            shards: shards.into(),
            max: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub(crate) fn get(&self, idx: usize) -> Option<&Shard<T, C>> {
        test_println!("-> get shard={}", idx);
        self.shards.get(idx)?.load(Acquire)
    }

    #[inline]
    pub(crate) fn current(&self) -> (Tid<C>, &Shard<T, C>) {
        let tid = Tid::<C>::current();
        test_println!("current: {:?}", tid);
        let idx = tid.as_usize();
        assert!(
            idx < self.shards.len(),
            "Thread count overflowed the configured max count. \
            Thread index = {}, max threads = {}.",
            idx,
            C::MAX_SHARDS,
        );
        // It's okay for this to be relaxed. The value is only ever stored by
        // the thread that corresponds to the index, and we are that thread.
        let shard = self.shards[idx].load(Relaxed).unwrap_or_else(|| {
            let ptr = Box::into_raw(Box::new(alloc::Track::new(Shard::new(idx))));
            test_println!("-> allocated new shard for index {} at {:p}", idx, ptr);
            self.shards[idx].set(ptr);
            let mut max = self.max.load(Acquire);
            while max < idx {
                match self.max.compare_exchange(max, idx, AcqRel, Acquire) {
                    Ok(_) => break,
                    Err(actual) => max = actual,
                }
            }
            test_println!("-> highest index={}, prev={}", std::cmp::max(max, idx), max);
            unsafe {
                // Safety: we just put it there!
                &*ptr
            }
            .get_ref()
        });
        (tid, shard)
    }

    pub(crate) fn iter_mut(&mut self) -> IterMut<'_, T, C> {
        test_println!("Array::iter_mut");
        let max = self.max.load(Acquire);
        test_println!("-> highest index={}", max);
        IterMut(self.shards[0..=max].iter_mut())
    }
}

impl<T, C: cfg::Config> Drop for Array<T, C> {
    fn drop(&mut self) {
        // XXX(eliza): this could be `with_mut` if we wanted to impl a wrapper for std atomics to change `get_mut` to `with_mut`...
        let max = self.max.load(Acquire);
        for shard in &self.shards[0..=max] {
            // XXX(eliza): this could be `with_mut` if we wanted to impl a wrapper for std atomics to change `get_mut` to `with_mut`...
            let ptr = shard.0.load(Acquire);
            if ptr.is_null() {
                continue;
            }
            let shard = unsafe {
                // Safety: this is the only place where these boxes are
                // deallocated, and we have exclusive access to the shard array,
                // because...we are dropping it...
                Box::from_raw(ptr)
            };
            drop(shard)
        }
    }
}

impl<T: fmt::Debug, C: cfg::Config> fmt::Debug for Array<T, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let max = self.max.load(Acquire);
        let mut set = f.debug_map();
        for shard in &self.shards[0..=max] {
            let ptr = shard.0.load(Acquire);
            if let Some(shard) = ptr::NonNull::new(ptr) {
                set.entry(&format_args!("{:p}", ptr), unsafe { shard.as_ref() });
            } else {
                set.entry(&format_args!("{:p}", ptr), &());
            }
        }
        set.finish()
    }
}

// === impl Ptr ===

impl<T, C: cfg::Config> Ptr<T, C> {
    #[inline]
    fn null() -> Self {
        Self(AtomicPtr::new(ptr::null_mut()))
    }

    #[inline]
    fn load(&self, order: Ordering) -> Option<&Shard<T, C>> {
        let ptr = self.0.load(order);
        test_println!("---> loaded={:p} (order={:?})", ptr, order);
        if ptr.is_null() {
            test_println!("---> null");
            return None;
        }
        let track = unsafe {
            // Safety: The returned reference will have the same lifetime as the
            // reference to the shard pointer, which (morally, if not actually)
            // owns the shard. The shard is only deallocated when the shard
            // array is dropped, and it won't be dropped while this pointer is
            // borrowed --- and the returned reference has the same lifetime.
            //
            // We know that the pointer is not null, because we just
            // null-checked it immediately prior.
            &*ptr
        };

        Some(track.get_ref())
    }

    #[inline]
    fn set(&self, new: *mut alloc::Track<Shard<T, C>>) {
        self.0
            .compare_exchange(ptr::null_mut(), new, AcqRel, Acquire)
            .expect("a shard can only be inserted by the thread that owns it, this is a bug!");
    }
}

// === Iterators ===

impl<'a, T, C> Iterator for IterMut<'a, T, C>
where
    T: 'a,
    C: cfg::Config + 'a,
{
    type Item = &'a Shard<T, C>;
    fn next(&mut self) -> Option<Self::Item> {
        test_println!("IterMut::next");
        loop {
            // Skip over empty indices if they are less than the highest
            // allocated shard. Some threads may have accessed the slab
            // (generating a thread ID) but never actually inserted data, so
            // they may have never allocated a shard.
            let next = self.0.next();
            test_println!("-> next.is_some={}", next.is_some());
            if let Some(shard) = next?.load(Acquire) {
                test_println!("-> done");
                return Some(shard);
            }
        }
    }
}
