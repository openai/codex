//! Content sent around agents, extensions, and LLMs
//! The various content types can be display to humans but also understood by models
//! They include optional annotations used to help inform agent usage
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{AnnotateAble, Annotated, resource::ResourceContents};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RawTextContent {
    pub text: String,
    /// Optional protocol-level metadata for this content block
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<super::Meta>,
}
pub type TextContent = Annotated<RawTextContent>;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RawImageContent {
    /// The base64-encoded image
    pub data: String,
    pub mime_type: String,
    /// Optional protocol-level metadata for this content block
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<super::Meta>,
}

pub type ImageContent = Annotated<RawImageContent>;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RawEmbeddedResource {
    /// Optional protocol-level metadata for this content block
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<super::Meta>,
    pub resource: ResourceContents,
}
pub type EmbeddedResource = Annotated<RawEmbeddedResource>;

impl EmbeddedResource {
    pub fn get_text(&self) -> String {
        match &self.resource {
            ResourceContents::TextResourceContents { text, .. } => text.clone(),
            _ => String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RawAudioContent {
    pub data: String,
    pub mime_type: String,
}

pub type AudioContent = Annotated<RawAudioContent>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum RawContent {
    Text(RawTextContent),
    Image(RawImageContent),
    Resource(RawEmbeddedResource),
    Audio(RawAudioContent),
    ResourceLink(super::resource::RawResource),
}

pub type Content = Annotated<RawContent>;

impl RawContent {
    pub fn json<S: Serialize>(json: S) -> Result<Self, crate::ErrorData> {
        let json = serde_json::to_string(&json).map_err(|e| {
            crate::ErrorData::internal_error(
                "fail to serialize response to json",
                Some(json!(
                    {"reason": e.to_string()}
                )),
            )
        })?;
        Ok(RawContent::text(json))
    }

    pub fn text<S: Into<String>>(text: S) -> Self {
        RawContent::Text(RawTextContent {
            text: text.into(),
            meta: None,
        })
    }

    pub fn image<S: Into<String>, T: Into<String>>(data: S, mime_type: T) -> Self {
        RawContent::Image(RawImageContent {
            data: data.into(),
            mime_type: mime_type.into(),
            meta: None,
        })
    }

    pub fn resource(resource: ResourceContents) -> Self {
        RawContent::Resource(RawEmbeddedResource {
            meta: None,
            resource,
        })
    }

    pub fn embedded_text<S: Into<String>, T: Into<String>>(uri: S, content: T) -> Self {
        RawContent::Resource(RawEmbeddedResource {
            meta: None,
            resource: ResourceContents::TextResourceContents {
                uri: uri.into(),
                mime_type: Some("text".to_string()),
                text: content.into(),
                meta: None,
            },
        })
    }

    /// Get the text content if this is a TextContent variant
    pub fn as_text(&self) -> Option<&RawTextContent> {
        match self {
            RawContent::Text(text) => Some(text),
            _ => None,
        }
    }

    /// Get the image content if this is an ImageContent variant
    pub fn as_image(&self) -> Option<&RawImageContent> {
        match self {
            RawContent::Image(image) => Some(image),
            _ => None,
        }
    }

    /// Get the resource content if this is an ImageContent variant
    pub fn as_resource(&self) -> Option<&RawEmbeddedResource> {
        match self {
            RawContent::Resource(resource) => Some(resource),
            _ => None,
        }
    }

    /// Get the resource link if this is a ResourceLink variant
    pub fn as_resource_link(&self) -> Option<&super::resource::RawResource> {
        match self {
            RawContent::ResourceLink(link) => Some(link),
            _ => None,
        }
    }

    /// Create a resource link content
    pub fn resource_link(resource: super::resource::RawResource) -> Self {
        RawContent::ResourceLink(resource)
    }
}

impl Content {
    pub fn text<S: Into<String>>(text: S) -> Self {
        RawContent::text(text).no_annotation()
    }

    pub fn image<S: Into<String>, T: Into<String>>(data: S, mime_type: T) -> Self {
        RawContent::image(data, mime_type).no_annotation()
    }

    pub fn resource(resource: ResourceContents) -> Self {
        RawContent::resource(resource).no_annotation()
    }

    pub fn embedded_text<S: Into<String>, T: Into<String>>(uri: S, content: T) -> Self {
        RawContent::embedded_text(uri, content).no_annotation()
    }

    pub fn json<S: Serialize>(json: S) -> Result<Self, crate::ErrorData> {
        RawContent::json(json).map(|c| c.no_annotation())
    }

    /// Create a resource link content
    pub fn resource_link(resource: super::resource::RawResource) -> Self {
        RawContent::resource_link(resource).no_annotation()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonContent<S: Serialize>(S);
/// Types that can be converted into a list of contents
pub trait IntoContents {
    fn into_contents(self) -> Vec<Content>;
}

impl IntoContents for Content {
    fn into_contents(self) -> Vec<Content> {
        vec![self]
    }
}

impl IntoContents for String {
    fn into_contents(self) -> Vec<Content> {
        vec![Content::text(self)]
    }
}

impl IntoContents for () {
    fn into_contents(self) -> Vec<Content> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use serde_json;

    use super::*;

    #[test]
    fn test_image_content_serialization() {
        let image_content = RawImageContent {
            data: "base64data".to_string(),
            mime_type: "image/png".to_string(),
            meta: None,
        };

        let json = serde_json::to_string(&image_content).unwrap();
        println!("ImageContent JSON: {}", json);

        // Verify it contains mimeType (camelCase) not mime_type (snake_case)
        assert!(json.contains("mimeType"));
        assert!(!json.contains("mime_type"));
    }

    #[test]
    fn test_audio_content_serialization() {
        let audio_content = RawAudioContent {
            data: "base64audiodata".to_string(),
            mime_type: "audio/wav".to_string(),
        };

        let json = serde_json::to_string(&audio_content).unwrap();
        println!("AudioContent JSON: {}", json);

        // Verify it contains mimeType (camelCase) not mime_type (snake_case)
        assert!(json.contains("mimeType"));
        assert!(!json.contains("mime_type"));
    }

    #[test]
    fn test_resource_link_serialization() {
        use super::super::resource::RawResource;

        let resource_link = RawContent::ResourceLink(RawResource {
            uri: "file:///test.txt".to_string(),
            name: "test.txt".to_string(),
            title: None,
            description: Some("A test file".to_string()),
            mime_type: Some("text/plain".to_string()),
            size: Some(100),
            icons: None,
        });

        let json = serde_json::to_string(&resource_link).unwrap();
        println!("ResourceLink JSON: {}", json);

        // Verify it contains the correct type tag
        assert!(json.contains("\"type\":\"resource_link\""));
        assert!(json.contains("\"uri\":\"file:///test.txt\""));
        assert!(json.contains("\"name\":\"test.txt\""));
    }

    #[test]
    fn test_resource_link_deserialization() {
        let json = r#"{
            "type": "resource_link",
            "uri": "file:///example.txt",
            "name": "example.txt",
            "description": "Example file",
            "mimeType": "text/plain"
        }"#;

        let content: RawContent = serde_json::from_str(json).unwrap();

        if let RawContent::ResourceLink(resource) = content {
            assert_eq!(resource.uri, "file:///example.txt");
            assert_eq!(resource.name, "example.txt");
            assert_eq!(resource.description, Some("Example file".to_string()));
            assert_eq!(resource.mime_type, Some("text/plain".to_string()));
        } else {
            panic!("Expected ResourceLink variant");
        }
    }
}
