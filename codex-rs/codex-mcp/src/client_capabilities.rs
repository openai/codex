use std::collections::HashMap;

use codex_protocol::mcp::ClientMcpExtensions;
use codex_protocol::mcp::MCP_APP_UI_EXTENSION_ID;
use codex_protocol::mcp::OPENAI_FORM_EXTENSION_ID;
use codex_protocol::mcp::OPENAI_STANDARD_FORM_INPUT_EXTENSION_ID;
use serde_json::Map;
use serde_json::Value;

/// Selects the MCP extensions Codex supports from those declared by the app-server host.
///
/// App-server clients may declare unrelated extensions. Codex retains only the
/// trusted extension namespaces it knows how to project downstream. The
/// legacy form capability is normalized into the same extension map.
pub fn client_mcp_extensions(
    extensions: Option<&HashMap<String, Value>>,
    legacy_openai_form_elicitation: bool,
) -> ClientMcpExtensions {
    let mut selected = extensions
        .into_iter()
        .flat_map(HashMap::iter)
        .filter(|(id, _)| {
            matches!(
                id.as_str(),
                OPENAI_FORM_EXTENSION_ID
                    | OPENAI_STANDARD_FORM_INPUT_EXTENSION_ID
                    | MCP_APP_UI_EXTENSION_ID
            )
        })
        .map(|(id, value)| (id.clone(), value.clone()))
        .collect::<HashMap<_, _>>();
    if legacy_openai_form_elicitation {
        selected
            .entry(OPENAI_FORM_EXTENSION_ID.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    ClientMcpExtensions::new(selected)
}

#[cfg(test)]
#[path = "client_capabilities_tests.rs"]
mod tests;
