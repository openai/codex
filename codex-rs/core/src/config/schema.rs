#[cfg(test)]
use crate::config::ConfigToml;
use crate::features::FEATURES;
use schemars::JsonSchema;
use schemars::r#gen::SchemaGenerator;
#[cfg(test)]
use schemars::r#gen::SchemaSettings;
use schemars::schema::InstanceType;
use schemars::schema::ObjectValidation;
#[cfg(test)]
use schemars::schema::RootSchema;
use schemars::schema::Schema;
use schemars::schema::SchemaObject;
use schemars::schema::SubschemaValidation;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
#[cfg(test)]
use std::path::Path;

#[cfg(test)]
pub(crate) fn config_schema() -> RootSchema {
    SchemaSettings::draft07()
        .with(|settings| {
            settings.option_add_null_type = false;
        })
        .into_generator()
        .into_root_schema_for::<ConfigToml>()
}

#[cfg(test)]
pub(crate) fn write_config_schema(out_path: &Path) -> anyhow::Result<()> {
    let schema = config_schema();
    let json = serde_json::to_vec_pretty(&schema)?;
    std::fs::write(out_path, json)?;
    Ok(())
}

pub(crate) fn features_schema(schema_gen: &mut SchemaGenerator) -> Schema {
    let mut object = SchemaObject {
        instance_type: Some(InstanceType::Object.into()),
        ..Default::default()
    };

    let mut validation = ObjectValidation::default();
    for feature in FEATURES {
        validation
            .properties
            .insert(feature.key.to_string(), schema_gen.subschema_for::<bool>());
    }
    validation.additional_properties = Some(Box::new(schema_gen.subschema_for::<bool>()));
    object.object = Some(Box::new(validation));

    Schema::Object(object)
}

pub(crate) fn mcp_servers_schema(schema_gen: &mut SchemaGenerator) -> Schema {
    let mut object = SchemaObject {
        instance_type: Some(InstanceType::Object.into()),
        ..Default::default()
    };

    let validation = ObjectValidation {
        additional_properties: Some(Box::new(mcp_server_schema(schema_gen))),
        ..Default::default()
    };
    object.object = Some(Box::new(validation));

    Schema::Object(object)
}

fn mcp_server_schema(schema_gen: &mut SchemaGenerator) -> Schema {
    let server = SchemaObject {
        subschemas: Some(Box::new(SubschemaValidation {
            one_of: Some(vec![
                schema_gen.subschema_for::<McpServerStdioSchema>(),
                schema_gen.subschema_for::<McpServerStreamableHttpSchema>(),
            ]),
            ..Default::default()
        })),
        ..Default::default()
    };
    Schema::Object(server)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct McpServerStdioSchema {
    command: String,
    #[serde(default)]
    args: Option<Vec<String>>,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
    #[serde(default)]
    env_vars: Option<Vec<String>>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    startup_timeout_sec: Option<f64>,
    #[serde(default)]
    startup_timeout_ms: Option<u64>,
    #[serde(default)]
    tool_timeout_sec: Option<f64>,
    #[serde(default)]
    enabled_tools: Option<Vec<String>>,
    #[serde(default)]
    disabled_tools: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct McpServerStreamableHttpSchema {
    url: String,
    #[serde(default)]
    bearer_token_env_var: Option<String>,
    #[serde(default)]
    http_headers: Option<HashMap<String, String>>,
    #[serde(default)]
    env_http_headers: Option<HashMap<String, String>>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    startup_timeout_sec: Option<f64>,
    #[serde(default)]
    startup_timeout_ms: Option<u64>,
    #[serde(default)]
    tool_timeout_sec: Option<f64>,
    #[serde(default)]
    enabled_tools: Option<Vec<String>>,
    #[serde(default)]
    disabled_tools: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn config_schema_matches_fixture() {
        let schema = config_schema();
        let schema_value = serde_json::to_value(schema).expect("serialize config schema");
        let fixture_path = codex_utils_cargo_bin::find_resource!("../../docs/config.schema.json")
            .expect("resolve config schema fixture path");
        let fixture = std::fs::read_to_string(fixture_path).expect("read config schema fixture");
        let fixture_value: serde_json::Value =
            serde_json::from_str(&fixture).expect("parse config schema fixture");
        assert_eq!(fixture_value, schema_value);
    }

    #[test]
    #[ignore]
    fn write_config_schema_fixture() {
        let fixture_path = codex_utils_cargo_bin::find_resource!("../../docs/config.schema.json")
            .expect("resolve config schema fixture path");
        write_config_schema(&fixture_path).expect("write config schema fixture");
    }
}
