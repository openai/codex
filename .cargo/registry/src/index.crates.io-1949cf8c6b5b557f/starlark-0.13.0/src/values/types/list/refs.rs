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

use std::convert::Infallible;
use std::fmt;
use std::fmt::Display;
use std::iter;
use std::ops::Deref;
use std::slice;

use ref_cast::ref_cast_custom;
use ref_cast::RefCastCustom;

use crate::coerce::coerce;
use crate::typing::Ty;
use crate::values::list::value::display_list;
use crate::values::list::value::FrozenListData;
use crate::values::list::value::ListGen;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::types::list::value::ListData;
use crate::values::FrozenValue;
use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueLike;

/// Reference to list content (mutable or frozen).
#[repr(transparent)]
#[derive(RefCastCustom)]
pub struct ListRef<'v> {
    pub(crate) content: [Value<'v>],
}

/// Reference to frozen list content.
#[repr(transparent)]
#[derive(RefCastCustom)]
pub struct FrozenListRef {
    pub(crate) content: [FrozenValue],
}

impl<'v> ListRef<'v> {
    /// `type([])`, which is `"list"`.
    pub const TYPE: &'static str = ListData::TYPE;

    #[ref_cast_custom]
    pub(crate) fn new<'a>(slice: &'a [Value<'v>]) -> &'a ListRef<'v>;

    /// Empty list reference.
    pub fn empty() -> &'v ListRef<'v> {
        ListRef::new(&[])
    }

    /// List elements.
    pub fn content(&self) -> &[Value<'v>] {
        &self.content
    }

    /// Iterate over the elements in the list.
    pub fn iter<'a>(&'a self) -> iter::Copied<slice::Iter<'a, Value<'v>>>
    where
        'v: 'a,
    {
        self.content.iter().copied()
    }

    /// Downcast the value to the list or frozen list (both are represented by `ListRef`).
    pub fn from_value(x: Value<'v>) -> Option<&'v ListRef<'v>> {
        if x.unpack_frozen().is_some() {
            x.downcast_ref::<ListGen<FrozenListData>>()
                .map(|x| ListRef::new(coerce(x.0.content())))
        } else {
            let ptr = x.downcast_ref::<ListGen<ListData>>()?;
            Some(ListRef::new(ptr.0.content()))
        }
    }

    /// Downcast the list.
    pub fn from_frozen_value<'f>(x: FrozenValue) -> Option<&'f ListRef<'f>> {
        x.downcast_ref::<ListGen<FrozenListData>>()
            .map(|x| ListRef::new(coerce(x.0.content())))
    }
}

impl FrozenListRef {
    /// `type([])`, which is `"list"`.
    pub const TYPE: &'static str = ListRef::TYPE;

    #[ref_cast_custom]
    fn new(slice: &[FrozenValue]) -> &FrozenListRef;

    /// Downcast to the frozen list.
    ///
    /// This function returns `None` if the value is not a list or the list is not frozen.
    pub fn from_value(x: Value) -> Option<&'static FrozenListRef> {
        Self::from_frozen_value(x.unpack_frozen()?)
    }

    /// Downcast to the frozen list.
    ///
    /// This function returns `None` if the value is not a frozen list.
    /// (Value cannot be a mutable list because value is frozen.)
    pub fn from_frozen_value(x: FrozenValue) -> Option<&'static FrozenListRef> {
        x.downcast_ref::<ListGen<FrozenListData>>()
            .map(|x| FrozenListRef::new(x.0.content()))
    }
}

impl<'v> Deref for ListRef<'v> {
    type Target = [Value<'v>];

    fn deref(&self) -> &[Value<'v>] {
        &self.content
    }
}

impl Deref for FrozenListRef {
    type Target = [FrozenValue];

    fn deref(&self) -> &[FrozenValue] {
        &self.content
    }
}

impl<'v> Display for ListRef<'v> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display_list(&self.content, f)
    }
}

impl Display for FrozenListRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display_list(coerce(&self.content), f)
    }
}

impl<'v> StarlarkTypeRepr for &'v ListRef<'v> {
    type Canonical = <Vec<Value<'v>> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        Vec::<Value<'v>>::starlark_type_repr()
    }
}

impl<'v> StarlarkTypeRepr for &'v FrozenListRef {
    type Canonical = <Vec<FrozenValue> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        Vec::<FrozenValue>::starlark_type_repr()
    }
}

impl<'v> UnpackValue<'v> for &'v ListRef<'v> {
    type Error = Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        Ok(ListRef::from_value(value))
    }
}

impl<'v> UnpackValue<'v> for &'v FrozenListRef {
    type Error = crate::Error;

    fn unpack_value_impl(value: Value<'v>) -> crate::Result<Option<Self>> {
        // TODO(nga): error if not frozen.
        Ok(FrozenListRef::from_value(value))
    }
}
