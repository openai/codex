use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use codex_config::HookHandlerConfig;
use codex_desktop_distribution::VerifiedDesktopDistribution;
use codex_desktop_distribution::locate_current_or_installed_distribution;
use codex_plugin::PluginHookSource;
use codex_plugin::PluginHookSourceKind;
use codex_plugin::PluginId;
use codex_protocol::protocol::HookEventName;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;

use crate::loader::load_plugin_hooks;
use crate::manifest::PluginManifestHooks;
use crate::manifest::PluginManifestPaths;
use crate::manifest::load_plugin_manifest;

const INTERNAL_HOOKS_REGISTRY_PATH: &str = "plugins/app-bundled-internal-hooks.json";
const INTERNAL_HOOKS_REGISTRY_SCHEMA_VERSION: u32 = 1;
const COMPUTER_USE_PLUGIN_NAME: &str = "computer-use";
const COMPUTER_USE_EXECUTABLE: &str = "Codex Computer Use.app/Contents/SharedSupport/SkyComputerUseClient.app/Contents/MacOS/SkyComputerUseClient";
const COMPUTER_USE_STOP_SUFFIX: &str = " codex-stop-hook";

pub(crate) trait AppBundledInternalHookLoader: Send + Sync {
    fn load(
        &self,
        plugin_id: &PluginId,
        plugin_data_root: &AbsolutePathBuf,
    ) -> Result<Vec<PluginHookSource>, AppBundledInternalHookError>;
}

pub(crate) struct DesktopAppBundledInternalHookLoader;

impl AppBundledInternalHookLoader for DesktopAppBundledInternalHookLoader {
    fn load(
        &self,
        plugin_id: &PluginId,
        plugin_data_root: &AbsolutePathBuf,
    ) -> Result<Vec<PluginHookSource>, AppBundledInternalHookError> {
        let distribution = locate_current_or_installed_distribution()
            .map_err(|error| AppBundledInternalHookError::new(error.stage(), error.to_string()))?;
        load_from_authenticated_resources(&distribution, plugin_id, plugin_data_root)
    }
}

#[derive(Debug)]
pub(crate) struct AppBundledInternalHookError {
    pub stage: &'static str,
    pub message: String,
}

impl AppBundledInternalHookError {
    fn new(stage: &'static str, message: impl Into<String>) -> Self {
        Self {
            stage,
            message: message.into(),
        }
    }
}

pub(crate) fn is_app_bundled_internal_candidate(plugin_id: &PluginId) -> bool {
    plugin_id.plugin_name == COMPUTER_USE_PLUGIN_NAME
        && plugin_id.marketplace_name == crate::OPENAI_BUNDLED_MARKETPLACE_NAME
}

trait AuthenticatedResources {
    fn directory(&self, relative_path: &Path) -> Result<AbsolutePathBuf, String>;
    fn file(&self, relative_path: &Path) -> Result<AbsolutePathBuf, String>;
    fn reverify(&self) -> Result<(), String>;
}

impl AuthenticatedResources for VerifiedDesktopDistribution {
    fn directory(&self, relative_path: &Path) -> Result<AbsolutePathBuf, String> {
        self.contained_directory(relative_path)
            .map_err(|error| error.to_string())
    }

    fn file(&self, relative_path: &Path) -> Result<AbsolutePathBuf, String> {
        self.contained_file(relative_path)
            .map_err(|error| error.to_string())
    }

