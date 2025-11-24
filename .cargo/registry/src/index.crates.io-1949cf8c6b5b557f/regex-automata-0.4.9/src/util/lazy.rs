/*!
A lazily initialized value for safe sharing between threads.

The principal type in this module is `Lazy`, which makes it easy to construct
values that are shared safely across multiple threads simultaneously.
*/

use core::fmt;

/// A lazily initialized value that implements `Deref` for `T`.
///
/// A `Lazy` takes an initialization function and permits callers from any
/// thread to access the result of that initialization function in a safe
/// manner. In effect, this permits one-time initialization of global resources
/// in a (possibly) multi-threaded program.
///
/// This type and its functionality are available even when neither the `alloc`
/// nor the `std` features are enabled. In exchange, a `Lazy` does **not**
/// guarantee that the given `create` function is called at most once. It
/// might be called multiple times. Moreover, a call to `Lazy::get` (either
/// explicitly or implicitly via `Lazy`'s `Deref` impl) may block until a `T`
/// is available.
///
/// This is very similar to `lazy_static` or `once_cell`, except it doesn't
/// guarantee that the initialization function will be run once and it works
/// in no-alloc no-std environments. With that said, if you need stronger
/// guarantees or a more flexible API, then it is recommended to use either
/// `lazy_static` or `once_cell`.
///
/// # Warning: may use a spin lock
///
/// When this crate is compiled _without_ the `alloc` feature, then this type
/// may used a spin lock internally. This can have subtle effects that may
/// be undesirable. See [Spinlocks Considered Harmful][spinharm] for a more
/// thorough treatment of this topic.
///
/// [spinharm]: https://matklad.github.io/2020/01/02/spinlocks-considered-harmful.html
///
/// # Example
///
/// This type is useful for creating regexes once, and then using them from
/// multiple threads simultaneously without worrying about synchronization.
///
/// ```
/// use regex_automata::{dfa::regex::Regex, util::lazy::Lazy, Match};
///
/// static RE: Lazy<Regex> = Lazy::new(|| Regex::new("foo[0-9]+bar").unwrap());
///
/// let expected = Some(Match::must(0, 3..14));
/// assert_eq!(expected, RE.find(b"zzzfoo12345barzzz"));
/// ```
pub struct Lazy<T, F = fn() -> T>(lazy::Lazy<T, F>);

impl<T, F> Lazy<T, F> {
    /// Create a new `Lazy` value that is initialized via the given function.
    ///
    /// The `T` type is automatically inferred from the return type of the
    /// `create` function given.
    pub const fn new(create: F) -> Lazy<T, F> {
        Lazy(lazy::Lazy::new(create))
    }
}

impl<T, F: Fn() -> T> Lazy<T, F> {
    /// Return a reference to the lazily initialized value.
    ///
    /// This routine may block if another thread is initializing a `T`.
    ///
    /// Note that given a `x` which has type `Lazy`, this must be called via
    /// `Lazy::get(x)` and not `x.get()`. This routine is defined this way
    /// because `Lazy` impls `Deref` with a target of `T`.
    ///
    /// # Panics
    ///
    /// This panics if the `create` function inside this lazy value panics.
    /// If the panic occurred in another thread, then this routine _may_ also
    /// panic (but is not guaranteed to do so).
    pub fn get(this: &Lazy<T, F>) -> &T {
        this.0.get()
    }
}

impl<T, F: Fn() -> T> core::ops::Deref for Lazy<T, F> {
    type Target = T;

    fn deref(&self) -> &T {
        Lazy::get(self)
    }
}

impl<T: fmt::Debug, F: Fn() -> T> fmt::Debug for Lazy<T, F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(feature = "alloc")]
mod lazy {
    use core::{
        fmt,
        marker::PhantomData,
        sync::atomic::{AtomicPtr, Ordering},
    };

    use alloc::boxed::Box;

