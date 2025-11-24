use core::mem::MaybeUninit;
use core::ptr;
use std::task::Waker;

const NUM_WAKERS: usize = 32;

/// A list of wakers to be woken.
///
/// # Invariants
///
/// The first `curr` elements of `inner` are initialized.
pub(crate) struct WakeList {
    inner: [MaybeUninit<Waker>; NUM_WAKERS],
    curr: usize,
}

impl WakeList {
    pub(crate) fn new() -> Self {
        const UNINIT_WAKER: MaybeUninit<Waker> = MaybeUninit::uninit();

        Self {
            inner: [UNINIT_WAKER; NUM_WAKERS],
            curr: 0,
        }
    }

    #[inline]
    pub(crate) fn can_push(&self) -> bool {
        self.curr < NUM_WAKERS
    }

    pub(crate) fn push(&mut self, val: Waker) {
        debug_assert!(self.can_push());

        self.inner[self.curr] = MaybeUninit::new(val);
        self.curr += 1;
    }

    pub(crate) fn wake_all(&mut self) {
        struct DropGuard {
            start: *mut Waker,
            end: *mut Waker,
        }

        impl Drop for DropGuard {
            fn drop(&mut self) {
                // SAFETY: Both pointers are part of the same object, with `start <= end`.
                let len = unsafe { self.end.offset_from(self.start) } as usize;
                let slice = ptr::slice_from_raw_parts_mut(self.start, len);
                // SAFETY: All elements in `start..len` are initialized, so we can drop them.
                unsafe { ptr::drop_in_place(slice) };
            }
        }

        debug_assert!(self.curr <= NUM_WAKERS);

        let mut guard = {
            let start = self.inner.as_mut_ptr().cast::<Waker>();
            // SAFETY: The resulting pointer is in bounds or one after the length of the same object.
            let end = unsafe { start.add(self.curr) };
            // Transfer ownership of the wakers in `inner` to `DropGuard`.
            self.curr = 0;
            DropGuard { start, end }
        };
        while !ptr::eq(guard.start, guard.end) {
            // SAFETY: `start` is always initialized if `start != end`.
            let waker = unsafe { ptr::read(guard.start) };
            // SAFETY: The resulting pointer is in bounds or one after the length of the same object.
            guard.start = unsafe { guard.start.add(1) };
            // If this panics, then `guard` will clean up the remaining wakers.
            waker.wake();
        }
    }
}

impl Drop for WakeList {
    fn drop(&mut self) {
        let slice =
            ptr::slice_from_raw_parts_mut(self.inner.as_mut_ptr().cast::<Waker>(), self.curr);
        // SAFETY: The first `curr` elements are initialized, so we can drop them.
        unsafe { ptr::drop_in_place(slice) };
    }
}
