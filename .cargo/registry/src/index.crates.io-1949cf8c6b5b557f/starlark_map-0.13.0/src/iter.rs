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

// We define a lot of iterators on top of other iterators
// so define a helper macro for that
macro_rules! def_iter {
    () => {
        #[inline]
        fn next(&mut self) -> Option<Self::Item> {
            self.iter.next().map(Self::map)
        }

        #[inline]
        fn nth(&mut self, n: usize) -> Option<Self::Item> {
            self.iter.nth(n).map(Self::map)
        }

        #[inline]
        fn last(self) -> Option<Self::Item> {
            self.iter.last().map(Self::map)
        }

        #[inline]
        fn size_hint(&self) -> (usize, Option<usize>) {
            self.iter.size_hint()
        }

        #[inline]
        fn count(self) -> usize {
            self.iter.count()
        }

        #[inline]
        fn collect<C>(self) -> C
        where
            C: std::iter::FromIterator<Self::Item>,
        {
            self.iter.map(Self::map).collect()
        }
    };
}

macro_rules! def_double_ended_iter {
    () => {
        #[inline]
        fn next_back(&mut self) -> Option<Self::Item> {
            self.iter.next_back().map(Self::map)
        }
    };
}

pub(crate) use def_double_ended_iter;
pub(crate) use def_iter;
