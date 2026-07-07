use semver::Version;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeSet;
use thiserror::Error;

pub const MAX_DEPENDENCIES: usize = 20;

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyEcosystem {
    Npm,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    Runtime,
    Development,
    Optional,
}

impl DependencyKind {
    pub fn npm_save_flag(self) -> Option<&'static str> {
        match self {
            Self::Runtime => None,
            Self::Development => Some("--save-dev"),
            Self::Optional => Some("--save-optional"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct DependencySpec {
    pub name: String,
    pub version: String,
}

impl DependencySpec {
    pub fn npm_specifier(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct DependencyCheckRequest {
    pub ecosystem: DependencyEcosystem,
    pub dependencies: Vec<DependencySpec>,
    pub dependency_kind: DependencyKind,
    #[serde(default)]
    pub workdir: Option<String>,
}

impl DependencyCheckRequest {
    pub fn validate(&self) -> Result<(), RequestValidationError> {
        if self.dependencies.is_empty() {
            return Err(RequestValidationError::EmptyDependencies);
        }
        if self.dependencies.len() > MAX_DEPENDENCIES {
            return Err(RequestValidationError::TooManyDependencies {
                actual: self.dependencies.len(),
                maximum: MAX_DEPENDENCIES,
            });
        }

        let mut names = BTreeSet::new();
        for dependency in &self.dependencies {
            validate_package_name(&dependency.name)?;
            Version::parse(&dependency.version).map_err(|source| {
                RequestValidationError::InvalidExactVersion {
                    name: dependency.name.clone(),
                    version: dependency.version.clone(),
                    source,
                }
            })?;
            if !names.insert(dependency.name.clone()) {
                return Err(RequestValidationError::DuplicateDependency(
                    dependency.name.clone(),
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum RequestValidationError {
    #[error("dependency_check requires at least one dependency")]
    EmptyDependencies,
    #[error("dependency_check accepts at most {maximum} dependencies, received {actual}")]
    TooManyDependencies { actual: usize, maximum: usize },
    #[error("invalid npm package name `{0}`")]
    InvalidPackageName(String),
    #[error(
        "dependency `{name}` must use an exact semantic version; `{version}` is invalid: {source}"
    )]
    InvalidExactVersion {
        name: String,
        version: String,
        #[source]
        source: semver::Error,
    },
    #[error("dependency `{0}` was provided more than once")]
    DuplicateDependency(String),
}

fn validate_package_name(name: &str) -> Result<(), RequestValidationError> {
    if name.is_empty() || name.len() > 214 || name.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(RequestValidationError::InvalidPackageName(name.to_string()));
    }

    let valid = if let Some(scoped) = name.strip_prefix('@') {
        let mut parts = scoped.split('/');
        matches!((parts.next(), parts.next(), parts.next()), (Some(scope), Some(package), None) if valid_package_segment(scope) && valid_package_segment(package))
    } else {
        valid_package_segment(name)
    };

    if valid {
        Ok(())
    } else {
        Err(RequestValidationError::InvalidPackageName(name.to_string()))
    }
}

fn valid_package_segment(segment: &str) -> bool {
    let mut bytes = segment.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_' | b'.' | b'~')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(dependencies: &[(&str, &str)]) -> DependencyCheckRequest {
        DependencyCheckRequest {
            ecosystem: DependencyEcosystem::Npm,
            dependencies: dependencies
                .iter()
                .map(|(name, version)| DependencySpec {
                    name: (*name).to_string(),
                    version: (*version).to_string(),
                })
                .collect(),
            dependency_kind: DependencyKind::Runtime,
            workdir: None,
        }
    }

    #[test]
    fn accepts_exact_scoped_and_unscoped_packages() {
        assert!(
            request(&[("zod", "3.23.8"), ("@types/node", "22.15.0")])
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn rejects_ranges_tags_and_duplicate_names() {
        assert!(matches!(
            request(&[("zod", "^3.23.8")]).validate(),
            Err(RequestValidationError::InvalidExactVersion { .. })
        ));
        assert!(matches!(
            request(&[("zod", "latest")]).validate(),
            Err(RequestValidationError::InvalidExactVersion { .. })
        ));
        assert!(matches!(
            request(&[("zod", "3.23.8"), ("zod", "3.24.0")]).validate(),
            Err(RequestValidationError::DuplicateDependency(name)) if name == "zod"
        ));
    }

    #[test]
    fn rejects_unsafe_package_names() {
        for name in ["../zod", "Zod", "@scope", "@scope/../pkg", ""] {
            assert!(matches!(
                request(&[(name, "1.0.0")]).validate(),
                Err(RequestValidationError::InvalidPackageName(actual)) if actual == name
            ));
        }
    }
}
