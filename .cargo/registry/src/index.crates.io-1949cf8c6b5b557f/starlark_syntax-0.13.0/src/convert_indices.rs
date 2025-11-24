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

fn bound(val: i32, limit: i32) -> usize {
    if val <= 0 {
        0
    } else if val >= limit {
        limit as usize
    } else {
        val as usize
    }
}

#[inline]
pub fn convert_indices(len: i32, start: Option<i32>, end: Option<i32>) -> (usize, usize) {
    let start = start.unwrap_or(0);
    let end = end.unwrap_or(len);
    let end = if end < 0 { end + len } else { end };
    let start = if start < 0 { start + len } else { start };
    (bound(start, len), bound(end, len))
}

#[inline]
pub fn convert_index(len: i32, start: i32) -> usize {
    let start = if start < 0 { start + len } else { start };
    bound(start, len)
}
