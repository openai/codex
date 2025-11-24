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

use either::Either;
use starlark_derive::starlark_module;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::values::int::int_or_big::StarlarkInt;
use crate::values::int::pointer_i32::PointerI32;
use crate::values::types::num::value::NumRef;
use crate::values::Heap;
use crate::values::ValueOf;
use crate::values::ValueOfUnchecked;

#[starlark_module]
pub(crate) fn register_int(globals: &mut GlobalsBuilder) {
    /// [int](
    /// https://github.com/bazelbuild/starlark/blob/master/spec.md#int
    /// ): convert a value to integer.
    ///
    /// `int(x[, base])` interprets its argument as an integer.
    ///
    /// If x is an `int`, the result is x.
    /// If x is a `float`, the result is the integer value nearest to x,
    /// truncating towards zero; it is an error if x is not finite (`NaN`,
    /// `+Inf`, `-Inf`).
    /// If x is a `bool`, the result is 0 for `False` or 1 for `True`.
    ///
    /// If x is a string, it is interpreted like a string literal;
    /// an optional base prefix (`0`, `0b`, `0B`, `0x`, `0X`) determines which
    /// base to use. The string may specify an arbitrarily large integer,
    /// whereas true integer literals are restricted to 64 bits.
    /// If a non-zero `base` argument is provided, the string is interpreted
    /// in that base and no base prefix is permitted; the base argument may
    /// specified by name.
    ///
    /// `int()` with no arguments returns 0.
    ///
    /// ```
    /// # starlark::assert::all_true(r#"
    /// int() == 0
    /// int(1) == 1
    /// int(False) == 0
    /// int(True) == 1
    /// int('1') == 1
    /// int('16') == 16
    /// int('16', 10) == 16
    /// int('16', 8) == 14
    /// int('16', 16) == 22
    /// int(0.0) == 0
    /// int(3.14) == 3
    /// int(-12345.6789) == -12345
    /// int(2e9) == 2000000000
    /// # "#);
    /// # starlark::assert::fail(r#"
    /// int("hello")   # error: Cannot parse
    /// # "#, "Cannot parse");
    /// # starlark::assert::fail(r#"
    /// int(float("nan"))   # error: cannot be represented as exact integer
    /// # "#, "cannot be represented as exact integer");
    /// # starlark::assert::fail(r#"
    /// int(float("inf"))   # error: cannot be represented as exact integer
    /// # "#, "cannot be represented as exact integer");
    /// ```
    #[starlark(as_type = PointerI32, speculative_exec_safe)]
    fn int<'v>(
        #[starlark(require = pos)] a: Option<
            ValueOf<'v, Either<Either<NumRef<'v>, bool>, &'v str>>,
        >,
        base: Option<i32>,
        heap: &'v Heap,
    ) -> starlark::Result<ValueOfUnchecked<'v, StarlarkInt>> {
        let Some(a) = a else {
            return Ok(ValueOfUnchecked::new(heap.alloc(0)));
        };
        let num_or_bool = match a.typed {
            Either::Left(num_or_bool) => num_or_bool,
            Either::Right(s) => {
                let base = base.unwrap_or(0);
                if base == 1 || base < 0 || base > 36 {
                    return Err(anyhow::anyhow!(
                        "{} is not a valid base, int() base must be >= 2 and <= 36",
                        base
                    )
                    .into());
                }
                let (negate, s) = {
                    match s.chars().next() {
                        Some('+') => (false, s.get(1..).unwrap()),
                        Some('-') => (true, s.get(1..).unwrap()),
                        _ => (false, s),
                    }
                };
                let base = if base == 0 {
                    match s.get(0..2) {
                        Some("0b") | Some("0B") => 2,
                        Some("0o") | Some("0O") => 8,
                        Some("0x") | Some("0X") => 16,
                        _ => 10,
                    }
                } else {
                    base as u32
                };
                let s = match base {
                    16 => {
                        if s.starts_with("0x") || s.starts_with("0X") {
                            s.get(2..).unwrap()
                        } else {
                            s
                        }
                    }
                    8 => {
                        if s.starts_with("0o") || s.starts_with("0O") {
                            s.get(2..).unwrap()
                        } else {
                            s
                        }
                    }
                    2 => {
                        if s.starts_with("0b") || s.starts_with("0B") {
                            s.get(2..).unwrap()
                        } else {
                            s
                        }
                    }
                    _ => s,
                };
                // We already handled the sign above, so we are not trying to parse another sign.
                if s.starts_with('-') || s.starts_with('+') {
                    return Err(anyhow::anyhow!("Cannot parse `{}` as an integer", s,).into());
                }

                let x = StarlarkInt::from_str_radix(s, base)?;
                let x = if negate { -x } else { x };
                return Ok(ValueOfUnchecked::new(heap.alloc(x)));
            }
        };

        if let Some(base) = base {
            return Err(anyhow::anyhow!(
                "int() cannot convert non-string with explicit base '{}'",
                base
            )
            .into());
        }

        match num_or_bool {
            Either::Left(NumRef::Int(_)) => Ok(ValueOfUnchecked::new(a.value)),
            Either::Left(NumRef::Float(f)) => Ok(ValueOfUnchecked::new(
                heap.alloc(StarlarkInt::from_f64_exact(f.0.trunc())?),
            )),
            Either::Right(b) => Ok(ValueOfUnchecked::new(heap.alloc(b as i32))),
        }
    }
}
