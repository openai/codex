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
use crate::eval::Arguments;
use crate::values::namespace::typing::TyNamespaceFunction;
use crate::values::namespace::value::MaybeDocHiddenValue;
use crate::values::namespace::FrozenNamespace;
use crate::values::namespace::Namespace;
use crate::values::Heap;

#[starlark_module]
pub fn register_namespace(builder: &mut GlobalsBuilder) {
    #[starlark(
        ty_custom_function = TyNamespaceFunction,
        as_type = FrozenNamespace,
    )]
    fn namespace<'v>(args: &Arguments<'v, '_>, heap: &'v Heap) -> starlark::Result<Namespace<'v>> {
        args.no_positional_args(heap)?;

        Ok(Namespace::new(
            args.names_map()?
                .into_iter()
                .map(|(k, v)| {
                    (
                        k,
                        MaybeDocHiddenValue {
                            value: v,
                            doc_hidden: false,
                            phantom: Default::default(),
                        },
                    )
                })
                .collect(),
        ))
    }
}
