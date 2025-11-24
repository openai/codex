/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use std::iter::Cloned;

use crate::Dupe;

pub trait IterDupedExt: Sized {
    /// Like `duped()`, but only works for types that implement `Dupe`.
    /// Note that the return type is deliberately `Cloned`, as that behaves
    /// the same as a `Duped` would be, but can take advantage of standard library
    /// optimisations.
    ///
    /// ```
    /// use std::rc::Rc;
    ///
    /// use dupe::IterDupedExt;
    /// let inputs = vec![Rc::new("Hello"), Rc::new("World")];
    /// let outputs = inputs.iter().duped().collect::<Vec<_>>();
    /// assert_eq!(inputs, outputs);
    /// ```
    /// use gazebo::prelude::*;
    /// use std::cmp::Ordering;
    fn duped(self) -> Cloned<Self>;
}

impl<'a, I, T> IterDupedExt for I
where
    I: Sized,
    I: Iterator<Item = &'a T>,
    T: 'a + Dupe,
{
    fn duped(self) -> Cloned<Self> {
        self.cloned()
    }
}
