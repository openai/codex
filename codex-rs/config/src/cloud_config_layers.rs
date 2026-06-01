//! Conversion from cloud-delivered config TOML fragments into config stack layers.
//!
//! Backend fragments arrive in backend priority order. This module parses each
//! fragment, resolves relative path fields against the cloud config base
//! directory, and returns layers in `ConfigLayerStack` order.

use crate::ConfigLayerEntry;
use crate::ConfigLayerSource;
use crate::TomlValue;
use crate::loader::resolve_relative_paths_in_config_toml;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::fmt;
use std::io;
use thiserror::Error;

/// Config fragment delivered by the cloud config bundle.
///
/// The bundle orders fragments from highest precedence to lowest precedence.
/// This module returns config layers in stack order, so callers can append the
/// result between system and user config without re-sorting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloudConfigFragment {
    pub id: String,
    pub name: String,
    pub contents: String,
}

impl CloudConfigFragment {
    fn source_ref(&self) -> CloudConfigFragmentSource {
        CloudConfigFragmentSource {
            id: self.id.clone(),
            name: self.name.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloudConfigFragmentSource {
    pub id: String,
    pub name: String,
}

impl fmt::Display for CloudConfigFragmentSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.id)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CloudConfigLayerError {
    #[error("failed to parse cloud config fragment {fragment}: {message}")]
    Parse {
        fragment: CloudConfigFragmentSource,
        message: String,
    },
    #[error("invalid cloud config fragment {fragment}: {message}")]
    Invalid {
        fragment: CloudConfigFragmentSource,
        message: String,
    },
}

pub fn cloud_config_layers_from_fragments(
    fragments: impl IntoIterator<Item = CloudConfigFragment>,
    base_dir: &AbsolutePathBuf,
) -> Result<Vec<ConfigLayerEntry>, CloudConfigLayerError> {
    let mut layers = Vec::new();
    for fragment in fragments {
        let source_ref = fragment.source_ref();
        let raw_toml = fragment.contents;
        let value: TomlValue =
            toml::from_str(&raw_toml).map_err(|err| CloudConfigLayerError::Parse {
                fragment: source_ref.clone(),
                message: err.to_string(),
            })?;
        let resolved =
            resolve_relative_paths_in_config_toml(value, base_dir.as_path()).map_err(|err| {
                CloudConfigLayerError::Invalid {
                    fragment: source_ref.clone(),
                    message: err.to_string(),
                }
            })?;
        layers.push(ConfigLayerEntry::new_with_raw_toml(
            ConfigLayerSource::EnterpriseManaged {
                id: fragment.id,
                name: fragment.name,
            },
            resolved,
            raw_toml,
            base_dir.clone(),
        ));
    }

    // Bundle fragments arrive highest-priority first, while ConfigLayerStack
    // folds lowest-priority to highest-priority.
    layers.reverse();
    Ok(layers)
}

impl From<CloudConfigLayerError> for io::Error {
    fn from(error: CloudConfigLayerError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, error)
    }
}

#[cfg(test)]
#[path = "cloud_config_layers_tests.rs"]
mod tests;
