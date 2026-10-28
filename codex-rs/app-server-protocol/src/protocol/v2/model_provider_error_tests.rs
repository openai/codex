//! Checks that generated provider-error schemas describe the serialized wire payloads.

use super::ModelProviderError;
use pretty_assertions::assert_eq;
use std::collections::BTreeSet;

#[test]
fn provider_error_schema_matches_wire_fields() -> anyhow::Result<()> {
    let schema = serde_json::to_value(schemars::schema_for!(ModelProviderError))?;
    for error in [
        ModelProviderError::ModelProviderMismatch {
            model_provider: "old-provider".to_string(),
            current_model_provider: "current-provider".to_string(),
        },
        ModelProviderError::ModelProviderNotConfigured {
            model_provider: "old-provider".to_string(),
        },
        ModelProviderError::ModelProviderPolicyConflict {
            model_provider: "old-provider".to_string(),
            required_model_provider: "required-provider".to_string(),
        },
    ] {
        let wire = serde_json::to_value(&error)?;
        let variant = schema["oneOf"]
            .as_array()
            .expect("tagged variants")
            .iter()
            .find(|variant| variant["properties"]["type"]["enum"][0] == wire["type"])
            .expect("schema variant for wire discriminator");
        let wire_fields: BTreeSet<_> = wire
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let schema_fields: BTreeSet<_> = variant["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let required_fields: BTreeSet<_> = variant["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert_eq!(schema_fields, wire_fields);
        assert_eq!(required_fields, wire_fields);
        assert_eq!(serde_json::from_value::<ModelProviderError>(wire)?, error);
    }
    Ok(())
}
