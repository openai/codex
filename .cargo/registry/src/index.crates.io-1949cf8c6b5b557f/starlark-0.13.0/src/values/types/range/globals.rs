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

use std::num::NonZeroI32;

use starlark_derive::starlark_module;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::values::range::Range;

#[starlark_module]
pub(crate) fn register_range(globals: &mut GlobalsBuilder) {
    /// [range](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#range
    /// ): return a range of integers
    ///
    /// `range` returns a tuple of integers defined by the specified interval
    /// and stride.
    ///
    /// ```python
    /// range(stop)                             # equivalent to range(0, stop)
    /// range(start, stop)                      # equivalent to range(start, stop, 1)
    /// range(start, stop, step)
    /// ```
    ///
    /// `range` requires between one and three integer arguments.
    /// With one argument, `range(stop)` returns the ascending sequence of
    /// non-negative integers less than `stop`.
    /// With two arguments, `range(start, stop)` returns only integers not less
    /// than `start`.
    ///
    /// With three arguments, `range(start, stop, step)` returns integers
    /// formed by successively adding `step` to `start` until the value meets or
    /// passes `stop`. A call to `range` fails if the value of `step` is
    /// zero.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// list(range(10))                         == [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
    /// list(range(3, 10))                      == [3, 4, 5, 6, 7, 8, 9]
    /// list(range(3, 10, 2))                   == [3, 5, 7, 9]
    /// list(range(10, 3, -2))                  == [10, 8, 6, 4]
    /// # "#);
    /// ```
    #[starlark(as_type = Range, speculative_exec_safe)]
    fn range(
        #[starlark(require = pos)] a1: i32,
        #[starlark(require = pos)] a2: Option<i32>,
        #[starlark(require = pos, default = 1)] step: i32,
    ) -> anyhow::Result<Range> {
        let start = match a2 {
            None => 0,
            Some(_) => a1,
        };
        let stop = a2.unwrap_or(a1);
        let step = match NonZeroI32::new(step) {
            Some(step) => step,
            None => {
                return Err(anyhow::anyhow!(
                    "Third argument of range (step) cannot be zero"
                ));
            }
        };
        Ok(Range::new(start, stop, step))
    }
}
