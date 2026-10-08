//! Per-part context sources, independent of message role, visibility, and trust.

use serde::Deserialize;
use serde::Serialize;

/// Harness-owned classification for one position in an item's content array.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct ContentItemKind(pub String);

impl ContentItemKind {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A callable namespace. Built-ins are typed; configured and MCP namespaces stay open.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(from = "String", into = "String")]
pub enum ContentItemNamespace {
    Clock,
    Functions,
    ToolSearch,
    Collaboration,
    MultiAgentV1,
    Other(String),
}

impl From<String> for ContentItemNamespace {
    fn from(namespace: String) -> Self {
        match namespace.as_str() {
            "clock" => Self::Clock,
            "functions" => Self::Functions,
            "tool_search" => Self::ToolSearch,
            "collaboration" => Self::Collaboration,
            "multi_agent_v1" => Self::MultiAgentV1,
            _ => Self::Other(namespace),
        }
    }
}

impl From<ContentItemNamespace> for String {
    fn from(namespace: ContentItemNamespace) -> Self {
        match namespace {
            ContentItemNamespace::Clock => "clock".to_string(),
            ContentItemNamespace::Functions => "functions".to_string(),
            ContentItemNamespace::ToolSearch => "tool_search".to_string(),
            ContentItemNamespace::Collaboration => "collaboration".to_string(),
            ContentItemNamespace::MultiAgentV1 => "multi_agent_v1".to_string(),
            ContentItemNamespace::Other(namespace) => namespace,
        }
    }
}

/// Who supplied a part. Tool-related context remains Tool even when the harness writes it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentItemProvenance {
    /// History saved before attribution, or content whose source is not known.
    #[default]
    #[serde(alias = "user_configured")]
    Unknown,
    /// The regular developer-instruction slot or additional requirements-layer instructions.
    DeveloperInstructions { from_additional_requirements: bool },
    /// Skill catalogs, usage guidance, and selected skill contents.
    Skills,
    /// Instructions loaded from AGENTS.md.
    AgentsMd,
    /// Harness-managed context; command-hook output is distinguished from built-in context.
    Harness {
        #[serde(default)]
        hook: bool,
    },
    /// A tool result or tool-specific context; an unlisted tool can have no known namespace.
    Tool {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        namespace: Option<ContentItemNamespace>,
    },
    /// Ordinary user input, not an injected instruction fragment with a user role.
    User,
}

/// Attribution for one position in a harness-emitted item's content array.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct ContentItemMetadata {
    pub provenance: ContentItemProvenance,
}

impl<'de> Deserialize<'de> for ContentItemMetadata {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Older records may lack attribution. Neither the old user_configured category
        // nor a bare harness_injected:false identifies a source precisely enough.
        #[derive(Deserialize)]
        struct StoredMetadata {
            #[serde(default)]
            provenance: Option<ContentItemProvenance>,
            #[serde(default)]
            harness_injected: Option<bool>,
            #[serde(default)]
            source_tool_namespace: Option<String>,
        }
        let stored = StoredMetadata::deserialize(deserializer)?;
        let provenance = stored.provenance.unwrap_or_else(|| {
            match (stored.source_tool_namespace, stored.harness_injected) {
                (Some(namespace), _) => ContentItemProvenance::Tool {
                    namespace: Some(namespace.into()),
                },
                (None, Some(true)) => ContentItemProvenance::Harness { hook: false },
                (None, Some(false) | None) => ContentItemProvenance::Unknown,
            }
        });
        Ok(Self { provenance })
    }
}

impl ContentItemMetadata {
    pub fn harness() -> Self {
        Self {
            provenance: ContentItemProvenance::Harness { hook: false },
        }
    }

    pub fn command_hook() -> Self {
        Self {
            provenance: ContentItemProvenance::Harness { hook: true },
        }
    }

    pub fn developer_instructions(from_additional_requirements: bool) -> Self {
        Self {
            provenance: ContentItemProvenance::DeveloperInstructions {
                from_additional_requirements,
            },
        }
    }

    pub fn skills() -> Self {
        Self {
            provenance: ContentItemProvenance::Skills,
        }
    }

    pub fn agents_md() -> Self {
        Self {
            provenance: ContentItemProvenance::AgentsMd,
        }
    }

    pub fn user() -> Self {
        Self {
            provenance: ContentItemProvenance::User,
        }
    }

    pub fn tool(namespace: Option<ContentItemNamespace>) -> Self {
        Self {
            provenance: ContentItemProvenance::Tool { namespace },
        }
    }
}

#[cfg(test)]
#[path = "item_metadata_tests.rs"]
mod tests;
