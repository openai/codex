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

use starlark_derive::starlark_module;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::eval::Evaluator;
use crate::values::typing::ty::AbstractType;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::Value;
use crate::values::ValueOfUnchecked;

#[starlark_module]
pub(crate) fn register_eval_type(globals: &mut GlobalsBuilder) {
    /// Create a runtime type object which can be used to check if a value matches the given type.
    fn eval_type<'v>(
        #[starlark(require = pos)] ty: ValueOfUnchecked<'v, AbstractType>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<TypeCompiled<Value<'v>>> {
        TypeCompiled::new(ty.get(), eval.heap())
    }

    /// Check if a value matches the given type.
    ///
    /// This operation can be very fast or very slow depending on how it is used.
    ///
    /// `isinstance(x, list)` is very fast,
    /// because it is compiled to a special bytecode instruction.
    ///
    /// `isinstance(x, list[str])` is `O(N)` operation
    /// because it checks every element in this list.
    ///
    /// `L = list; [isinstance(x, L) for x in y]` is slow when `L` is not a constant:
    /// `isinstance()` first converts `list` to a type in a loop, which is slow.
    ///
    /// But last operation can be optimized like this:
    /// `L = eval_type(list); [isinstance(x, L) for x in y]`:
    /// `eval_type()` converts `list` value into prepared type matcher.
    fn isinstance<'v>(
        #[starlark(require = pos)] value: Value<'v>,
        #[starlark(require = pos)] ty: ValueOfUnchecked<'v, AbstractType>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<bool> {
        Ok(TypeCompiled::new(ty.get(), eval.heap())?.matches(value))
    }
}

#[cfg(test)]
mod tests {
    use crate::assert;

    #[test]
    fn test_typechecking() {
        assert::fail(
            r#"
def test():
    isinstance(1, "")
"#,
            "Expected type `type` but got `str`",
        );
    }
}
