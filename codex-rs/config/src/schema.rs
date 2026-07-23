use crate::config_toml::ConfigToml;
use crate::types::RawMcpServerConfig;
use codex_features::FEATURES;
use codex_features::legacy_feature_keys;
use schemars::r#gen::SchemaGenerator;
use schemars::r#gen::SchemaSettings;
use schemars::schema::InstanceType;
use schemars::schema::ObjectValidation;
use schemars::schema::RootSchema;
use schemars::schema::Schema;
use schemars::schema::SchemaObject;
use schemars::schema::SubschemaValidation;
use serde_json::Map;
use serde_json::Value;
use std::path::Path;

/// Schema for the `[features]` map with known + legacy keys only.
pub fn features_schema(schema_gen: &mut SchemaGenerator) -> Schema {
    let mut object = SchemaObject {
        instance_type: Some(InstanceType::Object.into()),
        ..Default::default()
    };

    let mut validation = ObjectValidation::default();
    for feature in FEATURES {
        if feature.id == codex_features::Feature::Artifact {
            continue;
        }
        if feature.id == codex_features::Feature::CodeMode {
            validation.properties.insert(
                feature.key.to_string(),
                schema_gen.subschema_for::<codex_features::FeatureToml<
                    codex_features::CodeModeConfigToml,
                >>(),
            );
            continue;
        }
        if feature.id == codex_features::Feature::NonPrefixedMcpToolNames {
            validation.properties.insert(
                feature.key.to_string(),
                schema_gen.subschema_for::<codex_features::FeatureToml<
                    codex_features::NonPrefixedMcpToolNamesConfigToml,
                >>(),
            );
            continue;
        }
        if feature.id == codex_features::Feature::MultiAgentV2 {
            validation.properties.insert(
                feature.key.to_string(),
                schema_gen.subschema_for::<codex_features::FeatureToml<
                    codex_features::MultiAgentV2ConfigToml,
                >>(),
            );
            continue;
        }
        if feature.id == codex_features::Feature::TokenBudget {
            validation.properties.insert(
                feature.key.to_string(),
                schema_gen.subschema_for::<codex_features::FeatureToml<
                    codex_features::TokenBudgetConfigToml,
                >>(),
            );
            continue;
        }
        if feature.id == codex_features::Feature::RolloutBudget {
            validation.properties.insert(
                feature.key.to_string(),
                schema_gen.subschema_for::<codex_features::FeatureToml<
                    codex_features::RolloutBudgetConfigToml,
                >>(),
            );
            continue;
        }
        if feature.id == codex_features::Feature::CurrentTimeReminder {
            validation.properties.insert(
                feature.key.to_string(),
                schema_gen.subschema_for::<codex_features::FeatureToml<
                    codex_features::CurrentTimeReminderConfigToml,
                >>(),
            );
            continue;
        }
        if feature.id == codex_features::Feature::AppsMcpPathOverride {
            validation.properties.insert(
                feature.key.to_string(),
                removed_apps_mcp_path_override_schema(schema_gen),
            );
            continue;
        }
        if feature.id == codex_features::Feature::NetworkProxy {
            validation.properties.insert(
                feature.key.to_string(),
                schema_gen.subschema_for::<codex_features::FeatureToml<
                    codex_features::NetworkProxyConfigToml,
                >>(),
            );
            continue;
        }
        validation
            .properties
            .insert(feature.key.to_string(), schema_gen.subschema_for::<bool>());
    }
    for legacy_key in legacy_feature_keys() {
        validation
            .properties
            .insert(legacy_key.to_string(), schema_gen.subschema_for::<bool>());
    }
    validation.additional_properties = Some(Box::new(Schema::Bool(false)));
    object.object = Some(Box::new(validation));

    Schema::Object(object)
}

fn removed_apps_mcp_path_override_schema(schema_gen: &mut SchemaGenerator) -> Schema {
    let mut config_validation = ObjectValidation::default();
    config_validation
        .properties
        .insert("enabled".to_string(), schema_gen.subschema_for::<bool>());
    config_validation
        .properties
        .insert("path".to_string(), schema_gen.subschema_for::<String>());
    config_validation.additional_properties = Some(Box::new(Schema::Bool(false)));

    let config = Schema::Object(SchemaObject {
        instance_type: Some(InstanceType::Object.into()),
        object: Some(Box::new(config_validation)),
        ..Default::default()
    });
    Schema::Object(SchemaObject {
        subschemas: Some(Box::new(SubschemaValidation {
            any_of: Some(vec![schema_gen.subschema_for::<bool>(), config]),
            ..Default::default()
        })),
        ..Default::default()
    })
}

/// Schema for the `[mcp_servers]` map using the raw input shape.
pub fn mcp_servers_schema(schema_gen: &mut SchemaGenerator) -> Schema {
    let mut object = SchemaObject {
        instance_type: Some(InstanceType::Object.into()),
        ..Default::default()
    };

    let validation = ObjectValidation {
        additional_properties: Some(Box::new(schema_gen.subschema_for::<RawMcpServerConfig>())),
        ..Default::default()
    };
    object.object = Some(Box::new(validation));

    Schema::Object(object)
}

/// Build the config schema for `config.toml`.
pub fn config_schema() -> RootSchema {
    let mut schema = SchemaSettings::draft07()
        .with(|settings| {
            settings.option_add_null_type = false;
        })
        .into_generator()
        .into_root_schema_for::<ConfigToml>();
    add_shell_environment_policy_constraints(&mut schema);
    schema
}

fn add_shell_environment_policy_constraints(schema: &mut RootSchema) {
    let Some(Schema::Object(policy)) = schema.definitions.get_mut("ShellEnvironmentPolicyToml")
    else {
        return;
    };
    let all_of = policy
        .subschemas
        .get_or_insert_default()
        .all_of
        .get_or_insert_default();
    for fields in [["exclude", "filters"], ["filters", "include_only"]] {
        all_of.push(Schema::Object(SchemaObject {
            subschemas: Some(Box::new(SubschemaValidation {
                not: Some(Box::new(Schema::Object(SchemaObject {
                    object: Some(Box::new(ObjectValidation {
                        required: fields.into_iter().map(str::to_string).collect(),
                        ..Default::default()
                    })),
                    ..Default::default()
                }))),
                ..Default::default()
            })),
            ..Default::default()
        }));
    }
}

/// Canonicalize a JSON value by sorting its keys.
pub fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by_key(|(key, _)| *key);
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
    let value = canonicalize(&value);
    let json = serde_json::to_vec_pretty(&value)?;
    Ok(json)
}

/// Write the config schema fixture to disk.
pub fn write_config_schema(out_path: &Path) -> anyhow::Result<()> {
    let json = config_schema_json()?;
    std::fs::write(out_path, json)?;
    Ok(())
}
