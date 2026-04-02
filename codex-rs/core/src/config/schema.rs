pub use codex_config_schema::config_schema;
pub use codex_config_schema::config_schema_json;
use codex_config_schema::mcp_servers_schema as config_schema_mcp_servers_schema;
pub use codex_config_schema::write_config_schema;
use schemars::r#gen::SchemaGenerator;
#[cfg(test)]
use serde_json::Value;

/// Schema for the `[features]` map with known + legacy keys only.
pub(crate) fn features_schema(schema_gen: &mut SchemaGenerator) -> schemars::schema::Schema {
    codex_config_schema::features_schema(schema_gen)
}

/// Schema for the `[mcp_servers]` map using the raw input shape.
pub(crate) fn mcp_servers_schema(schema_gen: &mut SchemaGenerator) -> schemars::schema::Schema {
    config_schema_mcp_servers_schema(schema_gen)
}

/// Canonicalize a JSON value by sorting its keys.
#[cfg(test)]
fn canonicalize(value: &Value) -> Value {
    codex_config_schema::canonicalize(value)
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
