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

use std::cmp::Ordering;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::marker::PhantomData;
use std::mem;
use std::ptr;

use allocative::Allocative;
use dupe::Dupe;

use crate::any::AnyLifetime;
use crate::any::ProvidesStaticType;
use crate::cast::transmute;
use crate::collections::Hashed;
use crate::collections::StarlarkHashValue;
use crate::collections::StarlarkHasher;
use crate::docs::DocItem;
use crate::environment::Methods;
use crate::eval::Arguments;
use crate::eval::Evaluator;
use crate::private::Private;
use crate::typing::Ty;
use crate::values::demand::Demand;
use crate::values::int::pointer_i32::PointerI32;
use crate::values::layout::avalue::AValue;
use crate::values::layout::avalue::BlackHole;
use crate::values::layout::const_type_id::ConstTypeId;
use crate::values::layout::heap::repr::AValueHeader;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::layout::value_alloc_size::ValueAllocSize;
use crate::values::starlark_type_id::StarlarkTypeId;
use crate::values::traits::StarlarkValueVTable;
use crate::values::traits::StarlarkValueVTableGet;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::Tracer;
use crate::values::Value;

/// Untyped raw pointer to `StarlarkValue` without vtable.
///
/// * `'v` lifetime is heap lifetime
/// * `'a` lifetime is value lifetime
#[repr(C)]
#[derive(Copy, Clone, Dupe)]
pub(crate) struct StarlarkValueRawPtr {
    /// Points to the end of `AValueHeader`.
    /// May not be equal to to the start of `StarlarkValue` due to alignment.
    ptr: *const (),
}

impl StarlarkValueRawPtr {
    #[inline]
    pub(crate) fn new_header(ptr: &AValueHeader) -> Self {
        debug_assert!(ptr as *const AValueHeader as usize % AValueHeader::ALIGN == 0);

        unsafe {
            let ptr = (ptr as *const AValueHeader).add(1) as *const ();
            StarlarkValueRawPtr { ptr }
        }
    }

    #[inline]
    pub(crate) fn new_pointer_i32(ptr: &'static PointerI32) -> Self {
        let ptr = ptr as *const PointerI32 as *const ();
        StarlarkValueRawPtr { ptr }
    }

    #[inline]
    pub(crate) unsafe fn value_ptr<T>(self) -> *mut T {
        assert!(
            AValueRepr::<PointerI32>::padding_after_header() == 0,
            "There is no header for `PointerI32`, but following code should work"
        );

        // `self.ptr` is a pointer to the end of `AValueHeader`.
        // Align it up to `T` by adding padding between `AValueRepr` fields.
        let ptr = self.ptr as usize + AValueRepr::<T>::padding_after_header();
        debug_assert!(ptr % mem::align_of::<T>() == 0);
        ptr as *mut T
    }

    #[inline]
    pub(crate) unsafe fn value_ref<'v, T: StarlarkValue<'v>>(self) -> &'v T {
        &*self.value_ptr()
    }
}

pub(crate) struct AValueVTable {
    // Common `AValue` fields.
    pub(crate) static_type_of_value: ConstTypeId,
    pub(crate) starlark_type_id: StarlarkTypeId,
    pub(crate) type_name: &'static str,
    /// Cache `type_name` here to avoid computing hash.
    pub(crate) type_as_allocative_key: allocative::Key,

    // `StarlarkValue`
    pub(crate) starlark_value: StarlarkValueVTable,

    // `Drop`
    drop_in_place: fn(StarlarkValueRawPtr),

    // `AValue`
    pub(crate) is_str: bool,
    memory_size: fn(StarlarkValueRawPtr) -> ValueAllocSize,
    heap_freeze: fn(StarlarkValueRawPtr, &Freezer) -> FreezeResult<FrozenValue>,
    heap_copy: for<'v> fn(StarlarkValueRawPtr, &Tracer<'v>) -> Value<'v>,

    // `StarlarkValue` supertraits.
    display: unsafe fn(StarlarkValueRawPtr) -> *const dyn Display,
    debug: unsafe fn(StarlarkValueRawPtr) -> *const dyn Debug,
    erased_serde_serialize: unsafe fn(StarlarkValueRawPtr) -> *const dyn erased_serde::Serialize,
    allocative: unsafe fn(StarlarkValueRawPtr) -> *const dyn Allocative,
}

