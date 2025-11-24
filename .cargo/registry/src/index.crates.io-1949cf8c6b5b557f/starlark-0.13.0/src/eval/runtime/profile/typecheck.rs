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

//! Runtime typecheck profile.

use std::collections::HashMap;
use std::time::Duration;

use dupe::Dupe;
use starlark_map::Hashed;
use starlark_map::StarlarkHasherBuilder;

use crate::collections::SmallMap;
use crate::eval::runtime::profile::csv::CsvWriter;
use crate::eval::runtime::profile::data::ProfileData;
use crate::eval::runtime::profile::data::ProfileDataImpl;
use crate::eval::runtime::profile::profiler_type::ProfilerType;
use crate::eval::runtime::small_duration::SmallDuration;
use crate::eval::ProfileMode;
use crate::util::arc_str::ArcStr;
use crate::values::FrozenStringValue;

pub(crate) struct TypecheckProfilerType;

impl ProfilerType for TypecheckProfilerType {
    type Data = TypecheckProfileData;
    const PROFILE_MODE: ProfileMode = ProfileMode::Typecheck;

    fn data_from_generic(profile_data: &ProfileDataImpl) -> Option<&Self::Data> {
        match profile_data {
            ProfileDataImpl::Typecheck(data) => Some(data),
            _ => None,
        }
    }

    fn data_to_generic(data: Self::Data) -> ProfileDataImpl {
        ProfileDataImpl::Typecheck(data)
    }

    fn merge_profiles_impl(profiles: &[&Self::Data]) -> starlark_syntax::Result<Self::Data> {
        let mut by_function = SmallMap::new();
        for profile in profiles {
            for (name, time) in &profile.by_function {
                *by_function.entry(name.dupe()).or_default() += *time;
            }
        }
        Ok(TypecheckProfileData { by_function })
    }
}

#[derive(Debug, thiserror::Error)]
enum TypecheckProfileError {
    #[error("Typecheck profile not enabled")]
    NotEnabled,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub(crate) struct TypecheckProfileData {
    by_function: SmallMap<ArcStr, SmallDuration>,
}

#[derive(Default, Debug)]
pub(crate) struct TypecheckProfile {
    pub(crate) enabled: bool,
    by_function: HashMap<Hashed<FrozenStringValue>, SmallDuration, StarlarkHasherBuilder>,
}

impl TypecheckProfileData {
    pub(crate) fn gen_csv(&self) -> String {
        let total_time = self.by_function.values().sum::<SmallDuration>();

        let mut w = CsvWriter::new(["Function", "Time (s)"]);
        w.write_display("TOTAL");
        w.write_value(total_time);
        w.finish_row();

        let mut by_function = Vec::from_iter(&self.by_function);
        by_function.sort_by_key(|(name, t)| (u64::MAX - t.nanos, *name));

        for (name, t) in by_function {
            w.write_display(name.as_str());
            w.write_value(t);
            w.finish_row();
        }

        w.finish()
    }
}

impl TypecheckProfile {
    pub(crate) fn add(&mut self, function: FrozenStringValue, time: Duration) {
        assert!(self.enabled);
        *self.by_function.entry(function.get_hashed()).or_default() += time;
    }

    pub(crate) fn gen(&self) -> crate::Result<ProfileData> {
        if !self.enabled {
            return Err(crate::Error::new_other(TypecheckProfileError::NotEnabled));
        }
        Ok(ProfileData {
            profile: ProfileDataImpl::Typecheck(TypecheckProfileData {
                by_function: self
                    .by_function
                    .iter()
                    .map(|(k, v)| (ArcStr::from(k.as_str()), *v))
                    .collect(),
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use starlark_map::small_map::SmallMap;

    use crate::environment::Globals;
    use crate::environment::Module;
    use crate::eval::runtime::profile::mode::ProfileMode;
    use crate::eval::runtime::profile::profiler_type::ProfilerType;
    use crate::eval::runtime::profile::typecheck::TypecheckProfileData;
    use crate::eval::runtime::profile::typecheck::TypecheckProfilerType;
    use crate::eval::runtime::small_duration::SmallDuration;
    use crate::eval::Evaluator;
    use crate::syntax::AstModule;
    use crate::syntax::Dialect;
    use crate::util::arc_str::ArcStr;

    #[test]
    fn test_typecheck_profile() -> crate::Result<()> {
        let module = Module::new();
        let mut eval = Evaluator::new(&module);
        let program = r#"
def f(s: str):
    return int(s)

def g():
    for i in range(0, 1000):
        f(str(i))

g()
"#;
        let program = AstModule::parse(
            "test.star",
            program.to_owned(),
            &Dialect::AllOptionsInternal,
        )?;
        eval.enable_profile(&ProfileMode::Typecheck)?;
        eval.eval_module(program, &Globals::extended_internal())?;

        let csv = eval.typecheck_profile.gen()?.gen()?;
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!("Function,Time (s)", lines[0]);
        assert!(lines[1].starts_with("\"TOTAL\","), "{:?}", lines[1]);
        assert!(lines[2].starts_with("\"f\","), "{:?}", lines[2]);
        assert_eq!(3, lines.len());

        Ok(())
    }

    #[test]
    fn test_typecheck_profile_merge() {
        let a = TypecheckProfileData {
            by_function: SmallMap::from_iter([
                (ArcStr::from("a"), SmallDuration::from_millis(10)),
                (ArcStr::from("b"), SmallDuration::from_millis(20)),
            ]),
        };
        let b = TypecheckProfileData {
            by_function: SmallMap::from_iter([
                (ArcStr::from("b"), SmallDuration::from_millis(300)),
                (ArcStr::from("c"), SmallDuration::from_millis(400)),
            ]),
        };
        let merged = TypecheckProfilerType::merge_profiles_impl(&[&a, &b]).unwrap();

        let expected = TypecheckProfileData {
            by_function: SmallMap::from_iter([
                (ArcStr::from("a"), SmallDuration::from_millis(10)),
                (ArcStr::from("b"), SmallDuration::from_millis(320)),
                (ArcStr::from("c"), SmallDuration::from_millis(400)),
            ]),
        };
        assert_eq!(expected, merged);
    }
}
