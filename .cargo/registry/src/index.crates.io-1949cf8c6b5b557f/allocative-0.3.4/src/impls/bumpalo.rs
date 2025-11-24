/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(feature = "bumpalo")]

use std::mem;

use bumpalo::Bump;

use crate::allocative_trait::Allocative;
use crate::impls::common::DATA_NAME;
use crate::impls::common::PTR_NAME;
use crate::impls::common::UNUSED_CAPACITY_NAME;
use crate::visitor::Visitor;

struct BumpCounters {
    used: usize,
    unused: usize,
}

impl BumpCounters {
    fn collect(bump: &Bump) -> BumpCounters {
        let allocated = bump.allocated_bytes();
        let used = unsafe { bump.iter_allocated_chunks_raw().map(|(_, size)| size).sum() };
        let unused = allocated.wrapping_sub(used);
        BumpCounters { used, unused }
    }
}

impl Allocative for Bump {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        {
            let mut visitor = visitor.enter_unique(PTR_NAME, mem::size_of::<*const ()>());

            let counters = BumpCounters::collect(self);

            visitor.visit_simple(DATA_NAME.clone(), counters.used);
            visitor.visit_simple(UNUSED_CAPACITY_NAME.clone(), counters.unused);
            // TODO(nga): Ignores header size.
            visitor.exit();
        }
        visitor.exit();
    }
}

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use crate::impls::bumpalo::BumpCounters;

    #[test]
    fn test_bumpalo() {
        let bump = Bump::new();
        bump.alloc(10);

        let counters = BumpCounters::collect(&bump);
        assert_eq!(4, counters.used);
        assert!(counters.unused > 100);

        bump.alloc_str("xx");
        let counters_2 = BumpCounters::collect(&bump);
        assert_eq!(6, counters_2.used);
        assert_eq!(
            counters.used + counters.unused,
            counters_2.used + counters_2.unused
        );
    }

    #[test]
    fn test_bumpalo_large() {
        let bump = Bump::new();
        for i in 0..1000 {
            bump.alloc(i);
        }
        // Make sure total is odd, and bump allocates odd.
        bump.alloc(true);
        let counters = BumpCounters::collect(&bump);
        assert_eq!(4001, counters.used);
        assert!(counters.unused > 0);
    }
}
