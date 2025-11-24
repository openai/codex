/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use std::mem::ManuallyDrop;

use crate::Allocative;
use crate::Key;
use crate::Visitor;

impl<T: Allocative + ?Sized> Allocative for ManuallyDrop<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self(self);
        visitor.visit_field::<T>(Key::new("inner"), self);
        visitor.exit();
    }
}
