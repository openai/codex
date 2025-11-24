/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(feature = "indexmap")]

use std::mem;

use indexmap::IndexMap;
use indexmap::IndexSet;

use crate::allocative_trait::Allocative;
use crate::impls::common::UNUSED_CAPACITY_NAME;
use crate::impls::hashbrown_util::raw_table_alloc_size_for_len;
use crate::key::Key;
use crate::visitor::Visitor;

/// Add approximate allocations for hashbrown `RawTable`.
fn add_raw_table_for_len<T>(visitor: &mut Visitor, len: usize) {
    if len != 0 {
        // We don't depend on `RawTable`, so we don't know the size of `RawTable`.
        let size_of_raw_table = mem::size_of::<usize>();
        let mut visitor = visitor.enter_unique(Key::new("raw_table"), size_of_raw_table);
        visitor.visit_simple(Key::new("alloc"), raw_table_alloc_size_for_len::<T>(len));
        visitor.exit();
    }
}

impl<T: Allocative, S> Allocative for IndexSet<T, S> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        {
            let mut visitor = visitor.enter_unique(Key::new("data"), mem::size_of::<*const ()>());
            for v in self {
                visitor.visit_field(Key::new("value"), v);
            }
            let unused_capacity = self.capacity() - self.len();
            visitor.visit_simple(UNUSED_CAPACITY_NAME, unused_capacity * mem::size_of::<T>());
            visitor.exit();
        }
        add_raw_table_for_len::<usize>(&mut visitor, self.len());
    }
}

impl<K: Allocative, V: Allocative, S> Allocative for IndexMap<K, V, S> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        {
            let mut visitor = visitor.enter_unique(Key::new("data"), mem::size_of::<*const ()>());
            for (k, v) in self {
                visitor.visit_field(Key::new("key"), k);
                visitor.visit_field(Key::new("value"), v);
            }
            let unused_capacity = self.capacity() - self.len();
            visitor.visit_simple(
                UNUSED_CAPACITY_NAME,
                unused_capacity * mem::size_of::<(K, V)>(),
            );
            visitor.exit();
        }
        add_raw_table_for_len::<usize>(&mut visitor, self.len());
    }
}
