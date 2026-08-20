use crate::OPENAI_API_CURATED_MARKETPLACE_NAME;
use crate::OPENAI_CURATED_MARKETPLACE_NAME;
use crate::OPENAI_PRIMARY_RUNTIME_MARKETPLACE_NAME;
use crate::PluginLoadOutcome;
use crate::loader::curated_plugin_cache_version;
use crate::marketplace::MarketplacePluginSource;
use crate::marketplace::find_marketplace_plugin;
use crate::marketplace_policy::primary_runtime_marketplace_root;
use crate::plugin_metrics::PluginMetricsOperation;
use crate::plugin_metrics::ResolvedPluginMetricsOperation;
use crate::plugin_metrics::load_plugin_metrics_operations;
use crate::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use crate::startup_sync::curated_plugins_api_marketplace_path;
use crate::startup_sync::curated_plugins_repo_path;
use crate::startup_sync::read_curated_plugins_sha;
use crate::store::DEFAULT_PLUGIN_VERSION;
use crate::store::PluginStore;
use crate::store::plugin_version_for_source;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::GetMetadataOptions;
use codex_exec_server::ReadFileOptions;
use codex_plugin::PluginId;
use codex_protocol::items::is_safe_plugin_relative_path;
use codex_shell_command::bash::extract_bash_command;
use codex_shell_command::bash::parse_shell_lc_plain_commands;
use codex_shell_command::parse_command::is_pathish;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::Component;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
struct TrustedPluginRoot {
    plugin_id: PluginId,
    root: AbsolutePathBuf,
    metrics_operations_by_path: BTreeMap<String, PluginMetricsOperation>,
}

/// Trusted plugin command attribution safe to carry into command analytics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginCommandAttribution {
    pub plugin_id: PluginId,
    pub normalized_relative_path: String,
}

impl PluginCommandAttribution {
    /// Returns the paired fields used at command protocol boundaries.
    pub fn serialized_fields(&self) -> (String, String) {
        (
            self.plugin_id.as_key(),
            self.normalized_relative_path.clone(),
        )
    }
}

/// Active first-party roots eligible for command attribution.
/// Trusted means OpenAI-shipped synced or bundled runtime code, or a
/// server-installed global remote plugin cache entry, not a local override.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrustedPluginRoots {
    roots: Vec<TrustedPluginRoot>,
}

impl TrustedPluginRoots {
    pub fn from_plugin_load_outcome(loaded_plugins: &PluginLoadOutcome, codex_home: &Path) -> Self {
        let primary_runtime_marketplace_root = primary_runtime_marketplace_root();
        let Ok(store) = PluginStore::try_new(codex_home.to_path_buf()) else {
            return Self::default();
        };
        let mut seen = HashSet::new();
        let roots = loaded_plugins
            .plugins()
            .iter()
            .filter(|plugin| plugin.is_active())
            .filter_map(|plugin| {
                let plugin_id = PluginId::parse(&plugin.config_name).ok()?;
                let expected_root = Self::expected_plugin_root(
                    &store,
                    codex_home,
                    &plugin_id,
                    primary_runtime_marketplace_root.as_deref(),
                )?;
                if plugin.root != expected_root || !expected_root.as_path().is_dir() {
                    return None;
                }
                let root = expected_root.canonicalize().ok()?;
                root.as_path().is_dir().then(|| TrustedPluginRoot {
                    plugin_id,
                    metrics_operations_by_path: load_plugin_metrics_operations(&root)
                        .unwrap_or_default(),
                    root,
                })
            })
            .filter(|root| seen.insert((root.plugin_id.as_key(), root.root.clone())))
            .collect();
        Self { roots }
    }

