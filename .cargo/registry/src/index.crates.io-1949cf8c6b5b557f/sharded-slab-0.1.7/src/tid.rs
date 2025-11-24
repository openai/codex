use crate::{
    cfg::{self, CfgPrivate},
    page,
    sync::{
        atomic::{AtomicUsize, Ordering},
        lazy_static, thread_local, Mutex,
    },
    Pack,
};
use std::{
    cell::{Cell, UnsafeCell},
    collections::VecDeque,
    fmt,
    marker::PhantomData,
};

/// Uniquely identifies a thread.
pub(crate) struct Tid<C> {
    id: usize,
    _not_send: PhantomData<UnsafeCell<()>>,
    _cfg: PhantomData<fn(C)>,
}

#[derive(Debug)]
struct Registration(Cell<Option<usize>>);

struct Registry {
    next: AtomicUsize,
    free: Mutex<VecDeque<usize>>,
}

lazy_static! {
    static ref REGISTRY: Registry = Registry {
        next: AtomicUsize::new(0),
        free: Mutex::new(VecDeque::new()),
    };
}

thread_local! {
    static REGISTRATION: Registration = Registration::new();
}

// === impl Tid ===

impl<C: cfg::Config> Pack<C> for Tid<C> {
    const LEN: usize = C::MAX_SHARDS.trailing_zeros() as usize + 1;

    type Prev = page::Addr<C>;

    #[inline(always)]
    fn as_usize(&self) -> usize {
        self.id
    }

    #[inline(always)]
    fn from_usize(id: usize) -> Self {
        Self {
            id,
            _not_send: PhantomData,
            _cfg: PhantomData,
        }
    }
}

impl<C: cfg::Config> Tid<C> {
    #[inline]
    pub(crate) fn current() -> Self {
        REGISTRATION
            .try_with(Registration::current)
            .unwrap_or_else(|_| Self::poisoned())
    }

    pub(crate) fn is_current(self) -> bool {
        REGISTRATION
            .try_with(|r| self == r.current::<C>())
            .unwrap_or(false)
    }

    #[inline(always)]
    pub fn new(id: usize) -> Self {
        Self::from_usize(id)
    }
}

impl<C> Tid<C> {
    #[cold]
    fn poisoned() -> Self {
        Self {
            id: std::usize::MAX,
            _not_send: PhantomData,
            _cfg: PhantomData,
        }
    }

    /// Returns true if the local thread ID was accessed while unwinding.
    pub(crate) fn is_poisoned(&self) -> bool {
        self.id == std::usize::MAX
    }
}

impl<C> fmt::Debug for Tid<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_poisoned() {
            f.debug_tuple("Tid")
                .field(&format_args!("<poisoned>"))
                .finish()
        } else {
            f.debug_tuple("Tid")
                .field(&format_args!("{}", self.id))
                .finish()
        }
    }
}

impl<C> PartialEq for Tid<C> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<C> Eq for Tid<C> {}

impl<C: cfg::Config> Clone for Tid<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: cfg::Config> Copy for Tid<C> {}

// === impl Registration ===

impl Registration {
    fn new() -> Self {
        Self(Cell::new(None))
    }

    #[inline(always)]
    fn current<C: cfg::Config>(&self) -> Tid<C> {
        if let Some(tid) = self.0.get().map(Tid::new) {
            return tid;
        }

        self.register()
    }

    #[cold]
    fn register<C: cfg::Config>(&self) -> Tid<C> {
        let id = REGISTRY
            .free
            .lock()
            .ok()
            .and_then(|mut free| {
                if free.len() > 1 {
                    free.pop_front()
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                let id = REGISTRY.next.fetch_add(1, Ordering::AcqRel);
                if id > Tid::<C>::BITS {
                    panic_in_drop!(
                        "creating a new thread ID ({}) would exceed the \
                        maximum number of thread ID bits specified in {} \
                        ({})",
                        id,
                        std::any::type_name::<C>(),
                        Tid::<C>::BITS,
                    );
                }
                id
            });

        self.0.set(Some(id));
        Tid::new(id)
    }
}

// Reusing thread IDs doesn't work under loom, since this `Drop` impl results in
// an access to a `loom` lazy_static while the test is shutting down, which
// panics. T_T
// Just skip TID reuse and use loom's lazy_static macro to ensure we have a
// clean initial TID on every iteration, instead.
#[cfg(not(all(loom, any(feature = "loom", test))))]
impl Drop for Registration {
    fn drop(&mut self) {
        use std::sync::PoisonError;

        if let Some(id) = self.0.get() {
            let mut free_list = REGISTRY.free.lock().unwrap_or_else(PoisonError::into_inner);
            free_list.push_back(id);
        }
    }
}

#[cfg(all(test, not(loom)))]
pub(crate) fn with<R>(tid: usize, f: impl FnOnce() -> R) -> R {
    struct Guard(Option<usize>);

    impl Drop for Guard {
        fn drop(&mut self) {
            REGISTRATION.with(|r| r.0.set(self.0.take()));
        }
    }

    let prev = REGISTRATION.with(|r| r.0.replace(Some(tid)));
    let _guard = Guard(prev);
    f()
}
