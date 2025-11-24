/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(feature = "tokio")]

use tokio::sync::Mutex;
use tokio::sync::RwLock;

use crate::Allocative;
use crate::Key;
use crate::Visitor;

impl<T: Allocative> Allocative for RwLock<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        if let Ok(data) = self.try_read() {
            visitor.visit_field::<T>(Key::new("data"), &*data);
        }
        visitor.exit();
    }
}

impl<T: Allocative> Allocative for Mutex<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        if let Ok(data) = self.try_lock() {
            visitor.visit_field::<T>(Key::new("data"), &*data);
        }
        visitor.exit();
    }
}
