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

use crate::typing::Ty;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::types::num::value::Num;
use crate::values::types::num::value::NumRef;
use crate::values::UnpackValue;
use crate::values::Value;

/// Unpack `int` or `float` into `f64`.
pub struct UnpackFloat(pub f64);

impl StarlarkTypeRepr for UnpackFloat {
    type Canonical = <Num as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        <Self::Canonical as StarlarkTypeRepr>::starlark_type_repr()
    }
}

impl<'v> UnpackValue<'v> for UnpackFloat {
    type Error = <NumRef<'v> as UnpackValue<'v>>::Error;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        let Some(num) = NumRef::unpack_value_impl(value)? else {
            return Ok(None);
        };
        Ok(Some(UnpackFloat(num.as_float())))
    }
}

#[cfg(test)]
mod tests {
    use crate::values::float::UnpackFloat;
    use crate::values::Heap;
    use crate::values::UnpackValue;
    use crate::values::Value;

    #[test]
    fn test_unpack_float() {
        let heap = Heap::new();
        assert_eq!(
            1.0,
            UnpackFloat::unpack_value(Value::testing_new_int(1))
                .unwrap()
                .unwrap()
                .0
        );
        assert_eq!(
            1.0,
            UnpackFloat::unpack_value(heap.alloc(1.0))
                .unwrap()
                .unwrap()
                .0
        );
    }
}