    fn reverify(&self) -> Result<(), String> {
        self.reverify().map_err(|error| error.to_string())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InternalHooksRegistry {
    schema_version: u32,
    plugins: Vec<InternalHooksPlugin>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InternalHooksPlugin {
    plugin_id: String,
    hook_declarations: Vec<String>,
    referenced_files: Vec<String>,
}

#[derive(Deserialize)]
struct BundledMarketplace {
    name: String,
    plugins: Vec<BundledMarketplacePlugin>,
}

#[derive(Deserialize)]
struct BundledMarketplacePlugin {
    name: String,
    source: BundledMarketplacePluginSource,
}

#[derive(Deserialize)]
struct BundledMarketplacePluginSource {
    source: String,
    path: String,
}

fn load_from_authenticated_resources(
    resources: &dyn AuthenticatedResources,
    plugin_id: &PluginId,
    plugin_data_root: &AbsolutePathBuf,
) -> Result<Vec<PluginHookSource>, AppBundledInternalHookError> {
    if !is_app_bundled_internal_candidate(plugin_id) {
        return Err(AppBundledInternalHookError::new(
            "opt-in",
            "plugin is not in the core app-bundled internal hook allowlist",
        ));
    }

    let registry_path = resources
        .file(Path::new(INTERNAL_HOOKS_REGISTRY_PATH))
        .map_err(|error| AppBundledInternalHookError::new("registry containment", error))?;
    let registry: InternalHooksRegistry = read_json(&registry_path, "registry")?;
    if registry.schema_version != INTERNAL_HOOKS_REGISTRY_SCHEMA_VERSION {
        return Err(AppBundledInternalHookError::new(
            "registry identity",
            format!(
                "unsupported registry schema version {}",
                registry.schema_version
            ),
        ));
    }
    let matching_entries = registry
        .plugins
        .into_iter()
        .filter(|entry| entry.plugin_id == plugin_id.as_key())
        .collect::<Vec<_>>();
    let entry = match matching_entries.as_slice() {
        [] => {
            resources.reverify().map_err(|error| {
                AppBundledInternalHookError::new("distribution reverification", error)
            })?;
            return Ok(Vec::new());
        }
        [entry] => entry,
        _ => {
            return Err(AppBundledInternalHookError::new(
                "registry identity",
                "registry must not contain duplicate entries for the allowlisted plugin",
            ));
        }
    };
    require_unique_paths(&entry.hook_declarations, "hook declarations")?;
    require_unique_paths(&entry.referenced_files, "referenced files")?;
    if entry.hook_declarations.is_empty() || entry.referenced_files.is_empty() {
        return Err(AppBundledInternalHookError::new(
            "registry identity",
            "allowlisted internal hooks require explicit declarations and referenced files",
        ));
    }

    let marketplace_relative = PathBuf::from("plugins").join(&plugin_id.marketplace_name);
    let marketplace_root = resources
        .directory(&marketplace_relative)
        .map_err(|error| AppBundledInternalHookError::new("marketplace containment", error))?;
    let marketplace_path = resources
        .file(
            &marketplace_relative
                .join(".agents")
                .join("plugins")
                .join("marketplace.json"),
        )
        .map_err(|error| AppBundledInternalHookError::new("marketplace containment", error))?;
    let marketplace: BundledMarketplace = read_json(&marketplace_path, "marketplace")?;
    if marketplace.name != plugin_id.marketplace_name {
        return Err(AppBundledInternalHookError::new(
            "marketplace identity",
            "marketplace name does not match the allowlisted plugin id",
        ));
    }
    let marketplace_entries = marketplace
        .plugins
        .into_iter()
        .filter(|plugin| plugin.name == plugin_id.plugin_name)
        .collect::<Vec<_>>();
    let [marketplace_entry] = marketplace_entries.as_slice() else {
        return Err(AppBundledInternalHookError::new(
            "marketplace identity",
            "marketplace must contain exactly one matching plugin entry",
        ));
    };
    let expected_source_path = format!("./plugins/{}", plugin_id.plugin_name);
    if marketplace_entry.source.source != "local"
        || marketplace_entry.source.path != expected_source_path
    {
        return Err(AppBundledInternalHookError::new(
            "marketplace identity",
            "matching plugin must use the exact bundled local source path",
        ));
    }

    let plugin_relative = marketplace_relative
        .join("plugins")
        .join(&plugin_id.plugin_name);
    let plugin_root = resources
        .directory(&plugin_relative)
        .map_err(|error| AppBundledInternalHookError::new("plugin containment", error))?;
    if !plugin_root
        .as_path()
        .starts_with(marketplace_root.as_path())
    {
        return Err(AppBundledInternalHookError::new(
            "plugin containment",
            "plugin root escaped the authenticated marketplace",
        ));
    }
    resources
        .file(&plugin_relative.join(".codex-plugin").join("plugin.json"))
        .map_err(|error| AppBundledInternalHookError::new("manifest containment", error))?;
    let manifest = load_plugin_manifest(plugin_root.as_path()).ok_or_else(|| {
        AppBundledInternalHookError::new("manifest loading", "missing or invalid plugin manifest")
    })?;
    if manifest.name != plugin_id.plugin_name {
        return Err(AppBundledInternalHookError::new(
            "manifest identity",
            "plugin manifest name does not match the allowlisted plugin id",
        ));
    }

    let declaration_paths = entry
        .hook_declarations
        .iter()
        .map(|relative_path| {
            resources
                .file(&plugin_relative.join(relative_path))
                .map_err(|error| {
                    AppBundledInternalHookError::new("hook declaration containment", error)
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    for relative_path in &entry.referenced_files {
        resources
            .file(&plugin_relative.join(relative_path))
            .map_err(|error| {
                AppBundledInternalHookError::new("hook resource containment", error)
            })?;
    }

    let (mut sources, warnings) = load_plugin_hooks(
        &plugin_root,
        plugin_id,
        plugin_data_root,
        &PluginManifestPaths {
            skills: None,
            mcp_servers: None,
            apps: None,
            hooks: Some(PluginManifestHooks::Paths(declaration_paths)),
        },
    );
    if !warnings.is_empty() {
        return Err(AppBundledInternalHookError::new(
            "hook loading",
            warnings.join("; "),
        ));
    }
    if sources.is_empty() {
        return Err(AppBundledInternalHookError::new(
            "hook loading",
            "authenticated hook declarations contained no supported command hooks",
        ));
    }
    validate_hook_command_references(&sources, &entry.referenced_files)?;
    for source in &mut sources {
        source.kind = PluginHookSourceKind::AppBundledInternal;
    }
    resources
        .reverify()
        .map_err(|error| AppBundledInternalHookError::new("distribution reverification", error))?;
    Ok(sources)
}

fn validate_hook_command_references(
    sources: &[PluginHookSource],
    referenced_files: &[String],
) -> Result<(), AppBundledInternalHookError> {
    let referenced_files = referenced_files
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut handler_count = 0;
    for source in sources {
        for (event_name, groups) in source.hooks.clone().into_matcher_groups() {
            for group in groups {
                if event_name != HookEventName::Stop || group.matcher.is_some() {
                    return Err(AppBundledInternalHookError::new(
                        "hook contract",
                        "app-bundled Computer Use hooks must use an unfiltered Stop event",
                    ));
                }
                for handler in group.hooks {
                    handler_count += 1;
                    let HookHandlerConfig::Command {
                        command,
                        command_windows,
                        timeout_sec,
                        r#async,
                        status_message,
                    } = handler
                    else {
                        return Err(AppBundledInternalHookError::new(
                            "hook contract",
                            "app-bundled Computer Use hooks must be command hooks",
                        ));
                    };
                    if timeout_sec != Some(10) || r#async || status_message.is_some() {
                        return Err(AppBundledInternalHookError::new(
                            "hook contract",
                            "app-bundled Computer Use hooks must use the exact synchronous 10s contract",
                        ));
                    }
                    let (selected_command, prefix) = if cfg!(windows) {
                        let command_windows = command_windows.as_deref().ok_or_else(|| {
                            AppBundledInternalHookError::new(
                                "hook contract",
                                "app-bundled Computer Use hooks require an explicit Windows executable",
                            )
                        })?;
                        (command_windows, "\"%PLUGIN_ROOT%\\")
                    } else {
                        (command.as_str(), "\"${PLUGIN_ROOT}/")
                    };
                    let executable = validate_hook_command_reference(
                        selected_command,
                        prefix,
                        &referenced_files,
                    )?;
                    if cfg!(windows) && !executable.to_ascii_lowercase().ends_with(".exe") {
                        return Err(AppBundledInternalHookError::new(
                            "hook contract",
                            "app-bundled Windows hooks must directly execute a bundled .exe",
                        ));
                    }
                    if !cfg!(windows) && executable != COMPUTER_USE_EXECUTABLE {
                        return Err(AppBundledInternalHookError::new(
                            "hook contract",
                            "app-bundled Computer Use hook executable identity changed",
                        ));
                    }
                }
            }
        }
    }
    if handler_count != 1 {
        return Err(AppBundledInternalHookError::new(
            "hook contract",
            "app-bundled Computer Use hooks require exactly one handler",
        ));
    }
    Ok(())
}

fn validate_hook_command_reference(
    command: &str,
    prefix: &str,
    referenced_files: &HashSet<&str>,
) -> Result<String, AppBundledInternalHookError> {
    let Some(remainder) = command.strip_prefix(prefix) else {
        return Err(AppBundledInternalHookError::new(
            "hook command containment",
            "internal hook commands must execute an explicitly referenced file below PLUGIN_ROOT",
        ));
    };
    let Some(closing_quote) = remainder.find('"') else {
        return Err(AppBundledInternalHookError::new(
            "hook command containment",
            "internal hook executable path must be quoted",
        ));
    };
    let relative_path = &remainder[..closing_quote];
    let suffix = &remainder[closing_quote + 1..];
    if relative_path.is_empty()
        || relative_path
            .split(['/', '\\'])
            .any(|component| component.is_empty() || component == "." || component == "..")
        || suffix != COMPUTER_USE_STOP_SUFFIX
    {
        return Err(AppBundledInternalHookError::new(
            "hook command containment",
            "internal hook command has an invalid bundled executable path or argument boundary",
        ));
    }
    let relative_path = relative_path.replace('\\', "/");
    if !referenced_files.contains(relative_path.as_str()) {
        return Err(AppBundledInternalHookError::new(
            "hook command containment",
            "internal hook executable is not listed in the authenticated registry",
        ));
    }
    Ok(relative_path)
}

fn require_unique_paths(
    paths: &[String],
    label: &'static str,
) -> Result<(), AppBundledInternalHookError> {
    let unique = paths.iter().collect::<HashSet<_>>();
    if unique.len() == paths.len() {
        return Ok(());
    }
    Err(AppBundledInternalHookError::new(
        "registry identity",
        format!("registry contains duplicate {label}"),
    ))
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: &AbsolutePathBuf,
    label: &'static str,
) -> Result<T, AppBundledInternalHookError> {
    let contents = std::fs::read_to_string(path.as_path()).map_err(|error| {
        AppBundledInternalHookError::new(label, format!("failed to read {label}: {error}"))
    })?;
    serde_json::from_str(&contents).map_err(|error| {
        AppBundledInternalHookError::new(label, format!("failed to parse {label}: {error}"))
    })
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub(crate) struct TestAuthenticatedResources {
        root: AbsolutePathBuf,
        pub reverify_succeeds: bool,
    }

    impl TestAuthenticatedResources {
        pub(crate) fn new(root: AbsolutePathBuf) -> Self {
            let root =
                std::fs::canonicalize(root.as_path()).expect("canonical test resources path");
            let root = AbsolutePathBuf::try_from(root).expect("absolute test resources path");
            Self {
                root,
                reverify_succeeds: true,
            }
        }

        fn contained(
            &self,
            relative_path: &Path,
            directory: bool,
        ) -> Result<AbsolutePathBuf, String> {
            if relative_path
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
            {
                return Err("invalid relative path".to_string());
            }
            let candidate = self.root.join(relative_path);
            let canonical =
                std::fs::canonicalize(candidate.as_path()).map_err(|error| error.to_string())?;
            let canonical =
                AbsolutePathBuf::try_from(canonical).map_err(|error| error.to_string())?;
            if canonical == self.root || !canonical.as_path().starts_with(self.root.as_path()) {
                return Err("path escaped test resources".to_string());
            }
            let metadata = std::fs::symlink_metadata(canonical.as_path())
                .map_err(|error| error.to_string())?;
            if metadata.file_type().is_symlink()
                || (directory && !metadata.is_dir())
                || (!directory && !metadata.is_file())
            {
                return Err("unexpected test resource type".to_string());
            }
            Ok(canonical)
        }
    }

    impl AuthenticatedResources for TestAuthenticatedResources {
        fn directory(&self, relative_path: &Path) -> Result<AbsolutePathBuf, String> {
            self.contained(relative_path, /*directory*/ true)
        }

        fn file(&self, relative_path: &Path) -> Result<AbsolutePathBuf, String> {
            self.contained(relative_path, /*directory*/ false)
        }

        fn reverify(&self) -> Result<(), String> {
            self.reverify_succeeds
                .then_some(())
                .ok_or_else(|| "test reverification failed".to_string())
        }
    }

    pub(crate) fn load(
        resources: &TestAuthenticatedResources,
        plugin_id: &PluginId,
        plugin_data_root: &AbsolutePathBuf,
    ) -> Result<Vec<PluginHookSource>, AppBundledInternalHookError> {
        load_from_authenticated_resources(resources, plugin_id, plugin_data_root)
    }
}
