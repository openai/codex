use super::CompareCx;
use crate::ApiSchema;
use crate::SchemaId;
use crate::SchemaNode;
use crate::SchemaPath;
use crate::TypeSet;
use crate::UnionKind;
use crate::UnionSchema;
use crate::ValueSet;
use crate::VariantLabel;
use crate::Violation;
use anyhow::Result;
use serde_json::Value;

pub(super) fn compare_optional(
    base: Option<&UnionSchema>,
    current: Option<&UnionSchema>,
    base_schema: SchemaId,
    _current_schema: SchemaId,
    cx: &mut CompareCx<'_>,
    path: &SchemaPath,
) -> Result<()> {
    match (base, current) {
        (None, None) | (Some(_), None) => Ok(()),
        (None, Some(current)) => {
            let mut covered = false;
            for branch in &current.variants {
                if cx.probe(base_schema, *branch, path)?.is_empty() {
                    covered = true;
                    break;
                }
            }
            if !covered
                || (current.kind == UnionKind::OneOf
                    && !pairwise_disjoint(cx.current, &current.variants)?)
            {
                cx.constraint_changed(path, Value::Null, labels(cx.current, current)?);
            }
            Ok(())
        }
        (Some(base), Some(current)) => compare(base, current, cx, path),
    }
}

fn compare(
    base: &UnionSchema,
    current: &UnionSchema,
    cx: &mut CompareCx<'_>,
    path: &SchemaPath,
) -> Result<()> {
    let mut all_covered = true;
    for base_branch in &base.variants {
        let key = branch_key(cx.base, *base_branch)?;
        let mut matching_changes = None;
        let mut covered = false;
        for current_branch in &current.variants {
            let changes = cx.probe(*base_branch, *current_branch, path)?;
            if changes.is_empty() {
                covered = true;
            }
            if matching_changes.is_none() && branch_key(cx.current, *current_branch)? == key {
                matching_changes = Some(changes);
            }
        }
        if !covered && let Some(changes) = matching_changes {
            all_covered = false;
            cx.violations.extend(changes);
        } else if !covered {
            all_covered = false;
            cx.violations.push(Violation::UnionVariantRemoved {
                at: cx.location(path),
                variant: VariantLabel(key),
            });
        }
    }
    let all_equivalent = base.kind == UnionKind::OneOf
        && all_covered
        && base.variants.len() == current.variants.len()
        && equivalent_bijection(base, current, cx, path)?;
    if base.kind == UnionKind::OneOf
        && all_covered
        && !all_equivalent
        && !pairwise_disjoint(cx.current, &current.variants)?
    {
        cx.constraint_changed(path, labels(cx.base, base)?, labels(cx.current, current)?);
    }
    Ok(())
}

