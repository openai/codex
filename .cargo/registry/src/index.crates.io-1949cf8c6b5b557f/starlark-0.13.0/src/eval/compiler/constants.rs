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

use dupe::Dupe;
use once_cell::sync::Lazy;

use crate::environment::Globals;
use crate::values::namespace::FrozenNamespace;
use crate::values::FrozenValue;

#[derive(Copy, Clone, Dupe, Debug)]
pub(crate) struct BuiltinFn(pub(crate) FrozenValue);

impl PartialEq<FrozenValue> for BuiltinFn {
    fn eq(&self, other: &FrozenValue) -> bool {
        // Pointer equality works because `#[starlark_module]` proc macro
        // generates a singleton which allocates the function only once
        // even if builder function is called multiple times.
        self.0.to_value().ptr_eq(other.to_value())
    }
}

impl PartialEq<BuiltinFn> for FrozenValue {
    fn eq(&self, other: &BuiltinFn) -> bool {
        other == self
    }
}

pub(crate) struct Constants {
    pub(crate) fn_len: BuiltinFn,
    pub(crate) fn_type: BuiltinFn,
    pub(crate) fn_list: BuiltinFn,
    pub(crate) fn_dict: BuiltinFn,
    pub(crate) fn_tuple: BuiltinFn,
    pub(crate) fn_isinstance: BuiltinFn,
    pub(crate) fn_set: BuiltinFn,
    // Technically, this is not a function.
    pub(crate) typing_callable: BuiltinFn,
}

impl Constants {
    pub fn get() -> &'static Constants {
        static RES: Lazy<Constants> = Lazy::new(|| {
            let g = Globals::extended_internal();
            Constants {
                fn_len: BuiltinFn(g.get_frozen("len").unwrap()),
                fn_type: BuiltinFn(g.get_frozen("type").unwrap()),
                fn_list: BuiltinFn(g.get_frozen("list").unwrap()),
                fn_dict: BuiltinFn(g.get_frozen("dict").unwrap()),
                fn_tuple: BuiltinFn(g.get_frozen("tuple").unwrap()),
                fn_isinstance: BuiltinFn(g.get_frozen("isinstance").unwrap()),
                fn_set: BuiltinFn(g.get_frozen("set").unwrap()),
                typing_callable: {
                    let typing = g
                        .get_frozen("typing")
                        .unwrap()
                        .downcast_frozen_ref::<FrozenNamespace>()
                        .unwrap();
                    BuiltinFn(typing.as_ref().get("Callable").unwrap())
                },
            }
        });
        Lazy::force(&RES)
    }
}

#[cfg(test)]
mod tests {
    use crate::environment::Globals;
    use crate::eval::compiler::constants::Constants;

    #[test]
    fn test_constants() {
        assert_eq!(
            Globals::standard().get_frozen("len").unwrap(),
            Constants::get().fn_len
        );
        assert_eq!(
            Globals::extended_internal().get_frozen("len").unwrap(),
            Constants::get().fn_len
        );
    }
}
