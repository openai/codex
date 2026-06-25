use super::CompareCx;
use super::CompareNarrowing;
use crate::AdditionalProperties;
use crate::AdditionalPropertiesValue;
use crate::ObjectSchema;
use crate::SchemaPath;
use crate::Violation;
use anyhow::Result;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

pub(super) fn compare_optional(
    base: Option<&ObjectSchema>,
    current: Option<&ObjectSchema>,
    cx: &mut CompareCx<'_>,
    path: &SchemaPath,
) -> Result<()> {
    if base.is_none() && current.is_none() {
        return Ok(());
    }
    let unconstrained = ObjectSchema {
        properties: BTreeMap::new(),
        required: BTreeSet::new(),
        additional_properties: AdditionalProperties::Any,
    };
    base.unwrap_or(&unconstrained)
        .compare(current.unwrap_or(&unconstrained), cx, path)
}

impl CompareNarrowing for ObjectSchema {
    fn compare(&self, current: &Self, cx: &mut CompareCx<'_>, path: &SchemaPath) -> Result<()> {
        for (name, property) in &self.properties {
            let property_path = path.property(name);
            if let Some(current) = current.properties.get(name) {
                property
                    .schema
                    .compare(&current.schema, cx, &property_path)?;
            } else {
                cx.violations.push(Violation::PropertyRemoved {
                    at: cx.location(&property_path),
                });
            }
        }
        for name in current.required.difference(&self.required) {
            cx.violations.push(Violation::RequiredPropertyAdded {
                at: cx.location(&path.property(name)),
            });
        }
        compare_additional_properties(
            self.additional_properties,
            current.additional_properties,
            cx,
            &path.additional_properties(),
        )
    }
}

fn compare_additional_properties(
    base: AdditionalProperties,
    current: AdditionalProperties,
    cx: &mut CompareCx<'_>,
    path: &SchemaPath,
) -> Result<()> {
    match (base, current) {
        (AdditionalProperties::Forbidden, _) | (_, AdditionalProperties::Any) => Ok(()),
        (AdditionalProperties::Schema(base), AdditionalProperties::Schema(current)) => {
            base.compare(&current, cx, path)
        }
        (base, current) => {
            let before = snapshot(base, cx.base)?;
            let after = snapshot(current, cx.current)?;
            cx.violations.push(Violation::AdditionalPropertiesNarrowed {
                at: cx.location(path),
                before,
                after,
            });
            Ok(())
        }
    }
}

fn snapshot(
    value: AdditionalProperties,
    schema: &crate::ApiSchema,
) -> Result<AdditionalPropertiesValue> {
    Ok(match value {
        AdditionalProperties::Any => AdditionalPropertiesValue::Any,
        AdditionalProperties::Forbidden => AdditionalPropertiesValue::Forbidden,
        AdditionalProperties::Schema(id) => AdditionalPropertiesValue::Schema(schema.snapshot(id)?),
    })
}

#[cfg(test)]
#[path = "object_tests.rs"]
mod tests;
