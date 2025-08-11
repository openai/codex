use mcp_types::RequestId;

/// Utility to convert an MCP `RequestId` into a `String`.
pub(crate) fn request_id_to_string(id: &RequestId) -> String {
    match id {
        RequestId::String(s) => s.clone(),
        RequestId::Integer(i) => i.to_string(),
    }
}
