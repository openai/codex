/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(feature = "smallvec")]

use smallvec::Array;
use smallvec::SmallVec;

use crate::allocative_trait::Allocative;
use crate::impls::common::PTR_NAME;
use crate::key::Key;
use crate::visitor::Visitor;

impl<A> Allocative for SmallVec<A>
where
    A: Array,
    A::Item: Allocative,
{
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        if self.spilled() {
            let mut visitor = visitor.enter_unique(PTR_NAME, std::mem::size_of::<*const A::Item>());
            for item in self {
                visitor.visit_field(Key::new("data"), item);
            }
            // TODO(nga): spare capacity.
            visitor.exit();
        } else {
            for item in self {
                visitor.visit_field(Key::new("data"), item);
            }
        }
        visitor.exit();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
        // No test here because sizes depend on whether "union" feature is enabled.
    }
}
