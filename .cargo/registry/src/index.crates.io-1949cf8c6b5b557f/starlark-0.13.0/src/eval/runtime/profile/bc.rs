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

//! Bytecode profiler.

use std::collections::HashMap;
use std::iter::Sum;
use std::mem;
use std::ops::AddAssign;

use dupe::Dupe;

use crate::eval::bc::opcode::BcOpcode;
use crate::eval::runtime::profile::csv::CsvWriter;
use crate::eval::runtime::profile::data::ProfileDataImpl;
use crate::eval::runtime::profile::profiler_type::ProfilerType;
use crate::eval::ProfileData;
use crate::eval::ProfileMode;

pub(crate) struct BcProfilerType;
pub(crate) struct BcPairsProfilerType;

impl ProfilerType for BcProfilerType {
    type Data = Box<BcProfileData>;
    const PROFILE_MODE: ProfileMode = ProfileMode::Bytecode;

    fn data_from_generic(profile_data: &ProfileDataImpl) -> Option<&Self::Data> {
        match profile_data {
            ProfileDataImpl::Bc(bc) => Some(bc),
            _ => None,
        }
    }

    fn data_to_generic(data: Self::Data) -> ProfileDataImpl {
        ProfileDataImpl::Bc(data)
    }

    fn merge_profiles_impl(profiles: &[&Self::Data]) -> starlark_syntax::Result<Self::Data> {
        Ok(Box::new(BcProfileData::merge(
            profiles.iter().map(|x| &***x),
        )))
    }
}

impl ProfilerType for BcPairsProfilerType {
    type Data = BcPairsProfileData;
    const PROFILE_MODE: ProfileMode = ProfileMode::BytecodePairs;

    fn data_from_generic(profile_data: &ProfileDataImpl) -> Option<&Self::Data> {
        match profile_data {
            ProfileDataImpl::BcPairs(bc) => Some(bc),
            _ => None,
        }
    }

    fn data_to_generic(data: Self::Data) -> ProfileDataImpl {
        ProfileDataImpl::BcPairs(data)
    }

    fn merge_profiles_impl(profiles: &[&Self::Data]) -> starlark_syntax::Result<Self::Data> {
        Ok(BcPairsProfileData::merge(profiles.iter().map(|x| &**x)))
    }
}

#[derive(Debug, thiserror::Error)]
enum BcProfileError {
    #[error("Can't call `write_bc_profile` unless you first call `enable_bc_profile`.")]
    BcProfilingNotEnabled,
}

#[derive(Default, Clone, Dupe, Copy, Debug)]
struct BcInstrStat {
    count: u64,
}

impl<'a> Sum<&'a BcInstrStat> for BcInstrStat {
    fn sum<I: Iterator<Item = &'a BcInstrStat>>(iter: I) -> Self {
        let mut sum = BcInstrStat::default();
        for BcInstrStat { count } in iter {
            sum.count += *count;
        }
        sum
    }
}

impl<'a> AddAssign<&'a BcInstrStat> for BcInstrStat {
    fn add_assign(&mut self, other: &'a BcInstrStat) {
        let BcInstrStat { count } = other;
        self.count += count;
    }
}

#[derive(Default, Clone, Copy, Dupe, Debug)]
struct BcInstrPairsStat {
    count: u64,
    // We are not measuring time here, because even time for single opcode
    // is not very accurate or helpful, and time for pairs is even less helpful.
}

