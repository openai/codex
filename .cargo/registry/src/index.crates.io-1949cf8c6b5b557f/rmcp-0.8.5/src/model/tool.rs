use std::{borrow::Cow, sync::Arc};

use schemars::JsonSchema;
/// Tools represent a routine that a server can execute
/// Tool calls represent requests from the client to execute one
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Icon, JsonObject};

/// A tool that can be used by a model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Tool {
    /// The name of the tool
    pub name: Cow<'static, str>,
    /// A human-readable title for the tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// A description of what the tool does
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,
    /// A JSON Schema object defining the expected parameters for the tool
    pub input_schema: Arc<JsonObject>,
    /// An optional JSON Schema object defining the structure of the tool's output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Arc<JsonObject>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional additional tool information.
    pub annotations: Option<ToolAnnotations>,
    /// Optional list of icons for the tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<Icon>>,
}

/// Additional properties describing a Tool to clients.
///
/// NOTE: all properties in ToolAnnotations are **hints**.
/// They are not guaranteed to provide a faithful description of
/// tool behavior (including descriptive properties like `title`).
///
/// Clients should never make tool use decisions based on ToolAnnotations
/// received from untrusted servers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ToolAnnotations {
    /// A human-readable title for the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// If true, the tool does not modify its environment.
    ///
    /// Default: false
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,

    /// If true, the tool may perform destructive updates to its environment.
    /// If false, the tool performs only additive updates.
    ///
    /// (This property is meaningful only when `readOnlyHint == false`)
    ///
    /// Default: true
    /// A human-readable description of the tool's purpose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,

    /// If true, calling the tool repeatedly with the same arguments
    /// will have no additional effect on the its environment.
    ///
    /// (This property is meaningful only when `readOnlyHint == false`)
    ///
    /// Default: false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,

    /// If true, this tool may interact with an "open world" of external
    /// entities. If false, the tool's domain of interaction is closed.
    /// For example, the world of a web search tool is open, whereas that
    /// of a memory tool is not.
    ///
    /// Default: true
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,
}

impl ToolAnnotations {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_title<T>(title: T) -> Self
    where
        T: Into<String>,
    {
        ToolAnnotations {
            title: Some(title.into()),
            ..Self::default()
        }
    }
    pub fn read_only(self, read_only: bool) -> Self {
        ToolAnnotations {
            read_only_hint: Some(read_only),
            ..self
        }
    }
    pub fn destructive(self, destructive: bool) -> Self {
        ToolAnnotations {
            destructive_hint: Some(destructive),
            ..self
        }
    }
    pub fn idempotent(self, idempotent: bool) -> Self {
        ToolAnnotations {
            idempotent_hint: Some(idempotent),
            ..self
        }
    }
    pub fn open_world(self, open_world: bool) -> Self {
        ToolAnnotations {
            open_world_hint: Some(open_world),
            ..self
        }
    }

    /// If not set, defaults to true.
    pub fn is_destructive(&self) -> bool {
        self.destructive_hint.unwrap_or(true)
    }

    /// If not set, defaults to false.
    pub fn is_idempotent(&self) -> bool {
        self.idempotent_hint.unwrap_or(false)
    }
}

impl Tool {
    /// Create a new tool with the given name and description
    pub fn new<N, D, S>(name: N, description: D, input_schema: S) -> Self
    where
        N: Into<Cow<'static, str>>,
        D: Into<Cow<'static, str>>,
        S: Into<Arc<JsonObject>>,
    {
        Tool {
            name: name.into(),
            title: None,
            description: Some(description.into()),
            input_schema: input_schema.into(),
            output_schema: None,
            annotations: None,
            icons: None,
        }
    }

    pub fn annotate(self, annotations: ToolAnnotations) -> Self {
        Tool {
            annotations: Some(annotations),
            ..self
        }
    }

    /// Set the output schema using a type that implements JsonSchema
    pub fn with_output_schema<T: JsonSchema + 'static>(mut self) -> Self {
        self.output_schema = Some(crate::handler::server::tool::cached_schema_for_type::<T>());
        self
    }

    /// Set the input schema using a type that implements JsonSchema
    pub fn with_input_schema<T: JsonSchema + 'static>(mut self) -> Self {
        self.input_schema = crate::handler::server::tool::cached_schema_for_type::<T>();
        self
    }

    /// Get the schema as json value
    pub fn schema_as_json_value(&self) -> Value {
        Value::Object(self.input_schema.as_ref().clone())
    }
}
