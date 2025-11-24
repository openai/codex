/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use std::cell::OnceCell;
use std::cell::RefCell;
use std::sync::OnceLock;

use crate::impls::common::DATA_NAME;
use crate::Allocative;
use crate::Visitor;

impl<T: Allocative> Allocative for RefCell<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        if let Ok(v) = self.try_borrow() {
            visitor.visit_field(DATA_NAME, &*v);
        }
        visitor.exit();
    }
}

impl<T: Allocative> Allocative for OnceCell<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        if let Some(v) = self.get() {
            visitor.visit_field::<T>(DATA_NAME, v);
        }
        visitor.exit();
    }
}

impl<T: Allocative> Allocative for OnceLock<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        if let Some(v) = self.get() {
            visitor.visit_field::<T>(DATA_NAME, v);
        }
        visitor.exit();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::OnceCell;
    use std::cell::RefCell;

    use crate::golden::golden_test;

    #[test]
    fn test_default() {
        golden_test!(&RefCell::new("abc".to_owned()))
    }

    #[test]
    fn test_borrowed() {
        let cell = RefCell::new("abc".to_owned());
        let _lock = cell.borrow_mut();
        golden_test!(&cell)
    }

    #[test]
    fn test_once_cell() {
        let cell = OnceCell::new();
        cell.set("abc".to_owned()).unwrap();
        golden_test!(&cell)
    }
}