    /// A non-std lazy initialized value.
    ///
    /// This might run the initialization function more than once, but will
    /// never block.
    ///
    /// I wish I could get these semantics into the non-alloc non-std Lazy
    /// type below, but I'm not sure how to do it. If you can do an alloc,
    /// then the implementation becomes very simple if you don't care about
    /// redundant work precisely because a pointer can be atomically swapped.
    ///
    /// Perhaps making this approach work in the non-alloc non-std case
    /// requires asking the caller for a pointer? It would make the API less
    /// convenient I think.
    pub(super) struct Lazy<T, F> {
        data: AtomicPtr<T>,
        create: F,
        // This indicates to the compiler that this type can drop T. It's not
        // totally clear how the absence of this marker could lead to trouble,
        // but putting here doesn't have any downsides so we hedge until somone
        // can from the Unsafe Working Group can tell us definitively that we
        // don't need it.
        //
        // See: https://github.com/BurntSushi/regex-automata/issues/30
        owned: PhantomData<Box<T>>,
    }

    // SAFETY: So long as T and &T (and F and &F) can themselves be safely
    // shared among threads, so to can a Lazy<T, _>. Namely, the Lazy API only
    // permits accessing a &T and initialization is free of data races. So if T
    // is thread safe, then so to is Lazy<T, _>.
    //
    // We specifically require that T: Send in order for Lazy<T> to be Sync.
    // Without that requirement, it's possible to send a T from one thread to
    // another via Lazy's destructor.
    //
    // It's not clear whether we need F: Send+Sync for Lazy to be Sync. But
    // we're conservative for now and keep both.
    unsafe impl<T: Send + Sync, F: Send + Sync> Sync for Lazy<T, F> {}

    impl<T, F> Lazy<T, F> {
        /// Create a new alloc but non-std lazy value that is racily
        /// initialized. That is, the 'create' function may be called more than
        /// once.
        pub(super) const fn new(create: F) -> Lazy<T, F> {
            Lazy {
                data: AtomicPtr::new(core::ptr::null_mut()),
                create,
                owned: PhantomData,
            }
        }
    }

    impl<T, F: Fn() -> T> Lazy<T, F> {
        /// Get the underlying lazy value. If it hasn't been initialized
        /// yet, then always attempt to initialize it (even if some other
        /// thread is initializing it) and atomically attach it to this lazy
        /// value before returning it.
        pub(super) fn get(&self) -> &T {
            if let Some(data) = self.poll() {
                return data;
            }
            let data = (self.create)();
            let mut ptr = Box::into_raw(Box::new(data));
            // We attempt to stuff our initialized value into our atomic
            // pointer. Upon success, we don't need to do anything. But if
            // someone else beat us to the punch, then we need to make sure
            // our newly created value is dropped.
            let result = self.data.compare_exchange(
                core::ptr::null_mut(),
                ptr,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            if let Err(old) = result {
                // SAFETY: We created 'ptr' via Box::into_raw above, so turning
                // it back into a Box via from_raw is safe.
                drop(unsafe { Box::from_raw(ptr) });
                ptr = old;
            }
            // SAFETY: We just set the pointer above to a non-null value, even
            // in the error case, and set it to a fully initialized value
            // returned by 'create'.
            unsafe { &*ptr }
        }

        /// If this lazy value has been initialized successfully, then return
        /// that value. Otherwise return None immediately. This never attempts
        /// to run initialization itself.
        fn poll(&self) -> Option<&T> {
            let ptr = self.data.load(Ordering::Acquire);
            if ptr.is_null() {
                return None;
            }
            // SAFETY: We just checked that the pointer is not null. Since it's
            // not null, it must have been fully initialized by 'get' at some
            // point.
            Some(unsafe { &*ptr })
        }
    }

    impl<T: fmt::Debug, F: Fn() -> T> fmt::Debug for Lazy<T, F> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.debug_struct("Lazy").field("data", &self.poll()).finish()
        }
    }

    impl<T, F> Drop for Lazy<T, F> {
        fn drop(&mut self) {
            let ptr = *self.data.get_mut();
            if !ptr.is_null() {
                // SAFETY: We just checked that 'ptr' is not null. And since
                // we have exclusive access, there are no races to worry about.
                drop(unsafe { Box::from_raw(ptr) });
            }
        }
    }
}

#[cfg(not(feature = "alloc"))]
mod lazy {
    use core::{
        cell::Cell,
        fmt,
        mem::MaybeUninit,
        panic::{RefUnwindSafe, UnwindSafe},
        sync::atomic::{AtomicU8, Ordering},
    };

