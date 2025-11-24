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

use std::slice;
use std::vec;

use crate::typing::Ty;
use crate::values::tuple::TupleRef;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::UnpackValue;
use crate::values::Value;

/// Unpack a value of type `tuple[T, ...]` into a vec.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct UnpackTuple<T> {
    /// Unpacked items.
    pub items: Vec<T>,
}

impl<T: Default> Default for UnpackTuple<T> {
    fn default() -> Self {
        UnpackTuple { items: Vec::new() }
    }
}

impl<T: StarlarkTypeRepr> StarlarkTypeRepr for UnpackTuple<T> {
    type Canonical = UnpackTuple<T::Canonical>;

    fn starlark_type_repr() -> Ty {
        Ty::tuple_of(T::starlark_type_repr())
    }
}

impl<'v, T: UnpackValue<'v>> UnpackValue<'v> for UnpackTuple<T> {
    type Error = <T as UnpackValue<'v>>::Error;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        let Some(tuple) = TupleRef::from_value(value) else {
            return Ok(None);
        };
        // TODO(nga): should not allocate if the first element is of the wrong type.
        let mut items = Vec::with_capacity(tuple.len());
        for v in tuple.iter() {
            let Some(v) = T::unpack_value_impl(v)? else {
                return Ok(None);
            };
            items.push(v);
        }
        Ok(Some(UnpackTuple { items }))
    }
}

impl<T> IntoIterator for UnpackTuple<T> {
    type Item = T;
    type IntoIter = vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a UnpackTuple<T> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut UnpackTuple<T> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use crate::values::tuple::unpack::UnpackTuple;
    use crate::values::Heap;
    use crate::values::UnpackValue;

    #[test]
    fn test_unpack() {
        let heap = Heap::new();
        let v = heap.alloc(("a", "b"));
        assert_eq!(
            vec!["a", "b"],
            UnpackTuple::<&str>::unpack_value(v).unwrap().unwrap().items
        );
        assert!(UnpackTuple::<u32>::unpack_value(v).unwrap().is_none());
        assert!(
            UnpackTuple::<&str>::unpack_value(heap.alloc(1))
                .unwrap()
                .is_none()
        );
    }
}
