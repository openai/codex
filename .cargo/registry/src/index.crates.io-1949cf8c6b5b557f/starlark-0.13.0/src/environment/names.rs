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

use std::cell::RefCell;

use allocative::Allocative;
use starlark_syntax::syntax::ast::Visibility;

use crate::collections::Hashed;
use crate::collections::SmallMap;
use crate::environment::slots::ModuleSlotId;
use crate::values::FrozenStringValue;

/// MutableNames are how we allocate slots (index-based) to variables
/// (name-based). The slots field is the current active mapping of names to
/// index.
///
/// In a statement context there are things that define variables, e.g. x=...,
/// for x in ... Importantly, the expression x can refer to either a global x,
/// or a local x that hasn't yet been defined, based on the future
/// presence/absence of a statement defining x. Therefore, we first capture all
/// the definitions with collect_defines_lvalue, allocate them slots,
/// then replace variables with slot numbers when compiling.
///
/// Comprehensions are a bit different. Given [x for x in y] that defines x, but
/// in a way that shadows any existing x, and the definition immediately binds
/// x. We do that with add_scoped()/unscope(). On an add_scope, we allocate
/// fresh slots at the end, and bind them to the names in the comprehension.
/// On an unscope, we do the reverse, putting things back to how they were
/// before (apart from the total) number of slots required.
#[derive(Debug)]
pub(crate) struct MutableNames(RefCell<SmallMap<FrozenStringValue, (ModuleSlotId, Visibility)>>);

#[derive(Debug, Allocative)]
pub(crate) struct FrozenNames(SmallMap<FrozenStringValue, (ModuleSlotId, Visibility)>);

impl MutableNames {
    pub(crate) fn new() -> Self {
        Self(RefCell::new(SmallMap::new()))
    }

    pub(crate) fn slot_count(&self) -> u32 {
        self.0.borrow().len().try_into().unwrap()
    }

    /// Try and go back from a slot to a name.
    /// Inefficient - only use in error paths.
    pub(crate) fn get_slot(&self, slot: ModuleSlotId) -> Option<FrozenStringValue> {
        for (s, (i, _vis)) in &*self.0.borrow() {
            if *i == slot {
                return Some(*s);
            }
        }
        None
    }

    pub(crate) fn get_name(&self, name: Hashed<&str>) -> Option<(ModuleSlotId, Visibility)> {
        self.0.borrow().get_hashed(name).copied()
    }

    /// Add a name with explicit visibility to the module.
    pub(crate) fn add_name_visibility(
        &self,
        name: FrozenStringValue,
        vis: Visibility,
    ) -> ModuleSlotId {
        let mut x = self.0.borrow_mut();
        match x.get_mut_hashed(name.get_hashed().as_ref()) {
            Some((slot, stored_vis)) => {
                // Public visibility wins.
                if *stored_vis == Visibility::Private {
                    *stored_vis = vis;
                }
                *slot
            }
            None => {
                let slot = ModuleSlotId::new(x.len().try_into().unwrap());
                x.insert_hashed(name.get_hashed(), (slot, vis));
                slot
            }
        }
    }

    // Add an exported name, or if it's already there, return the existing name
    pub(crate) fn add_name(&self, name: FrozenStringValue) -> ModuleSlotId {
        self.add_name_visibility(name, Visibility::Public)
    }

    pub(crate) fn hide_name(&self, name: &str) {
        self.0.borrow_mut().shift_remove(name);
    }

    pub(crate) fn all_names_and_slots(&self) -> Vec<(FrozenStringValue, ModuleSlotId)> {
        self.0
            .borrow()
            .iter()
            .map(|(name, (slot, _vis))| (*name, *slot))
            .collect()
    }

    pub(crate) fn all_names_and_visibilities(&self) -> Vec<(FrozenStringValue, Visibility)> {
        self.0
            .borrow()
            .iter()
            .map(|(name, (_slot, vis))| (*name, *vis))
            .collect()
    }

    pub(crate) fn all_names_slots_and_visibilities(
        &self,
    ) -> Vec<(FrozenStringValue, ModuleSlotId, Visibility)> {
        self.0
            .borrow()
            .iter()
            .map(|(name, (slot, vis))| (*name, *slot, *vis))
            .collect()
    }

    pub(crate) fn freeze(self) -> FrozenNames {
        FrozenNames(self.0.into_inner())
    }
}

impl FrozenNames {
    pub(crate) fn get_name(&self, name: &str) -> Option<(ModuleSlotId, Visibility)> {
        self.0.get(name).copied()
    }

    /// Symbols including private.
    pub(crate) fn all_symbols(
        &self,
    ) -> impl Iterator<Item = (FrozenStringValue, ModuleSlotId)> + '_ {
        self.0.iter().map(|(name, (slot, _vis))| (*name, *slot))
    }

    /// Exported symbols.
    pub(crate) fn symbols(&self) -> impl Iterator<Item = (FrozenStringValue, ModuleSlotId)> + '_ {
        self.0.iter().filter_map(|(name, (slot, vis))| match vis {
            Visibility::Private => None,
            Visibility::Public => Some((*name, *slot)),
        })
    }
}
