/*
 * Copyright 2018 The Starlark in Rust Authors.
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

use std::fmt::Debug;

use crate::values::Heap;
use crate::values::Value;

/// Iterator of starlark values.
#[derive(Debug)]
pub struct StarlarkIterator<'v> {
    /// Iterator implementation. Typically an iterable itself.
    value: Value<'v>,
    /// Current index.
    index: usize,
    /// Heap to allocate values on.
    heap: &'v Heap,
}

impl<'v> StarlarkIterator<'v> {
    /// Construct iterator from the given value.
    #[inline]
    pub(crate) fn new(value: Value<'v>, heap: &'v Heap) -> StarlarkIterator<'v> {
        StarlarkIterator {
            value,
            index: 0,
            heap,
        }
    }

    /// Iterator yielding no values.
    #[inline]
    pub fn empty(heap: &'v Heap) -> StarlarkIterator<'v> {
        Self::new(Value::new_empty_tuple(), heap)
    }
}

impl<'v> Iterator for StarlarkIterator<'v> {
    type Item = Value<'v>;

    #[inline]
    fn next(&mut self) -> Option<Value<'v>> {
        let r = self.value.get_ref().iter_next(self.index, self.heap);
        if r.is_some() {
            self.index += 1;
        } else {
            self.value.get_ref().iter_stop();
            // We must call `iter_stop` exactly once, regardless of whether
            // iterator is exhausted or not, even if `next` is called after `None`.
            // So we replace `value` with empty tuple, for which we know that `iter_stop` is no-op.
            self.value = Value::new_empty_tuple();
            self.index = 0;
        }
        r
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.value.get_ref().iter_size_hint(self.index)
    }
}

impl<'v> Drop for StarlarkIterator<'v> {
    #[inline]
    fn drop(&mut self) {
        // `iter_stop` is no-op for empty tuple, this saves us from virtual call
        // after iterator is exhausted.
        if !self.value.ptr_eq(Value::new_empty_tuple()) {
            self.value.get_ref().iter_stop();
        }
    }
}
