//! Either `AtomicU64` or emulating `AtomicU64` through a `Mutex`.

// Use the `AtomicU64` from the standard library if we're on a platform that supports atomic
// 64-bit operations.
#[cfg(target_has_atomic = "64")]
pub(crate) use std::sync::atomic::AtomicU64;

#[cfg(not(target_has_atomic = "64"))]
mod impl_ {
    use std::sync::atomic::Ordering;
    use std::sync::Mutex;

    #[derive(Debug)]
    pub(crate) struct AtomicU64(Mutex<u64>);

    impl AtomicU64 {
        pub(crate) fn new(val: u64) -> Self {
            Self(Mutex::new(val))
        }

        pub(crate) fn load(&self, _: Ordering) -> u64 {
            *self.0.lock().unwrap()
        }

        pub(crate) fn fetch_max(&self, val: u64, _: Ordering) -> u64 {
            let mut lock = self.0.lock().unwrap();
            let old = *lock;
            *lock = old.max(val);
            old
        }
    }
}

#[cfg(not(target_has_atomic = "64"))]
pub(crate) use self::impl_::AtomicU64;
