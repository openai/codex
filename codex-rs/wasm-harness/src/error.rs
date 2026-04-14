use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarnessError(String);

impl HarnessError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HarnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for HarnessError {}

impl From<&str> for HarnessError {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for HarnessError {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<serde_json::Error> for HarnessError {
    fn from(value: serde_json::Error) -> Self {
        Self::new(value.to_string())
    }
}
