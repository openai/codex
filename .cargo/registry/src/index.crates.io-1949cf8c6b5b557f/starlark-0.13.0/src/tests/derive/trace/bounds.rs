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

#![allow(dead_code)]

use crate as starlark;
use crate::values::Trace;

#[derive(Trace)]
#[trace(bound = "A: Trace<'v>, B: 'static")]
struct TestTraceWithBounds<A, B> {
    a: A,
    #[trace(static)]
    b: B,
}

struct NotTrace;

fn assert_trace<'v, T: Trace<'v>>() {}

fn test() {
    assert_trace::<TestTraceWithBounds<String, NotTrace>>();
}
