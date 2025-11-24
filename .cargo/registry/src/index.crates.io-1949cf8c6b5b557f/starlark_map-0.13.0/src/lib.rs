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

//! Ordered map optimized for starlark-rust use cases.

// Hints we disagree with
#![allow(clippy::missing_safety_doc)]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![cfg_attr(rust_nightly, feature(core_intrinsics))]
#![cfg_attr(rust_nightly, feature(portable_simd))]
#![cfg_attr(rust_nightly, feature(cfg_version))]
#![cfg_attr(rust_nightly, allow(internal_features))]

mod hash_value;
mod hashed;
mod hasher;
mod iter;
mod mix_u32;
pub mod ordered_map;
pub mod ordered_set;
pub mod small_map;
pub mod small_set;
pub mod sorted_map;
pub mod sorted_set;
pub mod sorted_vec;
pub(crate) mod sorting;
pub mod unordered_map;
pub mod unordered_set;
pub mod vec2;
pub(crate) mod vec_map;

pub use equivalent::Equivalent;
pub use hash_value::StarlarkHashValue;
pub use hashed::Hashed;
pub use hasher::StarlarkHasher;
pub use hasher::StarlarkHasherBuilder;
