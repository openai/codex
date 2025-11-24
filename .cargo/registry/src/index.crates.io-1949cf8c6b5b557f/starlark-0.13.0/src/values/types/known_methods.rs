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

use dupe::Dupe;
use hashbrown::HashMap;
use once_cell::sync::Lazy;

use crate::environment::Methods;
use crate::eval::Arguments;
use crate::eval::Evaluator;
use crate::values::dict::value::dict_methods;
use crate::values::function::NativeMeth;
use crate::values::function::NativeMethod;
use crate::values::list::value::list_methods;
use crate::values::set::value::set_methods;
use crate::values::string::str_type::str_methods;
use crate::values::FrozenRef;
use crate::values::FrozenValueTyped;
use crate::values::Value;

/// Method and a `Methods` container which declares it.
#[derive(Clone, Copy, Dupe)]
pub(crate) struct KnownMethod {
    /// An object where the method is defined.
    pub(crate) type_methods: &'static Methods,
    /// The method.
    method: FrozenValueTyped<'static, NativeMethod>,
    /// Copied here from `method` to faster invocation (one fewer deref).
    imp: FrozenRef<'static, dyn NativeMeth>,
}

impl KnownMethod {
    #[inline]
    pub(crate) fn to_value<'v>(&self) -> Value<'v> {
        self.method.to_value()
    }

    #[inline]
    pub(crate) fn invoke_method<'v>(
        &self,
        this: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        self.imp.invoke(eval, this, args)
    }
}

/// Some of stdlib methods.
struct KnownMethods {
    methods: HashMap<&'static str, KnownMethod>,
}

impl KnownMethods {
    fn build() -> KnownMethods {
        let mut methods = HashMap::new();

        fn add_methods(
            methods: &mut HashMap<&'static str, KnownMethod>,
            type_methods: Option<&'static Methods>,
        ) {
            let type_methods = type_methods.unwrap();
            let mut has_at_least_one_method = false;
            for (name, member) in type_methods.members() {
                // Take methods, ignore attributes.
                if let Some(method) = FrozenValueTyped::new(member) {
                    // First wins, e. g. `list.clear` is hit, and `dict.clear` is miss.
                    methods.entry(name).or_insert(KnownMethod {
                        type_methods,
                        method,
                        imp: method.as_frozen_ref().map(|m| &*m.function),
                    });
                    has_at_least_one_method = true;
                }
            }
            // Sanity check.
            assert!(has_at_least_one_method);
        }

        // We don't need to add all the methods, only the most common ones. This is fine.
        add_methods(&mut methods, list_methods());
        add_methods(&mut methods, dict_methods());
        add_methods(&mut methods, set_methods());
        add_methods(&mut methods, str_methods());

        KnownMethods { methods }
    }
}

/// Get stdlib method by name, or `None` if method is not found
/// or method is not very common. Return arbitrary method if more than one
/// method is found (e. g. `list.clear` and `dict.clear`).
pub(crate) fn get_known_method(name: &str) -> Option<KnownMethod> {
    static ANY_METHODS: Lazy<KnownMethods> = Lazy::new(KnownMethods::build);
    ANY_METHODS.methods.get(name).copied()
}
