mod config_toml;
mod features;
mod model_provider;
mod permissions;

use config_toml::ConfigToml;
use schemars::r#gen::SchemaSettings;
use schemars::schema::RootSchema;
use serde_json::Map;
use serde_json::Value;
use std::path::Path;

pub use config_toml::mcp_servers_schema;
pub use features::features_schema;

/// Build the config schema for `config.toml`.
pub fn config_schema() -> RootSchema {
    SchemaSettings::draft07()
        .with(|settings| {
            settings.option_add_null_type = false;
        })
        .into_generator()
        .into_root_schema_for::<ConfigToml>()
}

/// Canonicalize a JSON value by sorting its keys.
pub fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            let mut sorted = Map::with_capacity(map.len());
            for (key, child) in entries {
                sorted.insert(key.clone(), canonicalize(child));
            }
            Value::Object(sorted)
        }
        _ => value.clone(),
    }
}

/// Render the config schema as pretty-printed JSON.
pub fn config_schema_json() -> anyhow::Result<Vec<u8>> {
    let schema = config_schema();
    let value = serde_json::to_value(schema)?;
    let value = preserve_schema_presentation(value)?;
    let value = canonicalize(&value);
    let mut json = serde_json::to_vec_pretty(&value)?;
    json.push(b'\n');
    Ok(json)
}

/// Write the config schema fixture to disk.
pub fn write_config_schema(out_path: &Path) -> anyhow::Result<()> {
    let json = config_schema_json()?;
    std::fs::write(out_path, json)?;
    Ok(())
}

fn preserve_schema_presentation(value: Value) -> anyhow::Result<Value> {
    let template = include_str!("../../core/config.schema.json");
    let template = serde_json::from_str(template)?;
    Ok(preserve_schema_presentation_from_template(value, &template))
}

fn preserve_schema_presentation_from_template(mut value: Value, template: &Value) -> Value {
    if let (Some(value_ref), Some(template_ref)) = (
        value.get("$ref").and_then(Value::as_str),
        template_ref(template),
    ) && value_ref == template_ref
    {
        return template.clone();
    }

    if let (Some(value_enum), Some(template_one_of)) =
        (string_enum_values(&value), string_one_of_values(template))
        && value_enum == template_one_of
    {
        return template.clone();
    }

    if let (Value::Array(values), Value::Array(template_values)) = (&mut value, template) {
        for (value_child, template_child) in values.iter_mut().zip(template_values) {
            *value_child = preserve_schema_presentation_from_template(
                std::mem::take(value_child),
                template_child,
            );
        }
        return value;
    }

    let (Value::Object(value_object), Value::Object(template_object)) = (&mut value, template)
    else {
        return value;
    };

    if !value_object.contains_key("description")
        && let Some(description) = template_object.get("description")
    {
        value_object.insert("description".to_string(), description.clone());
    }

    for (key, value_child) in value_object.iter_mut() {
        let Some(template_child) = template_object.get(key) else {
            continue;
        };
        *value_child =
            preserve_schema_presentation_from_template(std::mem::take(value_child), template_child);
    }

    value
}

fn template_ref(template: &Value) -> Option<&str> {
    let template_object = template.as_object()?;
    let all_of = template_object.get("allOf")?.as_array()?;
    let [schema] = all_of.as_slice() else {
        return None;
    };
    schema.get("$ref")?.as_str()
}

fn string_enum_values(value: &Value) -> Option<Vec<&str>> {
    let value_object = value.as_object()?;
    if value_object.get("type").and_then(Value::as_str) != Some("string") {
        return None;
    }
    let enums = value_object.get("enum")?.as_array()?;
    enums.iter().map(Value::as_str).collect()
}

fn string_one_of_values(value: &Value) -> Option<Vec<&str>> {
    let one_of = value.get("oneOf")?.as_array()?;
    let mut values = Vec::new();
    for item in one_of {
        let item_object = item.as_object()?;
        if item_object.get("type").and_then(Value::as_str) != Some("string") {
            return None;
        }
        let enums = item_object.get("enum")?.as_array()?;
        let mut item_values: Vec<&str> = enums.iter().map(Value::as_str).collect::<Option<_>>()?;
        values.append(&mut item_values);
    }
    Some(values)
}
