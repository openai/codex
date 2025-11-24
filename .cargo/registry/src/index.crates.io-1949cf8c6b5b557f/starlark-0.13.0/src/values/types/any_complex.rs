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

//! A type [`StarlarkAnyComplex`] which can wrap any Rust value into a [`Value`].

use std::any;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;

use allocative::Allocative;
use starlark_derive::starlark_value;
use starlark_derive::NoSerialize;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::values::AllocValue;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::Trace;
use crate::values::Value;
use crate::values::ValueLike;

/// Allocate arbitrary value on the starlark heap without implementing full [`StarlarkValue`].
///
/// This is useful for data not directly visible to starlark code.
///
/// This type is for "complex" values (with tracing during GC). For no GC version check
/// [`StarlarkAny`](crate::values::types::any::StarlarkAny).
#[derive(Trace, Freeze, Allocative, ProvidesStaticType, NoSerialize)]
pub struct StarlarkAnyComplex<T> {
    /// The value.
    pub value: T,
}

impl<'v, T> StarlarkAnyComplex<T>
where
    Self: StarlarkValue<'v>,
{
    /// Construct a new `StarlarkAnyComplex` value, which can be allocated on the heap.
    pub fn new(value: T) -> StarlarkAnyComplex<T> {
        StarlarkAnyComplex { value }
    }

    /// Obtain the value from a `Value`, if it is a `StarlarkAnyComplex<T>`.
    pub fn get(value: Value<'v>) -> Option<&'v T> {
        value.downcast_ref::<Self>().map(|x| &x.value)
    }

    /// Obtain the value from a `Value`, if it is a `StarlarkAnyComplex<T>`.
    pub fn get_err(value: Value<'v>) -> anyhow::Result<&'v T> {
        value.downcast_ref_err::<Self>().map(|x| &x.value)
    }
}

// Proper `Debug` is hard to require from users because of `Freeze` and `ProvidesStaticType`.
impl<T> Debug for StarlarkAnyComplex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(any::type_name::<T>())
            .finish_non_exhaustive()
    }
}

impl<T> Display for StarlarkAnyComplex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

#[starlark_value(type = "any_complex")]
impl<'v, T> StarlarkValue<'v> for StarlarkAnyComplex<T>
where
    T: Allocative + ProvidesStaticType<'v> + 'v,
    T::StaticType: Sized,
{
    type Canonical = Self;
}

impl<'v, T> AllocValue<'v> for StarlarkAnyComplex<T>
where
    Self: StarlarkValue<'v> + Freeze,
    T: Trace<'v>,
    <Self as Freeze>::Frozen: StarlarkValue<'static>,
{
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc_complex(self)
    }
}

#[cfg(test)]
mod tests {
    use allocative::Allocative;
    use starlark_derive::ProvidesStaticType;
    use starlark_derive::Trace;

    use crate as starlark;
    use crate::const_frozen_string;
    use crate::environment::Module;
    use crate::values::list::AllocList;
    use crate::values::types::any_complex::StarlarkAnyComplex;
    use crate::values::Freeze;
    use crate::values::FreezeResult;
    use crate::values::Freezer;
    use crate::values::FrozenStringValue;
    use crate::values::FrozenValue;
    use crate::values::StringValue;
    use crate::values::Value;

    #[test]
    fn test_any_complex() {
        #[derive(Trace, Allocative, ProvidesStaticType)]
        struct UnfrozenData<'v> {
            string: StringValue<'v>,
            other: Value<'v>,
        }

        impl<'v> Freeze for UnfrozenData<'v> {
            type Frozen = FrozenData;

            fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
                Ok(FrozenData {
                    string: self.string.freeze(freezer)?,
                    other: freezer.freeze(self.other)?,
                })
            }
        }

        #[derive(Allocative, ProvidesStaticType)]
        struct FrozenData {
            string: FrozenStringValue,
            other: FrozenValue,
        }

        let module = Module::new();

        let data = module.heap().alloc(StarlarkAnyComplex::new(UnfrozenData {
            string: module.heap().alloc_str("aaa"),
            other: module.heap().alloc(AllocList([1, 2])),
        }));

        assert_eq!(
            const_frozen_string!("aaa"),
            StarlarkAnyComplex::<UnfrozenData>::get_err(data)
                .unwrap()
                .string
        );

        module.set_extra_value(data);

        let module = module.freeze().unwrap();

        let data = module.extra_value().unwrap();
        assert_eq!(
            const_frozen_string!("aaa"),
            StarlarkAnyComplex::<FrozenData>::get_err(data.to_value())
                .unwrap()
                .string
        );
    }
}
