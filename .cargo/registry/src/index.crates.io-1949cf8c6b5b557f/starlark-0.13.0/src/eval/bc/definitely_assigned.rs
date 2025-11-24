/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use crate::eval::runtime::slots::LocalSlotId;

/// Tracker for local variables which are definitely assigned.
///
/// For example, when compiling an program like:
///
/// ```python
/// foo(x)
/// bar(x)
/// ```
///
/// To access `x` variable first time, when evaluating `foo(x)` we need to check
/// that `x` is assigned, and return an error if it is not.
/// But when evaluating `bar(x)` we don't need to check `x` is assigned,
/// because we know for sure it is assigned: we checked that when evaluating `foo(x)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BcDefinitelyAssigned {
    /// Map from local variable slot to flag indicating whether it is definitely assigned
    /// at the current program point.
    // TODO(nga): most functions have less than 64 locals, we could use a bitmap instead.
    definitely_assigned: Vec<bool>,
}

impl BcDefinitelyAssigned {
    pub(crate) fn new(local_count: u32) -> BcDefinitelyAssigned {
        BcDefinitelyAssigned {
            definitely_assigned: vec![false; local_count as usize],
        }
    }

    /// Is local variable definitely assigned at given program point?
    pub(crate) fn is_definitely_assigned(&self, local: LocalSlotId) -> bool {
        self.definitely_assigned[local.0 as usize]
    }

    /// Mark variable definitely assigned.
    ///
    /// For example, after execution of:
    ///
    /// ```python
    /// foo(x)
    /// ```
    ///
    /// both `foo` and `x` are definitely assigned.
    pub(crate) fn mark_definitely_assigned(&mut self, local: LocalSlotId) {
        self.definitely_assigned[local.0 as usize] = true;
    }

    /// Assert that each variable definitely assigned in self,
    /// also definitely assigned in other.
    pub(crate) fn assert_smaller_then(&self, other: &BcDefinitelyAssigned) {
        assert_eq!(
            self.definitely_assigned.len(),
            other.definitely_assigned.len()
        );
        for (&a, &b) in self
            .definitely_assigned
            .iter()
            .zip(other.definitely_assigned.iter())
        {
            assert!(b || !a)
        }
    }
}
