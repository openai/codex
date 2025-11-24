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

use std::borrow::Borrow;
use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;
use std::hash::Hasher;

use allocative::Allocative;
use dupe::Dupe;
use serde::Serialize;
use starlark_map::Equivalent;

use crate::coerce::Coerce;
use crate::coerce::CoerceKey;
use crate::collections::Hashed;
use crate::sealed::Sealed;
use crate::values::layout::static_string::VALUE_EMPTY_STRING;
use crate::values::string::str_type::StarlarkStr;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenValue;
use crate::values::FrozenValueTyped;
use crate::values::Trace;
use crate::values::Value;
use crate::values::ValueTyped;

/// Convenient type alias.
///
/// We use `FrozenValueTyped<StarlarkStr>` often, but also we define more operations
/// on `FrozenValueTyped<StarlarkStr>` than on generic `FrozenValueTyped<T>`.
///
/// Note there's a macro `const_frozen_string!` to statically allocate `FrozenStringValue`:
///
/// ```
/// use starlark::const_frozen_string;
/// use starlark::values::FrozenStringValue;
/// use starlark::values::FrozenValue;
///
/// let fv: FrozenStringValue = const_frozen_string!("magic");
/// assert_eq!("magic", fv.as_str());
/// ```
pub type FrozenStringValue = FrozenValueTyped<'static, StarlarkStr>;

/// Convenient type alias.
///
/// We use `ValueTyped<StarlarkStr>` often, but also we define more operations
/// on `ValueTyped<StarlarkStr>` than on generic `ValueTyped<T>`.
pub type StringValue<'v> = ValueTyped<'v, StarlarkStr>;

// TODO(nga): figure out how to make these operations generic over `T`.
unsafe impl<'v> Coerce<StringValue<'v>> for FrozenStringValue {}
unsafe impl<'v> CoerceKey<StringValue<'v>> for FrozenStringValue {}

impl Borrow<str> for FrozenStringValue {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl<'v> Borrow<str> for StringValue<'v> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl<'v> Equivalent<FrozenStringValue> for StringValue<'v> {
    fn equivalent(&self, key: &FrozenStringValue) -> bool {
        *self == key.to_string_value()
    }
}

impl<'v> Equivalent<StringValue<'v>> for FrozenStringValue {
    fn equivalent(&self, key: &StringValue<'v>) -> bool {
        self.to_string_value() == *key
    }
}

impl<'v> Default for StringValue<'v> {
    fn default() -> Self {
        FrozenStringValue::default().to_string_value()
    }
}

impl Default for FrozenStringValue {
    fn default() -> Self {
        VALUE_EMPTY_STRING.erase()
    }
}

impl FrozenStringValue {
    /// Get self along with the hash.
    pub fn get_hashed(self) -> Hashed<Self> {
        Hashed::new_unchecked(self.get_hash(), self)
    }

    /// Get the [`FrozenValue`] along with the hash.
    pub fn get_hashed_value(self) -> Hashed<FrozenValue> {
        Hashed::new_unchecked(self.get_hash(), self.to_frozen_value())
    }

    /// Get the string reference along with the hash.
    pub fn get_hashed_str(self) -> Hashed<&'static str> {
        Hashed::new_unchecked(self.get_hash(), self.as_str())
    }
}

impl<'v> StringValue<'v> {
    /// Convert a value to a [`FrozenStringValue`] using a supplied [`Freezer`].
    pub fn freeze(self, freezer: &Freezer) -> FreezeResult<FrozenStringValue> {
        Ok(unsafe { FrozenStringValue::new_unchecked(freezer.freeze(self.to_value())?) })
    }

    /// Get self along with the hash.
    pub fn get_hashed(self) -> Hashed<Self> {
        Hashed::new_unchecked(self.get_hash(), self)
    }

    /// Get the string reference along with the hash.
    pub fn get_hashed_str(self) -> Hashed<&'v str> {
        Hashed::new_unchecked(self.get_hash(), self.as_str())
    }

    /// Get the [`Value`] along with the hash.
    pub fn get_hashed_value(self) -> Hashed<Value<'v>> {
        Hashed::new_unchecked(self.get_hash(), self.to_value())
    }

    /// If this string value is frozen, return it.
    pub fn unpack_frozen(self) -> Option<FrozenStringValue> {
        self.to_value()
            .unpack_frozen()
            .map(|s| unsafe { FrozenStringValue::new_unchecked(s) })
    }

    #[inline]
    pub(crate) unsafe fn cast_lifetime<'w>(self) -> StringValue<'w> {
        StringValue::new_unchecked(self.to_value().cast_lifetime())
    }
}

/// Common type for [`StringValue`] and [`FrozenStringValue`].
pub trait StringValueLike<'v>:
    Trace<'v>
    + Freeze<Frozen = FrozenStringValue>
    + CoerceKey<StringValue<'v>>
    + Borrow<str>
    + Display
    + Debug
    + Default
    + Eq
    + Ord
    + Copy
    + Clone
    + Dupe
    + Serialize
    + Allocative
    + Sealed
    + 'v
{
    /// Convert to a [`StringValue`].
    fn to_string_value(self) -> StringValue<'v>;

    /// Convert to a [`str`].
    fn as_str(self) -> &'v str {
        self.to_string_value().as_str()
    }
}

impl<'v> Sealed for StringValue<'v> {}

impl<'v> StringValueLike<'v> for StringValue<'v> {
    fn to_string_value(self) -> StringValue<'v> {
        self
    }
}

impl Sealed for FrozenStringValue {}

impl<'v> StringValueLike<'v> for FrozenStringValue {
    fn to_string_value(self) -> StringValue<'v> {
        self.to_value_typed()
    }
}

impl<'v> PartialEq<StringValue<'v>> for FrozenStringValue {
    fn eq(&self, other: &StringValue<'v>) -> bool {
        &self.to_value_typed() == other
    }
}

impl<'v> PartialEq<FrozenStringValue> for StringValue<'v> {
    fn eq(&self, other: &FrozenStringValue) -> bool {
        self == &other.to_value_typed()
    }
}

impl<'v> Hash for StringValue<'v> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state)
    }
}

impl Hash for FrozenStringValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state)
    }
}

impl<'v> PartialOrd for StringValue<'v> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'v> Ord for StringValue<'v> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl PartialOrd for FrozenStringValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FrozenStringValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use crate::collections::Hashed;
    use crate::values::FrozenHeap;
    use crate::values::FrozenStringValue;
    use crate::values::FrozenValue;
    use crate::values::Heap;
    use crate::values::StringValue;
    use crate::values::Value;
    use crate::values::ValueLike;

    #[test]
    fn test_string_hashes() {
        let expected = Hashed::new("xyz").hash();

        let heap = Heap::new();
        let s: StringValue = heap.alloc_str("xyz");
        assert_eq!(expected, Hashed::new(s).hash());
        assert_eq!(s.get_hashed().hash(), s.hashed().unwrap().hash());
        let v: Value = heap.alloc_str("xyz").to_value();
        assert_eq!(expected, v.get_hashed().unwrap().hash());

        let heap = FrozenHeap::new();
        let fs: FrozenStringValue = heap.alloc_str("xyz");
        assert_eq!(expected, Hashed::new(fs).hash());
        let fv: FrozenValue = heap.alloc_str("xyz").to_frozen_value();
        assert_eq!(expected, fv.get_hashed().unwrap().hash());
    }
}
