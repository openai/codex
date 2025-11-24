/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

//! Manual implementations of `Allocative` for various types.

mod anyhow;
mod bumpalo;
pub(crate) mod common;
mod compact_str;
mod dashmap;
mod either;
mod futures;
pub(crate) mod hashbrown;
pub(crate) mod hashbrown_util;
mod indexmap;
mod lock_api;
mod num_bigint;
mod once_cell;
mod parking_lot;
mod prost_types;
mod relative_path;
mod serde_json;
mod slab;
mod smallvec;
mod sorted_vector_map;
mod std;
mod tokio;
mod triomphe;
