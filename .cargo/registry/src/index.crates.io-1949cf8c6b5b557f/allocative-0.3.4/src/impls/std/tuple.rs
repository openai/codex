/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use crate::allocative_trait::Allocative;
use crate::key::Key;
use crate::visitor::Visitor;

impl Allocative for () {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.visit_simple_sized::<Self>();
    }
}

impl<A: Allocative> Allocative for (A,) {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        visitor.visit_field(Key::new("0"), &self.0);
    }
}

impl<A: Allocative, B: Allocative> Allocative for (A, B) {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        visitor.visit_field(Key::new("0"), &self.0);
        visitor.visit_field(Key::new("1"), &self.1);
    }
}

impl<A: Allocative, B: Allocative, C: Allocative> Allocative for (A, B, C) {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        visitor.visit_field(Key::new("0"), &self.0);
        visitor.visit_field(Key::new("1"), &self.1);
        visitor.visit_field(Key::new("2"), &self.2);
    }
}

impl<A: Allocative, B: Allocative, C: Allocative, D: Allocative> Allocative for (A, B, C, D) {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        visitor.visit_field(Key::new("0"), &self.0);
        visitor.visit_field(Key::new("1"), &self.1);
        visitor.visit_field(Key::new("2"), &self.2);
        visitor.visit_field(Key::new("3"), &self.3);
    }
}

impl<A: Allocative, B: Allocative, C: Allocative, D: Allocative, E: Allocative> Allocative
    for (A, B, C, D, E)
{
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        visitor.visit_field(Key::new("0"), &self.0);
        visitor.visit_field(Key::new("1"), &self.1);
        visitor.visit_field(Key::new("2"), &self.2);
        visitor.visit_field(Key::new("3"), &self.3);
        visitor.visit_field(Key::new("4"), &self.4);
    }
}
