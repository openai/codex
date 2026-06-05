use codex_protocol::mcp::McpAppUiCapability;
use codex_protocol::mcp::McpClientCapabilities;
use rmcp::model::ExtensionCapabilities;
use rmcp::model::Meta;
use rmcp::model::PaginatedRequestParams;
use serde_json::Map;
use serde_json::Value as JsonValue;
use sha1::Digest;
use sha1::Sha1;

const MCP_APPS_UI_EXTENSION: &str = "io.modelcontextprotocol/ui";
const MCP_CLIENT_CAPABILITIES_META: &str = "openai/clientCapabilities";
const MCP_FUTURE_CLIENT_CAPABILITIES_META: &str = "io.modelcontextprotocol/clientCapabilities";
const MCP_APP_WEB_VIEW_MIME_TYPE: &str = "text/html;profile=mcp-app";
const MCP_APP_DECLARATIVE_UI_MIME_TYPE: &str = "text/x-dil;profile=mcp-app";

pub(crate) fn fingerprint(capabilities: &McpClientCapabilities) -> String {
    let canonical = serde_json::to_string(capabilities).unwrap_or_default();
    let digest = Sha1::digest(format!("v1:{canonical}").as_bytes());
    format!("{digest:x}")
}

fn app_ui_mime_types(capabilities: &McpClientCapabilities) -> Vec<&'static str> {
    capabilities
        .app_ui
        .iter()
        .map(|capability| match capability {
            McpAppUiCapability::WebView => MCP_APP_WEB_VIEW_MIME_TYPE,
            McpAppUiCapability::DeclarativeUi => MCP_APP_DECLARATIVE_UI_MIME_TYPE,
        })
        .collect()
}

fn as_json(capabilities: &McpClientCapabilities) -> JsonValue {
    serde_json::json!({
        "extensions": {
            MCP_APPS_UI_EXTENSION: {
                "mimeTypes": app_ui_mime_types(capabilities),
            }
        }
    })
}

pub(crate) fn as_extensions(capabilities: &McpClientCapabilities) -> ExtensionCapabilities {
    let settings = [(
        "mimeTypes".to_string(),
        JsonValue::Array(
            app_ui_mime_types(capabilities)
                .into_iter()
                .map(|mime_type| JsonValue::String(mime_type.to_string()))
                .collect(),
        ),
    )]
    .into_iter()
    .collect();
    [(MCP_APPS_UI_EXTENSION.to_string(), settings)]
        .into_iter()
        .collect()
}

pub(crate) fn add_meta(meta: &mut Option<Meta>, capabilities: Option<&McpClientCapabilities>) {
    if let Some(meta) = meta.as_mut() {
        meta.remove(MCP_CLIENT_CAPABILITIES_META);
        meta.remove(MCP_FUTURE_CLIENT_CAPABILITIES_META);
    }
    let Some(capabilities) = capabilities else {
        return;
    };
    meta.get_or_insert_with(Meta::new).insert(
        MCP_CLIENT_CAPABILITIES_META.to_string(),
        as_json(capabilities),
    );
}

pub(crate) fn add_json_meta(
    meta: &mut Option<JsonValue>,
    capabilities: Option<&McpClientCapabilities>,
) {
    if let Some(object) = meta.as_mut().and_then(JsonValue::as_object_mut) {
        object.remove(MCP_CLIENT_CAPABILITIES_META);
        object.remove(MCP_FUTURE_CLIENT_CAPABILITIES_META);
    }
    let Some(capabilities) = capabilities else {
        return;
    };
    if !meta.as_ref().is_some_and(JsonValue::is_object) {
        *meta = Some(JsonValue::Object(Map::new()));
    }
    meta.as_mut()
        .and_then(JsonValue::as_object_mut)
        .expect("metadata was normalized to an object")
        .insert(
            MCP_CLIENT_CAPABILITIES_META.to_string(),
            as_json(capabilities),
        );
}

pub(crate) fn paginated_params(
    cursor: Option<String>,
    capabilities: Option<&McpClientCapabilities>,
) -> Option<PaginatedRequestParams> {
    if cursor.is_none() && capabilities.is_none() {
        return None;
    }
    let mut params = PaginatedRequestParams::default().with_cursor(cursor);
    add_meta(&mut params.meta, capabilities);
    Some(params)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combined_capabilities() -> McpClientCapabilities {
        McpClientCapabilities {
            app_ui: [
                McpAppUiCapability::DeclarativeUi,
                McpAppUiCapability::WebView,
            ]
            .into_iter()
            .collect(),
        }
    }

    #[test]
    fn semantic_capabilities_map_to_mcp_mime_types() {
        assert_eq!(
            as_json(&combined_capabilities()),
            serde_json::json!({
                "extensions": {
                    "io.modelcontextprotocol/ui": {
                        "mimeTypes": [
                            "text/html;profile=mcp-app",
                            "text/x-dil;profile=mcp-app",
                        ],
                    },
                },
            })
        );
    }

    #[test]
    fn fingerprint_is_stable_for_sorted_enum_set() {
        let first = combined_capabilities();
        let second = McpClientCapabilities {
            app_ui: [
                McpAppUiCapability::WebView,
                McpAppUiCapability::DeclarativeUi,
            ]
            .into_iter()
            .collect(),
        };
        assert_eq!(fingerprint(&first), fingerprint(&second));
        assert_ne!(
            fingerprint(&first),
            fingerprint(&McpClientCapabilities::default())
        );
    }

    #[test]
    fn request_metadata_removes_both_reserved_forms() {
        let mut meta = Some(serde_json::json!({
            MCP_CLIENT_CAPABILITIES_META: {"spoofed": true},
            MCP_FUTURE_CLIENT_CAPABILITIES_META: {"spoofed": true},
            "caller": "preserved",
        }));
        add_json_meta(&mut meta, Some(&McpClientCapabilities::default()));

        assert_eq!(
            meta,
            Some(serde_json::json!({
                "caller": "preserved",
                MCP_CLIENT_CAPABILITIES_META: {
                    "extensions": {
                        MCP_APPS_UI_EXTENSION: {"mimeTypes": []},
                    },
                },
            }))
        );
    }

    #[test]
    fn request_metadata_replaces_non_object_values() {
        let mut meta = Some(serde_json::json!("invalid"));

        add_json_meta(&mut meta, Some(&McpClientCapabilities::default()));

        assert_eq!(
            meta,
            Some(serde_json::json!({
                MCP_CLIENT_CAPABILITIES_META: {
                    "extensions": {
                        MCP_APPS_UI_EXTENSION: {"mimeTypes": []},
                    },
                },
            }))
        );
    }
}
