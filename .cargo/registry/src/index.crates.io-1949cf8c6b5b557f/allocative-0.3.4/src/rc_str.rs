/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use std::borrow::Borrow;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Deref;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
#[allow(dead_code)]
pub(crate) struct RcStr(Rc<str>);

impl<'a> From<&'a str> for RcStr {
    fn from(s: &'a str) -> RcStr {
        RcStr(Rc::from(s))
    }
}

impl Default for RcStr {
    fn default() -> RcStr {
        RcStr::from("")
    }
}

impl RcStr {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for RcStr {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for RcStr {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

#[allow(clippy::derived_hash_with_manual_eq)]
impl Hash for RcStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}
