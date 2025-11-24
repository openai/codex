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

use std::marker::PhantomData;

use allocative::Allocative;
use dupe::Dupe;

use crate::values::layout::pointer::RawPointer;
use crate::values::Value;

/// An opaque value representing the identity of a given Value. Two values have the same identity
/// if and only if [`Value::ptr_eq`] would return [`true`] on them.
#[derive(Eq, PartialEq, Copy, Clone, Dupe, Hash, Debug, Allocative)]
pub struct ValueIdentity<'v> {
    identity: RawPointer,
    phantom: PhantomData<&'v ()>,
}

impl<'v> ValueIdentity<'v> {
    #[inline]
    pub(crate) fn new(value: Value<'v>) -> ValueIdentity<'v> {
        ValueIdentity {
            identity: value.ptr_value(),
            phantom: PhantomData,
        }
    }
}
