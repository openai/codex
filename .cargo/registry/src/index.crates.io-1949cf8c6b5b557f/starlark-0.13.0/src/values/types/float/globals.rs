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

use either::Either;
use starlark_derive::starlark_module;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::values::float::StarlarkFloat;
use crate::values::string::repr::string_repr;
use crate::values::types::num::value::NumRef;

#[starlark_module]
pub(crate) fn register_float(globals: &mut GlobalsBuilder) {
    /// [float](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#float
    /// ): interprets its argument as a floating-point number.
    ///
    /// If x is a `float`, the result is x.
    /// if x is an `int`, the result is the nearest floating point value to x.
    /// If x is a string, the string is interpreted as a floating-point literal.
    /// With no arguments, `float()` returns `0.0`.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// float() == 0.0
    /// float(1) == 1.0
    /// float('1') == 1.0
    /// float('1.0') == 1.0
    /// float('.25') == 0.25
    /// float('1e2') == 100.0
    /// float(False) == 0.0
    /// float(True) == 1.0
    /// # "#);
    /// # starlark::assert::fail(r#"
    /// float("hello")   # error: not a valid number
    /// # "#, "not a valid number");
    /// # starlark::assert::fail(r#"
    /// float([])   # error
    /// # "#, "doesn't match, expected");
    /// ```
    #[starlark(as_type = StarlarkFloat, speculative_exec_safe)]
    fn float(
        #[starlark(require = pos)] a: Option<Either<Either<NumRef, bool>, &str>>,
    ) -> anyhow::Result<f64> {
        if a.is_none() {
            return Ok(0.0);
        }
        let a = a.unwrap();
        match a {
            Either::Left(Either::Left(f)) => Ok(f.as_float()),
            Either::Left(Either::Right(b)) => Ok(if b { 1.0 } else { 0.0 }),
            Either::Right(s) => {
                match s.parse::<f64>() {
                    Ok(f) => {
                        if f.is_infinite() && !s.to_lowercase().contains("inf") {
                            // if a resulting float is infinite but the parsed string is not explicitly infinity then we should fail with an error
                            Err(anyhow::anyhow!(
                                "float() floating-point number too large: {}",
                                s
                            ))
                        } else {
                            Ok(f)
                        }
                    }
                    Err(x) => {
                        let mut repr = String::new();
                        string_repr(s, &mut repr);
                        Err(anyhow::anyhow!("{} is not a valid number: {}", repr, x,))
                    }
                }
            }
        }
    }
}
