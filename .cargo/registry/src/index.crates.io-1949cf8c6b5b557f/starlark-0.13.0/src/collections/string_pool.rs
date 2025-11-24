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

//! Reuse string allocation.

/// Pool of strings.
#[derive(Default, Debug)]
pub(crate) struct StringPool {
    /// Empty strings with (typically) non-zero capacity.
    strings: Vec<String>,
}

impl StringPool {
    /// Fetch a string from the pool or create an empty one.
    ///
    /// It is OK to not return a string to the pool.
    pub(crate) fn alloc(&mut self) -> String {
        let string = self.strings.pop().unwrap_or_default();
        debug_assert!(string.is_empty());
        string
    }

    /// Return the string back to the pool.
    ///
    /// Only strings previously allocated with this pool should be returned,
    /// otherwise pool may grow too much.
    pub(crate) fn release(&mut self, mut s: String) {
        s.clear();
        self.strings.push(s);
    }
}
