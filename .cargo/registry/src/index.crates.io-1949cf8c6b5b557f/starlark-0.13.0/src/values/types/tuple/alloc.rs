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

use std::iter;

use crate::typing::Ty;
use crate::values::tuple::UnpackTuple;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::Value;

/// Utility to allocate a tuple.
///
/// Note, for tuples of fixed sizes there are implementations for `(A,)` or `(A, B)`.
///
/// # Example
///
/// ```
/// # use starlark::values::{FrozenHeap, Heap};
/// # use starlark::values::tuple::AllocTuple;
/// # fn alloc(heap: &Heap, frozen_heap: &FrozenHeap) {
/// let l = heap.alloc(AllocTuple([1, 2, 3]));
/// let ls = frozen_heap.alloc(AllocTuple([1, 2, 3]));
/// # }
/// ```
pub struct AllocTuple<T>(pub T);

impl AllocTuple<iter::Empty<FrozenValue>> {
    /// Allocate an empty tuple.
    pub const EMPTY: AllocTuple<iter::Empty<FrozenValue>> = AllocTuple(iter::empty());
}

impl<T> StarlarkTypeRepr for AllocTuple<T>
where
    T: IntoIterator,
    T::Item: StarlarkTypeRepr,
{
    type Canonical = <UnpackTuple<T::Item> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        Ty::tuple_of(T::Item::starlark_type_repr())
    }
}

impl<'v, T> AllocValue<'v> for AllocTuple<T>
where
    T: IntoIterator,
    T::Item: AllocValue<'v>,
{
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc_tuple_iter(self.0.into_iter().map(|x| x.alloc_value(heap)))
    }
}

impl<T> AllocFrozenValue for AllocTuple<T>
where
    T: IntoIterator,
    T::Item: AllocFrozenValue,
{
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc_tuple_iter(self.0.into_iter().map(|x| x.alloc_frozen_value(heap)))
    }
}

#[cfg(test)]
mod tests {
    use crate::values::tuple::alloc::AllocTuple;
    use crate::values::tuple::FrozenTupleRef;
    use crate::values::tuple::TupleRef;
    use crate::values::FrozenHeap;
    use crate::values::Heap;

    #[test]
    fn test_alloc_tuple() {
        let heap = Heap::new();

        let a = heap.alloc(AllocTuple([""; 0]));
        let b = heap.alloc(AllocTuple([1, 2, 3].iter().copied().filter(|_| false)));
        assert_eq!(0, TupleRef::from_value(a).unwrap().content().len());
        assert!(a.ptr_eq(b));

        // Fixed length iterator.
        let c = heap.alloc(AllocTuple([1, 2]));
        assert_eq!(2, TupleRef::from_value(c).unwrap().content().len());

        // Iterator of unknown length.
        let d = heap.alloc(AllocTuple([1, 2, 3].iter().copied().filter(|c| *c > 1)));
        assert_eq!(2, TupleRef::from_value(d).unwrap().content().len());
    }

    #[test]
    fn test_alloc_frozen_tuple() {
        let heap = FrozenHeap::new();

        let a = heap.alloc(AllocTuple([""; 0]));
        let b = heap.alloc(AllocTuple([1, 2, 3].iter().copied().filter(|_| false)));
        assert_eq!(0, TupleRef::from_frozen_value(a).unwrap().content().len());
        assert!(a.to_value().ptr_eq(b.to_value()));

        // Fixed length iterator.
        let c = heap.alloc(AllocTuple([1, 2]));
        assert_eq!(
            2,
            FrozenTupleRef::from_frozen_value(c)
                .unwrap()
                .content()
                .len()
        );

        // Iterator of unknown length.
        let d = heap.alloc(AllocTuple([1, 2, 3].iter().copied().filter(|c| *c > 1)));
        assert_eq!(
            2,
            FrozenTupleRef::from_frozen_value(d)
                .unwrap()
                .content()
                .len()
        );
    }
}
