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

use std::any::type_name;
use std::any::TypeId;
use std::cmp;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::mem;

use allocative::Allocative;
use derive_more::Display;
use starlark_syntax::slice_vec_ext::SliceExt;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::collections::maybe_uninit_backport::maybe_uninit_write_slice;
use crate::collections::StarlarkHashValue;
use crate::eval::compiler::def::FrozenDef;
use crate::private::Private;
use crate::values::array::VALUE_EMPTY_ARRAY;
use crate::values::layout::aligned_size::AlignedSize;
use crate::values::layout::heap::arena::MIN_ALLOC;
use crate::values::layout::heap::repr::AValueForward;
use crate::values::layout::heap::repr::AValueHeader;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::layout::heap::repr::ForwardPtr;
use crate::values::layout::value_alloc_size::ValueAllocSize;
use crate::values::layout::vtable::AValueVTable;
use crate::values::list::value::ListGen;
use crate::values::string::str_type::StarlarkStr;
use crate::values::types::any_array::AnyArray;
use crate::values::types::array::Array;
use crate::values::types::list::value::FrozenListData;
use crate::values::types::list::value::ListData;
use crate::values::types::tuple::value::FrozenTuple;
use crate::values::types::tuple::value::Tuple;
use crate::values::ComplexValue;
use crate::values::FreezeError;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenValue;
use crate::values::StarlarkValue;
use crate::values::Trace;
use crate::values::Tracer;
use crate::values::Value;
use crate::values::ValueTyped;

pub(crate) const fn alloc_static<'v, A>(value: A::StarlarkValue) -> AValueRepr<AValueImpl<'v, A>>
where
    A: AValue<'v>,
{
    let payload = AValueImpl::<A>::new(value);
    AValueRepr::with_metadata(AValueVTable::new::<A>(), payload)
}

pub(crate) const VALUE_STR_A_VALUE_PTR: AValueHeader =
    AValueHeader::new_const::<StarlarkStrAValue>();

#[derive(Debug, thiserror::Error)]
enum AValueError {
    #[error("Value of type `{0}` cannot be frozen")]
    CannotBeFrozen(&'static str),
}

/// Extended vtable methods (those not covered by `StarlarkValue`).
pub(crate) trait AValue<'v>: Sized + 'v {
    /// Unwrapped type.
    type StarlarkValue: StarlarkValue<'v>;

    /// Certain types like `Tuple` or `StarlarkStr` have payload array
    /// placed in a heap after `Self`. This is the type of an element of that array.
    type ExtraElem: 'v;

    /// Payload array length.
    fn extra_len(value: &Self::StarlarkValue) -> usize;

    /// Offset of field holding content, in bytes.
    ///
    /// Return `mem::size_of::<Self>()` if there's no extra content.
    fn offset_of_extra() -> usize;

    /// Type is `StarlarkStr`.
    const IS_STR: bool = false;

    /// Memory size of starlark value including `AValueHeader`.
    fn alloc_size_for_extra_len(extra_len: usize) -> ValueAllocSize {
        assert!(
            Self::offset_of_extra() % mem::align_of::<Self::ExtraElem>() == 0,
            "extra must be aligned"
        );
        ValueAllocSize::new(cmp::max(
            cmp::max(
                AlignedSize::of::<AValueRepr<Self::StarlarkValue>>(),
                MIN_ALLOC,
            ),
            // Content is not necessarily aligned to end of `A`.
            AlignedSize::align_up(
                AValueRepr::<Self>::offset_of_extra()
                    + (mem::size_of::<Self::ExtraElem>() * extra_len),
            ),
        ))
    }

    unsafe fn heap_freeze(
        me: *mut AValueRepr<Self::StarlarkValue>,
        freezer: &Freezer,
    ) -> FreezeResult<FrozenValue>;

    unsafe fn heap_copy(me: *mut AValueRepr<Self::StarlarkValue>, tracer: &Tracer<'v>)
    -> Value<'v>;
}

#[inline]
pub(crate) fn starlark_str<'v>(
    len: usize,
    hash: StarlarkHashValue,
) -> AValueImpl<'v, impl AValue<'v, ExtraElem = usize> + Send + Sync> {
    AValueImpl::<StarlarkStrAValue>::new(unsafe { StarlarkStr::new(len, hash) })
}

pub(crate) fn tuple_avalue<'v>(
    len: usize,
) -> AValueImpl<'v, impl AValue<'v, ExtraElem = Value<'v>>> {
    AValueImpl::<AValueTuple>::new(unsafe { Tuple::new(len) })
}

