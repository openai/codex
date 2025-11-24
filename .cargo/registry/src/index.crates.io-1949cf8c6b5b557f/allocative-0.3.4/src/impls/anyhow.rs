/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(feature = "anyhow")]

use crate::allocative_trait::Allocative;
use crate::visitor::Visitor;

impl Allocative for anyhow::Error {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        // Don't know what the size is.
        let _ = visitor;
    }
}
