use super::discover_project_layers;
use super::layer_io;
use super::load_config_toml_for_required_layer_raw;
use super::load_requirements_toml;
use super::load_root_checkout_project_config;
use super::project_root_markers_from_config;
use super::project_trust_context;
use super::requirements_layers_from_legacy_scheme;
use super::system_config_toml_file_with_overrides;
use super::system_requirements_toml_file_with_overrides;
use crate::CONFIG_TOML_FILE;
use crate::ConfigLayerSource;
use crate::LoaderOverrides;
use crate::RequirementSource;
use crate::RequirementsLayerEntry;
use crate::default_project_root_markers;
use crate::merge_toml_values;
use codex_file_system::ExecutorFileSystem;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use toml::Value as TomlValue;

/// Executor-local configuration and requirements layers before schema-specific
/// path resolution or requirements composition.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalConfigLayers {
    pub config: LocalTomlLayerStack<ConfigLayerSource>,
    pub requirements: LocalTomlLayerStack<RequirementSource>,
}

impl LocalConfigLayers {
    /// Retains only the requested TOML paths and drops empty layers.
    ///
    /// An empty path selects the entire document. RPC boundaries should reject
    /// that form if whole-document reads are not part of their contract.
    pub fn project(self, config_paths: &[Vec<String>], requirements_paths: &[Vec<String>]) -> Self {
        Self {
            config: self.config.project(config_paths),
            requirements: self.requirements.project(requirements_paths),
        }
    }
}

/// One ordered set of executor-local TOML layers.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalTomlLayerStack<S> {
    /// Layers ordered from lowest to highest precedence.
    pub layers: Vec<LocalTomlLayer<S>>,
    /// Position at which a caller should insert cloud-provided layers.
    pub cloud_insertion_index: usize,
}

impl<S> LocalTomlLayerStack<S> {
    fn project(self, paths: &[Vec<String>]) -> Self {
        let selectors = SelectorNode::from_paths(paths);
        let mut projected_layers = Vec::new();
        let mut cloud_insertion_index = 0;
        for (index, layer) in self.layers.into_iter().enumerate() {
            let Some(toml) = project_toml(&layer.toml, &selectors) else {
                continue;
            };
            if index < self.cloud_insertion_index {
                cloud_insertion_index += 1;
            }
            projected_layers.push(LocalTomlLayer { toml, ..layer });
        }
        Self {
            layers: projected_layers,
            cloud_insertion_index,
        }
    }
}

/// One executor-local TOML source with the directory used to interpret its
/// relative paths.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalTomlLayer<S> {
    pub source: S,
    pub base_dir: AbsolutePathBuf,
    pub toml: TomlValue,
}

/// Loads the fixed executor-local configuration sources used by environment
/// config reads.
///
/// Cloud, selected profiles, session flags, and thread-provided layers are not
/// included. Project discovery uses only the executor's system and base-user
/// configuration.
pub async fn load_local_config_layers(
    fs: &dyn ExecutorFileSystem,
    codex_home: &Path,
    cwd: &AbsolutePathBuf,
) -> io::Result<LocalConfigLayers> {
    load_local_config_layers_with_overrides(fs, codex_home, cwd, &LoaderOverrides::default()).await
}

pub(super) async fn load_local_config_layers_with_overrides(
    fs: &dyn ExecutorFileSystem,
    codex_home: &Path,
    cwd: &AbsolutePathBuf,
    overrides: &LoaderOverrides,
) -> io::Result<LocalConfigLayers> {
    let codex_home = AbsolutePathBuf::from_absolute_path(codex_home)?;
    let loaded_managed = layer_io::load_config_layers_internal(
        fs,
        codex_home.as_path(),
        overrides.clone(),
        /*strict_config*/ false,
    )
    .await?;

    let system_file = system_config_toml_file_with_overrides(overrides)?;
    let system =
        load_config_toml_for_required_layer_raw(fs, &system_file, /*strict_config*/ false).await?;
    let user_file = codex_home.join(CONFIG_TOML_FILE);
    let user =
        load_config_toml_for_required_layer_raw(fs, &user_file, /*strict_config*/ false).await?;

    let mut discovery_config = TomlValue::Table(toml::map::Map::new());
    merge_toml_values(&mut discovery_config, &system.toml);
    merge_toml_values(&mut discovery_config, &user.toml);
    let project_root_markers = project_root_markers_from_config(&discovery_config)?
        .unwrap_or_else(default_project_root_markers);
    let trust_context = project_trust_context(
        fs,
        &discovery_config,
        cwd,
        &project_root_markers,
        codex_home.as_path(),
        &user_file,
    )
    .await?;
    let project_layers = discover_project_layers(
        fs,
        cwd,
        &trust_context.project_root,
        &trust_context,
        codex_home.as_path(),
        /*strict_config*/ false,
    )
    .await?;

    let mut config_layers = vec![
        LocalTomlLayer {
            source: ConfigLayerSource::System { file: system_file },
            base_dir: system.base_dir,
            toml: system.toml,
        },
        LocalTomlLayer {
            source: ConfigLayerSource::User {
                file: user_file,
                profile: None,
            },
            base_dir: user.base_dir,
            toml: user.toml,
        },
    ];
    append_project_layers(fs, &mut config_layers, project_layers.layers).await?;

    let requirements =
        local_requirements_layers(fs, codex_home.as_path(), overrides, loaded_managed.clone())
            .await?;
    append_legacy_config_layers(&mut config_layers, loaded_managed, &codex_home)?;

    Ok(LocalConfigLayers {
        config: LocalTomlLayerStack {
            layers: config_layers,
            // Cloud config follows the required system layer.
            cloud_insertion_index: 1,
        },
        requirements,
    })
}