    /// Our 'Lazy' value can be in one of three states:
    ///
    /// * INIT is where it starts, and also ends up back here if the
    /// 'create' routine panics.
    /// * BUSY is where it sits while initialization is running in exactly
    /// one thread.
    /// * DONE is where it sits after 'create' has completed and 'data' has
    /// been fully initialized.
    const LAZY_STATE_INIT: u8 = 0;
    const LAZY_STATE_BUSY: u8 = 1;
    const LAZY_STATE_DONE: u8 = 2;

    /// A non-alloc non-std lazy initialized value.
    ///
    /// This guarantees initialization only happens once, but uses a spinlock
    /// to block in the case of simultaneous access. Blocking occurs so that
    /// one thread waits while another thread initializes the value.
    ///
    /// I would much rather have the semantics of the 'alloc' Lazy type above.
    /// Namely, that we might run the initialization function more than once,
    /// but we never otherwise block. However, I don't know how to do that in
    /// a non-alloc non-std context.
    pub(super) struct Lazy<T, F> {
        state: AtomicU8,
        create: Cell<Option<F>>,
        data: Cell<MaybeUninit<T>>,
    }

    // SAFETY: So long as T and &T (and F and &F) can themselves be safely
    // shared among threads, so to can a Lazy<T, _>. Namely, the Lazy API only
    // permits accessing a &T and initialization is free of data races. So if T
    // is thread safe, then so to is Lazy<T, _>.
    unsafe impl<T: Send + Sync, F: Send + Sync> Sync for Lazy<T, F> {}
    // A reference to a Lazy is unwind safe because we specifically take
    // precautions to poison all accesses to a Lazy if the caller-provided
    // 'create' function panics.
    impl<T: UnwindSafe, F: UnwindSafe + RefUnwindSafe> RefUnwindSafe
        for Lazy<T, F>
    {
    }

    impl<T, F> Lazy<T, F> {
        /// Create a new non-alloc non-std lazy value that is initialized
        /// exactly once on first use using the given function.
        pub(super) const fn new(create: F) -> Lazy<T, F> {
            Lazy {
                state: AtomicU8::new(LAZY_STATE_INIT),
                create: Cell::new(Some(create)),
                data: Cell::new(MaybeUninit::uninit()),
            }
        }
    }

    impl<T, F: FnOnce() -> T> Lazy<T, F> {
        /// Get the underlying lazy value. If it isn't been initialized
        /// yet, then either initialize it or block until some other thread
        /// initializes it. If the 'create' function given to Lazy::new panics
        /// (even in another thread), then this panics too.
        pub(super) fn get(&self) -> &T {
            // This is effectively a spinlock. We loop until we enter a DONE
            // state, and if possible, initialize it ourselves. The only way
            // we exit the loop is if 'create' panics, we initialize 'data' or
            // some other thread initializes 'data'.
            //
            // Yes, I have read spinlocks considered harmful[1]. And that
            // article is why this spinlock is only active when 'alloc' isn't
            // enabled. I did this because I don't think there is really
            // another choice without 'alloc', other than not providing this at
            // all. But I think that's a big bummer.
            //
            // [1]: https://matklad.github.io/2020/01/02/spinlocks-considered-harmful.html
            while self.state.load(Ordering::Acquire) != LAZY_STATE_DONE {
                // Check if we're the first ones to get here. If so, we'll be
                // the ones who initialize.
                let result = self.state.compare_exchange(
                    LAZY_STATE_INIT,
                    LAZY_STATE_BUSY,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
                // This means we saw the INIT state and nobody else can. So we
                // must take responsibility for initializing. And by virtue of
                // observing INIT, we have also told anyone else trying to
                // get here that we are BUSY. If someone else sees BUSY, then
                // they will spin until we finish initialization.
                if let Ok(_) = result {
                    // Since we are guaranteed to be the only ones here, we
                    // know that 'create' is there... Unless someone else got
                    // here before us and 'create' panicked. In which case,
                    // 'self.create' is now 'None' and we forward the panic
                    // to the caller. (i.e., We implement poisoning.)
                    //
                    // SAFETY: Our use of 'self.state' guarantees that we are
                    // the only thread executing this line, and thus there are
                    // no races.
                    let create = unsafe {
                        (*self.create.as_ptr()).take().expect(
                            "Lazy's create function panicked, \
                             preventing initialization,
                             poisoning current thread",
                        )
                    };
                    let guard = Guard { state: &self.state };
                    // SAFETY: Our use of 'self.state' guarantees that we are
                    // the only thread executing this line, and thus there are
                    // no races.
                    unsafe {
                        (*self.data.as_ptr()).as_mut_ptr().write(create());
                    }
                    // All is well. 'self.create' ran successfully, so we
                    // forget the guard.
                    core::mem::forget(guard);
                    // Everything is initialized, so we can declare success.
                    self.state.store(LAZY_STATE_DONE, Ordering::Release);
                    break;
                }
                core::hint::spin_loop();
            }
            // We only get here if data is fully initialized, and thus poll
            // will always return something.
            self.poll().unwrap()
        }

        /// If this lazy value has been initialized successfully, then return
        /// that value. Otherwise return None immediately. This never blocks.
        fn poll(&self) -> Option<&T> {
            if self.state.load(Ordering::Acquire) == LAZY_STATE_DONE {
                // SAFETY: The DONE state only occurs when data has been fully
                // initialized.
                Some(unsafe { &*(*self.data.as_ptr()).as_ptr() })
            } else {
                None
            }
        }
    }

