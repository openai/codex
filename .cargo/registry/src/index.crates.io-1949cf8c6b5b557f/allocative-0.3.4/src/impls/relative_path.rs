/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(feature = "relative-path")]

use std::mem;

use relative_path::RelativePathBuf;

use crate::allocative_trait::Allocative;
use crate::impls::common::PTR_NAME;
use crate::key::Key;
use crate::visitor::Visitor;

impl Allocative for RelativePathBuf {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        {
            let mut visitor = visitor.enter_unique(PTR_NAME, mem::size_of::<*const u8>());
            visitor.visit_simple(Key::new("chars"), self.as_str().len());
            // TODO(nga): spare capacity.
            visitor.exit();
        }
        visitor.exit();
    }
}
