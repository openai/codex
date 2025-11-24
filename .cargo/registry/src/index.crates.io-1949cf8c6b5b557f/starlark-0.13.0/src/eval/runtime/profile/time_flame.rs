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

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::slice;

use dupe::Dupe;
use starlark_map::StarlarkHasherBuilder;
use starlark_syntax::slice_vec_ext::SliceExt;

use crate as starlark;
use crate::eval::runtime::profile::data::ProfileData;
use crate::eval::runtime::profile::data::ProfileDataImpl;
use crate::eval::runtime::profile::flamegraph::FlameGraphData;
use crate::eval::runtime::profile::flamegraph::FlameGraphNode;
use crate::eval::runtime::profile::instant::ProfilerInstant;
use crate::eval::runtime::profile::profiler_type::ProfilerType;
use crate::eval::runtime::small_duration::SmallDuration;
use crate::eval::ProfileMode;
use crate::util::arc_str::ArcStr;
use crate::values::layout::pointer::RawPointer;
use crate::values::FrozenValue;
use crate::values::Trace;
use crate::values::Tracer;
use crate::values::Value;

pub(crate) struct TimeFlameProfilerType;

impl ProfilerType for TimeFlameProfilerType {
    type Data = FlameGraphData;
    const PROFILE_MODE: ProfileMode = ProfileMode::TimeFlame;

    fn data_from_generic(profile_data: &ProfileDataImpl) -> Option<&Self::Data> {
        match profile_data {
            ProfileDataImpl::TimeFlameProfile(data) => Some(data),
            _ => None,
        }
    }

    fn data_to_generic(data: Self::Data) -> ProfileDataImpl {
        ProfileDataImpl::TimeFlameProfile(data)
    }

    fn merge_profiles_impl(profiles: &[&Self::Data]) -> starlark_syntax::Result<Self::Data> {
        Ok(FlameGraphData::merge(profiles.iter().copied()))
    }
}

#[derive(Debug, thiserror::Error)]
enum FlameProfileError {
    #[error("Flame profile not enabled")]
    NotEnabled,
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Dupe)]
struct MutableValueId(usize);
#[derive(Hash, PartialEq, Eq, Clone, Copy, Dupe)]
struct FrozenValueId(usize);

/// Index into FlameData.values
#[derive(Hash, PartialEq, Eq, Clone, Copy, Dupe)]
enum ValueId {
    // This struct is two words, can be made one.
    Mutable(MutableValueId),
    Frozen(FrozenValueId),
}

impl ValueId {
    fn lookup<'a, T>(self, mutable: &'a [T], frozen: &'a [T]) -> &'a T {
        match self {
            ValueId::Mutable(x) => &mutable[x.0],
            ValueId::Frozen(x) => &frozen[x.0],
        }
    }
}

/// Bimap between `Value` and `ValueId`.
/// In order to optimise GC (which otherwise quickly becomes O(n^2)) we have to
/// dedupe the values, so store them in `values`, with a fast map to get them in `map`.
/// Whenever we GC, regenerate map.
#[derive(Default)]
struct ValueIndex<'v> {
    /// Map from `MutableValueId` to `Value`.
    mutable_values: Vec<Value<'v>>,
    /// Map from `FrozenValueId` to `Value`.
    frozen_values: Vec<FrozenValue>,
    /// Map from `Value` to `MutableValueId`.
    mutable_map: HashMap<RawPointer, MutableValueId, StarlarkHasherBuilder>,
    /// Map from `Value` to `FrozenValueId`.
    frozen_map: HashMap<RawPointer, FrozenValueId, StarlarkHasherBuilder>,
}

unsafe impl<'v> Trace<'v> for ValueIndex<'v> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        // We only need to trace mutable values.
        self.mutable_values.trace(tracer);
        // Have to rebuild the map, as its keyed by ValuePtr which changes on GC
        self.mutable_map.clear();
        for (i, x) in self.mutable_values.iter().enumerate() {
            self.mutable_map.insert(x.ptr_value(), MutableValueId(i));
        }
    }
}

impl<'v> ValueIndex<'v> {
    /// Map `Value` to `ValueId`.
    fn index(&mut self, value: Value<'v>) -> ValueId {
        match value.unpack_frozen() {
            Some(frozen) => match self.frozen_map.entry(frozen.ptr_value()) {
                Entry::Occupied(e) => ValueId::Frozen(*e.get()),
                Entry::Vacant(e) => {
                    let res = FrozenValueId(self.frozen_values.len());
                    self.frozen_values.push(frozen);
                    e.insert(res);
                    ValueId::Frozen(res)
                }
            },
            None => match self.mutable_map.entry(value.ptr_value()) {
                Entry::Occupied(e) => ValueId::Mutable(*e.get()),
                Entry::Vacant(e) => {
                    let res = MutableValueId(self.mutable_values.len());
                    self.mutable_values.push(value);
                    e.insert(res);
                    ValueId::Mutable(res)
                }
            },
        }
    }
}

enum Frame {
    /// Entry recorded when we enter a function.
    Push(ValueId),
    /// Entry recorded when we exit a function.
    Pop,
}

#[derive(Trace)]
pub(crate) struct TimeFlameProfile<'v>(
    /// `Some` means enabled.
    Option<Box<FlameData<'v>>>,
);

#[derive(Default, Trace)]
struct FlameData<'v> {
    /// All events in the profile, i.e. function entry or exit with timestamp.
    frames: Vec<(Frame, ProfilerInstant)>,
    index: ValueIndex<'v>,
}

struct Stacks<'a> {
    name: &'a str,
    time: SmallDuration,
    children: HashMap<ValueId, Stacks<'a>, StarlarkHasherBuilder>,
}

