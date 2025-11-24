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

use crate::eval::Arguments;
use crate::eval::ParametersSpec;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueError;

/// Collect `N` arguments.
///
/// This function is called by generated code.
#[inline]
pub fn parse_signature<'v, const N: usize>(
    parser: &ParametersSpec<FrozenValue>,
    args: &Arguments<'v, '_>,
    heap: &'v Heap,
) -> crate::Result<[Option<Value<'v>>; N]> {
    parser.collect_into(args, heap)
}

/// Parse positional-only arguments, required and optional.
#[inline(always)]
pub fn parse_positional<'v, const R: usize, const O: usize>(
    args: &Arguments<'v, '_>,
    heap: &'v Heap,
) -> crate::Result<([Value<'v>; R], [Option<Value<'v>>; O])> {
    args.no_named_args()?;
    args.optional(heap)
}

#[inline(always)]
pub fn parse_positional_kwargs_alloc<'v, 'a, const R: usize, const O: usize>(
    args: &'a Arguments<'v, 'a>,
    heap: &'v Heap,
) -> crate::Result<([Value<'v>; R], [Option<Value<'v>>; O], Value<'v>)> {
    let (required, optional) = args.optional(heap)?;
    let kwargs = args.names_map()?;
    let kwargs = heap.alloc(kwargs);
    Ok((required, optional, kwargs))
}

/// Utility for checking a `this` parameter matches what you expect.
#[inline]
pub fn check_this<'v, T: UnpackValue<'v>>(this: Value<'v>) -> anyhow::Result<T> {
    T::unpack_named_param(this, "this")
}

/// Utility for checking a required parameter matches what you expect.
#[inline]
pub fn check_required<'v, T: UnpackValue<'v>>(
    name: &str,
    x: Option<Value<'v>>,
) -> anyhow::Result<T> {
    let x = x.ok_or_else(|| ValueError::MissingRequired(name.to_owned()))?;
    T::unpack_named_param(x, name)
}

/// Utility for checking an optional parameter matches what you expect.
#[inline]
pub fn check_optional<'v, T: UnpackValue<'v>>(
    name: &str,
    x: Option<Value<'v>>,
) -> anyhow::Result<Option<T>> {
    match x {
        None => Ok(None),
        Some(x) => Ok(Some(T::unpack_named_param(x, name)?)),
    }
}

#[inline]
pub fn check_defaulted<'v, T: UnpackValue<'v>>(
    name: &str,
    x: Option<Value<'v>>,
    default: impl FnOnce() -> T,
) -> anyhow::Result<T> {
    Ok(check_optional(name, x)?.unwrap_or_else(default))
}

/// We already know the parameter is set, so we just unpack it.
#[inline]
pub fn check_unpack<'v, T: UnpackValue<'v>>(name: &str, x: Value<'v>) -> anyhow::Result<T> {
    T::unpack_named_param(x, name)
}
