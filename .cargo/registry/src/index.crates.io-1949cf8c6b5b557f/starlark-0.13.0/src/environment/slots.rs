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
use std::cell::RefMut;

use allocative::Allocative;
use dupe::Dupe;

use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenValue;
use crate::values::Value;

#[derive(Clone, Copy, Dupe, Debug, PartialEq, Eq, Allocative, Hash)]
pub(crate) struct ModuleSlotId(pub(crate) u32);

impl ModuleSlotId {
    pub fn new(index: u32) -> Self {
        Self(index)
    }
}

// Indexed slots of a module. May contain unassigned values as `None`.
#[derive(Debug)]
pub(crate) struct MutableSlots<'v>(RefCell<Vec<Option<Value<'v>>>>);

// Indexed slots of a module. May contain unassigned values as `None`.
#[derive(Debug, Allocative)]
pub(crate) struct FrozenSlots(Vec<Option<FrozenValue>>);

impl<'v> MutableSlots<'v> {
    pub fn new() -> Self {
        Self(RefCell::new(Vec::new()))
    }

    pub(crate) fn get_slots_mut(&self) -> RefMut<Vec<Option<Value<'v>>>> {
        self.0.borrow_mut()
    }

    pub fn get_slot(&self, slot: ModuleSlotId) -> Option<Value<'v>> {
        self.0.borrow()[slot.0 as usize]
    }

    pub fn set_slot(&self, slot: ModuleSlotId, value: Value<'v>) {
        self.0.borrow_mut()[slot.0 as usize] = Some(value);
    }

    pub fn ensure_slot(&self, slot: ModuleSlotId) {
        // To ensure that `slot` exists, we need at least `slot + 1` slots.
        self.ensure_slots(slot.0 + 1);
    }

    pub fn ensure_slots(&self, count: u32) {
        let mut slots = self.0.borrow_mut();
        if slots.len() >= count as usize {
            return;
        }
        let extra = count as usize - slots.len();
        slots.reserve(extra);
        for _ in 0..extra {
            slots.push(None);
        }
    }

    pub(crate) fn values_by_slot_id(&self) -> Vec<(ModuleSlotId, Value<'v>)> {
        self.0
            .borrow()
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.map(|v| (ModuleSlotId::new(u32::try_from(i).unwrap()), v)))
            .collect()
    }

    pub(crate) fn freeze(self, freezer: &Freezer) -> FreezeResult<FrozenSlots> {
        let slots = self.0.into_inner().freeze(freezer)?;
        Ok(FrozenSlots(slots))
    }
}

impl FrozenSlots {
    pub fn get_slot(&self, slot: ModuleSlotId) -> Option<FrozenValue> {
        self.0[slot.0 as usize]
    }
}
