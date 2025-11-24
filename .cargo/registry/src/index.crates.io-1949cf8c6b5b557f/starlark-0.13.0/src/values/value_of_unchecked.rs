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
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::marker::PhantomData;

use allocative::Allocative;
use dupe::Clone_;
use dupe::Copy_;
use dupe::Dupe_;

use crate::coerce::Coerce;
use crate::typing::Ty;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::Trace;
use crate::values::Tracer;
use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueLifetimeless;
use crate::values::ValueLike;

/// Store value annotated with type, but do not check the type.
#[derive(Clone_, Copy_, Dupe_, Allocative)]
#[allocative(bound = "")]
pub struct ValueOfUncheckedGeneric<V: ValueLifetimeless, T: StarlarkTypeRepr>(
    V,
    PhantomData<fn() -> T>,
);

unsafe impl<V, U, T> Coerce<ValueOfUncheckedGeneric<V, T>> for ValueOfUncheckedGeneric<U, T>
where
    V: ValueLifetimeless,
    U: ValueLifetimeless,
    U: Coerce<V>,
    T: StarlarkTypeRepr,
{
}

impl<V: ValueLifetimeless, T: StarlarkTypeRepr> ValueOfUncheckedGeneric<V, T> {
    /// New.
    #[inline]
    pub fn new(value: V) -> Self {
        Self(value, PhantomData)
    }

    /// Cast to a different Rust type for the same Starlark type.
    #[inline]
    pub fn cast<U: StarlarkTypeRepr<Canonical = T::Canonical>>(
        self,
    ) -> ValueOfUncheckedGeneric<V, U> {
        ValueOfUncheckedGeneric::new(self.0)
    }

    /// Get the value.
    #[inline]
    pub fn get(self) -> V {
        self.0
    }

    /// Unpack the value.
    pub fn unpack<'v>(self) -> crate::Result<T>
    where
        V: ValueLike<'v>,
        T: UnpackValue<'v>,
    {
        Ok(T::unpack_value_err(self.get().to_value())?)
    }
}

impl<V: ValueLifetimeless, T: StarlarkTypeRepr> Debug for ValueOfUncheckedGeneric<V, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ValueOfUnchecked")
            .field(&self.get())
            .finish()
    }
}

impl<V: ValueLifetimeless, T: StarlarkTypeRepr> Display for ValueOfUncheckedGeneric<V, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.get(), f)
    }
}

impl<V: ValueLifetimeless, T: StarlarkTypeRepr> StarlarkTypeRepr for ValueOfUncheckedGeneric<V, T> {
    type Canonical = T::Canonical;

    fn starlark_type_repr() -> Ty {
        <Self as StarlarkTypeRepr>::Canonical::starlark_type_repr()
    }
}

impl<'v, V: ValueLike<'v>, T: StarlarkTypeRepr> AllocValue<'v> for ValueOfUncheckedGeneric<V, T> {
    fn alloc_value(self, _heap: &'v Heap) -> Value<'v> {
        self.0.to_value()
    }
}

impl<T: StarlarkTypeRepr> AllocFrozenValue for ValueOfUncheckedGeneric<FrozenValue, T> {
    fn alloc_frozen_value(self, _heap: &FrozenHeap) -> FrozenValue {
        self.0
    }
}

unsafe impl<'v, V, T> Trace<'v> for ValueOfUncheckedGeneric<V, T>
where
    // This is essentially `V: ValueLike<'v>`,
    // but for derive it is convenient to have these bounds.
    V: ValueLifetimeless + Trace<'v>,
    T: StarlarkTypeRepr,
{
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.0.trace(tracer)
    }
}

impl<V: ValueLifetimeless + Freeze, T: StarlarkTypeRepr> Freeze for ValueOfUncheckedGeneric<V, T> {
    type Frozen = ValueOfUncheckedGeneric<FrozenValue, T>;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let frozen = self.0.freeze(freezer)?;
        Ok(ValueOfUncheckedGeneric::new(frozen))
    }
}

/// Starlark value with type annotation.
///
/// Can be used in function signatures to provide types to the type checker.
///
/// Note this type does not actually check the type of the value.
/// Providing incorrect type annotation will result
/// in incorrect error reporting by the type checker.
pub type ValueOfUnchecked<'v, T> = ValueOfUncheckedGeneric<Value<'v>, T>;

/// Frozen starlark value with type annotation.
pub type FrozenValueOfUnchecked<'f, T> = ValueOfUncheckedGeneric<FrozenValue, T>;

impl<'v, T: StarlarkTypeRepr> ValueOfUnchecked<'v, T> {
    /// Construct after checking the type.
    #[inline]
    pub fn new_checked(value: Value<'v>) -> anyhow::Result<Self>
    where
        T: UnpackValue<'v>,
    {
        T::unpack_value_err(value)?;
        Ok(Self::new(value))
    }
}

impl<'v, V: ValueLike<'v>, T: StarlarkTypeRepr> ValueOfUncheckedGeneric<V, T> {
    /// Convert to a value.
    #[inline]
    pub fn to_value(self) -> ValueOfUnchecked<'v, T> {
        ValueOfUnchecked::new(self.0.to_value())
    }
}

impl<'v, T: StarlarkTypeRepr> UnpackValue<'v> for ValueOfUnchecked<'v, T> {
    type Error = Infallible;

    #[inline]
    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        Ok(Some(Self::new(value)))
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::const_frozen_string;
    use crate::typing::Ty;
    use crate::values::type_repr::StarlarkTypeRepr;
    use crate::values::FrozenValueOfUnchecked;
    use crate::values::ValueOfUnchecked;

    #[test]
    fn test_cast_example() {
        let a =
            ValueOfUnchecked::<String>::new_checked(const_frozen_string!("a").to_value()).unwrap();
        let _b: ValueOfUnchecked<&str> = a.cast();
    }

    #[test]
    fn test_frozen_value_of_unchecked_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        #[allow(dead_code)]
        struct ReprNotSendSync(Rc<String>);
        impl StarlarkTypeRepr for ReprNotSendSync {
            type Canonical = Self;
            fn starlark_type_repr() -> Ty {
                panic!("not needed in test")
            }
        }

        assert_send_sync::<FrozenValueOfUnchecked<ReprNotSendSync>>();
    }

    #[test]
    fn test_frozen_value_of_unchecked_covariant() {
        fn _assert_covariant<'a>(
            _value: FrozenValueOfUnchecked<'static, String>,
        ) -> FrozenValueOfUnchecked<'a, String> {
            panic!()
        }
    }
}
