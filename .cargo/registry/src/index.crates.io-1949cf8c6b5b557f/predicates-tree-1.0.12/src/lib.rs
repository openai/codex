// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/license/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Render `Case` as a tree.

#![cfg_attr(docsrs, feature(doc_auto_cfg))]

use std::fmt;

use predicates_core::reflection;

/// Render `Self` as a displayable tree.
pub trait CaseTreeExt {
    /// Render `Self` as a displayable tree.
    fn tree(&self) -> CaseTree;
}

impl CaseTreeExt for reflection::Case<'_> {
    fn tree(&self) -> CaseTree {
        CaseTree(convert(self))
    }
}

type CaseTreeInner = termtree::Tree<Displayable>;

fn convert(case: &reflection::Case<'_>) -> CaseTreeInner {
    let mut leaves: Vec<CaseTreeInner> = vec![];

    leaves.extend(case.predicate().iter().flat_map(|pred| {
        pred.parameters().map(|item| {
            let root = Displayable::new(&item);
            termtree::Tree::new(root).with_multiline(true)
        })
    }));

    leaves.extend(case.products().map(|item| {
        let root = Displayable::new(item);
        termtree::Tree::new(root).with_multiline(true)
    }));

    leaves.extend(case.children().map(convert));

    let root = case
        .predicate()
        .map(|p| Displayable::new(&p))
        .unwrap_or_default();
    CaseTreeInner::new(root).with_leaves(leaves)
}

/// A `Case` rendered as a tree for display.
#[allow(missing_debug_implementations)]
pub struct CaseTree(CaseTreeInner);

impl fmt::Display for CaseTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Default)]
struct Displayable {
    primary: String,
    alternate: String,
}

impl Displayable {
    fn new(display: &dyn std::fmt::Display) -> Self {
        let primary = format!("{}", display);
        let alternate = format!("{:#}", display);
        Self { primary, alternate }
    }
}

impl fmt::Display for Displayable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            self.alternate.fmt(f)
        } else {
            self.primary.fmt(f)
        }
    }
}
