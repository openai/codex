use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;

const NAMESPACED_TOOL_NAME_DELIMITER: &str = "__";

/// Identifies a callable tool, preserving the namespace split when the model
/// provides one.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ToolName {
    pub name: String,
    pub namespace: Option<String>,
}

impl ToolName {
    pub fn new(namespace: Option<String>, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            namespace,
        }
    }

    pub fn plain(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            namespace: None,
        }
    }

    pub fn namespaced(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            namespace: Some(namespace.into()),
        }
    }

    /// Canonical flat name with the `__` delimiter used when a boundary cannot
    /// preserve the namespace split. Stray underscores around the delimiter
    /// are trimmed to match Responses-facing flattened tool names.
    pub fn canonical_flat_name(&self) -> Cow<'_, str> {
        match self.namespace.as_deref() {
            Some(namespace) => Cow::Owned(join_namespaced_tool_name(namespace, &self.name)),
            None => Cow::Borrowed(self.name.as_str()),
        }
    }
}

fn join_namespaced_tool_name(namespace: &str, name: &str) -> String {
    let namespace = namespace.trim_end_matches('_');
    let name = name.trim_start_matches('_');
    format!("{namespace}{NAMESPACED_TOOL_NAME_DELIMITER}{name}")
}

impl fmt::Display for ToolName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.namespace {
            Some(namespace) => write!(f, "{namespace}{}", self.name),
            None => f.write_str(&self.name),
        }
    }
}

impl Ord for ToolName {
    fn cmp(&self, other: &Self) -> Ordering {
        let lhs = match &self.namespace {
            Some(namespace) => (namespace.as_str(), Some(self.name.as_str())),
            None => (self.name.as_str(), None),
        };
        let rhs = match &other.namespace {
            Some(namespace) => (namespace.as_str(), Some(other.name.as_str())),
            None => (other.name.as_str(), None),
        };
        lhs.cmp(&rhs)
    }
}

impl PartialOrd for ToolName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<String> for ToolName {
    fn from(name: String) -> Self {
        Self::plain(name)
    }
}

impl From<&str> for ToolName {
    fn from(name: &str) -> Self {
        Self::plain(name)
    }
}
