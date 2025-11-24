/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(feature = "either")]

use either::Either;

use crate::allocative_trait::Allocative;
use crate::key::Key;
use crate::visitor::Visitor;

impl<A: Allocative, B: Allocative> Allocative for Either<A, B> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        match self {
            Either::Left(a) => visitor.visit_field::<A>(Key::new("Left"), a),
            Either::Right(b) => visitor.visit_field::<B>(Key::new("Right"), b),
        }
    }
}

#[cfg(test)]
mod tests {
    use either::Either;

    use crate::golden::golden_test;

    #[test]
    fn test_golden() {
        golden_test!(&Either::<u32, String>::Left(1));
    }
}
