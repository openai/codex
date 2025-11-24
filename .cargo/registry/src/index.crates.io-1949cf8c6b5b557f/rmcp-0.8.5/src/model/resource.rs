use serde::{Deserialize, Serialize};

use super::{Annotated, Icon, Meta};

/// Represents a resource in the extension with metadata
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RawResource {
    /// URI representing the resource location (e.g., "file:///path/to/file" or "str:///content")
    pub uri: String,
    /// Name of the resource
    pub name: String,
    /// Human-readable title of the resource
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Optional description of the resource
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type of the resource content ("text" or "blob")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,

    /// The size of the raw resource content, in bytes (i.e., before base64 encoding or any tokenization), if known.
    ///
    /// This can be used by Hosts to display file sizes and estimate context window us
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    /// Optional list of icons for the resource
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<Icon>>,
}

pub type Resource = Annotated<RawResource>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RawResourceTemplate {
    pub uri_template: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

pub type ResourceTemplate = Annotated<RawResourceTemplate>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum ResourceContents {
    #[serde(rename_all = "camelCase")]
    TextResourceContents {
        uri: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        text: String,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    #[serde(rename_all = "camelCase")]
    BlobResourceContents {
        uri: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        blob: String,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
}

impl ResourceContents {
    pub fn text(text: impl Into<String>, uri: impl Into<String>) -> Self {
        Self::TextResourceContents {
            uri: uri.into(),
            mime_type: Some("text".into()),
            text: text.into(),
            meta: None,
        }
    }
}

impl RawResource {
    /// Creates a new Resource from a URI with explicit mime type
    pub fn new(uri: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            name: name.into(),
            title: None,
            description: None,
            mime_type: None,
            size: None,
            icons: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json;

    use super::*;

    #[test]
    fn test_resource_serialization() {
        let resource = RawResource {
            uri: "file:///test.txt".to_string(),
            title: None,
            name: "test".to_string(),
            description: Some("Test resource".to_string()),
            mime_type: Some("text/plain".to_string()),
            size: Some(100),
            icons: None,
        };

        let json = serde_json::to_string(&resource).unwrap();
        println!("Serialized JSON: {}", json);

        // Verify it contains mimeType (camelCase) not mime_type (snake_case)
        assert!(json.contains("mimeType"));
        assert!(!json.contains("mime_type"));
    }

    #[test]
    fn test_resource_contents_serialization() {
        let text_contents = ResourceContents::TextResourceContents {
            uri: "file:///test.txt".to_string(),
            mime_type: Some("text/plain".to_string()),
            text: "Hello world".to_string(),
            meta: None,
        };

        let json = serde_json::to_string(&text_contents).unwrap();
        println!("ResourceContents JSON: {}", json);

        // Verify it contains mimeType (camelCase) not mime_type (snake_case)
        assert!(json.contains("mimeType"));
        assert!(!json.contains("mime_type"));
    }
}
