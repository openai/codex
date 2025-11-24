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

#![doc(hidden)]
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::Value;

#[derive(Debug, thiserror::Error)]
enum TypingMacroRefsError {
    #[error("LHS is not a type: `{0}`")]
    LhsNotType(String),
}

/// Implementation of `bit_or` for `StarlarkValue` implementations which are types.
pub fn starlark_value_bit_or_for_type<'v, S: StarlarkValue<'v>>(
    this: &S,
    other: Value<'v>,
    heap: &'v Heap,
) -> crate::Result<Value<'v>> {
    let Some(this) = this.eval_type() else {
        let mut repr = String::new();
        this.collect_repr(&mut repr);
        return Err(crate::Error::new_other(TypingMacroRefsError::LhsNotType(
            repr,
        )));
    };
    let this = TypeCompiled::from_ty(&this, heap);
    let other = TypeCompiled::new(other, heap)?;
    Ok(TypeCompiled::type_any_of_two(this, other, heap).to_inner())
}
