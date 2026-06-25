use super::CompareCx;
use crate::ConstraintSet;
use crate::SchemaPath;
use crate::TypeSet;
use crate::ValueSet;
use crate::Violation;
use serde_json::Number;
use std::cmp::Ordering;

pub(super) fn compare_values(
    base: Option<&ValueSet>,
    current: Option<&ValueSet>,
    cx: &mut CompareCx<'_>,
    path: &SchemaPath,
) {
    let Some(current) = current else {
        return;
    };
    if base.is_none_or(|base| {
        base.values
            .iter()
            .any(|value| !current.values.contains(value))
    }) {
        cx.violations.push(Violation::EnumNarrowed {
            at: cx.location(path),
            before: base.cloned(),
            after: current.clone(),
        });
    }
}

pub(super) fn compare_types(
    base: Option<&TypeSet>,
    current: Option<&TypeSet>,
    cx: &mut CompareCx<'_>,
    path: &SchemaPath,
) {
    let Some(current) = current else {
        return;
    };
    if base.is_none_or(|base| !base.accepted_types().is_subset(&current.accepted_types())) {
        cx.violations.push(Violation::TypeNarrowed {
            at: cx.location(path),
            before: base.cloned(),
            after: current.clone(),
        });
    }
}

pub(super) fn compare_constraints(
    base: &ConstraintSet,
    current: &ConstraintSet,
    cx: &mut CompareCx<'_>,
    path: &SchemaPath,
) {
    let mut before = ConstraintSet::default();
    let mut after = ConstraintSet::default();
    for (kind, current_value) in &current.lower_bounds {
        let base_value = base.lower_bounds.get(kind);
        if base_value.is_none_or(|base_value| {
            compare_numbers(current_value, base_value) == Ordering::Greater
        }) {
            if let Some(base_value) = base_value {
                before.lower_bounds.insert(*kind, base_value.clone());
            }
            after.lower_bounds.insert(*kind, current_value.clone());
        }
    }
    for (kind, current_value) in &current.upper_bounds {
        let base_value = base.upper_bounds.get(kind);
        if base_value
            .is_none_or(|base_value| compare_numbers(current_value, base_value) == Ordering::Less)
        {
            if let Some(base_value) = base_value {
                before.upper_bounds.insert(*kind, base_value.clone());
            }
            after.upper_bounds.insert(*kind, current_value.clone());
        }
    }
    if current.unique_items == Some(true) && base.unique_items != Some(true) {
        before.unique_items = base.unique_items;
        after.unique_items = current.unique_items;
    }
    for (key, current_value) in &current.opaque {
        let base_value = base.opaque.get(key);
        if base_value != Some(current_value) {
            if let Some(base_value) = base_value {
                before.opaque.insert(key.clone(), base_value.clone());
            }
            after.opaque.insert(key.clone(), current_value.clone());
        }
    }
    if after != ConstraintSet::default() {
        cx.constraint_changed(path, before.to_json(), after.to_json());
    }
}

fn compare_numbers(left: &Number, right: &Number) -> Ordering {
    if let (Some(left), Some(right)) = (left.as_i64(), right.as_i64()) {
        return left.cmp(&right);
    }
    if let (Some(left), Some(right)) = (left.as_u64(), right.as_u64()) {
        return left.cmp(&right);
    }
    if let (Some(left), Some(right)) = (left.as_i64(), right.as_u64()) {
        return if left.is_negative() {
            Ordering::Less
        } else {
            (left as u64).cmp(&right)
        };
    }
    if let (Some(left), Some(right)) = (left.as_u64(), right.as_i64()) {
        return if right.is_negative() {
            Ordering::Greater
        } else {
            left.cmp(&(right as u64))
        };
    }
    left.as_f64()
        .partial_cmp(&right.as_f64())
        .unwrap_or(Ordering::Equal)
}

#[cfg(test)]
#[path = "value_tests.rs"]
mod tests;
