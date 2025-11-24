/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(feature = "dashmap")]

use std::hash::BuildHasher;
use std::hash::Hash;
use std::mem;

use dashmap::DashMap;
use dashmap::DashSet;

use crate::allocative_trait::Allocative;
use crate::impls::common::CAPACITY_NAME;
use crate::impls::common::DATA_NAME;
use crate::impls::common::KEY_NAME;
use crate::impls::common::PTR_NAME;
use crate::impls::common::UNUSED_CAPACITY_NAME;
use crate::impls::common::VALUE_NAME;
use crate::visitor::Visitor;

impl<K: Allocative + Eq + Hash, V: Allocative, S: BuildHasher + Clone> Allocative
    for DashMap<K, V, S>
{
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        let mut visitor2 = visitor.enter_unique(PTR_NAME, mem::size_of::<*const ()>());
        {
            let mut capacity_visitor =
                visitor2.enter(CAPACITY_NAME, self.capacity() * mem::size_of::<(K, V)>());
            for entry in self.iter() {
                capacity_visitor.visit_field(KEY_NAME, entry.key());
                capacity_visitor.visit_field(VALUE_NAME, entry.value());
            }
            capacity_visitor.visit_simple(
                UNUSED_CAPACITY_NAME,
                self.capacity().saturating_sub(self.len()) * mem::size_of::<(K, V)>(),
            );
            capacity_visitor.exit();
        }
        visitor2.exit();
        visitor.exit();
    }
}

impl<T: Allocative + Eq + Hash, S: BuildHasher + Clone> Allocative for DashSet<T, S> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        let mut visitor2 = visitor.enter_unique(PTR_NAME, mem::size_of::<*const ()>());
        {
            let mut capacity_visitor =
                visitor2.enter(CAPACITY_NAME, self.capacity() * mem::size_of::<T>());
            for entry in self.iter() {
                capacity_visitor.visit_field(DATA_NAME, entry.key());
            }
            capacity_visitor.visit_simple(
                UNUSED_CAPACITY_NAME,
                self.capacity().saturating_sub(self.len()) * mem::size_of::<T>(),
            );
            capacity_visitor.exit();
        }
    }
}
