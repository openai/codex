use crate::config::ConfigToml;
use crate::config::types::RawMcpServerConfig;
use crate::features::FEATURES;
use schemars::Schema;
use schemars::SchemaGenerator;
use schemars::generate::SchemaSettings;
use serde_json::Map;
use serde_json::Value;
use std::path::Path;

/// Schema for the `[features]` map with known + legacy keys only.
pub(crate) fn features_schema(schema_gen: &mut SchemaGenerator) -> Schema {
    let mut properties = Map::new();
    for feature in FEATURES {
        properties.insert(
            feature.key.to_string(),
            schema_gen.subschema_for::<bool>().to_value(),
        );
    }
    for legacy_key in crate::features::legacy_feature_keys() {
        properties.insert(
            legacy_key.to_string(),
            schema_gen.subschema_for::<bool>().to_value(),
        );
    }
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("object".into()));
    schema.insert("properties".into(), Value::Object(properties));
    schema.insert("additionalProperties".into(), Value::Bool(false));
    schema.into()
}

/// Schema for the `[mcp_servers]` map using the raw input shape.
pub(crate) fn mcp_servers_schema(schema_gen: &mut SchemaGenerator) -> Schema {
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("object".into()));
    schema.insert(
        "additionalProperties".into(),
        schema_gen.subschema_for::<RawMcpServerConfig>().to_value(),
    );
    schema.into()
}

/// Build the config schema for `config.toml`.
pub fn config_schema() -> Schema {
    SchemaSettings::draft07()
        .into_generator()
        .into_root_schema_for::<ConfigToml>()
}

/// Render the config schema as pretty-printed JSON.
pub fn config_schema_json() -> anyhow::Result<Vec<u8>> {
    let schema = config_schema();
    let json = serde_json::to_vec_pretty(&schema)?;
    Ok(json)
}

/// Write the config schema fixture to disk.
pub fn write_config_schema(out_path: &Path) -> anyhow::Result<()> {
    let json = config_schema_json()?;
    std::fs::write(out_path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::config_schema_json;
    use similar::TextDiff;

    #[test]
    fn config_schema_matches_fixture() {
        let fixture_path = codex_utils_cargo_bin::find_resource!("config.schema.json")
            .expect("resolve config schema fixture path");
        let fixture = std::fs::read_to_string(fixture_path).expect("read config schema fixture");
        let schema_json = config_schema_json().expect("serialize config schema");
        let schema_str = String::from_utf8(schema_json).expect("decode schema json");
        if fixture != schema_str {
            let diff = TextDiff::from_lines(&fixture, &schema_str)
                .unified_diff()
                .to_string();
            let short = diff.lines().take(50).collect::<Vec<_>>().join("\n");
            panic!(
                "Current schema for `config.toml` doesn't match the fixture. Run `just write-config-schema` to overwrite with your changes.\n\n{short}"
            );
        }
    }
}
