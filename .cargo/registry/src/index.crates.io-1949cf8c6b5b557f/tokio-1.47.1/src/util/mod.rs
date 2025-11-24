cfg_io_driver! {
    pub(crate) mod bit;
}

#[cfg(feature = "fs")]
pub(crate) mod as_ref;

#[cfg(feature = "rt")]
pub(crate) mod atomic_cell;

#[cfg(feature = "net")]
mod blocking_check;
#[cfg(feature = "net")]
#[allow(unused_imports)]
pub(crate) use blocking_check::check_socket_for_blocking;

pub(crate) mod metric_atomics;

mod wake;
pub(crate) use wake::{waker, Wake};

#[cfg(any(
    // io driver uses `WakeList` directly
    feature = "net",
    feature = "process",
    // `sync` enables `Notify` and `batch_semaphore`, which require `WakeList`.
    feature = "sync",
    // `fs` uses `batch_semaphore`, which requires `WakeList`.
    feature = "fs",
    // rt and signal use `Notify`, which requires `WakeList`.
    feature = "rt",
    feature = "signal",
    // time driver uses `WakeList` in `Handle::process_at_time`.
    feature = "time",
))]
mod wake_list;
#[cfg(any(
    feature = "net",
    feature = "process",
    feature = "sync",
    feature = "fs",
    feature = "rt",
    feature = "signal",
    feature = "time",
))]
pub(crate) use wake_list::WakeList;

#[cfg(any(
    feature = "fs",
    feature = "net",
    feature = "process",
    feature = "rt",
    feature = "sync",
    feature = "signal",
    feature = "time",
    fuzzing,
))]
pub(crate) mod linked_list;

cfg_rt! {
    pub(crate) mod sharded_list;
}

#[cfg(any(feature = "rt", feature = "macros"))]
pub(crate) mod rand;

cfg_rt! {
    mod idle_notified_set;
    pub(crate) use idle_notified_set::IdleNotifiedSet;

    pub(crate) use self::rand::RngSeedGenerator;

    pub(crate) use wake::{waker_ref, WakerRef};

    mod sync_wrapper;
    pub(crate) use sync_wrapper::SyncWrapper;

    mod rc_cell;
    pub(crate) use rc_cell::RcCell;
}

cfg_rt_multi_thread! {
    mod try_lock;
    pub(crate) use try_lock::TryLock;
}

pub(crate) mod trace;

#[cfg(feature = "fs")]
pub(crate) mod typeid;

pub(crate) mod error;

#[cfg(feature = "io-util")]
pub(crate) mod memchr;

pub(crate) mod markers;

pub(crate) mod cacheline;

cfg_io_driver_impl! {
    pub(crate) mod ptr_expose;
}

use std::{ops::DerefMut, pin::Pin};

/// Copy of [`std::pin::Pin::as_deref_mut`].
// TODO: Remove this once we bump the MSRV to 1.84.
pub(crate) fn pin_as_deref_mut<P: DerefMut>(ptr: Pin<&mut Pin<P>>) -> Pin<&mut P::Target> {
    unsafe { ptr.get_unchecked_mut() }.as_mut()
}
