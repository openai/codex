use std::{borrow::Cow, sync::Arc};

use schemars::JsonSchema;
/// Tools represent a routine that a server can execute
/// Tool calls represent requests from the client to execute one
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Icon, JsonObject, Meta};

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
    /// Execution-related configuration including task support mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<ToolExecution>,
    /// Optional list of icons for the tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<Icon>>,
    /// Optional additional metadata for this tool
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Per-tool task support mode as defined in the MCP specification.
///
/// This enum indicates whether a tool supports task-based invocation,
/// allowing clients to know how to properly call the tool.
///
/// See [Tool-Level Negotiation](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks#tool-level-negotiation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum TaskSupport {
    /// Clients MUST NOT invoke this tool as a task (default behavior).
    #[default]
    Forbidden,
    /// Clients MAY invoke this tool as either a task or a normal call.
    Optional,
    /// Clients MUST invoke this tool as a task.
    Required,
}

/// Execution-related configuration for a tool.
///
/// This struct contains settings that control how a tool should be executed,
/// including task support configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ToolExecution {
    /// Indicates whether this tool supports task-based invocation.
    ///
    /// When not present or set to `Forbidden`, clients MUST NOT invoke this tool as a task.
    /// When set to `Optional`, clients MAY invoke this tool as a task or normal call.
    /// When set to `Required`, clients MUST invoke this tool as a task.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_support: Option<TaskSupport>,
}

impl ToolExecution {
    /// Create a new empty ToolExecution configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the task support mode.
    pub fn with_task_support(mut self, task_support: TaskSupport) -> Self {
        self.task_support = Some(task_support);
        self
    }
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
            execution: None,
            icons: None,
            meta: None,
        }
    }

    pub fn annotate(self, annotations: ToolAnnotations) -> Self {
        Tool {
            annotations: Some(annotations),
            ..self
        }
    }

    /// Set the execution configuration for this tool.
    pub fn with_execution(self, execution: ToolExecution) -> Self {
        Tool {
            execution: Some(execution),
            ..self
        }
    }

    /// Returns the task support mode for this tool.
    ///
    /// Returns `TaskSupport::Forbidden` if not explicitly set.
    pub fn task_support(&self) -> TaskSupport {
        self.execution
            .as_ref()
            .and_then(|e| e.task_support)
            .unwrap_or_default()
    }

    /// Set the output schema using a type that implements JsonSchema
    ///
    /// # Panics
    ///
    /// Panics if the generated schema does not have root type "object" as required by MCP specification.
    pub fn with_output_schema<T: JsonSchema + 'static>(mut self) -> Self {
        let schema = crate::handler::server::tool::schema_for_output::<T>()
            .unwrap_or_else(|e| panic!("Invalid output schema for tool '{}': {}", self.name, e));
        self.output_schema = Some(schema);
        self
    }

    /// Set the input schema using a type that implements JsonSchema
    pub fn with_input_schema<T: JsonSchema + 'static>(mut self) -> Self {
        self.input_schema = crate::handler::server::tool::schema_for_type::<T>();
        self
    }

    /// Get the schema as json value
    pub fn schema_as_json_value(&self) -> Value {
        Value::Object(self.input_schema.as_ref().clone())
    }
}
