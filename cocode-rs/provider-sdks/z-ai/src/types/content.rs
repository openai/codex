//! Message content types for Z.AI SDK.

use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;

use super::Role;

/// Image URL content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    /// URL or base64 data URL.
    pub url: String,
}

impl ImageUrl {
    /// Create from URL.
    pub fn from_url(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }

    /// Create from base64 data.
    pub fn from_base64(data: impl Into<String>, media_type: &str) -> Self {
        Self {
            url: format!("data:{media_type};base64,{}", data.into()),
        }
    }
}

/// Content block in a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Text content.
    Text { text: String },
    /// Image URL content.
    ImageUrl { image_url: ImageUrl },
}

impl ContentBlock {
    /// Create a text content block.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create an image URL content block.
    pub fn image_url(url: impl Into<String>) -> Self {
        Self::ImageUrl {
            image_url: ImageUrl::from_url(url),
        }
    }

    /// Create an image content block from base64 data.
    pub fn image_base64(data: impl Into<String>, media_type: &str) -> Self {
        Self::ImageUrl {
            image_url: ImageUrl::from_base64(data, media_type),
        }
    }
}

/// A message parameter for chat completion requests.
#[derive(Debug, Clone)]
pub struct MessageParam {
    /// The role of the message author.
    pub role: Role,
    /// The content of the message.
    pub content: Vec<ContentBlock>,
    /// Tool call ID (for tool role messages).
    pub tool_call_id: Option<String>,
    /// Name of the function (for tool results).
    pub name: Option<String>,
}

impl MessageParam {
    /// Create a system message.
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: vec![ContentBlock::text(text)],
            tool_call_id: None,
            name: None,
        }
    }

    /// Create a user message with text.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::text(text)],
            tool_call_id: None,
            name: None,
        }
    }

    /// Create a user message with multiple content blocks.
    pub fn user_with_content(content: Vec<ContentBlock>) -> Self {
        Self {
            role: Role::User,
            content,
            tool_call_id: None,
            name: None,
        }
    }

    /// Create an assistant message.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlock::text(text)],
            tool_call_id: None,
            name: None,
        }
    }

    /// Create a tool result message.
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: vec![ContentBlock::text(content)],
            tool_call_id: Some(tool_call_id.into()),
            name: None,
        }
    }
}

impl Serialize for MessageParam {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("role", &self.role)?;

        // Custom content serialization:
        // - Single text block without cache_control -> serialize as string
        // - Multiple blocks or with cache_control -> serialize as array
        if self.content.len() == 1 {
            if let ContentBlock::Text { text } = &self.content[0] {
                map.serialize_entry("content", text)?;
            } else {
                map.serialize_entry("content", &self.content)?;
            }
        } else {
            map.serialize_entry("content", &self.content)?;
        }

        if let Some(ref tool_call_id) = self.tool_call_id {
            map.serialize_entry("tool_call_id", tool_call_id)?;
        }
        if let Some(ref name) = self.name {
            map.serialize_entry("name", name)?;
        }

        map.end()
    }
}

impl<'de> Deserialize<'de> for MessageParam {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct MessageParamHelper {
            role: Role,
            content: ContentOrString,
            tool_call_id: Option<String>,
            name: Option<String>,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ContentOrString {
            String(String),
            Blocks(Vec<ContentBlock>),
        }

        let helper = MessageParamHelper::deserialize(deserializer)?;
        let content = match helper.content {
            ContentOrString::String(s) => vec![ContentBlock::text(s)],
            ContentOrString::Blocks(blocks) => blocks,
        };

        Ok(Self {
            role: helper.role,
            content,
            tool_call_id: helper.tool_call_id,
            name: helper.name,
        })
    }
}
