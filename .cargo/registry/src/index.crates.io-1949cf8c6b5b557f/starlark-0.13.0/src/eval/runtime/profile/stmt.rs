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

use std::cmp::Reverse;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Write;

use dupe::Dupe;
use starlark_map::StarlarkHasherBuilder;
use starlark_syntax::codemap::CodeMaps;
use starlark_syntax::internal_error;

use crate::codemap::CodeMap;
use crate::codemap::CodeMapId;
use crate::codemap::FileSpan;
use crate::codemap::FileSpanRef;
use crate::codemap::ResolvedFileSpan;
use crate::codemap::Span;
use crate::eval::runtime::profile::csv::CsvWriter;
use crate::eval::runtime::profile::data::ProfileData;
use crate::eval::runtime::profile::data::ProfileDataImpl;
use crate::eval::runtime::profile::instant::ProfilerInstant;
use crate::eval::runtime::profile::profiler_type::ProfilerType;
use crate::eval::runtime::small_duration::SmallDuration;
use crate::eval::ProfileMode;

pub(crate) struct StmtProfilerType;
pub(crate) struct CoverageProfileType;

impl ProfilerType for StmtProfilerType {
    type Data = StmtProfileData;
    const PROFILE_MODE: ProfileMode = ProfileMode::Statement;

    fn data_from_generic(profile_data: &ProfileDataImpl) -> Option<&Self::Data> {
        match profile_data {
            ProfileDataImpl::Statement(data) => Some(data),
            _ => None,
        }
    }

    fn data_to_generic(data: Self::Data) -> ProfileDataImpl {
        ProfileDataImpl::Statement(data)
    }

    fn merge_profiles_impl(profiles: &[&Self::Data]) -> starlark_syntax::Result<Self::Data> {
        Ok(StmtProfileData::merge(profiles))
    }
}

impl ProfilerType for CoverageProfileType {
    type Data = StmtProfileData;
    const PROFILE_MODE: ProfileMode = ProfileMode::Coverage;

    fn data_from_generic(profile_data: &ProfileDataImpl) -> Option<&Self::Data> {
        match profile_data {
            ProfileDataImpl::Coverage(data) => Some(data),
            _ => None,
        }
    }

    fn data_to_generic(data: Self::Data) -> ProfileDataImpl {
        ProfileDataImpl::Coverage(data)
    }

    fn merge_profiles_impl(profiles: &[&Self::Data]) -> starlark_syntax::Result<Self::Data> {
        Ok(StmtProfileData::merge(profiles))
    }
}

#[derive(Debug, thiserror::Error)]
enum StmtProfileError {
    #[error("Statement or coverage profiling is not enabled")]
    NotEnabled,
}

pub(crate) struct StmtProfile(
    // Box because when profiling is not enabled, we want this to be small and cheap
    Option<Box<StmtProfileState>>,
);

#[derive(Clone)]
struct Last {
    file: CodeMapId,
    span: Span,
    start: ProfilerInstant,
}

// So we don't need a special case for the first time around,
// we have a special FileId of empty that we ignore when printing
#[derive(Clone)]
struct StmtProfileState {
    files: CodeMaps,
    stmts: HashMap<(CodeMapId, Span), (usize, SmallDuration), StarlarkHasherBuilder>,
    last: Option<Last>,
}

/// Result of running statement or coverage profiler.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct StmtProfileData {
    stmts: HashMap<FileSpan, (usize, SmallDuration), StarlarkHasherBuilder>,
}

impl StmtProfileState {
    fn new() -> Self {
        StmtProfileState {
            files: CodeMaps::default(),
            stmts: HashMap::default(),
            last: None,
        }
    }

    // Add the data from last_span into the entries
    fn add_last(&mut self, now: ProfilerInstant) {
        if let Some(last) = &self.last {
            let time = now - last.start;
            match self.stmts.entry((last.file, last.span)) {
                Entry::Occupied(mut x) => {
                    let v = x.get_mut();
                    v.0 += 1;
                    v.1 += SmallDuration::from_duration(time);
                }
                Entry::Vacant(x) => {
                    x.insert((1, SmallDuration::from_duration(time)));
                }
            }
        }
    }

    fn before_stmt(&mut self, span: Span, codemap: &CodeMap) {
        let now = ProfilerInstant::now();
        self.add_last(now);
        match &self.last {
            None => self.files.add(codemap),
            Some(last) => {
                if last.file != codemap.id() {
                    self.files.add(codemap);
                }
            }
        }
        self.last = Some(Last {
            file: codemap.id(),
            span,
            start: now,
        });
    }

