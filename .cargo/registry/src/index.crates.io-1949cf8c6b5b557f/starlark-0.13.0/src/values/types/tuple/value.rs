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

use std::cmp::Ordering;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::slice;

use allocative::Allocative;
use display_container::fmt_container;
use serde::ser::SerializeTuple;
use serde::Serialize;
use starlark_derive::starlark_value;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::coerce::coerce;
use crate::coerce::Coerce;
use crate::collections::StarlarkHasher;
use crate::private::Private;
use crate::typing::Ty;
use crate::values::comparison::compare_slice;
use crate::values::comparison::equals_slice;
use crate::values::index::apply_slice;
use crate::values::index::convert_index;
use crate::values::layout::avalue::alloc_static;
use crate::values::layout::avalue::AValueFrozenTuple;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueError;
use crate::values::ValueLifetimeless;
use crate::values::ValueLike;

/// Define the tuple type. See [`Tuple`] and [`FrozenTuple`] as the two aliases.
#[repr(C)]
#[derive(ProvidesStaticType, Allocative)]
pub(crate) struct TupleGen<V: ValueLifetimeless> {
    len: usize,
    /// The data stored by the tuple.
    content: [V; 0],
}

impl<'v, V: ValueLike<'v>> Display for TupleGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // For single-item tuples we need to add a trailing ',' and easier to just handle that ourself than configure display_container correctly
        if self.len() == 1 {
            if f.alternate() {
                write!(f, "( {:#}, )", self.content()[0])
            } else {
                write!(f, "({},)", self.content()[0])
            }
        } else {
            fmt_container(f, "(", ")", self.content().iter())
        }
    }
}

impl<'v, V: ValueLike<'v>> Debug for TupleGen<V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("TupleGen")
            .field("content", &self.content())
            .finish()
    }
}

impl<V: ValueLifetimeless> TupleGen<V> {
    /// `type(())`.
    pub(crate) const TYPE: &'static str = "tuple";

    pub(crate) const unsafe fn new(len: usize) -> TupleGen<V> {
        TupleGen { len, content: [] }
    }

    pub(crate) fn offset_of_content() -> usize {
        memoffset::offset_of!(Self, content)
    }
}

pub(crate) static VALUE_EMPTY_TUPLE: AValueRepr<AValueImpl<'static, AValueFrozenTuple>> =
    alloc_static(unsafe { FrozenTuple::new(0) });

/// Runtime type of unfrozen tuple.
pub(crate) type Tuple<'v> = TupleGen<Value<'v>>;
/// Runtime type of frozen tuple.
pub(crate) type FrozenTuple = TupleGen<FrozenValue>;

unsafe impl<'v> Coerce<Tuple<'v>> for FrozenTuple {}

impl<'v> Tuple<'v> {
    /// Downcast a value to a tuple.
    pub(crate) fn from_value(value: Value<'v>) -> Option<&'v Self> {
        if value.unpack_frozen().is_some() {
            value.downcast_ref::<FrozenTuple>().map(coerce)
        } else {
            value.downcast_ref::<Tuple<'v>>()
        }
    }
}

impl<'v, V: ValueLike<'v>> TupleGen<V> {
    /// Get the length of the tuple.
    pub(crate) fn len(&self) -> usize {
        self.content().len()
    }

    /// Tuple elements.
    pub(crate) fn content(&self) -> &[V] {
        unsafe { slice::from_raw_parts(self.content.as_ptr(), self.len) }
    }

    pub(crate) fn content_mut(&mut self) -> &mut [V] {
        unsafe { slice::from_raw_parts_mut(self.content.as_mut_ptr(), self.len) }
    }

    /// Iterate over the elements of the tuple.
    pub(crate) fn iter<'a>(&'a self) -> impl Iterator<Item = Value<'v>> + 'a
    where
        'v: 'a,
    {
        self.content().iter().map(|e| e.to_value())
    }
}

