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

use allocative::Allocative;
use dupe::Dupe;
use starlark_map::small_set::SmallSet;

use crate::util::arc_str::ArcStr;

/// Map strings to integers 0, 1, 2, ...
#[derive(Default, Clone, Allocative)]
pub(crate) struct StringIndex {
    strings: SmallSet<ArcStr>,
}

#[derive(Copy, Clone, Dupe, Debug, Eq, PartialEq, Hash, Allocative)]
pub(crate) struct StringId(
    /// Index in strings index.
    pub(crate) usize,
);

impl StringIndex {
    pub(crate) fn index(&mut self, s: &str) -> StringId {
        if let Some(index) = self.strings.get_index_of(s) {
            return StringId(index);
        }

        let inserted = self.strings.insert(ArcStr::from(s));
        assert!(inserted);
        StringId(self.strings.len() - 1)
    }

    pub(crate) fn get(&self, id: StringId) -> &ArcStr {
        self.strings.get_index(id.0).expect("invalid string id")
    }
}