struct GetTypeId<'v, T: StarlarkValue<'v>>(PhantomData<&'v T>);

impl<'v, T: StarlarkValue<'v>> GetTypeId<'v, T> {
    const TYPE_ID: ConstTypeId = ConstTypeId::of::<<T as ProvidesStaticType>::StaticType>();
    const STARLARK_TYPE_ID: StarlarkTypeId = StarlarkTypeId::of::<T>();
}

struct GetAllocativeKey<'v, T: StarlarkValue<'v>>(PhantomData<&'v T>);

impl<'v, T: StarlarkValue<'v>> GetAllocativeKey<'v, T> {
    const ALLOCATIVE_KEY: allocative::Key = allocative::Key::new(T::TYPE);
}

impl AValueVTable {
    pub(crate) fn new_black_hole() -> &'static AValueVTable {
        const BLACKHOLE_ALLOCATIVE_KEY: allocative::Key = allocative::Key::new("BlackHole");
        const BLACKHOLE_TYPE_ID: ConstTypeId = ConstTypeId::of::<BlackHole>();
        const BLACKHOLE_STARLARK_TYPE_ID: StarlarkTypeId =
            StarlarkTypeId::from_type_id(BLACKHOLE_TYPE_ID);
        &AValueVTable {
            drop_in_place: |_| {},

            is_str: false,
            memory_size: |p| unsafe { (*p.value_ptr::<BlackHole>()).0 },
            static_type_of_value: BLACKHOLE_TYPE_ID,
            starlark_type_id: BLACKHOLE_STARLARK_TYPE_ID,

            heap_freeze: |_, _| panic!("BlackHole"),
            heap_copy: |_, _| panic!("BlackHole"),
            type_name: "BlackHole",
            type_as_allocative_key: BLACKHOLE_ALLOCATIVE_KEY,

            display: |this| {
                let this = unsafe { &*this.value_ptr::<BlackHole>() };
                this as *const dyn Display
            },
            debug: |this| {
                let this = unsafe { &*this.value_ptr::<BlackHole>() };
                this as *const dyn Debug
            },
            erased_serde_serialize: |_this| unreachable!(),
            allocative: |this| {
                let this = unsafe { &*this.value_ptr::<BlackHole>() };
                this as *const dyn Allocative
            },
            starlark_value: StarlarkValueVTable::BLACK_HOLE,
        }
    }

    pub(crate) const fn new<'v, T: AValue<'v>>() -> &'static AValueVTable {
        &AValueVTable {
            drop_in_place: |p| unsafe {
                ptr::drop_in_place(p.value_ptr::<T::StarlarkValue>());
            },
            is_str: T::IS_STR,
            memory_size: |p| unsafe {
                let p = &*p.value_ptr::<T::StarlarkValue>();
                T::alloc_size_for_extra_len(T::extra_len(p))
            },
            heap_freeze: |p, freezer| unsafe {
                let p = &mut *AValueRepr::from_payload_ptr_mut(p.value_ptr::<T::StarlarkValue>());
                T::heap_freeze(p, transmute!(&Freezer, &Freezer, freezer))
            },
            heap_copy: |p, tracer| unsafe {
                let p = &mut *AValueRepr::from_payload_ptr_mut(p.value_ptr::<T::StarlarkValue>());
                let value = T::heap_copy(p, transmute!(&Tracer, &Tracer, tracer));
                transmute!(Value, Value, value)
            },
            static_type_of_value: GetTypeId::<T::StarlarkValue>::TYPE_ID,
            starlark_type_id: GetTypeId::<T::StarlarkValue>::STARLARK_TYPE_ID,
            type_name: T::StarlarkValue::TYPE,
            type_as_allocative_key: GetAllocativeKey::<T::StarlarkValue>::ALLOCATIVE_KEY,
            display: |this| unsafe {
                let this = this.value_ptr::<T::StarlarkValue>();
                let display = this as *const dyn Display;
                // Drop lifetime.
                mem::transmute(display)
            },
            debug: |this| unsafe {
                let this = this.value_ptr::<T::StarlarkValue>();
                let debug = this as *const dyn Debug;
                // Drop lifetime.
                mem::transmute(debug)
            },
            erased_serde_serialize: |this| unsafe {
                let this = this.value_ptr::<T::StarlarkValue>();
                let serialize = this as *const dyn erased_serde::Serialize;
                // Drop lifetime.
                mem::transmute(serialize)
            },
            allocative: |this| unsafe {
                let this = this.value_ptr::<T::StarlarkValue>();
                let allocative = this as *const dyn Allocative;
                // Drop lifetime.
                mem::transmute(allocative)
            },
            starlark_value: StarlarkValueVTableGet::<'v, T::StarlarkValue>::VTABLE,
        }
    }

    #[inline]
    pub(crate) fn type_value(&'static self) -> FrozenStringValue {
        (self.starlark_value.get_type_value_static)()
    }

    pub(crate) fn type_starlark_repr(&'static self) -> Ty {
        (self.starlark_value.get_type_starlark_repr)()
    }

    #[inline]
    pub(crate) fn methods(&'static self) -> Option<&'static Methods> {
        (self.starlark_value.get_methods)()
    }

    pub(crate) fn drop_in_place(&'static self, value: StarlarkValueRawPtr) {
        (self.drop_in_place)(value)
    }
}

