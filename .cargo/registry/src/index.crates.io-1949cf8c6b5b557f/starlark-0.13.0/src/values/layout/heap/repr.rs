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

use std::hash::Hash;
use std::mem;
use std::mem::ManuallyDrop;
use std::ptr;

use dupe::Dupe;

use crate::any::AnyLifetime;
use crate::cast;
use crate::values::layout::avalue::AValue;
use crate::values::layout::heap::heap_type::HeapKind;
use crate::values::layout::value_alloc_size::ValueAllocSize;
use crate::values::layout::vtable::AValueDyn;
use crate::values::layout::vtable::AValueVTable;
use crate::values::layout::vtable::StarlarkValueRawPtr;
use crate::values::FrozenValue;
use crate::values::StarlarkValue;
use crate::values::Value;

#[derive(Clone)]
#[repr(C)]
pub(crate) struct AValueHeader(pub(crate) &'static AValueVTable);

impl Hash for AValueHeader {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        ptr::hash(self.0, state);
    }
}

impl PartialEq for AValueHeader {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.0, other.0)
    }
}

impl Eq for AValueHeader {}

// Implements Copy so this is fine
impl Dupe for AValueHeader {}

/// How object is represented in arena.
#[repr(C, align(8))]
pub(crate) struct AValueRepr<T> {
    pub(crate) header: AValueHeader,
    /// Payload of the object, i.e. `StarlarkValue`.
    /// Note that `T` may have larger alignment that `AValueHeader`,
    /// so we cannot add fixed offset to `self` to get to the payload.
    pub(crate) payload: T,
}

/// "Forward" pointer (pointer to another heap during GC).
///
/// This pointer has `TAG_STR` bit set if it points to a string.
///
/// Lower bit (which is the same bit as `TAG_UNFROZEN`) is always unset
/// regardless of whether it points to frozen or unfrozen value.
/// User of this struct must set this bit explicitly if needed.
#[derive(Copy, Clone, Dupe)]
pub(crate) struct ForwardPtr(usize);

impl ForwardPtr {
    fn new(ptr: usize) -> ForwardPtr {
        debug_assert!(ptr & 1 == 0);
        ForwardPtr(ptr)
    }

    /// Create a forward pointer to a frozen value. This is used during heap freeze.
    pub(crate) fn new_frozen(value: FrozenValue) -> ForwardPtr {
        ForwardPtr::new(value.0.raw().ptr_value())
    }

    /// Create a forward pointer to an unfrozen value. This is used during heap GC.
    pub(crate) fn new_unfrozen(value: Value) -> ForwardPtr {
        debug_assert!(value.unpack_frozen().is_none());
        ForwardPtr::new(value.0.raw().ptr_value() & !1)
    }

    /// It's caller responsibility to ensure that forward pointer points to a frozen value.
    pub(crate) unsafe fn unpack_frozen_value(self) -> FrozenValue {
        FrozenValue::new_ptr_usize_with_str_tag(self.0)
    }

    /// It's caller responsibility to ensure that forward pointer points to an unfrozen value.
    pub(crate) unsafe fn unpack_unfrozen_value<'v>(self) -> Value<'v> {
        Value::new_ptr_usize_with_str_tag(self.0)
    }

    pub(crate) unsafe fn unpack_value<'v>(self, heap_kind: HeapKind) -> Value<'v> {
        match heap_kind {
            HeapKind::Unfrozen => self.unpack_unfrozen_value(),
            HeapKind::Frozen => self.unpack_frozen_value().to_value(),
        }
    }
}

/// This is object written over [`AValueRepr`] during GC.
#[repr(C)]
#[derive(Debug)]
pub(crate) struct AValueForward {
    /// Moved object pointer with lowest bit set.
    forward_ptr: usize,
    /// Size of `AValueRepr<T>` including extra.
    object_size: ValueAllocSize,
}

impl AValueForward {
    pub(crate) fn new(forward_ptr: ForwardPtr, object_size: ValueAllocSize) -> AValueForward {
        AValueForward {
            forward_ptr: forward_ptr.0 | 1,
            object_size,
        }
    }

    /// Unpack forward pointer.
    pub(crate) fn forward_ptr(&self) -> ForwardPtr {
        debug_assert!((self.forward_ptr & 1) != 0);
        ForwardPtr(self.forward_ptr & !1)
    }
}

/// Object on the heap, either a real object or a forward.
#[repr(C)]
pub(crate) union AValueOrForward {
    // We intentionally do not implement `Copy` for these types
    // to avoid accidentally copying them.
    header: ManuallyDrop<AValueHeader>,
    forward: ManuallyDrop<AValueForward>,
    flags: usize,
}

impl AValueOrForward {
    /// Is this pointer a value or forward?
    #[inline]
    fn is_forward(&self) -> bool {
        unsafe { (self.flags & 1) != 0 }
    }

    pub(crate) fn unpack(&self) -> AValueOrForwardUnpack {
        if self.is_forward() {
            AValueOrForwardUnpack::Forward(unsafe { &self.forward })
        } else {
            AValueOrForwardUnpack::Header(unsafe { &self.header })
        }
    }

    #[inline]
    pub(crate) unsafe fn unpack_header_unchecked(&self) -> &AValueHeader {
        debug_assert!(!self.is_forward());
        &self.header
    }

    pub(crate) fn unpack_header(&self) -> Option<&AValueHeader> {
        match self.unpack() {
            AValueOrForwardUnpack::Header(header) => Some(header),
            AValueOrForwardUnpack::Forward(_) => None,
        }
    }

    pub(crate) fn unpack_forward(&self) -> Option<&AValueForward> {
        match self.unpack() {
            AValueOrForwardUnpack::Header(_) => None,
            AValueOrForwardUnpack::Forward(forward) => Some(forward),
        }
    }

