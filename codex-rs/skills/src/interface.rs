use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;

use crate::SkillInterface;

const MAX_NAME_LEN: usize = 64;
const MAX_DESCRIPTION_LEN: usize = 1024;

/// Interface metadata deserialized from a skill's `agents/openai.yaml` file.
#[derive(Debug, Default, Deserialize)]
pub struct SkillInterfaceFile {
    display_name: Option<String>,
    short_description: Option<String>,
    icon_small: Option<PathBuf>,
    icon_large: Option<PathBuf>,
    brand_color: Option<String>,
    default_prompt: Option<String>,
}

/// Controls whether interface icons may resolve through a plugin's shared assets directory.
#[derive(Debug, Clone, Copy)]
pub enum SkillInterfaceAssetPolicy<'a> {
    LocalOnly,
    PluginShared { plugin_root: &'a AbsolutePathBuf },
}

/// Validates skill interface metadata and resolves its asset paths.
pub fn resolve_skill_interface(
    interface: Option<SkillInterfaceFile>,
    skill_dir: &AbsolutePathBuf,
    asset_policy: SkillInterfaceAssetPolicy<'_>,
) -> Option<SkillInterface> {
    let interface = interface?;
    let interface = SkillInterface {
        display_name: resolve_str(
            interface.display_name,
            MAX_NAME_LEN,
            "interface.display_name",
        ),
        short_description: resolve_str(
            interface.short_description,
            MAX_DESCRIPTION_LEN,
            "interface.short_description",
        ),
        icon_small: resolve_asset_path(
            skill_dir,
            asset_policy,
            "interface.icon_small",
            interface.icon_small,
        ),
        icon_large: resolve_asset_path(
            skill_dir,
            asset_policy,
            "interface.icon_large",
            interface.icon_large,
        ),
        brand_color: resolve_color_str(interface.brand_color, "interface.brand_color"),
        default_prompt: resolve_str(
            interface.default_prompt,
            MAX_DESCRIPTION_LEN,
            "interface.default_prompt",
        ),
    };
    let has_fields = interface.display_name.is_some()
        || interface.short_description.is_some()
        || interface.icon_small.is_some()
        || interface.icon_large.is_some()
        || interface.brand_color.is_some()
        || interface.default_prompt.is_some();
    has_fields.then_some(interface)
}

fn resolve_asset_path(
    skill_dir: &AbsolutePathBuf,
    asset_policy: SkillInterfaceAssetPolicy<'_>,
    field: &'static str,
    path: Option<PathBuf>,
) -> Option<AbsolutePathBuf> {
    let path = path?;
    if path.as_os_str().is_empty() {
        return None;
    }

    let assets_dir = skill_dir.join("assets");
    if path.is_absolute() {
        tracing::warn!(
            "ignoring {field}: icon must be a relative assets path (not {})",
            assets_dir.display()
        );
        return None;
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(component) => normalized.push(component),
            Component::ParentDir => {
                return resolve_plugin_shared_asset_path(skill_dir, asset_policy, field, &path);
            }
            Component::Prefix(_) | Component::RootDir => {
                tracing::warn!("ignoring {field}: icon path must be under assets/");
                return None;
            }
        }
    }

    let mut components = normalized.components();
    match components.next() {
        Some(Component::Normal(component)) if component == "assets" => {}
        _ => {
            tracing::warn!("ignoring {field}: icon path must be under assets/");
            return None;
        }
    }

    Some(skill_dir.join(normalized))
}

fn resolve_plugin_shared_asset_path(
    skill_dir: &AbsolutePathBuf,
    asset_policy: SkillInterfaceAssetPolicy<'_>,
    field: &'static str,
    path: &Path,
) -> Option<AbsolutePathBuf> {
    let SkillInterfaceAssetPolicy::PluginShared { plugin_root } = asset_policy else {
        tracing::warn!("ignoring {field}: icon path must not contain '..'");
        return None;
    };

    let plugin_assets_dir = lexically_normalize(plugin_root.join("assets").as_path());
    let resolved = lexically_normalize(skill_dir.join(path).as_path());
    if !resolved.starts_with(&plugin_assets_dir) {
        tracing::warn!("ignoring {field}: icon path with '..' must resolve under plugin assets/");
        return None;
    }

    AbsolutePathBuf::try_from(resolved)
        .map_err(|error| {
            tracing::warn!("ignoring {field}: icon path must resolve to an absolute path: {error}");
            error
        })
        .ok()
}

fn lexically_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn resolve_str(value: Option<String>, max_len: usize, field: &'static str) -> Option<String> {
    let value = value?;
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.is_empty() {
        tracing::warn!("ignoring {field}: value is empty");
        return None;
    }
    if value.chars().count() > max_len {
        tracing::warn!("ignoring {field}: exceeds maximum length of {max_len} characters");
        return None;
    }
    Some(value)
}

fn resolve_color_str(value: Option<String>, field: &'static str) -> Option<String> {
    let value = value?;
    let value = value.trim();
    if value.is_empty() {
        tracing::warn!("ignoring {field}: value is empty");
        return None;
    }
    let mut chars = value.chars();
    if value.len() == 7
        && chars.next() == Some('#')
        && chars.all(|character| character.is_ascii_hexdigit())
    {
        Some(value.to_string())
    } else {
        tracing::warn!("ignoring {field}: expected #RRGGBB, got {value}");
        None
    }
}

#[cfg(test)]
#[path = "interface_tests.rs"]
mod tests;
