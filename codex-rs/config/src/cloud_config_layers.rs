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
mod tests {
    use super::*;
    use crate::CONFIG_TOML_FILE;
    use crate::ConfigLayerStack;
    use crate::ConfigLayerStackOrdering;
    use crate::ConfigRequirements;
    use crate::ConfigRequirementsToml;
    use crate::config_toml::ConfigToml;
    use crate::first_layer_config_error_from_entries;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;
    use std::path::Path;

    fn fragment(id: &str, name: &str, contents: &str) -> CloudConfigFragment {
        CloudConfigFragment {
            id: id.to_string(),
            name: name.to_string(),
            contents: contents.to_string(),
        }
    }

    fn toml(contents: &str) -> TomlValue {
        toml::from_str(contents).expect("test TOML should parse")
    }

    fn base_dir() -> AbsolutePathBuf {
        test_path_buf("/var/lib/codex").abs()
    }

    #[test]
    fn layers_are_returned_in_stack_order() {
        let base_dir = base_dir();
        let layers = cloud_config_layers_from_fragments(
            vec![
                fragment("high", "High priority", "model = \"cloud-high\""),
                fragment("low", "Low priority", "model_provider = \"cloud-low\""),
            ],
            &base_dir,
        )
        .expect("cloud config layers should compose");

        assert_eq!(
            layers
                .iter()
                .map(|layer| layer.name.clone())
                .collect::<Vec<_>>(),
            vec![
                ConfigLayerSource::EnterpriseManaged {
                    id: "low".to_string(),
                    name: "Low priority".to_string(),
                },
                ConfigLayerSource::EnterpriseManaged {
                    id: "high".to_string(),
                    name: "High priority".to_string(),
                },
            ]
        );
    }

    #[test]
    fn enterprise_layers_precede_user_and_override_system() {
        let base_dir = base_dir();
        let mut layers = vec![ConfigLayerEntry::new(
            ConfigLayerSource::System {
                file: test_path_buf("/etc/codex/config.toml").abs(),
            },
            toml(
                r#"
model = "system"
model_provider = "system"
review_model = "system-review"
"#,
            ),
        )];
        layers.extend(
            cloud_config_layers_from_fragments(
                vec![
                    fragment("high", "High priority", "model_provider = \"cloud-high\""),
                    fragment("low", "Low priority", "review_model = \"cloud-low-review\""),
                ],
                &base_dir,
            )
            .expect("cloud config layers should compose"),
        );
        layers.push(ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: test_path_buf("/home/alice/.codex/config.toml").abs(),
                profile: None,
            },
            toml("model = \"user\""),
        ));

        let stack = ConfigLayerStack::new(
            layers,
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )
        .expect("stack should be ordered");

        assert_eq!(
            stack
                .get_layers(
                    ConfigLayerStackOrdering::LowestPrecedenceFirst,
                    /*include_disabled*/ false
                )
                .iter()
                .map(|layer| layer.name.clone())
                .collect::<Vec<_>>(),
            vec![
                ConfigLayerSource::System {
                    file: test_path_buf("/etc/codex/config.toml").abs(),
                },
                ConfigLayerSource::EnterpriseManaged {
                    id: "low".to_string(),
                    name: "Low priority".to_string(),
                },
                ConfigLayerSource::EnterpriseManaged {
                    id: "high".to_string(),
                    name: "High priority".to_string(),
                },
                ConfigLayerSource::User {
                    file: test_path_buf("/home/alice/.codex/config.toml").abs(),
                    profile: None,
                },
            ]
        );
        assert_eq!(
            stack.effective_config(),
            toml(
                r#"
model = "user"
model_provider = "cloud-high"
review_model = "cloud-low-review"
"#,
            )
        );
    }

    #[test]
    fn relative_absolute_path_fields_resolve_against_base_dir() {
        let base_dir = base_dir();
        let layers = cloud_config_layers_from_fragments(
            vec![fragment(
                "cfg_123",
                "Base policy",
                "model_instructions_file = \"instructions.md\"",
            )],
            &base_dir,
        )
        .expect("relative paths should match existing MDM semantics");

        let path = layers[0]
            .config
            .get("model_instructions_file")
            .and_then(TomlValue::as_str)
            .expect("path should be present");
        let expected =
            AbsolutePathBuf::resolve_path_against_base("instructions.md", base_dir.as_path());
        assert_eq!(path, expected.to_string_lossy());
    }

    #[test]
    fn home_relative_path_fields_are_allowed_and_resolved() {
        let base_dir = base_dir();
        let layers = cloud_config_layers_from_fragments(
            vec![fragment(
                "cfg_123",
                "Base policy",
                "model_instructions_file = \"~/instructions.md\"",
            )],
            &base_dir,
        )
        .expect("home-relative paths should be accepted");

        let path = layers[0]
            .config
            .get("model_instructions_file")
            .and_then(TomlValue::as_str)
            .expect("path should be present");
        let expected =
            AbsolutePathBuf::resolve_path_against_base("~/instructions.md", base_dir.as_path());
        assert_eq!(path, expected.to_string_lossy());
    }

    #[tokio::test]
    async fn raw_toml_diagnostics_use_enterprise_layer_name() {
        let base_dir = base_dir();
        let layers = cloud_config_layers_from_fragments(
            vec![fragment(
                "cfg_123",
                "Base policy",
                "model_instructions_file = \"instructions.md\"\nmodel = 1",
            )],
            &base_dir,
        )
        .expect("cloud config layers should parse");

        let error = first_layer_config_error_from_entries::<ConfigToml>(&layers, CONFIG_TOML_FILE)
            .await
            .expect("invalid raw TOML should produce a layer diagnostic");

        assert_eq!(
            error.path,
            Path::new("enterprise-managed config Base policy (cfg_123)").to_path_buf()
        );
        assert_eq!(error.range.start.line, 2);
        assert_eq!(error.range.start.column, 9);
        assert!(error.message.contains("invalid type: integer `1`"));
    }
}
