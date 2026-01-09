#[cfg(test)]
use crate::config::ConfigToml;
use crate::config::types::RawMcpServerConfig;
use crate::features::FEATURES;
use schemars::r#gen::SchemaGenerator;
#[cfg(test)]
use schemars::r#gen::SchemaSettings;
use schemars::schema::InstanceType;
use schemars::schema::ObjectValidation;
#[cfg(test)]
use schemars::schema::RootSchema;
use schemars::schema::Schema;
use schemars::schema::SchemaObject;
#[cfg(test)]
use std::path::Path;

/// Build the config schema used by the fixture test.
#[cfg(test)]
pub(crate) fn config_schema() -> RootSchema {
    SchemaSettings::draft07()
        .with(|settings| {
            settings.option_add_null_type = false;
        })
        .into_generator()
        .into_root_schema_for::<ConfigToml>()
}

/// Write the config schema fixture to disk.
#[cfg(test)]
pub(crate) fn write_config_schema(out_path: &Path) -> anyhow::Result<()> {
    let schema = config_schema();
    let json = serde_json::to_vec_pretty(&schema)?;
    std::fs::write(out_path, json)?;
    Ok(())
}

/// Schema for the `[features]` map with known keys only.
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
    validation.additional_properties = Some(Box::new(Schema::Bool(false)));
    object.object = Some(Box::new(validation));

    Schema::Object(object)
}

/// Schema for the `[mcp_servers]` map using the raw input shape.
pub(crate) fn mcp_servers_schema(schema_gen: &mut SchemaGenerator) -> Schema {
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
        assert_eq!(
            fixture_value, schema_value,
            "Current schema for `config.toml` doesn't match the fixture. Run `just write-config-schema` to overwrite with your changes."
        );
    }

    /// Overwrite the config schema fixture with the current schema.
    #[test]
    #[ignore]
    fn write_config_schema_fixture() {
        let fixture_path = codex_utils_cargo_bin::find_resource!("../../docs/config.schema.json")
            .expect("resolve config schema fixture path");
        write_config_schema(&fixture_path).expect("write config schema fixture");
    }
}
