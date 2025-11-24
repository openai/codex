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

//! Allocator for the Starlark heap.
//!
//! The idea of the allocator is this: most allocation should be as fast as pointer increment.
//! To achieve that, memory is allocated in chunks, and these pointer increments happen
//! inside the chunk. When the chunk is full, a new chunk is allocated.
//!
//! When heap construction is finished, we need to release the memory,
//! Last chunk is half full. So heaps keeps half of the chunk,
//! and the other half is shared with the following heap.

pub(crate) mod allocator;
pub(crate) mod chain;
pub(crate) mod chunk;
pub(crate) mod chunk_part;
pub(crate) mod per_thread;
