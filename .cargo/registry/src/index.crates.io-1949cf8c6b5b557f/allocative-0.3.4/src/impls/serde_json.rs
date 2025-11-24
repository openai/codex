/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(feature = "serde_json")]

use std::mem;

use crate::Allocative;
use crate::Key;
use crate::Visitor;

impl Allocative for serde_json::Number {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let visitor = visitor.enter_self_sized::<Self>();
        // Number may have allocations, but it is opaque.
        visitor.exit();
    }
}

impl Allocative for serde_json::Map<String, serde_json::Value> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        {
            let mut visitor = visitor.enter_unique(Key::new("data"), mem::size_of::<*mut ()>());
            for (k, v) in self.iter() {
                visitor.visit_field(Key::new("key"), k);
                visitor.visit_field(Key::new("value"), v);
            }
            visitor.exit();
        }
        visitor.exit();
    }
}

impl Allocative for serde_json::Value {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        match self {
            serde_json::Value::Null => {}
            serde_json::Value::Bool(_b) => {}
            serde_json::Value::Number(n) => visitor.visit_field(Key::new("Number"), n),
            serde_json::Value::String(s) => visitor.visit_field(Key::new("String"), s),
            serde_json::Value::Array(a) => visitor.visit_field(Key::new("Array"), a),
            serde_json::Value::Object(o) => visitor.visit_field(Key::new("Object"), o),
        }
        visitor.exit();
    }
}
