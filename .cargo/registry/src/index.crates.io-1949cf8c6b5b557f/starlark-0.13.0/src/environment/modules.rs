/*
 * Copyright 2018 The Starlark in Rust Authors.
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

//! The environment, called "Module" in [this spec](
//! https://github.com/bazelbuild/starlark/blob/master/spec.md)
//! is the list of variable in the current scope. It can be frozen, after which
//! all values from this environment become immutable.

use std::cell::Cell;
use std::cell::RefCell;
use std::mem;
use std::time::Duration;
use std::time::Instant;

use allocative::Allocative;
use dupe::Dupe;
use itertools::Itertools;
use starlark_syntax::syntax::ast::Visibility;

use crate::cast::transmute;
use crate::collections::Hashed;
use crate::docs::DocItem;
use crate::docs::DocMember;
use crate::docs::DocModule;
use crate::docs::DocString;
use crate::docs::DocStringKind;
use crate::environment::names::FrozenNames;
use crate::environment::names::MutableNames;
use crate::environment::slots::FrozenSlots;
use crate::environment::slots::ModuleSlotId;
use crate::environment::slots::MutableSlots;
use crate::environment::EnvironmentError;
use crate::environment::Globals;
use crate::errors::did_you_mean::did_you_mean;
use crate::eval::runtime::profile::heap::RetainedHeapProfileMode;
use crate::eval::ProfileData;
use crate::values::layout::heap::heap_type::HeapKind;
use crate::values::layout::heap::profile::aggregated::AggregateHeapProfileInfo;
use crate::values::layout::heap::profile::aggregated::RetainedHeapProfile;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenHeap;
use crate::values::FrozenHeapRef;
use crate::values::FrozenRef;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::OwnedFrozenValue;
use crate::values::Trace;
use crate::values::Tracer;
use crate::values::Value;

#[derive(Debug, thiserror::Error)]
enum ModuleError {
    #[error("Retained memory profiling is not enabled")]
    RetainedMemoryProfileNotEnabled,
    #[error("Extra value already set to a value of type `{}`", .0)]
    ExtraValueAlreadySet(&'static str),
}

/// The result of freezing a [`Module`], making it and its contained values immutable.
///
/// The values of this [`FrozenModule`] are stored on a frozen heap, a reference to which
/// can be obtained using [`frozen_heap`](FrozenModule::frozen_heap). Be careful not to use
/// these values after the [`FrozenModule`] has been released unless you obtain a reference
/// to the frozen heap.
#[derive(Debug, Clone, Dupe, Allocative)]
// We store the two elements separately since the FrozenHeapRef contains
// a copy of the FrozenModuleData inside it.
// Two Arc's should still be plenty cheap enough to qualify for `Dupe`.
pub struct FrozenModule {
    heap: FrozenHeapRef,
    module: FrozenRef<'static, FrozenModuleData>,
    extra_value: Option<FrozenValue>,
    /// Module evaluation duration:
    /// * evaluation of the top-level statements
    /// * optimizations during that evaluation
    /// * freezing and optimizations during freezing
    /// * does not include parsing time
    pub(crate) eval_duration: Duration,
}

#[derive(Debug, Allocative)]
pub(crate) struct FrozenModuleData {
    pub(crate) names: FrozenNames,
    pub(crate) slots: FrozenSlots,
    docstring: Option<String>,
    /// When heap profile enabled, this field stores retained memory info.
    heap_profile: Option<RetainedHeapProfile>,
}

/// A container for user values, used during execution.
///
/// A module contains both a [`FrozenHeap`] and [`Heap`] on which different values are allocated.
/// You can get references to these heaps with [`frozen_heap`](Module::frozen_heap) and
/// [`heap`](Module::heap). Be careful not to use these values after the [`Module`] has been
/// released unless you obtain a reference to the frozen heap.
#[derive(Debug)]
pub struct Module {
    heap: Heap,
    frozen_heap: FrozenHeap,
    names: MutableNames,
    // Should really be MutableSlots<'v>, where &'v self
    // Values are allocated from heap. Because of variance
    // you can inject the wrong values in, so make sure slots aren't
    // exported.
    slots: MutableSlots<'static>,
    docstring: RefCell<Option<String>>,
    /// Module evaluation duration:
    /// * evaluation of the top-level statements
    /// * optimizations during that evaluation
    /// * does not include freezing time
    /// * does not include parsing time
    eval_duration: Cell<Duration>,
    /// Field that can be used for any purpose you want.
    extra_value: Cell<Option<Value<'static>>>,
    /// When `Some`, heap profile is collected on freeze.
    heap_profile_on_freeze: Cell<Option<RetainedHeapProfileMode>>,
}

impl FrozenModule {
    /// Convert items in `globals` into a `FrozenModule`.
    /// This function can be used to implement starlark module
    /// using the `#[starlark_module]` attribute.
    ///
    /// This function does not return an error,
    /// but we prefer not to panic if there's some high level logic error.
    pub fn from_globals(globals: &Globals) -> FreezeResult<FrozenModule> {
        let module = Module::new();

        module.frozen_heap.add_reference(globals.heap());

        for (name, value) in globals.iter() {
            module.set(name, value.to_value());
        }

        if let Some(docstring) = globals.docstring() {
            module.set_docstring(String::from(docstring));
        }

        module.freeze()
    }

    fn get_any_visibility_option(&self, name: &str) -> Option<(OwnedFrozenValue, Visibility)> {
        self.module.names.get_name(name).and_then(|(slot, vis)|
        // This code is safe because we know the frozen module ref keeps the values alive
        self.module
            .slots
            .get_slot(slot)
            .map(|x| (unsafe { OwnedFrozenValue::new(self.heap.dupe(), x) }, vis)))
    }

    /// Get value, exported or private by name.
    // TODO(nga): separate private visibility into private (`_foo`) and imported (`load('foo')`).
    //   Users might want to access private variables, but not imported.
    #[doc(hidden)]
    pub fn get_any_visibility(&self, name: &str) -> anyhow::Result<(OwnedFrozenValue, Visibility)> {
        self.get_any_visibility_option(name).ok_or_else(|| {
            match did_you_mean(name, self.names().map(|s| s.as_str())) {
                Some(better) => EnvironmentError::ModuleHasNoSymbolDidYouMean(
                    name.to_owned(),
                    better.to_owned(),
                )
                .into(),
                None => EnvironmentError::ModuleHasNoSymbol(name.to_owned()).into(),
            }
        })
    }

    /// Get the value of the exported variable `name`.
    ///
    /// # Returns
    /// * `None` if symbol is not found
    /// * error if symbol is private
    pub fn get_option(&self, name: &str) -> anyhow::Result<Option<OwnedFrozenValue>> {
        match self.get_any_visibility_option(name) {
            None => Ok(None),
            Some((_, Visibility::Private)) => {
                Err(EnvironmentError::ModuleSymbolIsNotExported(name.to_owned()).into())
            }
            Some((value, Visibility::Public)) => Ok(Some(value)),
        }
    }

    /// Get the value of the exported variable `name`.
    /// Returns an error if the variable isn't defined in the module or it is private.
    pub fn get(&self, name: &str) -> anyhow::Result<OwnedFrozenValue> {
        self.get_any_visibility(name)
            .and_then(|(value, vis)| match vis {
                Visibility::Private => {
                    Err(EnvironmentError::ModuleSymbolIsNotExported(name.to_owned()).into())
                }
                Visibility::Public => Ok(value),
            })
    }

    /// Iterate through all the names defined in this module.
    /// Only includes symbols that are publicly exposed.
    pub fn names(&self) -> impl Iterator<Item = FrozenStringValue> + '_ {
        self.module.names()
    }

    /// Obtain the [`FrozenHeapRef`] which owns the storage of all values defined in this module.
    pub fn frozen_heap(&self) -> &FrozenHeapRef {
        &self.heap
    }

    /// Print out some approximation of the module definitions.
    pub fn describe(&self) -> String {
        self.module.describe()
    }

    pub(crate) fn all_items(&self) -> impl Iterator<Item = (FrozenStringValue, FrozenValue)> + '_ {
        self.module.all_items()
    }

    /// The documentation for the module, and all of its top level values
    ///
    /// Returns `(<module documentation>, { <symbol> : <that symbol's documentation> })`
    pub fn documentation(&self) -> DocModule {
        let members = self
            .all_items()
            .filter(|n| Module::default_visibility(n.0.as_str()) == Visibility::Public)
            // FIXME(JakobDegen): Throws out information
            .map(|(k, v)| {
                (
                    k.as_str().to_owned(),
                    DocItem::Member(DocMember::from_value(v.to_value())),
                )
            })
            .collect();

        DocModule {
            docs: self.module.documentation(),
            members,
        }
    }

    /// Retained memory info, or error if not enabled.
    pub fn heap_profile(&self) -> anyhow::Result<ProfileData> {
        match &self.module.heap_profile {
            None => Err(ModuleError::RetainedMemoryProfileNotEnabled.into()),
            Some(p) => Ok(p.to_profile()),
        }
    }

    /// `extra_value` field from `Module`, frozen.
    pub fn extra_value(&self) -> Option<FrozenValue> {
        self.extra_value
    }

    /// `extra_value` field from `Module`, frozen.
    pub fn owned_extra_value(&self) -> Option<OwnedFrozenValue> {
        self.extra_value
            .map(|v| unsafe { OwnedFrozenValue::new(self.heap.dupe(), v) })
    }
}

impl FrozenModuleData {
    fn names(&self) -> impl Iterator<Item = FrozenStringValue> + '_ {
        self.names.symbols().map(|x| x.0)
    }

    fn describe(&self) -> String {
        self.items()
            .map(|(name, val)| val.to_value().describe(&name))
            .join("\n")
    }

    fn items(&self) -> impl Iterator<Item = (FrozenStringValue, FrozenValue)> + '_ {
        self.names
            .symbols()
            .filter_map(|(name, slot)| Some((name, self.slots.get_slot(slot)?)))
    }

    fn all_items(&self) -> impl Iterator<Item = (FrozenStringValue, FrozenValue)> + '_ {
        self.names
            .all_symbols()
            .filter_map(|(name, slot)| Some((name, self.slots.get_slot(slot)?)))
    }

    pub(crate) fn get_slot(&self, slot: ModuleSlotId) -> Option<FrozenValue> {
        self.slots.get_slot(slot)
    }

    /// Try and go back from a slot to a name.
    /// Inefficient - only use in error paths.
    pub(crate) fn get_slot_name(&self, slot: ModuleSlotId) -> Option<FrozenStringValue> {
        for (s, i) in self.names.symbols() {
            if i == slot {
                return Some(s);
            }
        }
        None
    }

    fn documentation(&self) -> Option<DocString> {
        self.docstring
            .as_ref()
            .and_then(|d| DocString::from_docstring(DocStringKind::Starlark, d))
    }
}

impl Default for Module {
    fn default() -> Self {
        Self::new()
    }
}

impl Module {
    /// Create a new module environment with no contents.
    pub fn new() -> Self {
        Self {
            heap: Heap::new(),
            frozen_heap: FrozenHeap::new(),
            names: MutableNames::new(),
            slots: MutableSlots::new(),
            docstring: RefCell::new(None),
            eval_duration: Cell::new(Duration::ZERO),
            extra_value: Cell::new(None),
            heap_profile_on_freeze: Cell::new(None),
        }
    }

    pub(crate) fn enable_retained_heap_profile(&self, mode: RetainedHeapProfileMode) {
        self.heap_profile_on_freeze.set(Some(mode));
    }

    /// Get the heap on which values are allocated by this module.
    pub fn heap(&self) -> &Heap {
        &self.heap
    }

    /// Get the frozen heap on which frozen values are allocated by this module.
    pub fn frozen_heap(&self) -> &FrozenHeap {
        &self.frozen_heap
    }

    /// Iterate through all the names defined in this module.
    /// Only includes symbols that are publicly exposed.
    pub fn names(&self) -> impl Iterator<Item = FrozenStringValue> + '_ {
        self.names
            .all_names_and_visibilities()
            .into_iter()
            .filter_map(|(name, vis)| {
                if vis == Visibility::Public {
                    Some(name)
                } else {
                    None
                }
            })
    }

    pub(crate) fn values_by_slot_id<'v>(&'v self) -> Vec<(ModuleSlotId, Value<'v>)> {
        self.slots().values_by_slot_id()
    }

    /// Iterate through all the names defined in this module, including those that are private.
    pub fn names_and_visibilities(
        &self,
    ) -> impl Iterator<Item = (FrozenStringValue, Visibility)> + '_ {
        self.names.all_names_and_visibilities().into_iter()
    }

    pub(crate) fn mutable_names(&self) -> &MutableNames {
        &self.names
    }

    pub(crate) fn slots<'v>(&'v self) -> &'v MutableSlots<'v> {
        // Not true because of variance, but mostly true. Don't export further.
        unsafe { transmute!(&'v MutableSlots<'static>, &'v MutableSlots<'v>, &self.slots) }
    }

    /// Get value, exported or private by name.
    pub(crate) fn get_any_visibility<'v>(
        &'v self,
        name: Hashed<&str>,
    ) -> Option<(Value<'v>, Visibility)> {
        let (slot, vis) = self.names.get_name(name)?;
        let value = self.slots().get_slot(slot)?;
        Some((value, vis))
    }

    /// Get the value of the exported variable `name`.
    /// Returns [`None`] if the variable isn't defined in the module or it is private.
    pub fn get<'v>(&'v self, name: &str) -> Option<Value<'v>> {
        self.get_any_visibility(Hashed::new(name))
            .and_then(|(v, vis)| match vis {
                Visibility::Private => None,
                Visibility::Public => Some(v),
            })
    }

    /// Freeze the environment, all its value will become immutable afterwards.
    pub fn freeze(self) -> FreezeResult<FrozenModule> {
        let Module {
            names,
            slots,
            frozen_heap,
            heap,
            docstring,
            eval_duration,
            extra_value,
            heap_profile_on_freeze,
        } = self;
        let start = Instant::now();
        // This is when we do the GC/freeze, using the module slots as roots
        // Note that we even freeze anonymous slots, since they are accessed by
        // slot-index in the code, and we don't walk into them, so don't know if
        // they are used.
        let freezer = Freezer::new(frozen_heap);
        let slots = slots.freeze(&freezer)?;
        let extra_value = extra_value.into_inner().freeze(&freezer)?;
        let stacks = if let Some(mode) = heap_profile_on_freeze.get() {
            // TODO(nga): retained heap profile does not store information about data
            //   allocated in frozen heap before freeze starts.
            let heap_profile = AggregateHeapProfileInfo::collect(&heap, Some(HeapKind::Frozen));
            Some(RetainedHeapProfile {
                info: heap_profile,
                mode,
            })
        } else {
            None
        };
        let rest = FrozenModuleData {
            names: names.freeze(),
            slots,
            docstring: docstring.into_inner(),
            heap_profile: stacks,
        };
        let frozen_module_ref = freezer.heap.alloc_any(rest);
        for frozen_def in freezer.frozen_defs.borrow().as_slice() {
            frozen_def.post_freeze(frozen_module_ref, &heap, &freezer.heap);
        }
        // The values MUST be alive up until this point (as the above line uses them),
        // but can now be dropped
        mem::drop(heap);

        Ok(FrozenModule {
            heap: freezer.into_ref(),
            module: frozen_module_ref,
            extra_value,
            eval_duration: start.elapsed() + eval_duration.get(),
        })
    }

    /// Set the value of a variable in the environment.
    /// Modifying these variables while executing is ongoing can have
    /// surprising effects.
    pub fn set<'v>(&'v self, name: &str, value: Value<'v>) {
        let slot = self.names.add_name(self.frozen_heap.alloc_str_intern(name));
        let slots = self.slots();
        slots.ensure_slot(slot);
        slots.set_slot(slot, value);
    }

    /// Symbols starting with underscore are considered private.
    pub(crate) fn default_visibility(symbol: &str) -> Visibility {
        match symbol.starts_with('_') {
            true => Visibility::Private,
            false => Visibility::Public,
        }
    }

    /// Set the value of a variable in the environment. Set its visibliity to
    /// "private" to ensure that it is not re-exported
    pub(crate) fn set_private<'v>(&'v self, name: FrozenStringValue, value: Value<'v>) {
        let slot = self.names.add_name_visibility(name, Visibility::Private);
        let slots = self.slots();
        slots.ensure_slot(slot);
        slots.set_slot(slot, value);
    }

    /// Import symbols from a module, similar to what is done during `load()`.
    pub fn import_public_symbols(&self, module: &FrozenModule) {
        self.frozen_heap.add_reference(&module.heap);
        for (k, slot) in module.module.names.symbols() {
            if Self::default_visibility(&k) == Visibility::Public {
                if let Some(value) = module.module.slots.get_slot(slot) {
                    self.set_private(k, Value::new_frozen(value))
                }
            }
        }
    }

    pub(crate) fn load_symbol<'v>(
        &'v self,
        module: &FrozenModule,
        symbol: &str,
    ) -> crate::Result<Value<'v>> {
        if Self::default_visibility(symbol) != Visibility::Public {
            return Err(crate::Error::new_other(
                EnvironmentError::CannotImportPrivateSymbol(symbol.to_owned()),
            ));
        }
        match module.get_any_visibility(symbol)? {
            (v, Visibility::Public) => Ok(v.owned_value(self.frozen_heap())),
            (_, Visibility::Private) => Err(crate::Error::new_other(
                EnvironmentError::ModuleSymbolIsNotExported(symbol.to_owned()),
            )),
        }
    }

    pub(crate) fn set_docstring(&self, docstring: String) {
        self.docstring.replace(Some(docstring));
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn add_eval_duration(&self, duration: Duration) {
        self.eval_duration.set(self.eval_duration.get() + duration);
    }

    pub(crate) fn trace<'v>(&'v self, tracer: &Tracer<'v>) {
        self.slots().get_slots_mut().trace(tracer);

        let extra_value = self.extra_value();
        if let Some(mut extra_value) = extra_value {
            extra_value.trace(tracer);
            self.set_extra_value(extra_value);
        }

        self.heap().trace_interner(tracer);
    }

    /// Field that can be used for any purpose you want.
    pub fn set_extra_value<'v>(&'v self, v: Value<'v>) {
        // Cast lifetime.
        let v = unsafe { transmute!(Value, Value, v) };
        self.extra_value.set(Some(v));
    }

    /// Set extra value, but fail if it's already set.
    pub fn set_extra_value_no_overwrite<'v>(&'v self, v: Value<'v>) -> anyhow::Result<()> {
        if let Some(existing) = self.extra_value() {
            return Err(ModuleError::ExtraValueAlreadySet(existing.get_type()).into());
        }
        self.set_extra_value(v);
        Ok(())
    }

    /// Field that can be used for any purpose you want.
    pub fn extra_value<'v>(&'v self) -> Option<Value<'v>> {
        // Cast lifetime.
        unsafe { transmute!(Option<Value>, Option<Value>, self.extra_value.get()) }
    }
}

#[test]
fn test_send_sync()
where
    FrozenModule: Send + Sync,
{
}

#[cfg(test)]
mod tests {
    use starlark_derive::starlark_module;

    use crate as starlark;
    use crate::environment::FrozenModule;
    use crate::environment::Globals;
    use crate::environment::GlobalsBuilder;
    use crate::environment::Module;
    use crate::eval::runtime::profile::mode::ProfileMode;
    use crate::eval::Evaluator;
    use crate::syntax::AstModule;
    use crate::syntax::Dialect;
    use crate::values::list::ListRef;

    #[test]
    fn test_gen_heap_summary_profile() {
        let module = Module::new();
        {
            let mut eval = Evaluator::new(&module);
            eval.enable_profile(&ProfileMode::HeapSummaryRetained)
                .unwrap();
            eval.eval_module(
                AstModule::parse(
                    "x.star",
                    r"
def f(x):
    return list([x])

x = f(1)
"
                    .to_owned(),
                    &Dialect::AllOptionsInternal,
                )
                .unwrap(),
                &Globals::standard(),
            )
            .unwrap();
        }
        let module = module.freeze().unwrap();
        let heap_summary = module.heap_profile().unwrap().gen().unwrap();
        // Smoke test.
        assert!(heap_summary.contains("\"x.star.f\""), "{:?}", heap_summary);
    }

    #[test]
    fn test_frozen_module_from_globals() {
        #[starlark_module]
        fn some_globals(globals: &mut GlobalsBuilder) {
            fn foo() -> anyhow::Result<i32> {
                Ok(17)
            }

            const BAR: Vec<String> = Vec::new();
        }

        let mut globals = GlobalsBuilder::new();
        some_globals(&mut globals);
        let globals = globals.build();

        let module = FrozenModule::from_globals(&globals).unwrap();
        assert_eq!("function", module.get("foo").unwrap().value().get_type());
        assert_eq!(
            0,
            ListRef::from_value(module.get("BAR").unwrap().value())
                .unwrap()
                .len()
        );
    }
}
