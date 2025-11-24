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

use dupe::Dupe;
use starlark_map::small_map::SmallMap;

use crate::eval::runtime::profile::csv::CsvWriter;
use crate::eval::runtime::small_duration::SmallDuration;
use crate::util::arc_str::ArcStr;
use crate::values::layout::heap::profile::aggregated::AggregateHeapProfileInfo;
use crate::values::layout::heap::profile::aggregated::StackFrame;
use crate::values::layout::heap::profile::alloc_counts::AllocCounts;
use crate::values::layout::heap::profile::string_index::StringId;
use crate::values::layout::heap::profile::string_index::StringIndex;

/// Information relating to a function.
#[derive(Default, Debug, Clone)]
pub(crate) struct FuncInfo {
    /// Number of times this function was called
    pub calls: usize,
    /// Who called this function (and how many times each)
    pub callers: SmallMap<ArcStr, usize>,
    /// Time spent directly in this function
    pub time: SmallDuration,
    /// Time spent directly in this function and recursive functions.
    pub time_rec: SmallDuration,
    /// Allocations made by this function
    pub alloc: SmallMap<&'static str, AllocCounts>,
}

impl FuncInfo {
    pub(crate) fn merge<'a>(xs: impl Iterator<Item = &'a Self>) -> Self {
        let mut result = Self::default();
        for x in xs {
            result.calls += x.calls;
            result.time += x.time;
            for (k, v) in x.alloc.iter() {
                *result.alloc.entry(k).or_default() += *v;
            }
        }
        // Recursive time doesn't accumulate nicely, the time is the right value
        result.time_rec = result.time;
        result
    }

    /// Total number of allocations made by this function.
    fn alloc_count(&self) -> usize {
        self.alloc.values().map(|x| x.count).sum()
    }

    /// Total number of bytes allocated by this function.
    fn alloc_bytes(&self) -> usize {
        self.alloc.values().map(|x| x.bytes).sum()
    }
}

/// We morally have two pieces of information:
/// 1. Information about each function.
/// 2. The call stack.
///
/// However, we are always updating the top of the call stack,
/// so pull out top_stack/top_info as a cache.
pub(crate) struct HeapSummaryByFunction {
    /// Information about all functions.
    info: SmallMap<ArcStr, FuncInfo>,
}

impl HeapSummaryByFunction {
    pub(crate) fn init(stacks: &AggregateHeapProfileInfo) -> HeapSummaryByFunction {
        let mut info = HeapSummaryByFunction {
            info: SmallMap::new(),
        };
        info.init_children(&stacks.root, &ArcStr::new_static("(root)"), &stacks.strings);
        info
    }

    fn init_children(
        &mut self,
        frame: &StackFrame,
        name: &ArcStr,
        strings: &StringIndex,
    ) -> SmallDuration {
        let mut time_rec = SmallDuration::default();
        for (func, child) in &frame.callees {
            time_rec += self.init_child(*func, child, name.dupe(), strings);
        }
        time_rec
    }

    fn init_child(
        &mut self,
        func: StringId,
        frame: &StackFrame,
        caller: ArcStr,
        strings: &StringIndex,
    ) -> SmallDuration {
        let func_str = strings.get(func);
        self.info.entry(func_str.dupe()).or_default().time += frame.time_x2;
        self.info.entry(func_str.dupe()).or_default().calls += frame.calls_x2 as usize;
        *self
            .info
            .entry(func_str.dupe())
            .or_default()
            .callers
            .entry(caller)
            .or_insert(0) += 1;
        for (t, allocs) in &frame.allocs.summary {
            *self
                .info
                .entry(func_str.dupe())
                .or_default()
                .alloc
                .entry(t)
                .or_default() += *allocs;
        }

        let time_rec = frame.time_x2 + self.init_children(frame, func_str, strings);
        self.info.entry(func_str.dupe()).or_default().time_rec += time_rec;
        time_rec
    }

