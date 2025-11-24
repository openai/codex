use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Raised if a project ID cannot be parsed from a string.
#[derive(Debug, Error, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParseProjectIdError {
    /// Raised if an empty value is parsed.
    #[error("empty or missing project id")]
    EmptyValue,
}

/// Represents a project ID.
#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct ProjectId(String);

impl ProjectId {
    /// Creates a new project ID from its string representation.
    /// This assumes that the string is already well-formed and URL
    /// encoded/decoded.
    #[inline]
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }

    /// Returns the string representation of the project ID.
    #[inline]
    pub fn value(&self) -> &str {
        self.0.as_ref()
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for ProjectId {
    type Err = ParseProjectIdError;

    fn from_str(s: &str) -> Result<ProjectId, ParseProjectIdError> {
        if s.is_empty() {
            return Err(ParseProjectIdError::EmptyValue);
        }
        Ok(ProjectId::new(s))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_basic_api() {
        let id: ProjectId = "42".parse().unwrap();
        assert_eq!(id, ProjectId::new("42"));
        assert_eq!("42xxx".parse::<ProjectId>().unwrap().value(), "42xxx");
        assert_eq!(
            "".parse::<ProjectId>(),
            Err(ParseProjectIdError::EmptyValue)
        );
        assert_eq!(ProjectId::new("42").to_string(), "42");

        assert_eq!(
            serde_json::to_string(&ProjectId::new("42")).unwrap(),
            "\"42\""
        );
        assert_eq!(
            serde_json::from_str::<ProjectId>("\"42\"").unwrap(),
            ProjectId::new("42")
        );
    }
}
