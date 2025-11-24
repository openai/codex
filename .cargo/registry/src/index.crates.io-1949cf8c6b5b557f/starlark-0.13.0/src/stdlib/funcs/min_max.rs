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

use std::cmp::Ordering;

use starlark_derive::starlark_module;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::eval::Evaluator;
use crate::values::tuple::UnpackTuple;
use crate::values::Value;

fn min_max_iter<'v>(
    mut it: impl Iterator<Item = Value<'v>>,
    key: Option<Value<'v>>,
    eval: &mut Evaluator<'v, '_, '_>,
    // Select min on true, max on false.
    min: bool,
) -> crate::Result<Value<'v>> {
    let mut max = match it.next() {
        Some(x) => x,
        None => {
            return Err(anyhow::anyhow!(
                "Argument is an empty iterable, max() expect a non empty iterable"
            )
            .into());
        }
    };
    let update_max_ordering = if min {
        Ordering::Greater
    } else {
        Ordering::Less
    };
    match key {
        None => {
            for i in it {
                if max.compare(i)? == update_max_ordering {
                    max = i;
                }
            }
        }
        Some(key) => {
            let mut cached = key.invoke_pos(&[max], eval)?;
            for i in it {
                let keyi = key.invoke_pos(&[i], eval)?;
                if cached.compare(keyi)? == update_max_ordering {
                    max = i;
                    cached = keyi;
                }
            }
        }
    };
    Ok(max)
}

/// Common implementation of `min` and `max`.
fn min_max<'v>(
    mut args: UnpackTuple<Value<'v>>,
    key: Option<Value<'v>>,
    eval: &mut Evaluator<'v, '_, '_>,
    // Select min on true, max on false.
    min: bool,
) -> crate::Result<Value<'v>> {
    if args.items.len() == 1 {
        let it = args.items.swap_remove(0).iterate(eval.heap())?;
        min_max_iter(it, key, eval, min)
    } else {
        min_max_iter(args.items.into_iter(), key, eval, min)
    }
}

#[starlark_module]
pub(crate) fn register_min_max(globals: &mut GlobalsBuilder) {
    /// [max](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#max
    /// ): returns the maximum of a sequence.
    ///
    /// `max(x)` returns the greatest element in the iterable sequence x.
    ///
    /// It is an error if any element does not support ordered comparison,
    /// or if the sequence is empty.
    ///
    /// The optional named parameter `key` specifies a function to be applied
    /// to each element prior to comparison.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// max([3, 1, 4, 1, 5, 9])               == 9
    /// max("two", "three", "four")           == "two"    # the lexicographically greatest
    /// max("two", "three", "four", key=len)  == "three"  # the longest
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn max<'v>(
        #[starlark(args)] args: UnpackTuple<Value<'v>>,
        key: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        min_max(args, key, eval, false)
    }

    /// [min](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#min
    /// ): returns the minimum of a sequence.
    ///
    /// `min(x)` returns the least element in the iterable sequence x.
    ///
    /// It is an error if any element does not support ordered comparison,
    /// or if the sequence is empty.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// min([3, 1, 4, 1, 5, 9])                 == 1
    /// min("two", "three", "four")             == "four"  # the lexicographically least
    /// min("two", "three", "four", key=len)    == "two"   # the shortest
    /// # "#);
    /// ```
    #[starlark(speculative_exec_safe)]
    fn min<'v>(
        #[starlark(args)] args: UnpackTuple<Value<'v>>,
        key: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        min_max(args, key, eval, true)
    }
}
