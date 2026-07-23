use crate::OPENAI_API_CURATED_MARKETPLACE_NAME;
use crate::OPENAI_CURATED_MARKETPLACE_NAME;
use crate::PluginLoadOutcome;
use crate::loader::curated_plugin_cache_version;
use crate::marketplace::load_marketplace;
use crate::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use crate::startup_sync::curated_plugins_api_marketplace_path;
use crate::startup_sync::curated_plugins_repo_path;
use crate::startup_sync::read_curated_plugins_sha;
use crate::store::DEFAULT_PLUGIN_VERSION;
use crate::store::PluginStore;
use codex_plugin::PluginId;
use codex_protocol::items::is_safe_plugin_relative_path;
use codex_shell_command::bash::extract_bash_command;
use codex_shell_command::bash::parse_shell_lc_plain_commands;
use codex_shell_command::parse_command::is_pathish;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashSet;
use std::path::Component;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
struct TrustedPluginRoot {
    plugin_id: PluginId,
    root: AbsolutePathBuf,
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
/// Trusted means OpenAI-shipped synced code or a server-installed global
/// remote plugin cache entry, not a local override.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrustedPluginRoots {
    roots: Vec<TrustedPluginRoot>,
}

impl TrustedPluginRoots {
    pub fn from_plugin_load_outcome(loaded_plugins: &PluginLoadOutcome, codex_home: &Path) -> Self {
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
                let expected_root = match plugin_id.marketplace_name.as_str() {
                    REMOTE_GLOBAL_MARKETPLACE_NAME => {
                        let active_version = store.active_plugin_version(&plugin_id)?;
                        if active_version == DEFAULT_PLUGIN_VERSION
                            || store.remote_plugin_id(&plugin_id).ok().flatten().is_none()
                        {
                            return None;
                        }
                        store.plugin_root(&plugin_id, &active_version)
                    }
                    OPENAI_CURATED_MARKETPLACE_NAME | OPENAI_API_CURATED_MARKETPLACE_NAME => {
                        let curated_sha = read_curated_plugins_sha(codex_home)?;
                        let expected_root = store
                            .plugin_root(&plugin_id, &curated_plugin_cache_version(&curated_sha));
                        let marketplace_path = match plugin_id.marketplace_name.as_str() {
                            OPENAI_CURATED_MARKETPLACE_NAME => {
                                curated_plugins_repo_path(codex_home)
                                    .join(".agents/plugins/marketplace.json")
                            }
                            OPENAI_API_CURATED_MARKETPLACE_NAME => {
                                curated_plugins_api_marketplace_path(codex_home)
                            }
                            _ => return None,
                        };
                        let marketplace_path = AbsolutePathBuf::try_from(marketplace_path).ok()?;
                        let marketplace = load_marketplace(&marketplace_path).ok()?;
                        if marketplace.name != plugin_id.marketplace_name
                            || !marketplace
                                .plugins
                                .iter()
                                .any(|plugin| plugin.name == plugin_id.plugin_name)
                        {
                            return None;
                        }
                        expected_root
                    }
                    _ => return None,
                };
                if plugin.root != expected_root || !expected_root.as_path().is_dir() {
                    return None;
                }
                let root = expected_root.canonicalize().ok()?;
                root.as_path()
                    .is_dir()
                    .then_some(TrustedPluginRoot { plugin_id, root })
            })
            .filter(|root| seen.insert((root.plugin_id.as_key(), root.root.clone())))
            .collect();
        Self { roots }
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
        let script = script_argument(command.as_slice())?;
        let script = if Path::new(script).is_absolute() {
            AbsolutePathBuf::from_absolute_path_checked(script).ok()?
        } else {
            cwd.join(script)
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
}

/// Converts a path already proven to be below a trusted plugin root into the
/// only path shape that may leave the resolver: non-empty, relative, and
/// slash-separated with no traversal or platform-specific prefixes.
fn normalized_relative_script_path(relative_path: &Path) -> Option<String> {
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

fn script_argument(command: &[String]) -> Option<&str> {
    let [program, args @ ..] = command else {
        return None;
    };
    if let Some(interpreter) = interpreter_name(program) {
        return interpreter_script_argument(&interpreter, args);
    }
    is_pathish(program).then_some(program)
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

fn interpreter_script_argument<'a>(interpreter: &str, args: &'a [String]) -> Option<&'a str> {
    if matches!(interpreter, "powershell" | "pwsh") {
        let [file_flag, script, ..] = args else {
            return None;
        };
        return (file_flag.eq_ignore_ascii_case("-file") && !script.starts_with('-'))
            .then_some(script);
    }

    let mut args = args;
    loop {
        match args {
            [separator, script, ..] if separator == "--" && !script.starts_with('-') => {
                return Some(script);
            }
            [flag, remaining @ ..] if safe_interpreter_flag(interpreter, flag) => {
                args = remaining;
            }
            [script, ..] if !script.starts_with('-') => return Some(script),
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