    fn expected_plugin_root(
        store: &PluginStore,
        codex_home: &Path,
        plugin_id: &PluginId,
        primary_runtime_marketplace_root: Option<&Path>,
    ) -> Option<AbsolutePathBuf> {
        match plugin_id.marketplace_name.as_str() {
            REMOTE_GLOBAL_MARKETPLACE_NAME => {
                let active_version = store.active_plugin_version(plugin_id)?;
                if active_version == DEFAULT_PLUGIN_VERSION
                    || store.remote_plugin_id(plugin_id).ok().flatten().is_none()
                {
                    return None;
                }
                Some(store.plugin_root(plugin_id, &active_version))
            }
            OPENAI_CURATED_MARKETPLACE_NAME | OPENAI_API_CURATED_MARKETPLACE_NAME => {
                let curated_sha = read_curated_plugins_sha(codex_home)?;
                let expected_root =
                    store.plugin_root(plugin_id, &curated_plugin_cache_version(&curated_sha));
                let marketplace_path = match plugin_id.marketplace_name.as_str() {
                    OPENAI_CURATED_MARKETPLACE_NAME => curated_plugins_repo_path(codex_home)
                        .join(".agents/plugins/marketplace.json"),
                    OPENAI_API_CURATED_MARKETPLACE_NAME => {
                        curated_plugins_api_marketplace_path(codex_home)
                    }
                    _ => return None,
                };
                Self::marketplace_plugin(marketplace_path, plugin_id)?;
                Some(expected_root)
            }
            OPENAI_PRIMARY_RUNTIME_MARKETPLACE_NAME => {
                let marketplace_root = primary_runtime_marketplace_root?;
                let expected_source_root = AbsolutePathBuf::from_absolute_path_checked(
                    marketplace_root
                        .join("plugins")
                        .join(&plugin_id.plugin_name),
                )
                .ok()?;
                let marketplace_plugin = Self::marketplace_plugin(
                    marketplace_root.join(".agents/plugins/marketplace.json"),
                    plugin_id,
                )?;
                let MarketplacePluginSource::Local { path: source_root } =
                    marketplace_plugin.source
                else {
                    return None;
                };
                if source_root != expected_source_root {
                    return None;
                }
                let plugin_version = plugin_version_for_source(source_root.as_path()).ok()?;
                Some(store.plugin_root(plugin_id, &plugin_version))
            }
            _ => None,
        }
    }

    fn marketplace_plugin(
        marketplace_path: std::path::PathBuf,
        plugin_id: &PluginId,
    ) -> Option<crate::marketplace::ResolvedMarketplacePlugin> {
        let marketplace_path =
            AbsolutePathBuf::from_absolute_path_checked(marketplace_path).ok()?;
        let plugin = find_marketplace_plugin(&marketplace_path, &plugin_id.plugin_name).ok()?;
        (plugin.plugin_id == *plugin_id).then_some(plugin)
    }

    /// Resolves one exact command to one trusted plugin script.
    ///
    /// Complex shell syntax, missing files, symlink escapes, and overlapping
    /// matches are all unattributed by design.
    pub fn resolve_attribution(
        &self,
        command: &[String],
        cwd: &AbsolutePathBuf,
    ) -> Option<PluginCommandAttribution> {
        let command = single_plain_command(command)?;
        let invocation = script_invocation(command.as_slice())?;
        let script = if Path::new(invocation.script).is_absolute() {
            AbsolutePathBuf::from_absolute_path_checked(invocation.script).ok()?
        } else {
            cwd.join(invocation.script)
        }
        .canonicalize()
        .ok()?;
        if !script.as_path().is_file() {
            return None;
        }

        let mut matches = self.roots.iter().filter_map(|root| {
            let relative_path = script
                .as_path()
                .strip_prefix(root.root.as_path())
                .ok()
                .filter(|relative_path| !relative_path.as_os_str().is_empty())?;
            Some(PluginCommandAttribution {
                plugin_id: root.plugin_id.clone(),
                normalized_relative_path: normalized_relative_script_path(relative_path)?,
            })
        });
        let attribution = matches.next()?;
        matches.next().is_none().then_some(attribution)
    }

    /// Resolves one exact command to one trusted manifest-declared operation.
    pub fn resolve_metrics_operation(
        &self,
        command: &[String],
        cwd: &AbsolutePathBuf,
    ) -> Option<ResolvedPluginMetricsOperation> {
        let attribution = self.resolve_attribution(command, cwd)?;
        self.metrics_operation_for_attribution(attribution)
    }

    fn metrics_operation_for_attribution(
        &self,
        attribution: PluginCommandAttribution,
    ) -> Option<ResolvedPluginMetricsOperation> {
        let mut matches = self.roots.iter().filter_map(|root| {
            (root.plugin_id == attribution.plugin_id)
                .then(|| {
                    root.metrics_operations_by_path
                        .get(&attribution.normalized_relative_path)
                })
                .flatten()
        });
        let operation = matches.next()?.clone();
        matches
            .next()
            .is_none()
            .then_some(ResolvedPluginMetricsOperation {
                plugin_id: attribution.plugin_id,
                operation,
            })
    }

