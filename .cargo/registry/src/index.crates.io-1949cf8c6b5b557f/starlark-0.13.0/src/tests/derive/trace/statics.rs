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

use std::fmt::Display;

use starlark_derive::Trace;

use crate as starlark;
use crate::values::Value;

#[allow(dead_code)] // Just check it compiles.
#[derive(Trace)]
struct TraceWithStatic<'v> {
    actual_value: Value<'v>,
    // This field doesn't have a Trace trait, but should be ignored
    // because it looks like it is static
    ignored_because_static: StaticType<'static, String>,
    ignored_because_static_in_dyn: Box<dyn Display>,
    ignored_because_static_in_dyn_with_static_bound: Box<dyn Display + 'static>,
    #[trace(static)] // This is no-op, because it is inferred automatically.
    explicit_static: String,
}

#[allow(dead_code)]
struct StaticType<'a, T> {
    inner: &'a T,
}
