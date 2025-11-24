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

//! Utility for unpacking a value of type `list[T]` or `tuple[T, ...]` into a vec.

use std::slice;
use std::vec;

use either::Either;

use crate::typing::Ty;
use crate::values::list::UnpackList;
use crate::values::tuple::UnpackTuple;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::UnpackValue;
use crate::values::Value;

/// Unpack a value of type `list[T]` or `tuple[T, ...]` into a vec.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct UnpackListOrTuple<T> {
    /// Unpacked items of the list or tuple.
    pub items: Vec<T>,
}

impl<T> Default for UnpackListOrTuple<T> {
    fn default() -> Self {
        UnpackListOrTuple { items: Vec::new() }
    }
}

impl<T: StarlarkTypeRepr> StarlarkTypeRepr for UnpackListOrTuple<T> {
    type Canonical = <Either<UnpackList<T>, UnpackTuple<T>> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        Either::<UnpackList<T>, UnpackTuple<T>>::starlark_type_repr()
    }
}

impl<'v, T: UnpackValue<'v>> UnpackValue<'v> for UnpackListOrTuple<T> {
    type Error = <T as UnpackValue<'v>>::Error;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        match Either::<UnpackList<T>, UnpackTuple<T>>::unpack_value_impl(value)
            .map_err(|e| e.into_inner())?
        {
            Some(Either::Left(l)) => Ok(Some(UnpackListOrTuple { items: l.items })),
            Some(Either::Right(r)) => Ok(Some(UnpackListOrTuple { items: r.items })),
            None => Ok(None),
        }
    }
}

impl<T> IntoIterator for UnpackListOrTuple<T> {
    type Item = T;
    type IntoIter = vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a UnpackListOrTuple<T> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut UnpackListOrTuple<T> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use crate::values::types::list_or_tuple::UnpackListOrTuple;
    use crate::values::Heap;
    use crate::values::UnpackValue;

    #[test]
    fn test_unpack() {
        let heap = Heap::new();
        let list = heap.alloc(vec!["a", "b"]);
        let tuple = heap.alloc(("a", "b"));
        let list_of_ints = heap.alloc(vec![1, 2]);
        let tuple_of_ints = heap.alloc((1, 2));
        assert_eq!(
            vec!["a", "b"],
            UnpackListOrTuple::<&str>::unpack_value(list)
                .unwrap()
                .unwrap()
                .items
        );
        assert_eq!(
            vec!["a", "b"],
            UnpackListOrTuple::<&str>::unpack_value(tuple)
                .unwrap()
                .unwrap()
                .items
        );
        assert!(
            UnpackListOrTuple::<&str>::unpack_value(list_of_ints)
                .unwrap()
                .is_none()
        );
        assert!(
            UnpackListOrTuple::<&str>::unpack_value(tuple_of_ints)
                .unwrap()
                .is_none()
        );
        assert!(
            UnpackListOrTuple::<&str>::unpack_value(heap.alloc(1))
                .unwrap()
                .is_none()
        );
    }
}
