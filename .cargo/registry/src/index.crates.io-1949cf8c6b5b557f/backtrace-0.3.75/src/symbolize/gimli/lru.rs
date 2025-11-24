use core::mem::{self, MaybeUninit};
use core::ptr;

/// least-recently-used cache with static size
pub(crate) struct Lru<T, const N: usize> {
    // SAFETY: len <= initialized values
    len: usize,
    arr: [MaybeUninit<T>; N],
}

impl<T, const N: usize> Default for Lru<T, N> {
    fn default() -> Self {
        Lru {
            len: 0,
            arr: [const { MaybeUninit::uninit() }; N],
        }
    }
}

impl<T, const N: usize> Lru<T, N> {
    #[inline]
    pub fn clear(&mut self) {
        let len = self.len;
        self.len = 0;
        // SAFETY: we can't touch these values again due to setting self.len = 0
        unsafe { ptr::drop_in_place(ptr::addr_of_mut!(self.arr[0..len]) as *mut [T]) }
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.arr[0..self.len]
            .iter()
            // SAFETY: we only iterate initialized values due to our len invariant
            .map(|init| unsafe { init.assume_init_ref() })
    }

    #[inline]
    pub fn push_front(&mut self, value: T) -> Option<&mut T> {
        if N == 0 {
            return None;
        } else if self.len == N {
            self.len = N - 1;
            // SAFETY: we maintain len invariant and bail on N == 0
            unsafe { ptr::drop_in_place(self.arr.as_mut_ptr().cast::<T>().add(N - 1)) };
        };
        let len_to_init = self.len + 1;
        let mut last = MaybeUninit::new(value);
        for elem in self.arr[0..len_to_init].iter_mut() {
            // OPT(size): using `mem::swap` allows surprising size regressions
            last = mem::replace(elem, last);
        }
        self.len = len_to_init;

        self.arr
            .first_mut()
            // SAFETY: we just pushed it
            .map(|elem| unsafe { elem.assume_init_mut() })
    }

    #[inline]
    pub fn move_to_front(&mut self, idx: usize) -> Option<&mut T> {
        let elem = self.arr[0..self.len].get_mut(idx)?;
        // SAFETY: we already bailed if the index is bad, so our slicing will be infallible,
        // so it is permissible to allow the len invariant to decay, as we always restore it
        let mut last = mem::replace(elem, MaybeUninit::uninit());
        for elem in self.arr[0..=idx].iter_mut() {
            // OPT(size): using `mem::swap` allows surprising size regressions
            last = mem::replace(elem, last);
        }
        self.arr
            .first_mut()
            // SAFETY: we have restored the len invariant
            .map(|elem| unsafe { elem.assume_init_mut() })
    }
}