pub(crate) fn frozen_tuple_avalue(
    len: usize,
) -> AValueImpl<'static, impl AValue<'static, ExtraElem = FrozenValue>> {
    AValueImpl::<AValueFrozenTuple>::new(unsafe { FrozenTuple::new(len) })
}

pub(crate) fn list_avalue<'v>(
    content: ValueTyped<'v, Array<'v>>,
) -> AValueImpl<'v, impl AValue<'v, StarlarkValue = ListGen<ListData<'v>>, ExtraElem = ()>> {
    AValueImpl::<AValueList>::new(ListGen(ListData::new(content)))
}

pub(crate) fn frozen_list_avalue(
    len: usize,
) -> AValueImpl<'static, impl AValue<'static, ExtraElem = FrozenValue>> {
    AValueImpl::<AValueFrozenList>::new(unsafe { ListGen(FrozenListData::new(len)) })
}

pub(crate) fn array_avalue<'v>(
    cap: u32,
) -> AValueImpl<'v, impl AValue<'v, StarlarkValue = Array<'v>, ExtraElem = Value<'v>>> {
    AValueImpl::<AValueArray>::new(unsafe { Array::new(0, cap) })
}

pub(crate) fn any_array_avalue<T: Debug + 'static>(
    cap: usize,
) -> AValueImpl<'static, impl AValue<'static, StarlarkValue = AnyArray<T>, ExtraElem = T>> {
    AValueImpl::<AValueAnyArray<T>>::new(unsafe { AnyArray::new(cap) })
}

pub(crate) fn simple<T: StarlarkValue<'static> + Send + Sync>(
    x: T,
) -> AValueImpl<'static, impl AValue<'static, ExtraElem = ()> + Send + Sync> {
    assert!(!T::is_special(Private));
    AValueImpl::<AValueSimple<T>>::new(x)
}

pub(crate) fn complex<'v, C>(x: C) -> AValueImpl<'v, impl AValue<'v, ExtraElem = ()>>
where
    C: ComplexValue<'v>,
    C::Frozen: StarlarkValue<'static>,
{
    assert!(!C::is_special(Private));
    AValueImpl::<AValueComplex<C>>::new(x)
}

pub(crate) fn complex_no_freeze<'v, C>(x: C) -> AValueImpl<'v, impl AValue<'v, ExtraElem = ()>>
where
    C: StarlarkValue<'v> + Trace<'v>,
{
    assert!(!C::is_special(Private));
    AValueImpl::<AValueComplexNoFreeze<C>>::new(x)
}

/// A value with extended (`AValue`) vtable methods.
#[repr(C)]
pub(crate) struct AValueImpl<'v, T: AValue<'v>>(PhantomData<T>, pub(crate) T::StarlarkValue);

impl<'v, T: AValue<'v>> AValueImpl<'v, T> {
    pub(crate) const fn new(value: T::StarlarkValue) -> Self {
        AValueImpl(PhantomData, value)
    }
}

/// For types which are only allocated statically (never in heap).
/// Technically we can use `AValueSimple` for these, but this is more explicit and safe.
pub(crate) struct AValueBasic<T>(PhantomData<T>);

impl<'v, T: StarlarkValue<'v>> AValue<'v> for AValueBasic<T> {
    type StarlarkValue = T;

    type ExtraElem = ();

    fn extra_len(_value: &T) -> usize {
        unreachable!("Basic types don't appear in the heap")
    }

    fn offset_of_extra() -> usize {
        unreachable!("Basic types don't appear in the heap")
    }

    unsafe fn heap_freeze(
        _me: *mut AValueRepr<Self::StarlarkValue>,
        _freezer: &Freezer,
    ) -> FreezeResult<FrozenValue> {
        unreachable!("Basic types don't appear in the heap")
    }
    unsafe fn heap_copy(
        _me: *mut AValueRepr<Self::StarlarkValue>,
        _tracer: &Tracer<'v>,
    ) -> Value<'v> {
        unreachable!("Basic types don't appear in the heap")
    }
}

