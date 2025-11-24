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

use starlark_derive::starlark_module;
use starlark_map::small_map::SmallMap;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::eval::Arguments;
use crate::values::dict::value::FrozenDict;
use crate::values::dict::Dict;
use crate::values::dict::DictRef;
use crate::values::function::SpecialBuiltinFunction;
use crate::values::Heap;
use crate::values::Value;

fn unpack_pair<'v>(pair: Value<'v>, heap: &'v Heap) -> crate::Result<(Value<'v>, Value<'v>)> {
    let mut it = pair.iterate(heap)?;
    if let Some(first) = it.next() {
        if let Some(second) = it.next() {
            if it.next().is_none() {
                return Ok((first, second));
            }
        }
    }
    Err(anyhow::anyhow!(
        "Found a non-pair element in the positional argument of dict(): {}",
        pair.to_repr(),
    )
    .into())
}

#[starlark_module]
pub(crate) fn register_dict(globals: &mut GlobalsBuilder) {
    /// [dict](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#dict
    /// ): creates a dictionary.
    ///
    /// `dict` creates a dictionary. It accepts up to one positional argument,
    /// which is interpreted as an iterable of two-element sequences
    /// (pairs), each specifying a key/value pair in the
    /// resulting dictionary.
    ///
    /// `dict` also accepts any number of keyword arguments, each of which
    /// specifies a key/value pair in the resulting dictionary; each keyword
    /// is treated as a string.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// dict() == {}
    /// dict(**{'a': 1}) == {'a': 1}
    /// dict({'a': 1}) == {'a': 1}
    /// dict([(1, 2), (3, 4)]) == {1: 2, 3: 4}
    /// dict([(1, 2), ['a', 'b']]) == {1: 2, 'a': 'b'}
    /// dict(one=1, two=2) == {'one': 1, 'two': 2}
    /// dict([(1, 2)], x=3) == {1: 2, 'x': 3}
    /// dict([('x', 2)], x=3) == {'x': 3}
    /// # "#);
    /// # starlark::assert::is_true(r#"
    /// x = {'a': 1}
    /// y = dict([('x', 2)], **x)
    /// x == {'a': 1} and y == {'x': 2, 'a': 1}
    /// # "#);
    /// ```
    #[starlark(
    as_type = FrozenDict,
    speculative_exec_safe,
    special_builtin_function = SpecialBuiltinFunction::Dict,
    )]
    fn dict<'v>(args: &Arguments<'v, '_>, heap: &'v Heap) -> starlark::Result<Dict<'v>> {
        // Dict is super hot, and has a slightly odd signature, so we can do a bunch of special cases on it.
        // In particular, we don't generate the kwargs if there are no positional arguments.
        // Therefore we make it take the raw Arguments.
        // It might have one positional argument, which could be a dict or an array of pairs.
        // It might have named/kwargs arguments, which we copy over (afterwards).

        let pos = args.optional1(heap)?;
        let kwargs = args.names()?;

        match pos {
            None => Ok(kwargs),
            Some(pos) => {
                let mut result: Dict = match DictRef::from_value(pos) {
                    Some(pos) => {
                        let mut result = (*pos).clone();
                        result.reserve(kwargs.len());
                        result
                    }
                    None => {
                        let it = pos.iterate(heap)?;
                        let mut result = SmallMap::with_capacity(it.size_hint().0 + kwargs.len());
                        for el in it {
                            let (k, v) = unpack_pair(el, heap)?;
                            let k = k.get_hashed()?;
                            result.insert_hashed(k, v);
                        }
                        Dict::new(result)
                    }
                };
                for (k, v) in kwargs.iter_hashed() {
                    result.insert_hashed(k, v);
                }
                Ok(result)
            }
        }
    }
}
