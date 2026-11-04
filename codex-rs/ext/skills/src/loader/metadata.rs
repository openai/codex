use std::io;

use codex_exec_server::EnvironmentAccess;
use codex_exec_server::EnvironmentAccessExt;
use codex_exec_server::GetMetadataOptions;
use codex_exec_server::ReadFileOptions;
use codex_skills::ParsedSkillMetadata;
use codex_skills::SkillDependencies;
use codex_skills::SkillInterface;
use codex_skills::SkillInterfaceAssetPolicy;
use codex_skills::SkillParseError;
use codex_skills::SkillPolicy;
use codex_skills::parse_skill_metadata;
use codex_skills::resolve_skill_interface;
use codex_utils_absolute_path::AbsolutePathBuf;

use super::SKILLS_METADATA_FILENAME;
use super::discovery::SkillMetadataDiscovery;

#[derive(Default)]
pub(super) struct LoadedSkillMetadata {
    pub(super) interface: Option<SkillInterface>,
    pub(super) dependencies: Option<SkillDependencies>,
    pub(super) policy: Option<SkillPolicy>,
}

pub(super) async fn load_host_skill_metadata(
    file_system: &dyn EnvironmentAccess,
    skill_path: &AbsolutePathBuf,
    metadata: &SkillMetadataDiscovery,
    plugin_root: Option<&AbsolutePathBuf>,
) -> LoadedSkillMetadata {
    // Fail open: optional metadata should not block loading SKILL.md.
    let Some(skill_dir) = skill_path.parent() else {
        return LoadedSkillMetadata::default();
    };
    let metadata_path = match metadata {
        SkillMetadataDiscovery::Present(path) => path,
        SkillMetadataDiscovery::Absent => return LoadedSkillMetadata::default(),
        SkillMetadataDiscovery::Probe(path) => {
            match file_system
                .get_metadata(path, GetMetadataOptions::default())
                .await
            {
                Ok(metadata) if metadata.is_file => {}
                Ok(_) => return LoadedSkillMetadata::default(),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return LoadedSkillMetadata::default();
                }
                Err(error) => {
                    tracing::warn!(
                        "ignoring {path}: failed to stat {label}: {error}",
                        label = SKILLS_METADATA_FILENAME
                    );
                    return LoadedSkillMetadata::default();
                }
            }
            path
        }
    };

    let contents = match file_system
        .read_file_text(metadata_path, ReadFileOptions::default())
        .await
    {
        Ok(contents) => contents,
        Err(error) => {
            tracing::warn!(
                "ignoring {metadata_path}: failed to read {label}: {error}",
                label = SKILLS_METADATA_FILENAME
            );
            return LoadedSkillMetadata::default();
        }
    };

    let parsed = match parse_skill_metadata(&contents) {
        Ok(parsed) => parsed,
        Err(error) => {
            tracing::warn!(
                "ignoring {metadata_path}: invalid {label}: {error}",
                label = SKILLS_METADATA_FILENAME
            );
            return LoadedSkillMetadata::default();
        }
    };

    let ParsedSkillMetadata {
        interface,
        dependencies,
        policy,
    } = parsed;
    let asset_policy = match plugin_root {
        Some(plugin_root) => SkillInterfaceAssetPolicy::PluginShared { plugin_root },
        None => SkillInterfaceAssetPolicy::LocalOnly,
    };
    LoadedSkillMetadata {
        interface: resolve_skill_interface(interface, &skill_dir, asset_policy),
        dependencies,
        policy,
    }
}
pub(super) fn sanitize_single_line(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn validate_len(
    value: &str,
    max_len: usize,
    field_name: &'static str,
) -> Result<(), SkillParseError> {
    if value.is_empty() {
        return Err(SkillParseError::MissingField(field_name));
    }
    if value.chars().count() > max_len {
        return Err(SkillParseError::InvalidField {
            field: field_name,
            reason: format!("exceeds maximum length of {max_len} characters"),
        });
    }
    Ok(())
}
