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

use starlark_derive::starlark_module;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::values::function::SpecialBuiltinFunction;
use crate::values::set::refs::SetRef;
use crate::values::set::value::FrozenSet;
use crate::values::set::value::SetData;
use crate::values::typing::StarlarkIter;
use crate::values::Heap;
use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueOfUnchecked;

#[starlark_module]
pub(crate) fn register_set(globals: &mut GlobalsBuilder) {
    #[starlark(
        as_type = FrozenSet,
        speculative_exec_safe,
        special_builtin_function = SpecialBuiltinFunction::Set,
    )]
    fn set<'v>(
        #[starlark(require = pos)] arg: Option<ValueOfUnchecked<'v, StarlarkIter<Value<'v>>>>,
        heap: &'v Heap,
    ) -> starlark::Result<SetData<'v>> {
        let set = match arg {
            Some(pos) => match SetRef::unpack_value_opt(pos.get()) {
                Some(set) => (set.aref).clone(),
                None => {
                    let it = pos.get().iterate(heap)?;
                    let mut data = SetData::default();
                    for el in it {
                        let el = el.get_hashed()?;
                        data.content.insert_hashed(el);
                    }
                    data
                }
            },
            None => SetData::default(),
        };
        Ok(set)
    }
}
#[cfg(test)]
mod tests {
    use crate::assert;

    #[test]
    fn test_set_type_as_type_compile_time() {
        assert::fail(
            r"
def f_fail_ct(x: set[int]):
    return x

s = set(['not_int'])

f_fail_ct(s)
",
            //Is it actually runtime or compile time error?
            r#"Value `set(["not_int"])` of type `set` does not match the type annotation `set[int]` for argument `x`"#,
        );
    }

    #[test]
    fn test_return_set_type_as_type_compile_time() {
        assert::fail(
            r"
def f_fail_ct(x: str) -> set[int]:
    return set([x])

f_fail_ct('not_int')
",
            //Is it actually runtime or compile time error?
            r#"Value `set(["not_int"])` of type `set` does not match the type annotation `set[int]` for return type"#,
        );
    }

    #[test]
    fn test_set_type_as_type_run_time() {
        assert::fail(
            r"
def f_fail_rt(x: set[int]):
    return x

s = set(['not_int'])

noop(f_fail_rt)(s)
",
            r#"Value `set(["not_int"])` of type `set` does not match the type annotation `set[int]` for argument `x`"#,
        );
    }
}
