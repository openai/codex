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

/// Depth of the loop. For example,
///
/// ```text
/// def foo():
///   for i in range(10): # depth 0
///     for j in range(20): # depth 1
///       pass
/// ```
#[derive(
    Default,
    Debug,
    Copy,
    Clone,
    Dupe,
    derive_more::Display,
    Eq,
    PartialEq,
    Ord,
    PartialOrd
)]
pub(crate) struct LoopDepth(pub(crate) u32);