    fn finish(&self) -> crate::Result<StmtProfileData> {
        // The statement that was running last won't have been properly updated.
        // However, at this point, we have probably run some post-execution code,
        // so it probably wouldn't have a "fair" timing anyway.
        // We do our best though, and give it a time of now.
        // Clone first, since we don't want to impact the real timing with our odd
        // final execution finish.
        let now = ProfilerInstant::now();
        let mut data = self.clone();
        data.add_last(now);

        Ok(StmtProfileData {
            stmts: data
                .stmts
                .iter()
                .map(|((file, span), v)| {
                    Ok::<_, crate::Error>((
                        FileSpan {
                            file: data
                                .files
                                .get(*file)
                                .ok_or_else(|| internal_error!("no file corresponding to file id"))?
                                .dupe(),
                            span: *span,
                        },
                        *v,
                    ))
                })
                .collect::<crate::Result<_>>()?,
        })
    }
}

impl StmtProfileData {
    pub(crate) fn write_to_string(&self) -> String {
        struct Item {
            span: FileSpan,
            time: SmallDuration,
            count: usize,
        }
        // There should be one EMPTY span entry
        let mut items = Vec::with_capacity(self.stmts.len() - 1);
        let mut total_time = SmallDuration::default();
        let mut total_count = 0;
        for (file_span, &(count, time)) in &self.stmts {
            // EMPTY represents the first time special-case
            if file_span.file.id() != CodeMapId::EMPTY {
                total_time += time;
                total_count += count;
                items.push(Item {
                    span: file_span.dupe(),
                    time,
                    count,
                })
            }
        }

        items.sort_by_key(|x| (Reverse(x.time), Reverse(x.count), x.span.dupe()));

        let mut csv = CsvWriter::new(["File", "Span", "Duration(s)", "Count"]);
        csv.write_value("TOTAL");
        csv.write_value("");
        csv.write_value(total_time);
        csv.write_value(total_count);
        csv.finish_row();

        for x in items {
            csv.write_value(x.span.file.filename());
            csv.write_display(x.span.file.resolve_span(x.span.span));
            csv.write_value(x.time);
            csv.write_value(x.count);
            csv.finish_row();
        }

        csv.finish()
    }

    pub(crate) fn write_coverage(&self) -> String {
        let mut s = String::new();
        let mut keys: Vec<_> = self
            .stmts
            .keys()
            .filter(|file_span| file_span.file.id() != CodeMapId::EMPTY)
            .map(|file_span| file_span.resolve())
            .collect();
        keys.sort();
        for key in keys {
            writeln!(s, "{}", key).unwrap();
        }
        s
    }

    fn coverage(&self) -> HashSet<ResolvedFileSpan> {
        self.stmts
            .keys()
            .filter(|file_span| file_span.file.id() != CodeMapId::EMPTY)
            .map(|file_span| file_span.resolve())
            .collect()
    }

    fn merge(profiles: &[&StmtProfileData]) -> StmtProfileData {
        let mut result = StmtProfileData::default();
        let StmtProfileData { stmts } = &mut result;
        for profile in profiles {
            for (file_span, (count, time)) in &profile.stmts {
                match stmts.entry(file_span.dupe()) {
                    Entry::Occupied(mut x) => {
                        let v = x.get_mut();
                        v.0 += count;
                        v.1 += *time;
                    }
                    Entry::Vacant(x) => {
                        x.insert((*count, *time));
                    }
                }
            }
        }
        result
    }
}

impl StmtProfile {
    pub(crate) fn new() -> Self {
        Self(None)
    }

    pub(crate) fn enable(&mut self) {
        self.0 = Some(Box::new(StmtProfileState::new()))
    }

    pub(crate) fn before_stmt(&mut self, span: FileSpanRef) {
        if let Some(data) = &mut self.0 {
            data.before_stmt(span.span, span.file)
        }
    }

    // None = not applicable because not enabled
    pub(crate) fn gen(&self) -> crate::Result<ProfileData> {
        match &self.0 {
            Some(data) => Ok(ProfileData {
                profile: ProfileDataImpl::Statement(data.finish()?),
            }),
            None => Err(crate::Error::new_other(StmtProfileError::NotEnabled)),
        }
    }

    pub(crate) fn coverage(&self) -> crate::Result<HashSet<ResolvedFileSpan>> {
        Ok(self
            .0
            .as_ref()
            .ok_or_else(|| crate::Error::new_other(StmtProfileError::NotEnabled))?
            .finish()?
            .coverage())
    }

