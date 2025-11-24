/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(feature = "compact_str")]

use compact_str::CompactString;

use crate::allocative_trait::Allocative;
use crate::impls::common::PTR_NAME;
use crate::impls::common::UNUSED_CAPACITY_NAME;
use crate::key::Key;
use crate::visitor::Visitor;

impl Allocative for CompactString {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        if self.is_heap_allocated() {
            let mut visitor = visitor.enter_unique(PTR_NAME, std::mem::size_of::<*const u8>());
            visitor.visit_simple(Key::new("str"), self.len());
            let unused_capacity = self.capacity() - self.len();
            visitor.visit_simple(UNUSED_CAPACITY_NAME, unused_capacity);
        }
        visitor.exit();
    }
}

#[cfg(test)]
mod tests {
    use compact_str::CompactString;

    use crate::golden::golden_test;

    #[test]
    fn test_inline() {
        golden_test!(&CompactString::from("abc"));
    }

    #[test]
    fn test_heap() {
        golden_test!(&CompactString::from("abc".repeat(100)));
    }
}
