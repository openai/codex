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
use crate::assert::Assert;
use crate::environment::GlobalsBuilder;
use crate::values::Heap;
use crate::values::StringValue;

#[starlark_module]
fn functions(builder: &mut GlobalsBuilder) {
    fn non_standard_heap_name<'v>(
        heap: &str,
        starlark_heap: &'v Heap,
    ) -> anyhow::Result<StringValue<'v>> {
        Ok(starlark_heap.alloc_str_concat(heap, "!"))
    }
}

#[test]
fn test_non_standard_param_names() {
    let mut a = Assert::new();
    a.globals_add(functions);
    a.eq("'x!'", "non_standard_heap_name('x')");
}