#[starlark_value(type = Tuple::TYPE)]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for TupleGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn is_special(_: Private) -> bool
    where
        Self: Sized,
    {
        true
    }

    fn to_bool(&self) -> bool {
        self.len() != 0
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        for v in self.content() {
            v.write_hash(hasher)?;
        }
        Ok(())
    }

    fn equals(&self, other: Value<'v>) -> crate::Result<bool> {
        match Tuple::from_value(other) {
            None => Ok(false),
            Some(other) => equals_slice(self.content(), other.content(), |x, y| x.equals(*y)),
        }
    }

    fn compare(&self, other: Value<'v>) -> crate::Result<Ordering> {
        match Tuple::from_value(other) {
            None => ValueError::unsupported_with(self, "cmp()", other),
            Some(other) => compare_slice(self.content(), other.content(), |x, y| x.compare(*y)),
        }
    }

    fn at(&self, index: Value, _heap: &'v Heap) -> crate::Result<Value<'v>> {
        let i = convert_index(index, self.len() as i32)? as usize;
        Ok(self.content()[i].to_value())
    }

    fn length(&self) -> crate::Result<i32> {
        Ok(self.len() as i32)
    }

    fn is_in(&self, other: Value<'v>) -> crate::Result<bool> {
        for x in self.content() {
            if x.equals(other)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn slice(
        &self,
        start: Option<Value>,
        stop: Option<Value>,
        stride: Option<Value>,
        heap: &'v Heap,
    ) -> crate::Result<Value<'v>> {
        Ok(heap.alloc_tuple(&apply_slice(coerce(self.content()), start, stop, stride)?))
    }

    unsafe fn iterate(&self, me: Value<'v>, _heap: &'v Heap) -> crate::Result<Value<'v>> {
        Ok(me)
    }

    unsafe fn iter_size_hint(&self, index: usize) -> (usize, Option<usize>) {
        debug_assert!(index <= self.len());
        let rem = self.len() - index;
        (rem, Some(rem))
    }

    unsafe fn iter_next(&self, index: usize, _heap: &'v Heap) -> Option<Value<'v>> {
        self.content().get(index).map(|v| v.to_value())
    }

    unsafe fn iter_stop(&self) {}

    fn add(&self, other: Value<'v>, heap: &'v Heap) -> Option<crate::Result<Value<'v>>> {
        if let Some(other) = Tuple::from_value(other) {
            let mut result = Vec::with_capacity(self.len() + other.len());
            for x in self.iter() {
                result.push(x);
            }
            for x in other.iter() {
                result.push(x);
            }
            Some(Ok(heap.alloc_tuple(&result)))
        } else {
            None
        }
    }

    fn mul(&self, other: Value, heap: &'v Heap) -> Option<crate::Result<Value<'v>>> {
        let l = match i32::unpack_value(other) {
            Ok(Some(l)) => l,
            Ok(None) => return None,
            Err(e) => return Some(Err(e)),
        };
        let mut result = Vec::new();
        for _i in 0..l {
            result.extend(self.content().iter().map(|e| e.to_value()));
        }
        Some(Ok(heap.alloc_tuple(&result)))
    }

    fn rmul(&self, lhs: Value<'v>, heap: &'v Heap) -> Option<crate::Result<Value<'v>>> {
        self.mul(lhs, heap)
    }

    fn collect_repr_cycle(&self, collector: &mut String) {
        collector.push_str("(...)");
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        Some(Ty::any_tuple())
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any_tuple()
    }
}

impl<'v, V: ValueLike<'v>> Serialize for TupleGen<V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut tuple_serializer = serializer.serialize_tuple(self.len)?;

        for e in self.content().iter() {
            tuple_serializer.serialize_element(e)?;
        }

        tuple_serializer.end()
    }
}

#[cfg(test)]
mod tests {
    use crate::assert;
    use crate::assert::Assert;

    #[test]
    fn test_to_str() {
        assert::all_true(
            r#"
str((1, 2, 3)) == "(1, 2, 3)"
str((1, (2, 3))) == "(1, (2, 3))"
str((1,)) == "(1,)"
"#,
        );
    }

    #[test]
    fn test_repr_cycle() {
        let mut a = Assert::new();
        // TODO(nga): fix and enable.
        a.disable_static_typechecking();
        a.eq("l = []; t = (l,); l.append(t); repr(t)", "'([(...)],)'");
        a.eq("l = []; t = (l,); l.append(t); str(t)", "'([(...)],)'");
    }

    #[test]
    fn test_tuple_ellipsis_runtime() {
        assert::is_true("isinstance((), tuple[int, ...])");
        assert::is_true("isinstance((1, ), tuple[int, ...])");
        assert::is_true("isinstance((1, 2), tuple[int, ...])");
        assert::is_false("isinstance(('x', 2), tuple[int, ...])");
    }
}
