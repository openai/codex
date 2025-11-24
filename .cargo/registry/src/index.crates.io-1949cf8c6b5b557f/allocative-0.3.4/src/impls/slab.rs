/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(feature = "slab")]

use std::mem;

use slab::Slab;

use crate::allocative_trait::Allocative;
use crate::impls::common::PTR_NAME;
use crate::impls::common::UNUSED_CAPACITY_NAME;
use crate::visitor::Visitor;

impl<T> Allocative for Slab<T>
where
    T: Allocative,
{
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();

        if self.capacity() != 0 && mem::size_of::<T>() != 0 {
            let mut visitor = visitor.enter_unique(PTR_NAME, mem::size_of::<*const T>());
            // NOTE: this ignores the size of the 'Entry' within the slab, but it's probably okay
            // as a good approximation of the size.
            visitor.visit_iter(self.iter().map(|(_k, v)| v));
            visitor.visit_simple(
                UNUSED_CAPACITY_NAME,
                (self.capacity() - self.len()) * mem::size_of::<T>(),
            );
        }
        visitor.exit();
    }
}