    /// Resolves a trusted script on the selected executor filesystem.
    ///
    /// Remote commands can use a path convention that the app-server host cannot
    /// canonicalize. Match the target-native path to one trusted local plugin
    /// script, then require the executor-side file to have the same contents.
    pub async fn resolve_executor_attribution(
        &self,
        command: &[String],
        cwd: &PathUri,
        file_system: &dyn ExecutorFileSystem,
    ) -> Option<PluginCommandAttribution> {
        let command = single_plain_command(command)?;
        let invocation = script_invocation(command.as_slice())?;
        let script = cwd.join(invocation.script).ok()?;
        let candidate = self.local_candidate_for_executor_script(&script)?;
        let script = file_system
            .canonicalize(&script, /*sandbox*/ None)
            .await
            .ok()?;
        if !executor_plugin_root_matches(&script, &candidate.attribution) {
            return None;
        }
        let metadata = file_system
            .get_metadata(
                &script,
                GetMetadataOptions::default(),
                /*sandbox*/ None,
            )
            .await
            .ok()?;
        if !metadata.is_file || metadata.size != candidate.contents.len() as u64 {
            return None;
        }
        let contents = file_system
            .read_file(&script, ReadFileOptions::default(), /*sandbox*/ None)
            .await
            .ok()?;
        (contents == candidate.contents).then_some(candidate.attribution)
    }

    /// Resolves one trusted executor script to one manifest-declared operation.
    pub async fn resolve_metrics_operation_in_filesystem(
        &self,
        command: &[String],
        cwd: &PathUri,
        file_system: &dyn ExecutorFileSystem,
    ) -> Option<ResolvedPluginMetricsOperation> {
        let attribution = self
            .resolve_executor_attribution(command, cwd, file_system)
            .await?;
        self.metrics_operation_for_attribution(attribution)
    }

    fn local_candidate_for_executor_script(
        &self,
        script: &PathUri,
    ) -> Option<ExecutorAttributionCandidate> {
        let suffixes = normalized_script_suffixes(script);
        let mut matches = self.roots.iter().filter_map(|root| {
            let (script, normalized_relative_path) = suffixes.iter().find_map(|suffix| {
                let script = root.root.join(suffix).canonicalize().ok()?;
                let relative_path = script.as_path().strip_prefix(root.root.as_path()).ok()?;
                if !script.as_path().is_file() {
                    return None;
                }
                let normalized_relative_path = normalized_relative_script_path(relative_path)?;
                Some((script, normalized_relative_path))
            })?;
            Some(ExecutorAttributionCandidate {
                attribution: PluginCommandAttribution {
                    plugin_id: root.plugin_id.clone(),
                    normalized_relative_path,
                },
                contents: std::fs::read(script.as_path()).ok()?,
            })
        });
        let candidate = matches.next()?;
        matches.next().is_none().then_some(candidate)
    }
}

struct ExecutorAttributionCandidate {
    attribution: PluginCommandAttribution,
    contents: Vec<u8>,
}

fn normalized_script_suffixes(script: &PathUri) -> Vec<String> {
    let path = script.inferred_native_path_string().replace('\\', "/");
    let components = path
        .split('/')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>();
    (0..components.len())
        .filter_map(|start| {
            let suffix = components[start..].join("/");
            is_safe_plugin_relative_path(&suffix).then_some(suffix)
        })
        .collect()
}

fn executor_plugin_root_matches(script: &PathUri, attribution: &PluginCommandAttribution) -> bool {
    let relative_depth = attribution.normalized_relative_path.split('/').count();
    let Some(root) = script.ancestors().nth(relative_depth) else {
        return false;
    };
    let path = root.inferred_native_path_string().replace('\\', "/");
    let components = path
        .split('/')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>();
    let [.., plugins, cache, marketplace, plugin, version] = components.as_slice() else {
        return false;
    };
    *plugins == "plugins"
        && *cache == "cache"
        && *marketplace == attribution.plugin_id.marketplace_name.as_str()
        && *plugin == attribution.plugin_id.plugin_name.as_str()
        && !version.is_empty()
}

/// Returns the structurally parsed arguments following a single script command.
/// Callers must keep the values in-process and must not log or serialize them.
pub fn command_script_arguments(command: &[String]) -> Option<Vec<String>> {
    let command = single_plain_command(command)?;
    Some(script_invocation(command.as_slice())?.arguments.to_vec())
}

