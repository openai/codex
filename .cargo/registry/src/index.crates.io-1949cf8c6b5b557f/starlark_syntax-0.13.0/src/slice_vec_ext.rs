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

/// Optimised collect iterator into Vec, which might be a Result.
///
/// If we do a standard .collect() on the iterator it will never have a good size hint,
/// as the lower bound will always be zero, so might reallocate several times.
/// We know the Vec will either be thrown away, or exactly `len`, so aim if we do allocate,
/// make sure it is at `len`. However, if the first element throws an error, we don't need
/// to allocate at all, so special case that.
fn collect_result<T, E>(mut it: impl ExactSizeIterator<Item = Result<T, E>>) -> Result<Vec<T>, E> {
    match it.next() {
        None => Ok(Vec::new()),
        Some(Err(e)) => Err(e),
        Some(Ok(x)) => {
            // +1 for the element we have already consumed
            let mut res = Vec::with_capacity(it.len() + 1);
            res.push(x);
            for x in it {
                res.push(x?);
            }
            Ok(res)
        }
    }
}

/// Extension traits on slices/[`Vec`].
pub trait SliceExt {
    type Item;

    /// A shorthand for `iter().map(f).collect::<Vec<_>>()`. For example:
    fn map<'a, B, F>(&'a self, f: F) -> Vec<B>
    where
        F: FnMut(&'a Self::Item) -> B;

    /// A shorthand for `iter().map(f).collect::<Result<Vec<_>, _>>()`. For example:
    fn try_map<'a, B, E, F>(&'a self, f: F) -> Result<Vec<B>, E>
    where
        F: FnMut(&'a Self::Item) -> Result<B, E>;
}

impl<T> SliceExt for [T] {
    type Item = T;

    fn map<'a, B, F>(&'a self, f: F) -> Vec<B>
    where
        F: FnMut(&'a Self::Item) -> B,
    {
        self.iter().map(f).collect()
    }

    fn try_map<'a, B, E, F>(&'a self, f: F) -> Result<Vec<B>, E>
    where
        F: FnMut(&'a Self::Item) -> Result<B, E>,
    {
        collect_result(self.iter().map(f))
    }
}

/// Extension traits on [`Vec`].
pub trait VecExt {
    type Item;

    /// A shorthand for `into_iter().map(f).collect::<Vec<_>>()`. For example:
    fn into_map<B, F>(self, f: F) -> Vec<B>
    where
        F: FnMut(Self::Item) -> B;

    /// A shorthand for `into_iter().map(f).collect::<Result<Vec<_>, _>>()`. For example:
    fn into_try_map<B, E, F>(self, f: F) -> Result<Vec<B>, E>
    where
        F: FnMut(Self::Item) -> Result<B, E>;
}

impl<T> VecExt for Vec<T> {
    type Item = T;

    fn into_map<B, F>(self, f: F) -> Vec<B>
    where
        F: FnMut(Self::Item) -> B,
    {
        self.into_iter().map(f).collect()
    }

    fn into_try_map<B, E, F>(self, f: F) -> Result<Vec<B>, E>
    where
        F: FnMut(Self::Item) -> Result<B, E>,
    {
        collect_result(self.into_iter().map(f))
    }
}