impl<'a> AddAssign<&'a BcInstrPairsStat> for BcInstrPairsStat {
    fn add_assign(&mut self, other: &'a BcInstrPairsStat) {
        let BcInstrPairsStat { count } = other;
        self.count += count;
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BcProfileData {
    by_instr: [BcInstrStat; BcOpcode::COUNT],
}

impl<'a> AddAssign<&'a BcProfileData> for BcProfileData {
    fn add_assign(&mut self, rhs: &'a BcProfileData) {
        for (lhs, rhs) in self.by_instr.iter_mut().zip(rhs.by_instr.iter()) {
            *lhs += rhs;
        }
    }
}

#[derive(Default, Clone, Debug)]
pub(crate) struct BcPairsProfileData {
    last: Option<BcOpcode>,
    by_instr: HashMap<[BcOpcode; 2], BcInstrPairsStat>,
}

impl<'a> AddAssign<&'a BcPairsProfileData> for BcPairsProfileData {
    fn add_assign(&mut self, rhs: &'a BcPairsProfileData) {
        self.last = None;
        for (pair, stat) in &rhs.by_instr {
            *self.by_instr.entry(*pair).or_default() += stat;
        }
    }
}

// Derive doesn't work here.
impl Default for BcProfileData {
    fn default() -> Self {
        BcProfileData {
            by_instr: [BcInstrStat::default(); BcOpcode::COUNT],
        }
    }
}

impl BcProfileData {
    fn before_instr(&mut self, opcode: BcOpcode) {
        self.by_instr[opcode as usize].count += 1;
    }

    pub(crate) fn gen_csv(&self) -> String {
        let mut by_instr: Vec<_> = self
            .by_instr
            .iter()
            .enumerate()
            .map(|(i, st)| (BcOpcode::by_number(i as u32).unwrap(), st))
            .collect();
        by_instr.sort_by_key(|(_opcode, st)| u64::MAX - st.count);
        let total: BcInstrStat = by_instr.iter().map(|(_opcode, st)| *st).sum();
        let mut csv = CsvWriter::new(["Opcode", "Count", "Count / Total"]);
        {
            csv.write_display("TOTAL");
            csv.write_value(total.count);
            csv.write_display(format!("{:.3}", 1.0));
            csv.finish_row();
        }
        for (opcode, instr_stats) in &by_instr {
            csv.write_debug(opcode);
            csv.write_value(instr_stats.count);
            csv.write_display(format!(
                "{:.3}",
                instr_stats.count as f64 / total.count as f64
            ));
            csv.finish_row();
        }
        csv.finish()
    }

    fn merge<'a>(iter: impl IntoIterator<Item = &'a BcProfileData>) -> BcProfileData {
        let mut sum = BcProfileData::default();
        for profile in iter {
            sum += profile;
        }
        sum
    }
}

impl BcPairsProfileData {
    fn before_instr(&mut self, opcode: BcOpcode) {
        if let Some(last_opcode) = self.last {
            self.by_instr
                .entry([last_opcode, opcode])
                .or_default()
                .count += 1;
        }
        self.last = Some(opcode);
    }

    pub(crate) fn gen_csv(&self) -> String {
        let mut by_instr: Vec<_> = self
            .by_instr
            .iter()
            .map(|(opcodes, stat)| (*opcodes, stat))
            .collect();
        by_instr.sort_by_key(|(opcodes, st)| (u64::MAX - st.count, *opcodes));
        let count_total = by_instr.iter().map(|(_, st)| st.count).sum::<u64>();
        let mut csv = CsvWriter::new(["Opcode[0]", "Opcode[1]", "Count", "Count / Total"]);
        for ([o0, o1], instr_stats) in &by_instr {
            csv.write_debug(o0);
            csv.write_debug(o1);
            csv.write_value(instr_stats.count);
            csv.write_display(format!(
                "{:.3}",
                instr_stats.count as f64 / count_total as f64
            ));
            csv.finish_row();
        }
        csv.finish()
    }

    fn merge<'a>(iter: impl IntoIterator<Item = &'a BcPairsProfileData>) -> BcPairsProfileData {
        let mut sum = BcPairsProfileData::default();
        for profile in iter {
            sum += profile;
        }
        sum
    }
}

enum BcProfileDataMode {
    Bc(Box<BcProfileData>),
    BcPairs(Box<BcPairsProfileData>),
    Disabled,
}

pub(crate) struct BcProfile {
    data: BcProfileDataMode,
}

impl BcProfile {
    pub(crate) fn new() -> BcProfile {
        BcProfile {
            data: BcProfileDataMode::Disabled,
        }
    }

    pub(crate) fn enable_1(&mut self) {
        self.data = BcProfileDataMode::Bc(Default::default());
    }