    impl<T: fmt::Debug, F: FnMut() -> T> fmt::Debug for Lazy<T, F> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.debug_struct("Lazy")
                .field("state", &self.state.load(Ordering::Acquire))
                .field("create", &"<closure>")
                .field("data", &self.poll())
                .finish()
        }
    }

    impl<T, F> Drop for Lazy<T, F> {
        fn drop(&mut self) {
            if *self.state.get_mut() == LAZY_STATE_DONE {
                // SAFETY: state is DONE if and only if data has been fully
                // initialized. At which point, it is safe to drop.
                unsafe {
                    self.data.get_mut().assume_init_drop();
                }
            }
        }
    }

    /// A guard that will reset a Lazy's state back to INIT when dropped. The
    /// idea here is to 'forget' this guard on success. On failure (when a
    /// panic occurs), the Drop impl runs and causes all in-progress and future
    /// 'get' calls to panic. Without this guard, all in-progress and future
    /// 'get' calls would spin forever. Crashing is much better than getting
    /// stuck in an infinite loop.
    struct Guard<'a> {
        state: &'a AtomicU8,
    }

    impl<'a> Drop for Guard<'a> {
        fn drop(&mut self) {
            // We force ourselves back into an INIT state. This will in turn
            // cause any future 'get' calls to attempt calling 'self.create'
            // again which will in turn panic because 'self.create' will now
            // be 'None'.
            self.state.store(LAZY_STATE_INIT, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    fn assert_unwind<T: core::panic::UnwindSafe>() {}
    fn assert_refunwind<T: core::panic::RefUnwindSafe>() {}

    #[test]
    fn oibits() {
        assert_send::<Lazy<u64>>();
        assert_sync::<Lazy<u64>>();
        assert_unwind::<Lazy<u64>>();
        assert_refunwind::<Lazy<u64>>();
    }

    // This is a regression test because we used to rely on the inferred Sync
    // impl for the Lazy type defined above (for 'alloc' mode). In the
    // inferred impl, it only requires that T: Sync for Lazy<T>: Sync. But
    // if we have that, we can actually make use of the fact that Lazy<T> drops
    // T to create a value on one thread and drop it on another. This *should*
    // require T: Send, but our missing bounds before let it sneak by.
    //
    // Basically, this test should not compile, so we... comment it out. We
    // don't have a great way of testing compile-fail tests right now.
    //
    // See: https://github.com/BurntSushi/regex-automata/issues/30
    /*
    #[test]
    fn sync_not_send() {
        #[allow(dead_code)]
        fn inner<T: Sync + Default>() {
            let lazy = Lazy::new(move || T::default());
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    Lazy::get(&lazy); // We create T in this thread
                });
            });
            // And drop in this thread.
            drop(lazy);
            // So we have send a !Send type over threads. (with some more
            // legwork, its possible to even sneak the value out of drop
            // through thread local)
        }
    }
    */
}
