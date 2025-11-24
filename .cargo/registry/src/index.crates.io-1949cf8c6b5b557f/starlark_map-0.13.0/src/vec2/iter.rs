/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::marker::PhantomData;
use std::ptr;
use std::ptr::NonNull;
use std::slice;

use dupe::Clone_;

use crate::vec2::Vec2;

/// Iterator over [`Vec2`] elements.
#[derive(Clone_)]
pub struct Iter<'a, A, B> {
    pub(crate) aaa: slice::Iter<'a, A>,
    pub(crate) bbb: NonNull<B>,
    pub(crate) _marker: PhantomData<slice::Iter<'a, B>>,
}

unsafe impl<A: Sync, B: Sync> Sync for Iter<'_, A, B> {}
unsafe impl<A: Sync, B: Sync> Send for Iter<'_, A, B> {}

impl<'s, A, B> Iterator for Iter<'s, A, B> {
    type Item = (&'s A, &'s B);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let a = self.aaa.next()?;
        let b = unsafe { &*self.bbb.as_ptr() };
        self.bbb = unsafe { NonNull::new_unchecked(self.bbb.as_ptr().add(1)) };
        Some((a, b))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.aaa.size_hint()
    }
}

impl<'s, A, B> ExactSizeIterator for Iter<'s, A, B> {
    #[inline]
    fn len(&self) -> usize {
        self.aaa.len()
    }
}

impl<'s, A, B> DoubleEndedIterator for Iter<'s, A, B> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let a = self.aaa.next_back()?;
        let b = unsafe { &*self.bbb.as_ptr().add(self.aaa.len()) };
        Some((a, b))
    }
}

/// Iterator which consumes the [`Vec2`].
pub struct IntoIter<A, B> {
    /// Pointer to the next `A`. Updated as we iterate.
    pub(crate) aaa_begin: NonNull<A>,
    /// Pointer to the next `B`. Updated as we iterate.
    pub(crate) bbb_begin: NonNull<B>,
    /// Pointer to the end of the `bbb`. Updated as we iterate.
    pub(crate) bbb_end: NonNull<B>,
    /// The layout of `Vec2` is `[padding, aaa, bbb]`.
    /// This field is a pointer to `bbb`. Used for `Drop`.
    pub(crate) bbb_ptr: NonNull<B>,
    /// `Vec2` capacity. Used for `Drop`.
    pub(crate) cap: usize,
}

unsafe impl<A: Send, B: Send> Send for IntoIter<A, B> {}
unsafe impl<A: Sync, B: Sync> Sync for IntoIter<A, B> {}

impl<A, B> Iterator for IntoIter<A, B> {
    type Item = (A, B);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.bbb_begin == self.bbb_end {
            None
        } else {
            unsafe {
                let a = ptr::read(self.aaa_begin.as_ref());
                let b = ptr::read(self.bbb_begin.as_ref());
                self.aaa_begin = NonNull::new_unchecked(self.aaa_begin.as_ptr().add(1));
                self.bbb_begin = NonNull::new_unchecked(self.bbb_begin.as_ptr().add(1));
                Some((a, b))
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let rem = self.len();
        (rem, Some(rem))
    }
}

impl<A, B> Drop for IntoIter<A, B> {
    fn drop(&mut self) {
        unsafe {
            let rem = self.len();
            ptr::drop_in_place(slice::from_raw_parts_mut(self.aaa_begin.as_ptr(), rem));
            ptr::drop_in_place(slice::from_raw_parts_mut(self.bbb_begin.as_ptr(), rem));
            Vec2::<A, B>::dealloc_impl(self.bbb_ptr, self.cap);
        }
    }
}

impl<A, B> ExactSizeIterator for IntoIter<A, B> {
    #[inline]
    fn len(&self) -> usize {
        unsafe { self.bbb_end.as_ptr().offset_from(self.bbb_begin.as_ptr()) as usize }
    }
}

impl<A, B> DoubleEndedIterator for IntoIter<A, B> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.bbb_begin == self.bbb_end {
            None
        } else {
            unsafe {
                self.bbb_end = NonNull::new_unchecked(self.bbb_end.as_ptr().sub(1));
                let new_len = self.len();

                debug_assert!(ptr::eq(
                    self.bbb_begin.as_ptr().add(new_len),
                    self.bbb_end.as_ptr()
                ));

                let a = ptr::read(self.aaa_begin.as_ptr().add(new_len));
                let b = ptr::read(self.bbb_end.as_ptr());

                Some((a, b))
            }
        }
    }
}
