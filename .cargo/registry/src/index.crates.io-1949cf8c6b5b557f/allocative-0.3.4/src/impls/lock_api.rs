/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(feature = "parking_lot")]

use parking_lot::lock_api::Mutex;
use parking_lot::lock_api::RawMutex;
use parking_lot::lock_api::RawRwLock;
use parking_lot::lock_api::RwLock;

use crate::allocative_trait::Allocative;
use crate::key::Key;
use crate::visitor::Visitor;

impl<R: RawMutex + 'static, T: Allocative> Allocative for Mutex<R, T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        if let Some(data) = self.try_lock() {
            visitor.visit_field(Key::new("data"), &*data);
        }
        visitor.exit();
    }
}

impl<R: RawRwLock + 'static, T: Allocative> Allocative for RwLock<R, T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        if let Some(data) = self.try_read() {
            visitor.visit_field(Key::new("data"), &*data);
        }
        visitor.exit();
    }
}