pub(crate) struct StarlarkStrAValue;

impl<'v> AValue<'v> for StarlarkStrAValue {
    type StarlarkValue = StarlarkStr;

    type ExtraElem = usize;

    fn extra_len(value: &StarlarkStr) -> usize {
        StarlarkStr::payload_len_for_len(value.len())
    }

    fn offset_of_extra() -> usize {
        StarlarkStr::offset_of_content()
    }

    const IS_STR: bool = true;

    unsafe fn heap_freeze(
        me: *mut AValueRepr<Self::StarlarkValue>,
        freezer: &Freezer,
    ) -> FreezeResult<FrozenValue> {
        debug_assert!(
            (*me).payload.len() > 1,
            "short strings are allocated statically"
        );

        let s = (*me).payload.as_str();
        let fv = freezer.alloc(s);
        debug_assert!(fv.is_str());
        AValueHeader::overwrite_with_forward::<Self::StarlarkValue>(me, ForwardPtr::new_frozen(fv));
        Ok(fv)
    }

    unsafe fn heap_copy(
        me: *mut AValueRepr<Self::StarlarkValue>,
        tracer: &Tracer<'v>,
    ) -> Value<'v> {
        debug_assert!(
            (*me).payload.len() > 1,
            "short strings are allocated statically"
        );

        let s = (*me).payload.as_str();
        let v = tracer.alloc_str(s);
        debug_assert!(v.is_str());
        AValueHeader::overwrite_with_forward::<Self::StarlarkValue>(
            me,
            ForwardPtr::new_unfrozen(v),
        );
        v
    }
}

pub(crate) struct AValueTuple;

impl<'v> AValue<'v> for AValueTuple {
    type StarlarkValue = Tuple<'v>;

    type ExtraElem = Value<'v>;

    fn extra_len(value: &Tuple<'v>) -> usize {
        value.len()
    }

    fn offset_of_extra() -> usize {
        Tuple::offset_of_content()
    }

    unsafe fn heap_freeze(
        me: *mut AValueRepr<Self::StarlarkValue>,
        freezer: &Freezer,
    ) -> FreezeResult<FrozenValue> {
        debug_assert!(
            (*me).payload.len() != 0,
            "empty tuple is allocated statically"
        );

        AValueForward::assert_does_not_overwrite_extra::<Self>();
        let content = (*me).payload.content();

        let (fv, r, extra) = freezer.reserve_with_extra::<AValueFrozenTuple>(content.len());
        AValueHeader::overwrite_with_forward::<Self::StarlarkValue>(me, ForwardPtr::new_frozen(fv));

        // TODO: this allocation is unnecessary
        let frozen_values = content.try_map(|v| freezer.freeze(*v))?;
        r.fill(FrozenTuple::new(content.len()));

        let extra = &mut *extra;
        maybe_uninit_write_slice(extra, &frozen_values);

        Ok(fv)
    }

    unsafe fn heap_copy(
        me: *mut AValueRepr<Self::StarlarkValue>,
        tracer: &Tracer<'v>,
    ) -> Value<'v> {
        debug_assert!(
            (*me).payload.len() != 0,
            "empty tuple is allocated statically"
        );

        AValueForward::assert_does_not_overwrite_extra::<Self>();
        let content = (*me).payload.content_mut();

        let (v, r, extra) = tracer.reserve_with_extra::<Self>(content.len());
        let x = AValueHeader::overwrite_with_forward::<Self::StarlarkValue>(
            me,
            ForwardPtr::new_unfrozen(v),
        );

        debug_assert_eq!(content.len(), x.len());

        for elem in content.iter_mut() {
            tracer.trace(elem);
        }
        r.fill(x);
        let extra = unsafe { &mut *extra };
        maybe_uninit_write_slice(extra, content);
        v
    }
}

pub(crate) struct AValueFrozenTuple;

impl<'v> AValue<'v> for AValueFrozenTuple {
    type StarlarkValue = FrozenTuple;

    type ExtraElem = FrozenValue;

    fn extra_len(value: &FrozenTuple) -> usize {
        value.len()
    }

    fn offset_of_extra() -> usize {
        FrozenTuple::offset_of_content()
    }

