//! Projects complete required provider definitions before config parsing.
//! Selection is applied with the other exact requirements after config parsing.

use crate::ConfigRequirementsToml;
use crate::config_toml::validate_model_providers;
use std::io;
use toml::Value;

pub(crate) fn to_config(requirements: &ConfigRequirementsToml) -> io::Result<Value> {
    let mut config = toml::Table::new();
    if let Some(providers) = &requirements.model_providers {
        validate_model_providers(providers)
            .map_err(|message| io::Error::new(io::ErrorKind::InvalidData, message))?;
        config.insert(
            "model_providers".to_string(),
            Value::try_from(providers)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
        );
    }
    Ok(Value::Table(config))
}

pub(crate) fn apply(config: &mut Value, requirements: &Value) {
    let Some(config) = config.as_table_mut() else {
        return;
    };
    if let Some(required) = requirements
        .get("model_providers")
        .and_then(Value::as_table)
    {
        let providers = config
            .entry("model_providers")
            .or_insert_with(|| Value::Table(Default::default()));
        if !providers.is_table() {
            *providers = Value::Table(Default::default());
        }
        if let Some(providers) = providers.as_table_mut() {
            providers.extend(required.clone());
        }
    }
}

#[cfg(test)]
#[path = "model_provider_requirements_tests.rs"]
mod tests;
