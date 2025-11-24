/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use std::mem;

/// Approximate allocated memory for hashbrown `RawTable`.
#[allow(dead_code)]
pub(crate) fn raw_table_alloc_size_for_len<T>(len: usize) -> usize {
    let buckets = if len == 0 {
        0
    } else if len < 4 {
        4
    } else {
        len.next_power_of_two()
    };
    let size_of_control_byte = 1;
    (mem::size_of::<T>() + size_of_control_byte) * buckets
}
