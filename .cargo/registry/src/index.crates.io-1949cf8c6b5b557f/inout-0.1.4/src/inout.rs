use crate::InOutBuf;
use core::{marker::PhantomData, ptr};
use generic_array::{ArrayLength, GenericArray};

/// Custom pointer type which contains one immutable (input) and one mutable
/// (output) pointer, which are either equal or non-overlapping.
pub struct InOut<'inp, 'out, T> {
    pub(crate) in_ptr: *const T,
    pub(crate) out_ptr: *mut T,
    pub(crate) _pd: PhantomData<(&'inp T, &'out mut T)>,
}

impl<'inp, 'out, T> InOut<'inp, 'out, T> {
    /// Reborrow `self`.
    #[inline(always)]
    pub fn reborrow<'a>(&'a mut self) -> InOut<'a, 'a, T> {
        Self {
            in_ptr: self.in_ptr,
            out_ptr: self.out_ptr,
            _pd: PhantomData,
        }
    }

    /// Get immutable reference to the input value.
    #[inline(always)]
    pub fn get_in<'a>(&'a self) -> &'a T {
        unsafe { &*self.in_ptr }
    }

    /// Get mutable reference to the output value.
    #[inline(always)]
    pub fn get_out<'a>(&'a mut self) -> &'a mut T {
        unsafe { &mut *self.out_ptr }
    }

    /// Convert `self` to a pair of raw input and output pointers.
    #[inline(always)]
    pub fn into_raw(self) -> (*const T, *mut T) {
        (self.in_ptr, self.out_ptr)
    }

    /// Create `InOut` from raw input and output pointers.
    ///
    /// # Safety
    /// Behavior is undefined if any of the following conditions are violated:
    /// - `in_ptr` must point to a properly initialized value of type `T` and
    /// must be valid for reads.
    /// - `out_ptr` must point to a properly initialized value of type `T` and
    /// must be valid for both reads and writes.
    /// - `in_ptr` and `out_ptr` must be either equal or non-overlapping.
    /// - If `in_ptr` and `out_ptr` are equal, then the memory referenced by
    /// them must not be accessed through any other pointer (not derived from
    /// the return value) for the duration of lifetime 'a. Both read and write
    /// accesses are forbidden.
    /// - If `in_ptr` and `out_ptr` are not equal, then the memory referenced by
    /// `out_ptr` must not be accessed through any other pointer (not derived from
    /// the return value) for the duration of lifetime `'a`. Both read and write
    /// accesses are forbidden. The memory referenced by `in_ptr` must not be
    /// mutated for the duration of lifetime `'a`, except inside an `UnsafeCell`.
    #[inline(always)]
    pub unsafe fn from_raw(in_ptr: *const T, out_ptr: *mut T) -> InOut<'inp, 'out, T> {
        Self {
            in_ptr,
            out_ptr,
            _pd: PhantomData,
        }
    }
}

impl<'inp, 'out, T: Clone> InOut<'inp, 'out, T> {
    /// Clone input value and return it.
    #[inline(always)]
    pub fn clone_in(&self) -> T {
        unsafe { (&*self.in_ptr).clone() }
    }
}

impl<'a, T> From<&'a mut T> for InOut<'a, 'a, T> {
    #[inline(always)]
    fn from(val: &'a mut T) -> Self {
        let p = val as *mut T;
        Self {
            in_ptr: p,
            out_ptr: p,
            _pd: PhantomData,
        }
    }
}

impl<'inp, 'out, T> From<(&'inp T, &'out mut T)> for InOut<'inp, 'out, T> {
    #[inline(always)]
    fn from((in_val, out_val): (&'inp T, &'out mut T)) -> Self {
        Self {
            in_ptr: in_val as *const T,
            out_ptr: out_val as *mut T,
            _pd: Default::default(),
        }
    }
}

impl<'inp, 'out, T, N: ArrayLength<T>> InOut<'inp, 'out, GenericArray<T, N>> {
    /// Returns `InOut` for the given position.
    ///
    /// # Panics
    /// If `pos` greater or equal to array length.
    #[inline(always)]
    pub fn get<'a>(&'a mut self, pos: usize) -> InOut<'a, 'a, T> {
        assert!(pos < N::USIZE);
        unsafe {
            InOut {
                in_ptr: (self.in_ptr as *const T).add(pos),
                out_ptr: (self.out_ptr as *mut T).add(pos),
                _pd: PhantomData,
            }
        }
    }

    /// Convert `InOut` array to `InOutBuf`.
    #[inline(always)]
    pub fn into_buf(self) -> InOutBuf<'inp, 'out, T> {
        InOutBuf {
            in_ptr: self.in_ptr as *const T,
            out_ptr: self.out_ptr as *mut T,
            len: N::USIZE,
            _pd: PhantomData,
        }
    }
}

impl<'inp, 'out, N: ArrayLength<u8>> InOut<'inp, 'out, GenericArray<u8, N>> {
    /// XOR `data` with values behind the input slice and write
    /// result to the output slice.
    ///
    /// # Panics
    /// If `data` length is not equal to the buffer length.
    #[inline(always)]
    #[allow(clippy::needless_range_loop)]
    pub fn xor_in2out(&mut self, data: &GenericArray<u8, N>) {
        unsafe {
            let input = ptr::read(self.in_ptr);
            let mut temp = GenericArray::<u8, N>::default();
            for i in 0..N::USIZE {
                temp[i] = input[i] ^ data[i];
            }
            ptr::write(self.out_ptr, temp);
        }
    }
}

impl<'inp, 'out, N, M> InOut<'inp, 'out, GenericArray<GenericArray<u8, N>, M>>
where
    N: ArrayLength<u8>,
    M: ArrayLength<GenericArray<u8, N>>,
{
    /// XOR `data` with values behind the input slice and write
    /// result to the output slice.
    ///
    /// # Panics
    /// If `data` length is not equal to the buffer length.
    #[inline(always)]
    #[allow(clippy::needless_range_loop)]
    pub fn xor_in2out(&mut self, data: &GenericArray<GenericArray<u8, N>, M>) {
        unsafe {
            let input = ptr::read(self.in_ptr);
            let mut temp = GenericArray::<GenericArray<u8, N>, M>::default();
            for i in 0..M::USIZE {
                for j in 0..N::USIZE {
                    temp[i][j] = input[i][j] ^ data[i][j];
                }
            }
            ptr::write(self.out_ptr, temp);
        }
    }
}