    pub(crate) fn gen_coverage(&self) -> crate::Result<ProfileData> {
        match &self.0 {
            Some(data) => Ok(ProfileData {
                profile: ProfileDataImpl::Coverage(data.finish()?),
            }),
            None => Err(crate::Error::new_other(StmtProfileError::NotEnabled)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use starlark_syntax::codemap::CodeMap;
    use starlark_syntax::codemap::CodeMaps;
    use starlark_syntax::codemap::FileSpan;
    use starlark_syntax::codemap::FileSpanRef;
    use starlark_syntax::codemap::Pos;
    use starlark_syntax::codemap::Span;

    use crate::assert::test_functions;
    use crate::environment::GlobalsBuilder;
    use crate::environment::Module;
    use crate::eval::runtime::profile::data::ProfileDataImpl;
    use crate::eval::runtime::profile::instant::ProfilerInstant;
    use crate::eval::runtime::profile::mode::ProfileMode;
    use crate::eval::runtime::profile::stmt::StmtProfile;
    use crate::eval::runtime::profile::stmt::StmtProfileData;
    use crate::eval::runtime::small_duration::SmallDuration;
    use crate::eval::Evaluator;
    use crate::eval::ProfileData;
    use crate::syntax::AstModule;
    use crate::syntax::Dialect;

    #[test]
    fn test_coverage() {
        let module = Module::new();
        let mut eval = Evaluator::new(&module);

        let module = AstModule::parse(
            "cov.star",
            r#"
def xx(x):
    return noop(x)

xx(*[1])
xx(*[2])
"#
            .to_owned(),
            &Dialect::AllOptionsInternal,
        )
        .unwrap();
        eval.enable_profile(&ProfileMode::Coverage).unwrap();
        let mut globals = GlobalsBuilder::standard();
        test_functions(&mut globals);
        eval.eval_module(module, &globals.build()).unwrap();

        let mut coverage: Vec<String> = eval
            .coverage()
            .unwrap()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        coverage.sort();
        assert_eq!(
            [
                "cov.star:2:1-5:1",
                "cov.star:3:5-19",
                "cov.star:5:1-9",
                "cov.star:6:1-9"
            ]
            .as_slice(),
            coverage
        );
    }

    #[test]
    fn test_merge() {
        let x = CodeMap::new("x.star".to_owned(), "def a(): pass".to_owned());
        let y = CodeMap::new("y.star".to_owned(), "def b(): pass".to_owned());
        let z = CodeMap::new("z.star".to_owned(), "def c(): pass".to_owned());

        let mut all_files = CodeMaps::default();
        all_files.add(&x);
        all_files.add(&y);
        all_files.add(&z);

        let mut a = StmtProfile::new();
        a.enable();
        a.before_stmt(FileSpanRef {
            file: &x,
            span: Span::new(Pos::new(1), Pos::new(2)),
        });
        a.before_stmt(FileSpanRef {
            file: &y,
            span: Span::new(Pos::new(2), Pos::new(4)),
        });
        let a = a.gen().unwrap();

        let mut b = StmtProfile::new();
        b.enable();
        b.before_stmt(FileSpanRef {
            file: &y,
            span: Span::new(Pos::new(2), Pos::new(4)),
        });
        b.before_stmt(FileSpanRef {
            file: &z,
            span: Span::new(Pos::new(3), Pos::new(5)),
        });
        let b = b.gen().unwrap();

        let ProfileDataImpl::Statement(merged) = ProfileData::merge([&a, &b]).unwrap().profile
        else {
            panic!("Expected statement profile data");
        };

        assert_eq!(
            StmtProfileData {
                stmts: HashMap::from_iter([
                    (
                        FileSpan {
                            file: x,
                            span: Span::new(Pos::new(1), Pos::new(2))
                        },
                        (
                            1,
                            SmallDuration::from_millis(ProfilerInstant::TEST_TICK_MILLIS)
                        )
                    ),
                    (
                        FileSpan {
                            file: y,
                            span: Span::new(Pos::new(2), Pos::new(4))
                        },
                        (
                            2,
                            SmallDuration::from_millis(ProfilerInstant::TEST_TICK_MILLIS * 2)
                        )
                    ),
                    (
                        FileSpan {
                            file: z,
                            span: Span::new(Pos::new(3), Pos::new(5))
                        },
                        (
                            1,
                            SmallDuration::from_millis(ProfilerInstant::TEST_TICK_MILLIS)
                        )
                    ),
                ]),
            },
            merged
        );
    }
}