impl<'a> Stacks<'a> {
    fn blank(name: &'a str) -> Self {
        Stacks {
            name,
            time: SmallDuration::default(),
            children: HashMap::with_hasher(StarlarkHasherBuilder),
        }
    }

    fn new(
        mutable_names: &'a [String],
        frozen_names: &'a [String],
        frames: &[(Frame, ProfilerInstant)],
    ) -> Self {
        let mut res = Stacks::blank("root");
        let Some(mut last_time) = frames.first().map(|x| x.1) else {
            return res;
        };
        res.add(
            mutable_names,
            frozen_names,
            &mut frames.iter(),
            &mut last_time,
        );
        res
    }

    fn add(
        &mut self,
        mutable_names: &'a [String],
        frozen_names: &'a [String],
        frames: &mut slice::Iter<(Frame, ProfilerInstant)>,
        last_time: &mut ProfilerInstant,
    ) {
        while let Some((frame, time)) = frames.next() {
            self.time += time.duration_since(*last_time);
            *last_time = *time;
            match frame {
                Frame::Pop => return,
                Frame::Push(i) => match self.children.entry(*i) {
                    Entry::Occupied(mut e) => {
                        e.get_mut()
                            .add(mutable_names, frozen_names, frames, last_time)
                    }
                    Entry::Vacant(e) => e
                        .insert(Stacks::blank(
                            i.lookup(mutable_names, frozen_names).as_str(),
                        ))
                        .add(mutable_names, frozen_names, frames, last_time),
                },
            }
        }
    }

    fn render_with_buffer(&self, node: &mut FlameGraphNode) {
        let node = node.child(ArcStr::from(self.name));
        let count = self.time.to_duration().as_millis();
        if count > 0 {
            node.add(count as u64);
        }
        for x in self.children.values() {
            x.render_with_buffer(node);
        }
    }

    fn render(&self) -> FlameGraphData {
        let mut data = FlameGraphData::default();
        self.render_with_buffer(data.root());
        data
    }
}

impl<'v> TimeFlameProfile<'v> {
    pub(crate) fn new() -> Self {
        Self(None)
    }

    pub(crate) fn enable(&mut self) {
        self.0 = Some(Box::default());
    }

    pub(crate) fn record_call_enter(&mut self, function: Value<'v>) {
        if let Some(x) = &mut self.0 {
            let ind = x.index.index(function);
            x.frames.push((Frame::Push(ind), ProfilerInstant::now()))
        }
    }

    pub(crate) fn record_call_exit(&mut self) {
        if let Some(x) = &mut self.0 {
            x.frames.push((Frame::Pop, ProfilerInstant::now()))
        }
    }

    // We could expose profile on the Heap, but it's an implementation detail that it works here.
    pub(crate) fn gen(&self) -> crate::Result<ProfileData> {
        match &self.0 {
            None => Err(crate::Error::new_other(FlameProfileError::NotEnabled)),
            Some(x) => Ok(Self::gen_profile(x)),
        }
    }

    fn gen_profile(x: &FlameData) -> ProfileData {
        // Need to write out lines which look like:
        // root;calls1;calls2 1
        // All the numbers at the end must be whole numbers (we use milliseconds)
        let mutable_names = x.index.mutable_values.map(|x| x.to_repr());
        let frozen_names = x.index.frozen_values.map(|x| x.to_value().to_repr());
        ProfileData {
            profile: ProfileDataImpl::TimeFlameProfile(
                Stacks::new(&mutable_names, &frozen_names, &x.frames).render(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::thread;
    use std::time::Duration;

    use anyhow::Context;
    use starlark_derive::starlark_module;

    use crate as starlark;
    use crate::assert::Assert;
    use crate::environment::Globals;
    use crate::environment::GlobalsBuilder;
    use crate::environment::Module;
    use crate::eval::runtime::file_loader::ReturnOwnedFileLoader;
    use crate::eval::runtime::profile::mode::ProfileMode;
    use crate::eval::Evaluator;
    use crate::syntax::AstModule;
    use crate::syntax::Dialect;
    use crate::values::none::NoneType;

    #[test]
    fn test_time_flame_works_inside_frozen_module() {
        #[starlark_module]
        fn register_sleep(globals: &mut GlobalsBuilder) {
            fn sleep() -> anyhow::Result<NoneType> {
                thread::sleep(Duration::from_millis(2));
                Ok(NoneType)
            }
        }

        let mut a = Assert::new();
        a.globals_add(register_sleep);
        let a_bzl = a.pass_module(
            r#"
def foo():
    for i in range(5):
        # Must sleep otherwise time flame will round the duration to zero and erase it.
        sleep()
    "#,
        );

        let modules = HashMap::from_iter([("a.bzl".to_owned(), a_bzl)]);
        let loader = ReturnOwnedFileLoader { modules };

        let module = Module::new();
        let mut eval = Evaluator::new(&module);
        eval.enable_profile(&ProfileMode::TimeFlame).unwrap();
        eval.set_loader(&loader);
        eval.eval_module(
            AstModule::parse(
                "x.star",
                r#"
load("a.bzl", "foo")

def bar():
    for i in range(10):
        foo()

bar()
"#
                .to_owned(),
                &Dialect::Standard,
            )
            .unwrap(),
            &Globals::standard(),
        )
        .unwrap();

        let profile = eval.gen_profile().unwrap().gen().unwrap();
        let the_line = profile
            .lines()
            .find(|l| l.contains("foo"))
            .with_context(|| {
                format!(
                    "There must be a line with `foo` in the profile: {:?}",
                    profile
                )
            })
            .unwrap();
        assert!(
            the_line.contains("bar"),
            "Profile must contain a line `bar.*foo`: {:?}",
            profile
        );
    }
}
