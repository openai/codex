/// Model-visible namespace metadata for a contributed tool bundle.
///
/// The namespace name participates in the callable tool identity, while the
/// optional description is host-visible metadata used when rendering namespace
/// specs for the model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolNamespace {
    name: String,
    description: Option<String>,
}

impl ToolNamespace {
    /// Creates namespace metadata with no explicit description.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
        }
    }

    /// Sets the model-visible namespace description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Returns the callable namespace name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Returns the optional model-visible namespace description.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}
