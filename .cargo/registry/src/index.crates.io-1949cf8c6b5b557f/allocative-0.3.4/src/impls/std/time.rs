/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use crate::allocative_trait::Allocative;
use crate::visitor::Visitor;

impl Allocative for Instant {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.enter_self_sized::<Self>().exit();
    }
}

impl Allocative for SystemTime {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.enter_self_sized::<Self>().exit();
    }
}

impl Allocative for Duration {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.enter_self_sized::<Self>().exit();
    }
}
