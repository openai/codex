// Std
use std::{
    ffi::{OsStr, OsString},
    iter::{Cloned, Flatten},
    slice::Iter,
};

use crate::builder::ArgPredicate;
use crate::parser::ValueSource;
use crate::util::eq_ignore_case;
use crate::util::AnyValue;
use crate::util::AnyValueId;
use crate::INTERNAL_ERROR_MSG;

#[derive(Debug, Clone)]
pub(crate) struct MatchedArg {
    source: Option<ValueSource>,
    indices: Vec<usize>,
    type_id: Option<AnyValueId>,
    vals: Vec<Vec<AnyValue>>,
    raw_vals: Vec<Vec<OsString>>,
    ignore_case: bool,
}

impl MatchedArg {
    pub(crate) fn new_arg(arg: &crate::Arg) -> Self {
        let ignore_case = arg.is_ignore_case_set();
        Self {
            source: None,
            indices: Vec::new(),
            type_id: Some(arg.get_value_parser().type_id()),
            vals: Vec::new(),
            raw_vals: Vec::new(),
            ignore_case,
        }
    }

    pub(crate) fn new_group() -> Self {
        let ignore_case = false;
        Self {
            source: None,
            indices: Vec::new(),
            type_id: None,
            vals: Vec::new(),
            raw_vals: Vec::new(),
            ignore_case,
        }
    }

    pub(crate) fn new_external(cmd: &crate::Command) -> Self {
        let ignore_case = false;
        Self {
            source: None,
            indices: Vec::new(),
            type_id: Some(
                cmd.get_external_subcommand_value_parser()
                    .expect(INTERNAL_ERROR_MSG)
                    .type_id(),
            ),
            vals: Vec::new(),
            raw_vals: Vec::new(),
            ignore_case,
        }
    }

    pub(crate) fn indices(&self) -> Cloned<Iter<'_, usize>> {
        self.indices.iter().cloned()
    }

    pub(crate) fn get_index(&self, index: usize) -> Option<usize> {
        self.indices.get(index).cloned()
    }

    pub(crate) fn push_index(&mut self, index: usize) {
        self.indices.push(index);
    }

    pub(crate) fn vals(&self) -> Iter<'_, Vec<AnyValue>> {
        self.vals.iter()
    }

    pub(crate) fn into_vals(self) -> Vec<Vec<AnyValue>> {
        self.vals
    }

    pub(crate) fn vals_flatten(&self) -> Flatten<Iter<'_, Vec<AnyValue>>> {
        self.vals.iter().flatten()
    }

    pub(crate) fn into_vals_flatten(self) -> Flatten<std::vec::IntoIter<Vec<AnyValue>>> {
        self.vals.into_iter().flatten()
    }

    pub(crate) fn raw_vals(&self) -> Iter<'_, Vec<OsString>> {
        self.raw_vals.iter()
    }

    pub(crate) fn raw_vals_flatten(&self) -> Flatten<Iter<'_, Vec<OsString>>> {
        self.raw_vals.iter().flatten()
    }

    pub(crate) fn first(&self) -> Option<&AnyValue> {
        self.vals_flatten().next()
    }

    #[cfg(test)]
    pub(crate) fn first_raw(&self) -> Option<&OsString> {
        self.raw_vals_flatten().next()
    }

    pub(crate) fn new_val_group(&mut self) {
        self.vals.push(vec![]);
        self.raw_vals.push(vec![]);
    }

    pub(crate) fn append_val(&mut self, val: AnyValue, raw_val: OsString) {
        // We assume there is always a group created before.
        self.vals.last_mut().expect(INTERNAL_ERROR_MSG).push(val);
        self.raw_vals
            .last_mut()
            .expect(INTERNAL_ERROR_MSG)
            .push(raw_val);
    }

    pub(crate) fn num_vals(&self) -> usize {
        self.vals.iter().map(|v| v.len()).sum()
    }

    // Will be used later
    #[allow(dead_code)]
    pub(crate) fn num_vals_last_group(&self) -> usize {
        self.vals.last().map(|x| x.len()).unwrap_or(0)
    }

    pub(crate) fn check_explicit(&self, predicate: &ArgPredicate) -> bool {
        if self.source.map(|s| !s.is_explicit()).unwrap_or(false) {
            return false;
        }

        match predicate {
            ArgPredicate::Equals(val) => self.raw_vals_flatten().any(|v| {
                if self.ignore_case {
                    // If `v` isn't utf8, it can't match `val`, so `OsStr::to_str` should be fine
                    eq_ignore_case(&v.to_string_lossy(), &val.to_string_lossy())
                } else {
                    OsString::as_os_str(v) == OsStr::new(val)
                }
            }),
            ArgPredicate::IsPresent => true,
        }
    }

    pub(crate) fn source(&self) -> Option<ValueSource> {
        self.source
    }

    pub(crate) fn set_source(&mut self, source: ValueSource) {
        if let Some(existing) = self.source {
            self.source = Some(existing.max(source));
        } else {
            self.source = Some(source);
        }
    }

    pub(crate) fn type_id(&self) -> Option<AnyValueId> {
        self.type_id
    }

    pub(crate) fn infer_type_id(&self, expected: AnyValueId) -> AnyValueId {
        self.type_id()
            .or_else(|| {
                self.vals_flatten()
                    .map(|v| v.type_id())
                    .find(|actual| *actual != expected)
            })
            .unwrap_or(expected)
    }
}

impl PartialEq for MatchedArg {
    fn eq(&self, other: &MatchedArg) -> bool {
        let MatchedArg {
            source: self_source,
            indices: self_indices,
            type_id: self_type_id,
            vals: _,
            raw_vals: self_raw_vals,
            ignore_case: self_ignore_case,
        } = self;
        let MatchedArg {
            source: other_source,
            indices: other_indices,
            type_id: other_type_id,
            vals: _,
            raw_vals: other_raw_vals,
            ignore_case: other_ignore_case,
        } = other;
        self_source == other_source
            && self_indices == other_indices
            && self_type_id == other_type_id
            && self_raw_vals == other_raw_vals
            && self_ignore_case == other_ignore_case
    }
}

impl Eq for MatchedArg {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grouped_vals_first() {
        let mut m = MatchedArg::new_group();
        m.new_val_group();
        m.new_val_group();
        m.append_val(AnyValue::new(String::from("bbb")), "bbb".into());
        m.append_val(AnyValue::new(String::from("ccc")), "ccc".into());
        assert_eq!(m.first_raw(), Some(&OsString::from("bbb")));
    }
}