    pub(crate) fn enable_2(&mut self) {
        self.data = BcProfileDataMode::BcPairs(Default::default());
    }

    pub(crate) fn enabled(&self) -> bool {
        match self.data {
            BcProfileDataMode::Bc(..) => true,
            BcProfileDataMode::BcPairs(..) => true,
            BcProfileDataMode::Disabled => false,
        }
    }

    pub(crate) fn gen_bc_profile(&mut self) -> crate::Result<ProfileData> {
        match mem::replace(&mut self.data, BcProfileDataMode::Disabled) {
            BcProfileDataMode::Bc(bc) => Ok(ProfileData {
                profile: ProfileDataImpl::Bc(bc),
            }),
            _ => Err(crate::Error::new_other(
                BcProfileError::BcProfilingNotEnabled,
            )),
        }
    }

    pub(crate) fn gen_bc_pairs_profile(&mut self) -> crate::Result<ProfileData> {
        match mem::replace(&mut self.data, BcProfileDataMode::Disabled) {
            BcProfileDataMode::BcPairs(bc_pairs) => Ok(ProfileData {
                profile: ProfileDataImpl::BcPairs(*bc_pairs),
            }),
            _ => Err(crate::Error::new_other(
                BcProfileError::BcProfilingNotEnabled,
            )),
        }
    }

    /// Called from bytecode.
    pub(crate) fn before_instr(&mut self, opcode: BcOpcode) {
        match &mut self.data {
            BcProfileDataMode::Bc(data) => data.before_instr(opcode),
            BcProfileDataMode::BcPairs(data) => data.before_instr(opcode),
            BcProfileDataMode::Disabled => {
                unreachable!("this code is unreachable when bytecode profiling is not enabled")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::environment::Globals;
    use crate::environment::Module;
    use crate::eval::bc::opcode::BcOpcode;
    use crate::eval::runtime::profile::bc::BcPairsProfileData;
    use crate::eval::runtime::profile::bc::BcProfileData;
    use crate::eval::runtime::profile::mode::ProfileMode;
    use crate::eval::Evaluator;
    use crate::syntax::AstModule;
    use crate::syntax::Dialect;

    #[test]
    fn test_smoke() {
        let module = Module::new();
        let globals = Globals::standard();
        let mut eval = Evaluator::new(&module);
        eval.enable_profile(&ProfileMode::Bytecode).unwrap();
        eval.eval_module(
            AstModule::parse("bc.star", "repr([1, 2])".to_owned(), &Dialect::Standard).unwrap(),
            &globals,
        )
        .unwrap();
        let csv = eval.gen_bc_profile().unwrap().gen().unwrap();
        assert!(
            csv.contains(&format!("\n\"{:?}\",1,", BcOpcode::CallFrozenNativePos)),
            "{:?}",
            csv
        );
    }

    #[test]
    fn test_smoke_2() {
        let module = Module::new();
        let globals = Globals::standard();
        let mut eval = Evaluator::new(&module);
        eval.enable_profile(&ProfileMode::BytecodePairs).unwrap();
        eval.eval_module(
            AstModule::parse("bc.star", "repr([1, 2])".to_owned(), &Dialect::Standard).unwrap(),
            &globals,
        )
        .unwrap();
        let csv = eval.gen_bc_pairs_profile().unwrap().gen().unwrap();
        assert!(
            csv.contains(&format!(
                "\n\"{:?}\",\"{:?}\",1",
                BcOpcode::ListOfConsts,
                BcOpcode::CallFrozenNativePos
            )),
            "{:?}",
            csv
        );
    }

    #[test]
    fn test_bc_profile_data_merge() {
        let bc = BcProfileData::default();
        // Smoke test.
        BcProfileData::merge([&bc, &bc, &bc]);
    }

    #[test]
    fn test_bc_pairs_profile_data_merge() {
        let bc = BcPairsProfileData::default();
        // Smoke test.
        BcPairsProfileData::merge([&bc, &bc, &bc]);
    }
}
