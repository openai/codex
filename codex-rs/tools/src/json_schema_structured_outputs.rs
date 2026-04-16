use crate::AdditionalProperties;
use crate::JsonSchema;
use crate::JsonSchemaPrimitiveType;
use crate::JsonSchemaType;
use std::collections::BTreeSet;

/// Validates the subset of JSON Schema invariants required by OpenAI
/// Structured Outputs for the object-heavy MCP regression tests in this crate.
///
/// Source of truth:
/// https://developers.openai.com/api/docs/guides/structured-outputs#supported-schemas
///
/// This validator currently focuses on the object constraints that matter for
/// the `start` / `end` nested-object regression:
/// - the root schema must be an object and must not use root-level `anyOf`
/// - every object must set `additionalProperties: false`
/// - every object property must appear in `required`
/// - nested `anyOf` branches and array items must themselves satisfy the same
///   subset whenever they contain objects
///
/// It intentionally does not yet enforce the documented global size limits
/// (property count, nesting depth, enum count, total string budget). Those are
/// broader policy checks and can be layered on later without changing the
/// regression tests that pin the object-shape bug fixed in PR #18159.
pub(crate) fn validate_structured_outputs_schema(schema: &JsonSchema) -> Result<(), String> {
    validate_structured_outputs_schema_at_path(schema, "root", /*is_root*/ true)
}

fn validate_structured_outputs_schema_at_path(
    schema: &JsonSchema,
    path: &str,
    is_root: bool,
) -> Result<(), String> {
    if is_root {
        if schema.any_of.is_some() {
            return Err(format!(
                "{path}: root schema must not use `anyOf`; see https://developers.openai.com/api/docs/guides/structured-outputs#supported-schemas"
            ));
        }

        if schema.schema_type != Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Object)) {
            return Err(format!(
                "{path}: root schema must be an object; see https://developers.openai.com/api/docs/guides/structured-outputs#supported-schemas"
            ));
        }
    }

    if let Some(any_of) = &schema.any_of {
        for (index, variant) in any_of.iter().enumerate() {
            validate_structured_outputs_schema_at_path(
                variant,
                &format!("{path}.anyOf[{index}]"),
                /*is_root*/ false,
            )?;
        }
    }

    if let Some(items) = &schema.items {
        validate_structured_outputs_schema_at_path(
            items,
            &format!("{path}.items"),
            /*is_root*/ false,
        )?;
    }

    if let Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Object)) = &schema.schema_type {
        let Some(properties) = schema.properties.as_ref() else {
            return Err(format!(
                "{path}: object schemas must carry a properties map"
            ));
        };

        if schema.additional_properties != Some(AdditionalProperties::Boolean(false)) {
            return Err(format!(
                "{path}: object schemas must set `additionalProperties: false`; see https://developers.openai.com/api/docs/guides/structured-outputs#supported-schemas"
            ));
        }

        let property_names = properties.keys().cloned().collect::<BTreeSet<_>>();
        let Some(required) = schema.required.as_ref() else {
            return Err(format!(
                "{path}: object schemas must list every field in `required`; see https://developers.openai.com/api/docs/guides/structured-outputs#supported-schemas"
            ));
        };
        let required_names = required.iter().cloned().collect::<BTreeSet<_>>();

        if required_names != property_names {
            return Err(format!(
                "{path}: object schema `required` entries must exactly match declared properties; see https://developers.openai.com/api/docs/guides/structured-outputs#supported-schemas"
            ));
        }

        for (property_name, property_schema) in properties {
            validate_structured_outputs_schema_at_path(
                property_schema,
                &format!("{path}.{property_name}"),
                /*is_root*/ false,
            )?;
        }
    }

    Ok(())
}