#[derive(Copy, Clone, Dupe)]
#[repr(C)]
pub(crate) struct AValueDyn<'v> {
    value: StarlarkValueRawPtr,
    vtable: &'static AValueVTable,
    _marker: PhantomData<&'v ()>,
}

impl<'v> Debug for AValueDyn<'v> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("AValueDyn").finish_non_exhaustive()
    }
}

impl<'v> AValueDyn<'v> {
    #[inline]
    pub(crate) unsafe fn new(
        value: StarlarkValueRawPtr,
        vtable: &'static AValueVTable,
    ) -> AValueDyn<'v> {
        AValueDyn {
            value,
            vtable,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub(crate) fn vtable(self) -> &'static AValueVTable {
        self.vtable
    }

    #[inline]
    pub(crate) fn memory_size(self) -> ValueAllocSize {
        (self.vtable.memory_size)(self.value)
    }

    pub(crate) fn as_allocative(self) -> &'v dyn Allocative {
        unsafe { &*(self.vtable.allocative)(self.value) }
    }

    pub(crate) fn total_memory(self) -> usize {
        (self.memory_size().bytes() as usize)
            + allocative::size_of_unique_allocated_data(self.as_allocative())
    }

    #[inline]
    pub(crate) unsafe fn heap_freeze(self, freezer: &Freezer) -> FreezeResult<FrozenValue> {
        (self.vtable.heap_freeze)(self.value, freezer)
    }

    #[inline]
    pub(crate) unsafe fn heap_copy(self, tracer: &Tracer<'v>) -> Value<'v> {
        (self.vtable.heap_copy)(self.value, tracer)
    }

    #[inline]
    pub(crate) fn documentation(self) -> DocItem {
        (self.vtable.starlark_value.documentation)(self.value)
    }

    pub(crate) fn typechecker_ty(self) -> Option<Ty> {
        (self.vtable.starlark_value.typechecker_ty)(self.value)
    }

    pub(crate) fn eval_type(self) -> Option<Ty> {
        (self.vtable.starlark_value.eval_type)(self.value)
    }

    #[inline]
    pub(crate) fn at(self, index: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.at)(self.value, index, heap)
    }

    pub(crate) fn at2(
        self,
        index0: Value<'v>,
        index1: Value<'v>,
        heap: &'v Heap,
    ) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.at2)(self.value, index0, index1, heap, Private)
    }

    #[inline]
    pub(crate) fn is_in(self, collection: Value<'v>) -> crate::Result<bool> {
        (self.vtable.starlark_value.is_in)(self.value, collection)
    }

    #[inline]
    pub(crate) fn slice(
        self,
        start: Option<Value<'v>>,
        stop: Option<Value<'v>>,
        step: Option<Value<'v>>,
        heap: &'v Heap,
    ) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.slice)(self.value, start, stop, step, heap)
    }

    #[inline]
    pub(crate) fn get_attr(self, name: &str, heap: &'v Heap) -> Option<Value<'v>> {
        (self.vtable.starlark_value.get_attr)(self.value, name, heap)
    }

    #[inline]
    pub(crate) fn get_attr_hashed(self, name: Hashed<&str>, heap: &'v Heap) -> Option<Value<'v>> {
        (self.vtable.starlark_value.get_attr_hashed)(self.value, name, heap)
    }

    #[inline]
    pub(crate) fn has_attr(self, name: &str, heap: &'v Heap) -> bool {
        (self.vtable.starlark_value.has_attr)(self.value, name, heap)
    }

    #[inline]
    pub(crate) fn dir_attr(self) -> Vec<String> {
        (self.vtable.starlark_value.dir_attr)(self.value)
    }

    #[inline]
    pub(crate) fn bit_and(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.bit_and)(self.value, other, heap)
    }

    #[inline]
    pub(crate) fn bit_or(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.bit_or)(self.value, other, heap)
    }

    #[inline]
    pub(crate) fn bit_xor(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.bit_xor)(self.value, other, heap)
    }

    #[inline]
    pub(crate) fn bit_not(self, heap: &'v Heap) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.bit_not)(self.value, heap)
    }

    #[inline]
    pub(crate) fn to_bool(self) -> bool {
        (self.vtable.starlark_value.to_bool)(self.value)
    }

    #[inline]
    pub(crate) fn length(self) -> crate::Result<i32> {
        (self.vtable.starlark_value.length)(self.value)
    }

    #[inline]
    pub(crate) fn iterate(self, me: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.iterate)(self.value, me, heap)
    }

    #[inline]
    pub(crate) fn iter_next(self, index: usize, heap: &'v Heap) -> Option<Value<'v>> {
        (self.vtable.starlark_value.iter_next)(self.value, index, heap)
    }

    #[inline]
    pub(crate) fn iter_size_hint(self, index: usize) -> (usize, Option<usize>) {
        (self.vtable.starlark_value.iter_size_hint)(self.value, index)
    }

    #[inline]
    pub(crate) fn iter_stop(self) {
        (self.vtable.starlark_value.iter_stop)(self.value)
    }

    #[inline]
    pub(crate) fn get_hash(self) -> crate::Result<StarlarkHashValue> {
        (self.vtable.starlark_value.get_hash)(self.value, Private)
    }

    #[inline]
    pub(crate) fn plus(self, heap: &'v Heap) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.plus)(self.value, heap)
    }

    #[inline]
    pub(crate) fn minus(self, heap: &'v Heap) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.minus)(self.value, heap)
    }

    #[inline]
    pub(crate) fn add(self, other: Value<'v>, heap: &'v Heap) -> Option<crate::Result<Value<'v>>> {
        (self.vtable.starlark_value.add)(self.value, other, heap)
    }

    #[inline]
    pub(crate) fn radd(self, other: Value<'v>, heap: &'v Heap) -> Option<crate::Result<Value<'v>>> {
        (self.vtable.starlark_value.radd)(self.value, other, heap)
    }

    #[inline]
    pub(crate) fn sub(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.sub)(self.value, other, heap)
    }

    #[inline]
    pub(crate) fn mul(self, other: Value<'v>, heap: &'v Heap) -> Option<crate::Result<Value<'v>>> {
        (self.vtable.starlark_value.mul)(self.value, other, heap)
    }

    #[inline]
    pub(crate) fn rmul(self, other: Value<'v>, heap: &'v Heap) -> Option<crate::Result<Value<'v>>> {
        (self.vtable.starlark_value.rmul)(self.value, other, heap)
    }

    #[inline]
    pub(crate) fn div(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.div)(self.value, other, heap)
    }

    #[inline]
    pub(crate) fn floor_div(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.floor_div)(self.value, other, heap)
    }

    #[inline]
    pub(crate) fn percent(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.percent)(self.value, other, heap)
    }

    #[inline]
    pub(crate) fn left_shift(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.left_shift)(self.value, other, heap)
    }

    #[inline]
    pub(crate) fn right_shift(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        (self.vtable.starlark_value.right_shift)(self.value, other, heap)
    }

    #[inline]
    pub(crate) fn collect_repr(self, collector: &mut String) {
        (self.vtable.starlark_value.collect_repr)(self.value, collector)
    }

    #[inline]
    pub(crate) fn collect_repr_cycle(self, collector: &mut String) {
        (self.vtable.starlark_value.collect_repr_cycle)(self.value, collector)
    }

    #[inline]
    pub(crate) fn downcast_ref<T: StarlarkValue<'v>>(self) -> Option<&'v T> {
        if self.vtable.static_type_of_value.get() == T::static_type_id() {
            // SAFETY: just checked whether we are pointing to the correct type.
            unsafe { Some(self.value.value_ref()) }
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn equals(self, other: Value<'v>) -> crate::Result<bool> {
        (self.vtable.starlark_value.equals)(self.value, other)
    }

    #[inline]
    pub(crate) fn compare(self, other: Value<'v>) -> crate::Result<Ordering> {
        (self.vtable.starlark_value.compare)(self.value, other)
    }

    #[inline]
    pub(crate) fn name_for_call_stack(self, me: Value<'v>) -> String {
        (self.vtable.starlark_value.name_for_call_stack)(self.value, me)
    }

    #[inline]
    pub(crate) fn export_as(
        self,
        variable_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<()> {
        (self.vtable.starlark_value.export_as)(self.value, variable_name, eval)
    }

    #[inline]
    pub(crate) fn set_at(self, index: Value<'v>, new_value: Value<'v>) -> crate::Result<()> {
        (self.vtable.starlark_value.set_at)(self.value, index, new_value)
    }

    #[inline]
    pub(crate) fn set_attr(self, attribute: &str, new_value: Value<'v>) -> crate::Result<()> {
        (self.vtable.starlark_value.set_attr)(self.value, attribute, new_value)
    }

    #[inline]
    pub(crate) fn write_hash(self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        (self.vtable.starlark_value.write_hash)(self.value, hasher)
    }

    #[inline]
    pub(crate) fn type_matches_value(self, value: Value<'v>) -> bool {
        (self.vtable.starlark_value.type_matches_value)(self.value, value, Private)
    }

    #[inline]
    pub(crate) fn as_display(self) -> &'v dyn Display {
        unsafe { &*(self.vtable.display)(self.value) }
    }

    #[inline]
    pub(crate) fn as_debug(self) -> &'v dyn Debug {
        unsafe { &*(self.vtable.debug)(self.value) }
    }

    #[inline]
    pub(crate) fn as_serialize(self) -> &'v dyn erased_serde::Serialize {
        unsafe { &*(self.vtable.erased_serde_serialize)(self.value) }
    }

    #[inline]
    pub(crate) fn provide(self, demand: &mut Demand<'_, 'v>) {
        (self.vtable.starlark_value.provide)(self.value, demand)
    }
}

/// Raw pointer, vtable and `Value`.
pub(crate) struct AValueDynFull<'v> {
    avalue: AValueDyn<'v>,
    value: Value<'v>,
}

impl<'v> AValueDynFull<'v> {
    #[inline]
    pub(crate) unsafe fn new(avalue: AValueDyn<'v>, value: Value<'v>) -> AValueDynFull<'v> {
        AValueDynFull { avalue, value }
    }

    #[inline]
    pub(crate) fn invoke(
        self,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        (self.avalue.vtable.starlark_value.invoke)(self.avalue.value, self.value, args, eval)
    }
}
