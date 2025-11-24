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

use std::sync::atomic;
use std::sync::atomic::AtomicU64;

use allocative::Allocative;
use dupe::Dupe;
use starlark_derive::Freeze;

use crate as starlark;
use crate::values::FreezeResult;

/// Globally unique identifier for a type, like record type or enum type.
#[derive(
    Debug, Copy, Clone, Dupe, Hash, Eq, PartialEq, Ord, PartialOrd, Allocative, Freeze
)]
pub struct TypeInstanceId(u64);

impl TypeInstanceId {
    /// Generate a new unique identifier.
    pub fn gen() -> TypeInstanceId {
        static LAST_ID: AtomicU64 = AtomicU64::new(0);
        TypeInstanceId(LAST_ID.fetch_add(1, atomic::Ordering::SeqCst) + 1)
    }
}
