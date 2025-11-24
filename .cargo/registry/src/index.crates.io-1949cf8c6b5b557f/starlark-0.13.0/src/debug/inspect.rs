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

use crate::collections::SmallMap;
use crate::eval::compiler::def::Def;
use crate::eval::compiler::def::FrozenDef;
use crate::eval::runtime::slots::LocalSlotIdCapturedOrNot;
use crate::eval::Evaluator;
use crate::values::FrozenStringValue;
use crate::values::Value;
use crate::values::ValueLike;

pub(crate) fn to_scope_names_by_local_slot_id<'v>(x: Value<'v>) -> Option<&'v [FrozenStringValue]> {
    if x.unpack_frozen().is_some() {
        x.downcast_ref::<FrozenDef>()
            .map(|x| x.def_info.used.as_ref())
    } else {
        x.downcast_ref::<Def>().map(|x| x.def_info.used.as_ref())
    }
}

impl<'v> Evaluator<'v, '_, '_> {
    /// Obtain the local variables currently in scope. When at top-level these will be
    /// [`Module`](crate::environment::Module) variables, otherwise local definitions. The precise number of variables
    /// may change over time due to optimisation. The only legitimate use of this function is for debugging.
    pub fn local_variables(&self) -> SmallMap<String, Value<'v>> {
        inspect_local_variables(self).unwrap_or_else(|| inspect_module_variables(self))
    }
}

fn inspect_local_variables<'v>(
    eval: &Evaluator<'v, '_, '_>,
) -> Option<SmallMap<String, Value<'v>>> {
    // First we find the first entry on the call_stack which contains a Def (and thus has locals)
    let xs = eval.call_stack.to_function_values();
    let names = xs
        .into_iter()
        .rev()
        .find_map(to_scope_names_by_local_slot_id)?;
    let mut res = SmallMap::new();
    for (slot, name) in names.iter().enumerate() {
        // TODO(nga): correctly handle captured.
        if let Some(v) = eval
            .current_frame
            .get_slot_slow(LocalSlotIdCapturedOrNot(slot as u32))
        {
            res.insert(name.as_str().to_owned(), v);
        }
    }
    Some(res)
}

fn inspect_module_variables<'v>(eval: &Evaluator<'v, '_, '_>) -> SmallMap<String, Value<'v>> {
    let mut res = SmallMap::new();
    for (name, slot) in eval.module_env.mutable_names().all_names_and_slots() {
        if let Some(v) = eval.module_env.slots().get_slot(slot) {
            res.insert(name.as_str().to_owned(), v);
        }
    }
    res
}

#[cfg(test)]
mod tests {
    use starlark_derive::starlark_module;
    use starlark_syntax::slice_vec_ext::SliceExt;

    use crate as starlark;
    use crate::assert;
    use crate::coerce::coerce;
    use crate::collections::SmallMap;
    use crate::environment::GlobalsBuilder;
    use crate::eval::Evaluator;
    use crate::values::dict::Dict;

    #[starlark_module]
    fn debugger(builder: &mut GlobalsBuilder) {
        fn debug_inspect_stack(eval: &mut Evaluator) -> anyhow::Result<Vec<String>> {
            Ok(eval.call_stack().into_frames().map(ToString::to_string))
        }

        fn debug_inspect_variables<'v>(
            eval: &mut Evaluator<'v, '_, '_>,
        ) -> anyhow::Result<Dict<'v>> {
            let mut sm = SmallMap::new();
            for (k, v) in eval.local_variables() {
                sm.insert_hashed(eval.heap().alloc_str(&k).get_hashed(), v);
            }
            Ok(Dict::new(coerce(sm)))
        }
    }

    #[test]
    fn test_debug_stack() {
        let mut a = assert::Assert::new();
        a.globals_add(debugger);
        a.pass(
            r#"
def assert_stack(want):
    stack = debug_inspect_stack()
    assert_eq([x.split(' ')[0] for x in stack[:-2]], want)

assert_stack([])

def f(): assert_stack(["g", "f"])
def g(): f()
g()
"#,
        );
    }

    #[test]
    fn test_debug_variables() {
        let mut a = assert::Assert::new();
        a.globals_add(debugger);
        a.pass(
            r#"
root = 12
_ignore = [x for x in [True]]
def f(x = 1, y = "test"):
    z = x + 5
    for _magic in [False, True]:
        continue
    assert_eq(debug_inspect_variables(), {"x": 1, "y": "hello", "z": 6, "_magic": True})
f(y = "hello")
assert_eq(debug_inspect_variables(), {"root": 12, "f": f, "_ignore": [True]})
"#,
        );
    }
}
