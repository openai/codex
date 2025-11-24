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

use std::alloc::Layout;

use allocative::Allocative;

use crate::values::layout::aligned_size::AlignedSize;
use crate::values::layout::heap::arena::MIN_ALLOC;

/// Size of `AValue` with `AValueHeader` added.
/// This is the size of the value as it is stored in the heap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
pub(crate) struct ValueAllocSize {
    size: AlignedSize,
}

impl ValueAllocSize {
    #[inline]
    pub(crate) fn try_new(size: AlignedSize) -> Option<ValueAllocSize> {
        if size < MIN_ALLOC {
            None
        } else {
            Some(ValueAllocSize { size })
        }
    }

    #[inline]
    pub(crate) fn new(size: AlignedSize) -> ValueAllocSize {
        match ValueAllocSize::try_new(size) {
            Some(value) => value,
            None => panic!("{size} is too small for a value (minimum is {MIN_ALLOC})"),
        }
    }

    #[inline]
    pub(crate) fn layout(self) -> Layout {
        self.size.layout()
    }

    #[inline]
    pub(crate) fn size(self) -> AlignedSize {
        self.size
    }

    #[inline]
    pub(crate) const fn bytes(self) -> u32 {
        self.size.bytes()
    }
}
