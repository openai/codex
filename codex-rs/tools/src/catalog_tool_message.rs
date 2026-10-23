//! Applies sparse catalog overrides while preserving bundled tool transport requirements.
//! Invalid parameters retain the bundled schema; other tool fields remain harness-owned.

use crate::ResponsesApiTool;
use crate::parse_catalog_tool_parameters;
use codex_protocol::openai_models::ToolMessage;

/// Apply catalog descriptions and parameters without changing the tool's identity or transport.
pub fn with_catalog_tool_message(
    mut tool: ResponsesApiTool,
    messages: Option<&ToolMessage>,
) -> ResponsesApiTool {
    let Some(messages) = messages else {
        return tool;
    };
    if let Some(description) = &messages.description {
        tool.description.clone_from(description);
    }
    if let Some(parameters) = &messages.parameters {
        let parameters = parse_catalog_tool_parameters(parameters).and_then(|mut parameters| {
            if let Some(properties) = &tool.parameters.properties {
                for (name, bundled_property) in properties.iter() {
                    if bundled_property.encrypted == Some(true) {
                        let property = parameters
                            .properties
                            .as_mut()
                            .and_then(|properties| properties.get_mut(name))
                            .ok_or("schema omits an encrypted parameter")?;
                        property.encrypted = Some(true);
                    }
                }
            }
            Ok(parameters)
        });
        match parameters {
            Ok(parameters) => tool.parameters = parameters,
            Err(reason) => tracing::warn!(
                tool = %tool.name,
                reason,
                "Invalid catalog tool parameters; using bundled parameters"
            ),
        }
    }
    tool
}

#[cfg(test)]
#[path = "catalog_tool_message_tests.rs"]
mod tests;