    unsafe fn heap_freeze(
        _me: *mut AValueRepr<Self::StarlarkValue>,
        _freezer: &Freezer,
    ) -> FreezeResult<FrozenValue> {
        panic!("already frozen");
    }

    unsafe fn heap_copy(
        _me: *mut AValueRepr<Self::StarlarkValue>,
        _tracer: &Tracer<'v>,
    ) -> Value<'v> {
        panic!("shouldn't be copying frozen values");
    }
}

pub(crate) struct AValueList;

impl<'v> AValue<'v> for AValueList {
    type StarlarkValue = ListGen<ListData<'v>>;

    type ExtraElem = ();

    fn extra_len(_value: &ListGen<ListData<'v>>) -> usize {
        0
    }

    fn offset_of_extra() -> usize {
        mem::size_of::<Self>()
    }

    unsafe fn heap_freeze(
        me: *mut AValueRepr<Self::StarlarkValue>,
        freezer: &Freezer,
    ) -> FreezeResult<FrozenValue> {
        let content = (*me).payload.0.content();

        if content.is_empty() {
            let fv = FrozenValue::new_empty_list();
            AValueHeader::overwrite_with_forward::<Self::StarlarkValue>(
                me,
                ForwardPtr::new_frozen(fv),
            );
            return Ok(fv);
        }

        let (fv, r, extra) = freezer.reserve_with_extra::<AValueFrozenList>(content.len());
        AValueHeader::overwrite_with_forward::<Self::StarlarkValue>(me, ForwardPtr::new_frozen(fv));
        r.fill(ListGen(FrozenListData::new(content.len())));
        let extra = unsafe { &mut *extra };
        assert_eq!(extra.len(), content.len());
        for (elem_place, elem) in extra.iter_mut().zip(content) {
            elem_place.write(freezer.freeze(*elem)?);
        }
        Ok(fv)
    }

    unsafe fn heap_copy(
        me: *mut AValueRepr<Self::StarlarkValue>,
        tracer: &Tracer<'v>,
    ) -> Value<'v> {
        heap_copy_impl::<Self>(me, tracer, Trace::trace)
    }
}

pub(crate) struct AValueFrozenList;

impl<'v> AValue<'v> for AValueFrozenList {
    type StarlarkValue = ListGen<FrozenListData>;

    type ExtraElem = FrozenValue;

    fn extra_len(value: &ListGen<FrozenListData>) -> usize {
        value.0.len()
    }

    fn offset_of_extra() -> usize {
        ListGen::<FrozenListData>::offset_of_content()
    }

    unsafe fn heap_freeze(
        _me: *mut AValueRepr<Self::StarlarkValue>,
        _freezer: &Freezer,
    ) -> FreezeResult<FrozenValue> {
        panic!("already frozen");
    }

    unsafe fn heap_copy(
        _me: *mut AValueRepr<Self::StarlarkValue>,
        _tracer: &Tracer<'v>,
    ) -> Value<'v> {
        panic!("shouldn't be copying frozen values");
    }
}

pub(crate) struct AValueArray;

impl<'v> AValue<'v> for AValueArray {
    type StarlarkValue = Array<'v>;

    type ExtraElem = Value<'v>;

    fn extra_len(value: &Array<'v>) -> usize {
        // Note we return capacity, not length here.
        value.capacity()
    }

    fn offset_of_extra() -> usize {
        Array::offset_of_content()
    }

    unsafe fn heap_freeze(
        _me: *mut AValueRepr<Self::StarlarkValue>,
        _freezer: &Freezer,
    ) -> FreezeResult<FrozenValue> {
        panic!("arrays should not be frozen")
    }

    unsafe fn heap_copy(
        me: *mut AValueRepr<Self::StarlarkValue>,
        tracer: &Tracer<'v>,
    ) -> Value<'v> {
        debug_assert!(
            (*me).payload.capacity() != 0,
            "empty array is allocated statically"
        );

        if (*me).payload.len() == 0 {
            return FrozenValue::new_repr(VALUE_EMPTY_ARRAY.repr()).to_value();
        }

        AValueForward::assert_does_not_overwrite_extra::<Self>();
        let content = (*me).payload.content_mut();

