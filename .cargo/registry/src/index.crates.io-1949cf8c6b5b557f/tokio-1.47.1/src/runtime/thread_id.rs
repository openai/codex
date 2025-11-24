use std::num::NonZeroU64;

#[derive(Eq, PartialEq, Clone, Copy, Hash, Debug)]
pub(crate) struct ThreadId(NonZeroU64);

impl ThreadId {
    pub(crate) fn next() -> Self {
        use crate::loom::sync::atomic::{Ordering::Relaxed, StaticAtomicU64};

        static NEXT_ID: StaticAtomicU64 = StaticAtomicU64::new(0);

        let mut last = NEXT_ID.load(Relaxed);
        loop {
            let id = match last.checked_add(1) {
                Some(id) => id,
                None => exhausted(),
            };

            match NEXT_ID.compare_exchange_weak(last, id, Relaxed, Relaxed) {
                Ok(_) => return ThreadId(NonZeroU64::new(id).unwrap()),
                Err(id) => last = id,
            }
        }
    }
}

#[cold]
#[allow(dead_code)]
fn exhausted() -> ! {
    panic!("failed to generate unique thread ID: bitspace exhausted")
}
