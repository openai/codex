//! JSON Schema generation utilities.
//!
//! This module provides functions to generate JSON Schema from the protocol types.

use schemars::JsonSchema;
use schemars::schema_for;
use std::fs;
use std::path::Path;

use crate::config::CodexAgentOptions;
use crate::control::ControlRequestEnvelope;
use crate::control::ControlResponseEnvelope;
use crate::events::ThreadEvent;
use crate::hooks::HookInput;
use crate::hooks::HookOutput;
use crate::messages::CliMessage;
use crate::messages::SdkMessage;

/// Generate all JSON schemas and write to the specified directory.
pub fn generate_all_schemas(output_dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(output_dir)?;

    // Generate individual schemas
    generate_schema::<ThreadEvent>(output_dir, "ThreadEvent")?;
    generate_schema::<SdkMessage>(output_dir, "SdkMessage")?;
    generate_schema::<CliMessage>(output_dir, "CliMessage")?;
    generate_schema::<ControlRequestEnvelope>(output_dir, "ControlRequest")?;
    generate_schema::<ControlResponseEnvelope>(output_dir, "ControlResponse")?;
    generate_schema::<CodexAgentOptions>(output_dir, "CodexAgentOptions")?;
    generate_schema::<HookInput>(output_dir, "HookInput")?;
    generate_schema::<HookOutput>(output_dir, "HookOutput")?;

    // Generate combined schema
    generate_combined_schema(output_dir)?;

    Ok(())
}

/// Generate a JSON schema for a single type.
fn generate_schema<T: JsonSchema>(output_dir: &Path, name: &str) -> std::io::Result<()> {
    let schema = schema_for!(T);
    let json = serde_json::to_string_pretty(&schema)?;
    let path = output_dir.join(format!("{name}.schema.json"));
    fs::write(path, json)?;
    Ok(())
}

/// Generate a combined schema with all types.
fn generate_combined_schema(output_dir: &Path) -> std::io::Result<()> {
    // Create a combined schema object
    let combined = serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "Codex SDK Protocol",
        "description": "Protocol types for Codex SDK communication",
        "definitions": {
            "ThreadEvent": schema_for!(ThreadEvent),
            "SdkMessage": schema_for!(SdkMessage),
            "CliMessage": schema_for!(CliMessage),
            "ControlRequest": schema_for!(ControlRequestEnvelope),
            "ControlResponse": schema_for!(ControlResponseEnvelope),
            "CodexAgentOptions": schema_for!(CodexAgentOptions),
            "HookInput": schema_for!(HookInput),
            "HookOutput": schema_for!(HookOutput),
        }
    });

    let json = serde_json::to_string_pretty(&combined)?;
    let path = output_dir.join("sdk-protocol.schema.json");
    fs::write(path, json)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    #[ignore] // Run with: cargo test -p codex-sdk-protocol generate_schemas -- --ignored
    fn generate_schemas() {
        let output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema");
        generate_all_schemas(&output_dir).expect("Failed to generate schemas");
        println!("Schemas generated in {:?}", output_dir);
    }
}