        let (v, r, extra) = tracer.reserve_with_extra::<Self>(content.len());
        let x = AValueHeader::overwrite_with_forward::<Self::StarlarkValue>(
            me,
            ForwardPtr::new_unfrozen(v),
        );

        debug_assert_eq!(content.len(), x.len());

        content.trace(tracer);

        // Note when copying we are dropping extra capacity.
        r.fill(Array::new(content.len() as u32, content.len() as u32));
        let extra = unsafe { &mut *extra };
        maybe_uninit_write_slice(extra, content);
        v
    }
}

pub(crate) struct AValueAnyArray<T>(PhantomData<T>);

impl<'v, T: Debug + 'static> AValue<'v> for AValueAnyArray<T> {
    type StarlarkValue = AnyArray<T>;
    type ExtraElem = T;

    fn extra_len(value: &AnyArray<T>) -> usize {
        value.len
    }

    fn offset_of_extra() -> usize {
        AnyArray::<T>::offset_of_content()
    }

    unsafe fn heap_freeze(
        _me: *mut AValueRepr<Self::StarlarkValue>,
        _freezer: &Freezer,
    ) -> FreezeResult<FrozenValue> {
        panic!("AnyArray for now can only be allocated in FrozenHeap");
    }

    unsafe fn heap_copy(
        _me: *mut AValueRepr<Self::StarlarkValue>,
        _tracer: &Tracer<'v>,
    ) -> Value<'v> {
        panic!("AnyArray for now can only be allocated in FrozenHeap");
    }
}

/// If `A` provides a statically allocated frozen value,
/// replace object with the forward to that frozen value instead of using default freeze.
unsafe fn try_freeze_static<'v, A>(me: *mut AValueRepr<A::StarlarkValue>) -> Option<FrozenValue>
where
    A: AValue<'v>,
{
    let f = (*me).payload.try_freeze_static()?;

    drop(AValueHeader::overwrite_with_forward::<A::StarlarkValue>(
        me,
        ForwardPtr::new_frozen(f),
    ));
    Some(f)
}

/// `heap_freeze` implementation for simple `StarlarkValue` and `StarlarkFloat`
/// (`StarlarkFloat` is logically a simple type, but it is not considered simple type).
unsafe fn heap_freeze_simple_impl<'v, A>(
    me: *mut AValueRepr<A::StarlarkValue>,
    freezer: &Freezer,
) -> FreezeResult<FrozenValue>
where
    A: AValue<'v, ExtraElem = ()>,
{
    let (fv, r) = freezer.reserve::<A>();
    let x =
        AValueHeader::overwrite_with_forward::<A::StarlarkValue>(me, ForwardPtr::new_frozen(fv));
    r.fill(x);
    Ok(fv)
}

pub(crate) struct AValueSimple<T>(PhantomData<T>);

impl<T: StarlarkValue<'static>> AValue<'static> for AValueSimple<T> {
    type StarlarkValue = T;

    type ExtraElem = ();

    fn extra_len(_value: &T) -> usize {
        0
    }

    fn offset_of_extra() -> usize {
        mem::size_of::<Self>()
    }

    unsafe fn heap_freeze(
        me: *mut AValueRepr<Self::StarlarkValue>,
        freezer: &Freezer,
    ) -> FreezeResult<FrozenValue> {
        if let Some(f) = try_freeze_static::<Self>(me) {
            return Ok(f);
        }

        heap_freeze_simple_impl::<Self>(me, freezer)
    }

    unsafe fn heap_copy(
        me: *mut AValueRepr<Self::StarlarkValue>,
        tracer: &Tracer<'static>,
    ) -> Value<'static> {
        heap_copy_impl::<Self>(me, tracer, |_v, _tracer| {})
    }
}

/// Common `heap_copy` implementation for types without extra.
unsafe fn heap_copy_impl<'v, A>(
    me: *mut AValueRepr<A::StarlarkValue>,
    tracer: &Tracer<'v>,
    trace: impl FnOnce(&mut A::StarlarkValue, &Tracer<'v>),
) -> Value<'v>
where
    A: AValue<'v, ExtraElem = ()>,
{
    let (v, r) = tracer.reserve::<A>();
    let mut x =
        AValueHeader::overwrite_with_forward::<A::StarlarkValue>(me, ForwardPtr::new_unfrozen(v));
    // We have to put the forwarding node in _before_ we trace in case there are cycles
    trace(&mut x, tracer);
    r.fill(x);
    v
}

