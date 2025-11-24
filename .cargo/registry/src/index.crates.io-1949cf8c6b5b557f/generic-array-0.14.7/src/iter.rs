//! `GenericArray` iterator implementation.

use super::{ArrayLength, GenericArray};
use core::iter::FusedIterator;
use core::mem::ManuallyDrop;
use core::{cmp, fmt, mem, ptr};

/// An iterator that moves out of a `GenericArray`
pub struct GenericArrayIter<T, N: ArrayLength<T>> {
    // Invariants: index <= index_back <= N
    // Only values in array[index..index_back] are alive at any given time.
    // Values from array[..index] and array[index_back..] are already moved/dropped.
    array: ManuallyDrop<GenericArray<T, N>>,
    index: usize,
    index_back: usize,
}

#[cfg(test)]
mod test {
    use super::*;

    fn send<I: Send>(_iter: I) {}

    #[test]
    fn test_send_iter() {
        send(GenericArray::from([1, 2, 3, 4]).into_iter());
    }
}

impl<T, N> GenericArrayIter<T, N>
where
    N: ArrayLength<T>,
{
    /// Returns the remaining items of this iterator as a slice
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.array.as_slice()[self.index..self.index_back]
    }

    /// Returns the remaining items of this iterator as a mutable slice
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.array.as_mut_slice()[self.index..self.index_back]
    }
}

impl<T, N> IntoIterator for GenericArray<T, N>
where
    N: ArrayLength<T>,
{
    type Item = T;
    type IntoIter = GenericArrayIter<T, N>;

    fn into_iter(self) -> Self::IntoIter {
        GenericArrayIter {
            array: ManuallyDrop::new(self),
            index: 0,
            index_back: N::USIZE,
        }
    }
}

// Based on work in rust-lang/rust#49000
impl<T: fmt::Debug, N> fmt::Debug for GenericArrayIter<T, N>
where
    N: ArrayLength<T>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("GenericArrayIter")
            .field(&self.as_slice())
            .finish()
    }
}

impl<T, N> Drop for GenericArrayIter<T, N>
where
    N: ArrayLength<T>,
{
    #[inline]
    fn drop(&mut self) {
        if mem::needs_drop::<T>() {
            // Drop values that are still alive.
            for p in self.as_mut_slice() {
                unsafe {
                    ptr::drop_in_place(p);
                }
            }
        }
    }
}

// Based on work in rust-lang/rust#49000
impl<T: Clone, N> Clone for GenericArrayIter<T, N>
where
    N: ArrayLength<T>,
{
    fn clone(&self) -> Self {
        // This places all cloned elements at the start of the new array iterator,
        // not at their original indices.

        let mut array = unsafe { ptr::read(&self.array) };
        let mut index_back = 0;

        for (dst, src) in array.as_mut_slice().into_iter().zip(self.as_slice()) {
            unsafe { ptr::write(dst, src.clone()) };
            index_back += 1;
        }

        GenericArrayIter {
            array,
            index: 0,
            index_back,
        }
    }
}

impl<T, N> Iterator for GenericArrayIter<T, N>
where
    N: ArrayLength<T>,
{
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        if self.index < self.index_back {
            let p = unsafe { Some(ptr::read(self.array.get_unchecked(self.index))) };

            self.index += 1;

            p
        } else {
            None
        }
    }

    fn fold<B, F>(mut self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        let ret = unsafe {
            let GenericArrayIter {
                ref array,
                ref mut index,
                index_back,
            } = self;

            let remaining = &array[*index..index_back];

            remaining.iter().fold(init, |acc, src| {
                let value = ptr::read(src);

                *index += 1;

                f(acc, value)
            })
        };

        // ensure the drop happens here after iteration
        drop(self);

        ret
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    fn nth(&mut self, n: usize) -> Option<T> {
        // First consume values prior to the nth.
        let ndrop = cmp::min(n, self.len());

        for p in &mut self.array[self.index..self.index + ndrop] {
            self.index += 1;

            unsafe {
                ptr::drop_in_place(p);
            }
        }

        self.next()
    }

    #[inline]
    fn last(mut self) -> Option<T> {
        // Note, everything else will correctly drop first as `self` leaves scope.
        self.next_back()
    }
}

impl<T, N> DoubleEndedIterator for GenericArrayIter<T, N>
where
    N: ArrayLength<T>,
{
    fn next_back(&mut self) -> Option<T> {
        if self.index < self.index_back {
            self.index_back -= 1;

            unsafe { Some(ptr::read(self.array.get_unchecked(self.index_back))) }
        } else {
            None
        }
    }

    fn rfold<B, F>(mut self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        let ret = unsafe {
            let GenericArrayIter {
                ref array,
                index,
                ref mut index_back,
            } = self;

            let remaining = &array[index..*index_back];

            remaining.iter().rfold(init, |acc, src| {
                let value = ptr::read(src);

                *index_back -= 1;

                f(acc, value)
            })
        };

        // ensure the drop happens here after iteration
        drop(self);

        ret
    }
}

impl<T, N> ExactSizeIterator for GenericArrayIter<T, N>
where
    N: ArrayLength<T>,
{
    fn len(&self) -> usize {
        self.index_back - self.index
    }
}

impl<T, N> FusedIterator for GenericArrayIter<T, N> where N: ArrayLength<T> {}

// TODO: Implement `TrustedLen` when stabilized
