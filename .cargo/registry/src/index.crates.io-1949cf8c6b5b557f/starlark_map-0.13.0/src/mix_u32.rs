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

// This is important when used with hashbrown, HashMap and IndexMap,
// which uses the upper 7 bits of the hash for a tag, then compares 16 tags in
// parallel with a SIMD instruction. Without the mix, typical low-valued u32 ids
// would all have tag 0.
#[inline(always)]
pub(crate) fn mix_u32(n: u32) -> u64 {
    (n as u64).wrapping_mul(0x9e3779b97f4a7c15)
}
