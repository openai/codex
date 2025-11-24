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

use crate::values::layout::avalue;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::avalue::AValueSimple;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::FrozenValue;
use crate::values::FrozenValueTyped;
use crate::values::StarlarkValue;

/// Allocate simple value statically.
pub struct AllocStaticSimple<T: StarlarkValue<'static>>(
    AValueRepr<AValueImpl<'static, AValueSimple<T>>>,
);

impl<T: StarlarkValue<'static>> AllocStaticSimple<T> {
    /// Allocate a value statically.
    pub const fn alloc(value: T) -> Self {
        AllocStaticSimple(avalue::alloc_static::<AValueSimple<T>>(value))
    }

    /// Get the value.
    pub fn unpack(&'static self) -> FrozenValueTyped<'static, T> {
        FrozenValueTyped::new_repr(&self.0)
    }

    /// Get the value.
    pub fn to_frozen_value(&'static self) -> FrozenValue {
        self.unpack().to_frozen_value()
    }
}

#[cfg(test)]
mod tests {
    use allocative::Allocative;
    use starlark_derive::starlark_value;
    use starlark_derive::NoSerialize;
    use starlark_derive::ProvidesStaticType;

    use crate as starlark;
    use crate::values::AllocStaticSimple;
    use crate::values::StarlarkValue;

    #[test]
    fn test_alloc_static_simple() {
        #[derive(
            Debug,
            derive_more::Display,
            ProvidesStaticType,
            NoSerialize,
            Allocative
        )]
        #[display("MySimpleValue")]
        struct MySimpleValue(u32);

        #[starlark_value(type = "MySimpleValue")]
        impl<'v> StarlarkValue<'v> for MySimpleValue {}

        static VALUE: AllocStaticSimple<MySimpleValue> =
            AllocStaticSimple::alloc(MySimpleValue(17));
        assert_eq!(17, VALUE.unpack().as_ref().0);
    }
}