async fn append_project_layers(
    fs: &dyn ExecutorFileSystem,
    output: &mut Vec<LocalTomlLayer<ConfigLayerSource>>,
    layers: Vec<super::DiscoveredProjectLayer>,
) -> io::Result<()> {
    for layer in layers {
        if layer.disabled_reason.is_some() {
            continue;
        }
        let mut config = layer.config;
        if layer.hooks_config_folder_override.is_some()
            && let Some(table) = config.as_table_mut()
        {
            table.remove("hooks");
        }
        output.push(LocalTomlLayer {
            source: ConfigLayerSource::Project {
                dot_codex_folder: layer.dot_codex_folder.clone(),
            },
            base_dir: layer.dot_codex_folder,
            toml: config,
        });

        let Some(hooks_config_folder) = layer.hooks_config_folder_override else {
            continue;
        };
        let root_config =
            load_root_checkout_project_config(fs, &hooks_config_folder, /*is_trusted*/ true)
                .await?;
        let Some(hooks) = root_config.get("hooks") else {
            continue;
        };
        output.push(LocalTomlLayer {
            source: ConfigLayerSource::Project {
                dot_codex_folder: hooks_config_folder.clone(),
            },
            base_dir: hooks_config_folder,
            toml: TomlValue::Table(toml::map::Map::from_iter([(
                "hooks".to_string(),
                hooks.clone(),
            )])),
        });
    }
    Ok(())
}

fn append_legacy_config_layers(
    output: &mut Vec<LocalTomlLayer<ConfigLayerSource>>,
    loaded: layer_io::LoadedConfigLayers,
    codex_home: &AbsolutePathBuf,
) -> io::Result<()> {
    if let Some(config) = loaded.managed_config {
        let base_dir = config.file.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Managed config file {} has no parent directory",
                    config.file.as_path().display()
                ),
            )
        })?;
        output.push(LocalTomlLayer {
            source: ConfigLayerSource::LegacyManagedConfigTomlFromFile { file: config.file },
            base_dir,
            toml: config.managed_config,
        });
    }
    if let Some(config) = loaded.managed_config_from_mdm {
        output.push(LocalTomlLayer {
            source: ConfigLayerSource::LegacyManagedConfigTomlFromMdm,
            base_dir: codex_home.clone(),
            toml: config.managed_config,
        });
    }
    Ok(())
}

async fn local_requirements_layers(
    fs: &dyn ExecutorFileSystem,
    codex_home: &Path,
    overrides: &LoaderOverrides,
    loaded_managed: layer_io::LoadedConfigLayers,
) -> io::Result<LocalTomlLayerStack<RequirementSource>> {
    let system_file = system_requirements_toml_file_with_overrides(overrides)?;
    let system = load_requirements_toml(fs, &system_file).await?;
    let cloud_insertion_index = usize::from(system.is_some());
    let mut entries = Vec::new();
    entries.extend(system);
    entries.extend(requirements_layers_from_legacy_scheme(
        loaded_managed,
        codex_home,
    )?);

    #[cfg(target_os = "macos")]
    {
        let codex_home = AbsolutePathBuf::from_absolute_path(codex_home)?;
        entries.extend(
            super::macos::load_managed_admin_requirements_layer(
                overrides
                    .macos_managed_config_requirements_base64
                    .as_deref(),
            )
            .await?
            .map(|layer| layer.with_base_dir(codex_home)),
        );
    }

    let mut layers = Vec::with_capacity(entries.len());
    for entry in entries {
        layers.push(local_requirements_layer(entry)?);
    }
    Ok(LocalTomlLayerStack {
        layers,
        cloud_insertion_index,
    })
}

fn local_requirements_layer(
    entry: RequirementsLayerEntry,
) -> io::Result<LocalTomlLayer<RequirementSource>> {
    let (source, toml, base_dir) = entry.into_raw_parts()?;
    let base_dir = base_dir.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("requirements layer {source} has no base directory"),
        )
    })?;
    Ok(LocalTomlLayer {
        source,
        base_dir,
        toml,
    })
}

#[derive(Default)]
struct SelectorNode {
    terminal: bool,
    children: BTreeMap<String, SelectorNode>,
}

impl SelectorNode {
    fn from_paths(paths: &[Vec<String>]) -> Self {
        let mut root = Self::default();
        for path in paths {
            root.insert(path);
        }
        root
    }

    fn insert(&mut self, path: &[String]) {
        if self.terminal {
            return;
        }
        let Some((segment, remaining)) = path.split_first() else {
            self.terminal = true;
            self.children.clear();
            return;
        };
        self.children
            .entry(segment.clone())
            .or_default()
            .insert(remaining);
    }
}

fn project_toml(value: &TomlValue, selector: &SelectorNode) -> Option<TomlValue> {
    let projected = project_toml_value(value, selector);
    if !selector.terminal && projected.as_table().is_some_and(toml::map::Map::is_empty) {
        return None;
    }
    Some(projected)
}

fn project_toml_value(value: &TomlValue, selector: &SelectorNode) -> TomlValue {
    if selector.terminal {
        return value.clone();
    }
    let Some(table) = value.as_table() else {
        // Preserve a non-table ancestor so it can still override lower layers.
        return value.clone();
    };
    let mut projected = toml::map::Map::new();
    for (key, value) in table {
        let Some(child_selector) = selector.children.get(key) else {
            continue;
        };
        projected.insert(key.clone(), project_toml_value(value, child_selector));
    }
    TomlValue::Table(projected)
}