fn equivalent_bijection(
    base: &UnionSchema,
    current: &UnionSchema,
    cx: &CompareCx<'_>,
    path: &SchemaPath,
) -> Result<bool> {
    let mut equivalent = vec![vec![false; current.variants.len()]; base.variants.len()];
    for (base_index, base_branch) in base.variants.iter().enumerate() {
        for (current_index, current_branch) in current.variants.iter().enumerate() {
            equivalent[base_index][current_index] =
                cx.probe(*base_branch, *current_branch, path)?.is_empty()
                    && cx
                        .reverse_probe(*current_branch, *base_branch, path)?
                        .is_empty();
        }
    }
    let mut assigned = vec![None; current.variants.len()];
    for base_index in 0..base.variants.len() {
        let mut seen = vec![false; current.variants.len()];
        if !assign_equivalent(base_index, &equivalent, &mut assigned, &mut seen) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn assign_equivalent(
    base_index: usize,
    equivalent: &[Vec<bool>],
    assigned: &mut [Option<usize>],
    seen: &mut [bool],
) -> bool {
    for current_index in 0..assigned.len() {
        if !equivalent[base_index][current_index] || seen[current_index] {
            continue;
        }
        seen[current_index] = true;
        if assigned[current_index]
            .is_none_or(|previous| assign_equivalent(previous, equivalent, assigned, seen))
        {
            assigned[current_index] = Some(base_index);
            return true;
        }
    }
    false
}

fn labels(schema: &ApiSchema, union: &UnionSchema) -> Result<Value> {
    let mut labels = union
        .variants
        .iter()
        .map(|variant| branch_key(schema, *variant).map(Value::String))
        .collect::<Result<Vec<_>>>()?;
    labels.sort_by_key(|label| serde_json::to_string(label).unwrap_or_default());
    Ok(Value::Array(labels))
}

fn branch_key(schema: &ApiSchema, id: SchemaId) -> Result<String> {
    let (_, node) = schema.resolve(id)?;
    let SchemaNode::Rules(rules) = node else {
        return Ok(format!(
            "schema={}",
            serde_json::to_string(&schema.snapshot(id)?)?
        ));
    };
    if let Some(values) = &rules.values {
        return Ok(format!("enum={}", serde_json::to_string(&values.values)?));
    }
    if let Some((name, values)) = discriminator(schema, id)? {
        return Ok(format!("{name}={}", serde_json::to_string(&values.values)?));
    }
    if let Some(types) = &rules.types {
        return Ok(format!("type={}", serde_json::to_string(&types.to_json())?));
    }
    Ok(format!(
        "schema={}",
        serde_json::to_string(&schema.snapshot(id)?)?
    ))
}

fn pairwise_disjoint(schema: &ApiSchema, variants: &[SchemaId]) -> Result<bool> {
    for (index, left) in variants.iter().enumerate() {
        for right in &variants[index + 1..] {
            if disjoint(schema, *left, *right)? {
                continue;
            }
            return Ok(false);
        }
    }
    Ok(true)
}

fn disjoint(schema: &ApiSchema, left: SchemaId, right: SchemaId) -> Result<bool> {
    let left_types = types(schema, left)?;
    let right_types = types(schema, right)?;
    if matches!((left_types, right_types), (Some(left), Some(right)) if left.accepted_types().is_disjoint(&right.accepted_types()))
    {
        return Ok(true);
    }
    let left_values = values(schema, left)?;
    let right_values = values(schema, right)?;
    if matches!((left_values, right_values), (Some(left), Some(right)) if left.values.iter().all(|value| !right.values.contains(value)))
    {
        return Ok(true);
    }
    let left_discriminator = discriminator(schema, left)?;
    let right_discriminator = discriminator(schema, right)?;
    Ok(matches!(
        (left_discriminator, right_discriminator),
        (Some((left_name, left)), Some((right_name, right)))
            if left_name == right_name
                && left.values.iter().all(|value| !right.values.contains(value))
    ))
}

fn types(schema: &ApiSchema, id: SchemaId) -> Result<Option<&TypeSet>> {
    let (_, node) = schema.resolve(id)?;
    Ok(match node {
        SchemaNode::Rules(rules) => rules.types.as_ref(),
        SchemaNode::Any | SchemaNode::Never => None,
        SchemaNode::Reference(_) => unreachable!("resolve returns concrete schema nodes"),
    })
}

fn values(schema: &ApiSchema, id: SchemaId) -> Result<Option<&ValueSet>> {
    let (_, node) = schema.resolve(id)?;
    Ok(match node {
        SchemaNode::Rules(rules) => rules.values.as_ref(),
        SchemaNode::Any | SchemaNode::Never => None,
        SchemaNode::Reference(_) => unreachable!("resolve returns concrete schema nodes"),
    })
}

fn discriminator(schema: &ApiSchema, id: SchemaId) -> Result<Option<(String, ValueSet)>> {
    let (_, node) = schema.resolve(id)?;
    let SchemaNode::Rules(rules) = node else {
        return Ok(None);
    };
    let Some(object) = &rules.object else {
        return Ok(None);
    };
    for (name, property) in &object.properties {
        if object.required.contains(name)
            && let Some(values) = values(schema, property.schema)?
        {
            return Ok(Some((name.clone(), values.clone())));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "union_tests.rs"]
mod tests;