/// Converts a path already proven to be below a trusted plugin root into the
/// only path shape that may leave the resolver: non-empty, relative, and
/// slash-separated with no traversal or platform-specific prefixes.
pub(crate) fn normalized_relative_script_path(relative_path: &Path) -> Option<String> {
    let normalized = relative_path
        .components()
        .map(|component| {
            let Component::Normal(component) = component else {
                return None;
            };
            component.to_str()
        })
        .collect::<Option<Vec<_>>>()?
        .join("/");

    is_safe_plugin_relative_path(&normalized).then_some(normalized)
}

fn single_plain_command(command: &[String]) -> Option<Vec<String>> {
    if let Some(commands) = parse_shell_lc_plain_commands(command) {
        let [command] = commands.as_slice() else {
            return None;
        };
        return single_plain_command(command);
    }
    if let Some(script) = windows_shell_script(command) {
        let wrapper = ["sh".to_string(), "-lc".to_string(), script.to_string()];
        return single_plain_command(&wrapper);
    }
    if extract_bash_command(command).is_some() {
        return None;
    }
    Some(command.to_vec())
}

struct ScriptInvocation<'a> {
    script: &'a str,
    arguments: &'a [String],
}

fn script_invocation(command: &[String]) -> Option<ScriptInvocation<'_>> {
    let [program, args @ ..] = command else {
        return None;
    };
    if let Some(interpreter) = interpreter_name(program) {
        return interpreter_script_invocation(&interpreter, args);
    }
    is_pathish(program).then_some(ScriptInvocation {
        script: program,
        arguments: args,
    })
}

fn interpreter_name(program: &str) -> Option<String> {
    let basename = executable_basename(program)?;
    let basename = basename.to_ascii_lowercase();
    let basename = basename.strip_suffix(".exe").unwrap_or(&basename);
    matches!(
        basename,
        "bash"
            | "node"
            | "nodejs"
            | "perl"
            | "php"
            | "powershell"
            | "pwsh"
            | "python"
            | "python3"
            | "ruby"
            | "sh"
            | "zsh"
    )
    .then(|| basename.to_string())
}

fn interpreter_script_invocation<'a>(
    interpreter: &str,
    args: &'a [String],
) -> Option<ScriptInvocation<'a>> {
    if matches!(interpreter, "powershell" | "pwsh") {
        let [file_flag, script, arguments @ ..] = args else {
            return None;
        };
        return (file_flag.eq_ignore_ascii_case("-file") && !script.starts_with('-'))
            .then_some(ScriptInvocation { script, arguments });
    }

    let mut args = args;
    loop {
        match args {
            [separator, script, arguments @ ..]
                if separator == "--" && !script.starts_with('-') =>
            {
                return Some(ScriptInvocation { script, arguments });
            }
            [flag, remaining @ ..] if safe_interpreter_flag(interpreter, flag) => {
                args = remaining;
            }
            [script, arguments @ ..] if !script.starts_with('-') => {
                return Some(ScriptInvocation { script, arguments });
            }
            _ => return None,
        }
    }
}

fn safe_interpreter_flag(interpreter: &str, flag: &str) -> bool {
    matches!(
        (interpreter, flag),
        ("python" | "python3", "-u") | ("bash" | "sh" | "zsh", "-e")
    )
}

fn executable_basename(program: &str) -> Option<&str> {
    program
        .rsplit(['/', '\\'])
        .next()
        .filter(|basename| !basename.is_empty())
}

fn windows_shell_script(command: &[String]) -> Option<&str> {
    let [program, args @ ..] = command else {
        return None;
    };
    let basename = executable_basename(program)?.to_ascii_lowercase();
    if matches!(basename.as_str(), "cmd" | "cmd.exe") {
        let [flag, script] = args else {
            return None;
        };
        return flag.eq_ignore_ascii_case("/c").then_some(script);
    }
    if !matches!(
        basename.as_str(),
        "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe"
    ) {
        return None;
    }

    let [flags @ .., command_flag, script] = args else {
        return None;
    };
    if !matches!(
        command_flag.to_ascii_lowercase().as_str(),
        "-command" | "-c"
    ) {
        return None;
    }
    flags
        .iter()
        .all(|flag| {
            matches!(
                flag.to_ascii_lowercase().as_str(),
                "-nologo" | "-noprofile" | "-noninteractive"
            )
        })
        .then_some(script)
}

#[cfg(test)]
#[path = "script_attribution_tests.rs"]
mod tests;