pub(crate) struct AValueComplex<T>(PhantomData<T>);

impl<'v, T> AValue<'v> for AValueComplex<T>
where
    T: ComplexValue<'v>,
    T::Frozen: StarlarkValue<'static>,
{
    type StarlarkValue = T;

    type ExtraElem = ();

    fn extra_len(_value: &T) -> usize {
        0
    }

    fn offset_of_extra() -> usize {
        mem::size_of::<Self>()
    }

    unsafe fn heap_freeze(
        me: *mut AValueRepr<Self::StarlarkValue>,
        freezer: &Freezer,
    ) -> FreezeResult<FrozenValue> {
        if let Some(f) = try_freeze_static::<Self>(me) {
            return Ok(f);
        }

        let (fv, r) = freezer.reserve::<AValueSimple<T::Frozen>>();
        let x = AValueHeader::overwrite_with_forward::<Self::StarlarkValue>(
            me,
            ForwardPtr::new_frozen(fv),
        );
        let res = x.freeze(freezer)?;
        r.fill(res);
        if TypeId::of::<T::Frozen>() == TypeId::of::<FrozenDef>() {
            let frozen_def = fv.downcast_frozen_ref().unwrap();
            freezer.frozen_defs.borrow_mut().push(frozen_def);
        }
        Ok(fv)
    }

    unsafe fn heap_copy(
        me: *mut AValueRepr<Self::StarlarkValue>,
        tracer: &Tracer<'v>,
    ) -> Value<'v> {
        heap_copy_impl::<Self>(me, tracer, Trace::trace)
    }
}

pub(crate) struct AValueComplexNoFreeze<T>(PhantomData<T>);

impl<'v, T> AValue<'v> for AValueComplexNoFreeze<T>
where
    T: StarlarkValue<'v> + Trace<'v>,
{
    type StarlarkValue = T;

    type ExtraElem = ();

    fn extra_len(_value: &T) -> usize {
        0
    }

    fn offset_of_extra() -> usize {
        mem::size_of::<Self::StarlarkValue>()
    }

    unsafe fn heap_freeze(
        _me: *mut AValueRepr<Self::StarlarkValue>,
        _freezer: &Freezer,
    ) -> FreezeResult<FrozenValue> {
        Err(FreezeError::new(
            AValueError::CannotBeFrozen(type_name::<T>()).to_string(),
        ))
    }

    unsafe fn heap_copy(
        me: *mut AValueRepr<Self::StarlarkValue>,
        tracer: &Tracer<'v>,
    ) -> Value<'v> {
        heap_copy_impl::<Self>(me, tracer, Trace::trace)
    }
}

#[derive(Debug, Display, ProvidesStaticType, Allocative)]
#[display("BlackHole")]
pub(crate) struct BlackHole(pub(crate) ValueAllocSize);

#[cfg(test)]
mod tests {
    use crate::environment::Module;
    use crate::values::dict::AllocDict;
    use crate::values::types::list::value::ListData;
    use crate::values::UnpackValue;
    use crate::values::Value;

    #[test]
    fn tuple_cycle_freeze() {
        let module = Module::new();
        let list = module.heap().alloc_list(&[]);
        let tuple = module.heap().alloc_tuple(&[list]);
        ListData::from_value_mut(list)
            .unwrap()
            .push(tuple, module.heap());
        module.set("t", tuple);
        module.freeze().unwrap();
    }

    #[test]
    fn test_try_freeze_static() {
        // `try_freeze_static` is only implemented for `dict` at the moment of writing,
        // so use it for the test.

        let module = Module::new();
        let d0 = module.heap().alloc(AllocDict::EMPTY);
        let d1 = module.heap().alloc(AllocDict::EMPTY);
        // Pointers are not equal.
        assert_ne!(d0.0.raw(), d1.0.raw());

        module.set_extra_value(module.heap().alloc((d0, d1)));

        let module = module.freeze().unwrap();
        let (d0, d1) =
            <(Value, Value)>::unpack_value_err(module.extra_value().unwrap().to_value()).unwrap();
        // Pointers are equal.
        assert_eq!(d0.0.raw(), d1.0.raw());
    }
}
