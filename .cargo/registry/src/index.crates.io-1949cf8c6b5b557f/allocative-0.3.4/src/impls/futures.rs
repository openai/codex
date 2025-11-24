/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(feature = "futures")]

use std::future::Future;
use std::mem;

use futures::future::Shared;
use futures::task::AtomicWaker;

use crate::allocative_trait::Allocative;
use crate::impls::common::PTR_NAME;
use crate::visitor::Visitor;

impl<F: Future + 'static> Allocative for Shared<F>
where
    F::Output: Allocative + Clone,
{
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        if let Some(result) = self.peek() {
            if let Some(mut visitor) = visitor.enter_shared(
                PTR_NAME,
                mem::size_of::<*const ()>(),
                result as *const _ as *const (),
            ) {
                result.visit(&mut visitor);
                visitor.exit();
            }
        }
        visitor.exit();
    }
}

impl Allocative for AtomicWaker {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.enter_self_sized::<Self>().exit()
    }
}
