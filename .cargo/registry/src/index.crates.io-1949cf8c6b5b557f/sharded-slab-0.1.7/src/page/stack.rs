use crate::cfg;
use crate::sync::atomic::{AtomicUsize, Ordering};
use std::{fmt, marker::PhantomData};

pub(super) struct TransferStack<C = cfg::DefaultConfig> {
    head: AtomicUsize,
    _cfg: PhantomData<fn(C)>,
}

impl<C: cfg::Config> TransferStack<C> {
    pub(super) fn new() -> Self {
        Self {
            head: AtomicUsize::new(super::Addr::<C>::NULL),
            _cfg: PhantomData,
        }
    }

    pub(super) fn pop_all(&self) -> Option<usize> {
        let val = self.head.swap(super::Addr::<C>::NULL, Ordering::Acquire);
        test_println!("-> pop {:#x}", val);
        if val == super::Addr::<C>::NULL {
            None
        } else {
            Some(val)
        }
    }

    fn push(&self, new_head: usize, before: impl Fn(usize)) {
        // We loop to win the race to set the new head. The `next` variable
        // is the next slot on the stack which needs to be pointed to by the
        // new head.
        let mut next = self.head.load(Ordering::Relaxed);
        loop {
            test_println!("-> next {:#x}", next);
            before(next);

            match self
                .head
                .compare_exchange(next, new_head, Ordering::Release, Ordering::Relaxed)
            {
                // lost the race!
                Err(actual) => {
                    test_println!("-> retry!");
                    next = actual;
                }
                Ok(_) => {
                    test_println!("-> successful; next={:#x}", next);
                    return;
                }
            }
        }
    }
}

impl<C: cfg::Config> super::FreeList<C> for TransferStack<C> {
    fn push<T>(&self, new_head: usize, slot: &super::Slot<T, C>) {
        self.push(new_head, |next| slot.set_next(next))
    }
}

impl<C> fmt::Debug for TransferStack<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransferStack")
            .field(
                "head",
                &format_args!("{:#0x}", &self.head.load(Ordering::Relaxed)),
            )
            .finish()
    }
}

#[cfg(all(loom, test))]
mod test {
    use super::*;
    use crate::{sync::UnsafeCell, test_util};
    use loom::thread;
    use std::sync::Arc;

    #[test]
    fn transfer_stack() {
        test_util::run_model("transfer_stack", || {
            let causalities = [UnsafeCell::new(999), UnsafeCell::new(999)];
            let shared = Arc::new((causalities, TransferStack::<cfg::DefaultConfig>::new()));
            let shared1 = shared.clone();
            let shared2 = shared.clone();

            let t1 = thread::spawn(move || {
                let (causalities, stack) = &*shared1;
                stack.push(0, |prev| {
                    causalities[0].with_mut(|c| unsafe {
                        *c = 0;
                    });
                    test_println!("prev={:#x}", prev)
                });
            });
            let t2 = thread::spawn(move || {
                let (causalities, stack) = &*shared2;
                stack.push(1, |prev| {
                    causalities[1].with_mut(|c| unsafe {
                        *c = 1;
                    });
                    test_println!("prev={:#x}", prev)
                });
            });

            let (causalities, stack) = &*shared;
            let mut idx = stack.pop_all();
            while idx == None {
                idx = stack.pop_all();
                thread::yield_now();
            }
            let idx = idx.unwrap();
            causalities[idx].with(|val| unsafe {
                assert_eq!(
                    *val, idx,
                    "UnsafeCell write must happen-before index is pushed to the stack!"
                );
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }
}
