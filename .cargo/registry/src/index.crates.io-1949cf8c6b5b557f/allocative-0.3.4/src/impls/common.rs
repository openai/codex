/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use crate::key::Key;

/// "Field" describing allocated but unused capacity (e.g. in `Vec`).
pub(crate) const UNUSED_CAPACITY_NAME: Key = Key::new("unused_capacity");

/// "Field" describing all capacity (e.g. in `Vec`).
pub(crate) const CAPACITY_NAME: Key = Key::new("capacity");

/// Generic pointee field in types like `Box`.
pub(crate) const PTR_NAME: Key = Key::new("ptr");

/// Generic name for useful data (e.g. in `Vec`).
pub(crate) const DATA_NAME: Key = Key::new("data");

pub(crate) const KEY_NAME: Key = Key::new("key");
pub(crate) const VALUE_NAME: Key = Key::new("value");