    /// Size of allocation for this object:
    /// following object is allocated at `self + alloc_size + align up`.
    pub(crate) fn alloc_size(&self) -> ValueAllocSize {
        match self.unpack() {
            AValueOrForwardUnpack::Header(ptr) => ptr.unpack().memory_size(),
            AValueOrForwardUnpack::Forward(forward) => {
                // Overwritten, so the next word will be the size of the memory
                forward.object_size
            }
        }
    }
}

/// `AValueOrForward` as enum.
pub(crate) enum AValueOrForwardUnpack<'a> {
    Header(&'a AValueHeader),
    Forward(&'a AValueForward),
}

impl AValueForward {
    pub(crate) fn assert_does_not_overwrite_extra<'v, T: AValue<'v>>() {
        assert!(mem::size_of::<AValueForward>() <= AValueRepr::<T>::offset_of_extra());
    }
}

impl AValueHeader {
    /// Alignment of objects in Starlark heap.
    /// We must use 8 byte alignment because we use three lowest bits for tags.
    /// Note the alignment of `AValueHeader` may be smaller than this.
    pub(crate) const ALIGN: usize = 8;

    pub(crate) fn new<'v, T: AValue<'v>>() -> AValueHeader {
        let header = AValueHeader::new_const::<T>();

        let vtable_ptr = header.0 as *const AValueVTable as usize;
        // Check that the LSB is not set, as we reuse that for overwrite
        debug_assert!(vtable_ptr & 1 == 0);

        header
    }

    pub(crate) const fn new_const<'v, T: AValue<'v>>() -> AValueHeader {
        AValueHeader(AValueVTable::new::<T>())
    }

    #[inline]
    pub(crate) fn payload_ptr(&self) -> StarlarkValueRawPtr {
        StarlarkValueRawPtr::new_header(self)
    }

    pub(crate) unsafe fn payload<'v, T: StarlarkValue<'v>>(&self) -> &T {
        debug_assert_eq!(self.0.static_type_of_value.get(), T::static_type_id());
        &*self.payload_ptr().value_ptr::<T>()
    }

    pub(crate) unsafe fn unpack_value<'v>(&'v self, heap_kind: HeapKind) -> Value<'v> {
        match heap_kind {
            HeapKind::Unfrozen => Value::new_ptr_query_is_str(self),
            HeapKind::Frozen => {
                FrozenValue::new_ptr_query_is_str(cast::ptr_lifetime(self)).to_value()
            }
        }
    }

    pub(crate) fn unpack<'v>(&'v self) -> AValueDyn<'v> {
        unsafe {
            // TODO: this assertion does not belong here.
            //   Instead, `Value` should be a `Pointer<AValueOrForward>`
            //   instead of `Pointer<AValueHeader>`,
            //   and assertion should be where we unpack the pointer.
            debug_assert!(
                !(*(self as *const AValueHeader as *const AValueOrForward)).is_forward(),
                "value is a forward pointer; value cannot be unpacked during GC or freeze"
            );
        }
        unsafe { AValueDyn::new(self.payload_ptr(), self.0) }
    }

    /// After performing the overwrite any existing pointers to this value
    /// are corrupted.
    pub unsafe fn overwrite_with_forward<'v, T: StarlarkValue<'v>>(
        me: *mut AValueRepr<T>,
        forward_ptr: ForwardPtr,
    ) -> T {
        // TODO(nga): we don't need to do virtual call to obtain memory size
        let sz = (*me).header.unpack().memory_size();
        let p = me as *const AValueRepr<T>;
        let res = ptr::read(p).payload;
        let p = me as *mut AValueForward;
        *p = AValueForward::new(forward_ptr, sz);
        res
    }

    /// Cast header pointer to repr pointer.
    #[inline]
    pub(crate) unsafe fn as_repr<'v, T: StarlarkValue<'v>>(&self) -> &AValueRepr<T> {
        debug_assert_eq!(T::static_type_id(), self.0.static_type_of_value.get());
        &*(self as *const AValueHeader as *const AValueRepr<T>)
    }

    fn as_avalue_or_header(&self) -> &AValueOrForward {
        unsafe { &*(self as *const AValueHeader as *const AValueOrForward) }
    }

    /// Size of allocation for this object: following object is allocated at `self + alloc_size`.
    pub(crate) fn alloc_size(&self) -> ValueAllocSize {
        self.as_avalue_or_header().alloc_size()
    }
}

impl<T> AValueRepr<T> {
    const _ASSERTIONS: () = {
        assert!(mem::align_of::<Self>() == AValueHeader::ALIGN);
    };

    pub(crate) const fn with_metadata(
        metadata: &'static AValueVTable,
        payload: T,
    ) -> AValueRepr<T> {
        AValueRepr {
            header: AValueHeader(metadata),
            payload,
        }
    }

    pub(crate) fn offset_of_payload() -> usize {
        memoffset::offset_of!(Self, payload)
    }

    /// Padding between header and payload.
    /// Non-zero when alignment of payload is larger than alignment of pointer.
    pub(crate) fn padding_after_header() -> usize {
        Self::offset_of_payload() - mem::size_of::<AValueHeader>()
    }

    /// Offset of value extra content relative to `AValueRepr` start.
    pub(crate) fn offset_of_extra<'v>() -> usize
    where
        T: AValue<'v>,
    {
        Self::offset_of_payload() + T::offset_of_extra()
    }

    pub(crate) fn from_payload_ptr_mut(payload_ptr: *mut T) -> *mut AValueRepr<T> {
        let payload_ptr = payload_ptr as usize;
        let header_ptr = payload_ptr - Self::offset_of_payload();
        header_ptr as *mut AValueRepr<T>
    }
}
