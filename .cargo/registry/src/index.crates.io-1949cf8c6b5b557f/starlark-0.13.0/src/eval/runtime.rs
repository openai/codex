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

pub(crate) mod arguments;
pub(crate) mod before_stmt;
pub(crate) mod cheap_call_stack;
pub(crate) mod evaluator;
pub(crate) mod file_loader;
pub(crate) mod frame_span;
pub(crate) mod frozen_file_span;
pub(crate) mod inlined_frame;
pub(crate) mod params;
pub(crate) mod profile;
pub(crate) mod rust_loc;
pub(crate) mod slots;
pub(crate) mod small_duration;
pub(crate) mod visit_span;