    fn totals(&self) -> FuncInfo {
        FuncInfo::merge(self.info.values())
    }

    pub(crate) fn info(&self) -> Vec<(&ArcStr, &FuncInfo)> {
        self.info.iter().collect::<Vec<_>>()
    }

    pub(crate) fn gen_csv(&self) -> String {
        // Add a totals column
        let totals = self.totals();
        let mut columns: Vec<(&'static str, AllocCounts)> =
            totals.alloc.iter().map(|(k, v)| (*k, *v)).collect();

        columns.sort_by_key(|x| -(x.1.count as isize));

        let mut info = self.info();
        info.sort_by_key(|x| -(x.1.time.nanos as i128));

        enum RowKind {
            Total,
            Func,
        }

        let totals_str = ArcStr::new_static("TOTALS");
        let info = [(&totals_str, &totals, RowKind::Total)]
            .into_iter()
            .chain(info.into_iter().map(|(k, v)| (k, v, RowKind::Func)));

        let mut csv = CsvWriter::new(
            [
                "Function",
                "Time(s)",
                "TimeRec(s)",
                "Calls",
                "Callers",
                "TopCaller",
                "TopCallerCount",
                "Allocs",
                "AllocBytes",
            ]
            .iter()
            .copied()
            .chain(columns.iter().map(|c| c.0)),
        );
        for (rowname, info, _row_kind) in info {
            let blank = ArcStr::new_static("");
            let callers = info
                .callers
                .iter()
                .max_by_key(|x| x.1)
                .unwrap_or((&blank, &0));
            assert!(
                info.calls % 2 == 0,
                "we enter calls twice, for drop and non_drop"
            );
            // We divide calls and time by two
            // because we could calls twice: for drop and non-drop bumps.
            csv.write_value(&**rowname);
            csv.write_value(info.time / 2);
            csv.write_value(info.time_rec / 2);
            csv.write_value(info.calls / 2);
            csv.write_value(info.callers.len());
            csv.write_value(callers.0.as_str());
            csv.write_value(callers.1);
            csv.write_value(info.alloc_count());
            csv.write_value(info.alloc_bytes());
            for c in &columns {
                csv.write_value(info.alloc.get(c.0).unwrap_or(&AllocCounts::default()).count);
            }
            csv.finish_row();
        }
        csv.finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::environment::Globals;
    use crate::environment::Module;
    use crate::eval::runtime::profile::mode::ProfileMode;
    use crate::eval::Evaluator;
    use crate::syntax::AstModule;
    use crate::syntax::Dialect;
    use crate::values::layout::heap::profile::aggregated::AggregateHeapProfileInfo;
    use crate::values::layout::heap::profile::summary_by_function::FuncInfo;
    use crate::values::layout::heap::profile::summary_by_function::HeapSummaryByFunction;

    // Test data is collected from both drop and non-drop heaps.
    #[test]
    fn drop_non_drop() {
        let ast = AstModule::parse(
            "x.star",
            "\
_ignore = {1: 2}       # allocate a dict in drop
_ignore = str([1])     # allocate a string in non_drop
        "
            .to_owned(),
            &Dialect::AllOptionsInternal,
        )
        .unwrap();

        let globals = Globals::standard();
        let module = Module::new();
        let mut eval = Evaluator::new(&module);
        eval.enable_profile(&ProfileMode::HeapSummaryAllocated)
            .unwrap();

        eval.eval_module(ast, &globals).unwrap();

        let stacks = AggregateHeapProfileInfo::collect(eval.heap(), None);

        let info = HeapSummaryByFunction::init(&stacks);

        // Run the assertions.
        info.gen_csv();

        let total = FuncInfo::merge(info.info.values());
        // from non-drop heap
        assert_eq!(total.alloc.get("string").unwrap().count, 1);
        // from drop heap
        assert_eq!(total.alloc.get("dict").unwrap().count, 1);
    }
}
