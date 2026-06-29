use crate::config_types::EnvironmentVariablePattern;
use crate::config_types::ShellEnvironmentPolicy;
use crate::config_types::ShellEnvironmentPolicyInherit;
use std::collections::HashMap;

pub const CODEX_THREAD_ID_ENV_VAR: &str = "CODEX_THREAD_ID";

/// Construct a shell environment from the supplied process environment and
/// shell-environment policy.
pub fn create_env(
    policy: &ShellEnvironmentPolicy,
    thread_id: Option<&str>,
) -> HashMap<String, String> {
    create_env_from_vars(std::env::vars(), policy, thread_id)
}

pub fn create_env_from_vars<I>(
    vars: I,
    policy: &ShellEnvironmentPolicy,
    thread_id: Option<&str>,
) -> HashMap<String, String>
where
    I: IntoIterator<Item = (String, String)>,
{
    let mut env_map = populate_env(vars, policy, thread_id);

    if cfg!(target_os = "windows") {
        // This is a workaround to address the failures we are seeing in the
        // following tests when run via Bazel on Windows:
        //
        // ```
        // suite::shell_command::unicode_output::with_login
        // suite::shell_command::unicode_output::without_login
        // ```
        //
        // Currently, we can only reproduce these failures in CI, which makes
        // iteration times long, so we include this quick fix for now to unblock
        // getting the Windows Bazel build running.
        if !env_map.keys().any(|k| k.eq_ignore_ascii_case("PATHEXT")) {
            env_map.insert("PATHEXT".to_string(), ".COM;.EXE;.BAT;.CMD".to_string());
        }
    }
    env_map
}

/// Returns whether the policy explicitly opts into inheriting `RUST_LOG`.
pub fn policy_explicitly_includes_inherited_rust_log(policy: &ShellEnvironmentPolicy) -> bool {
    !policy.include_only.is_empty()
        && policy
            .include_only
            .iter()
            .any(|pattern| pattern.matches("RUST_LOG"))
}

pub fn populate_env<I>(
    vars: I,
    policy: &ShellEnvironmentPolicy,
    thread_id: Option<&str>,
) -> HashMap<String, String>
where
    I: IntoIterator<Item = (String, String)>,
{
    // Step 1 - determine the starting set of variables based on the
    // `inherit` strategy.
    let mut env_map: HashMap<String, String> = match policy.inherit {
        ShellEnvironmentPolicyInherit::All => vars.into_iter().collect(),
        ShellEnvironmentPolicyInherit::None => HashMap::new(),
        ShellEnvironmentPolicyInherit::Core => {
            #[cfg(not(target_os = "windows"))]
            let core_env_vars = UNIX_CORE_ENV_VARS;
            #[cfg(target_os = "windows")]
            let core_env_vars = WINDOWS_CORE_ENV_VARS;

            vars.into_iter()
                .filter(|(k, _)| {
                    core_env_vars
                        .iter()
                        .any(|allowed| allowed.eq_ignore_ascii_case(k))
                })
                .collect()
        }
    };

    let matches_any = |name: &str, patterns: &[EnvironmentVariablePattern]| -> bool {
        patterns.iter().any(|pattern| pattern.matches(name))
    };

    // Step 2 - Harnesses like Codex Desktop and the VS Code extension spawn the
    // app-server with `RUST_LOG=warn` by default. Do not pass that inherited
    // value to subprocesses unless the user explicitly includes it.
    if !policy_explicitly_includes_inherited_rust_log(policy) {
        env_map.retain(|key, _| !is_rust_log(key));
    }

    // Step 3 - Apply the default exclude if not disabled.
    if !policy.ignore_default_excludes {
        let default_excludes = vec![
            EnvironmentVariablePattern::new_case_insensitive("*KEY*"),
            EnvironmentVariablePattern::new_case_insensitive("*SECRET*"),
            EnvironmentVariablePattern::new_case_insensitive("*TOKEN*"),
        ];
        env_map.retain(|k, _| !matches_any(k, &default_excludes));
    }

    // Step 4 - Apply custom excludes.
    if !policy.exclude.is_empty() {
        env_map.retain(|k, _| !matches_any(k, &policy.exclude));
    }

    // Step 5 - Apply user-provided overrides.
    for (key, val) in &policy.r#set {
        env_map.insert(key.clone(), val.clone());
    }

    // Step 6 - If include_only is non-empty, keep only the matching vars.
    if !policy.include_only.is_empty() {
        env_map.retain(|k, _| matches_any(k, &policy.include_only));
    }

    // Step 7 - Populate the thread ID environment variable when provided.
    if let Some(thread_id) = thread_id {
        env_map.insert(CODEX_THREAD_ID_ENV_VAR.to_string(), thread_id.to_string());
    }

    env_map
}

fn is_rust_log(name: &str) -> bool {
    if cfg!(target_os = "windows") {
        name.eq_ignore_ascii_case("RUST_LOG")
    } else {
        name == "RUST_LOG"
    }
}

#[cfg(not(target_os = "windows"))]
const UNIX_CORE_ENV_VARS: &[&str] = &[
    "PATH", "SHELL", "TMPDIR", "TEMP", "TMP", "HOME", "LANG", "LC_ALL", "LC_CTYPE", "LOGNAME",
    "USER",
];

#[cfg(target_os = "windows")]
pub const WINDOWS_CORE_ENV_VARS: &[&str] = &[
    // Core path resolution
    "PATH",
    "PATHEXT",
    // Shell and system roots
    "SHELL",
    "COMSPEC",
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    // User context and profiles
    "USERNAME",
    "USERDOMAIN",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    // Program locations
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "PROGRAMW6432",
    "PROGRAMDATA",
    // App data and caches
    "LOCALAPPDATA",
    "APPDATA",
    // Temp locations
    "TEMP",
    "TMP",
    "TMPDIR",
    // Common shells/pwsh hints
    "POWERSHELL",
    "PWSH",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherited_rust_log_is_removed() {
        let vars = vec![
            ("PATH".to_string(), "/usr/bin".to_string()),
            ("RUST_LOG".to_string(), "warn".to_string()),
        ];

        let result = populate_env(
            vars,
            &ShellEnvironmentPolicy::default(),
            /*thread_id*/ None,
        );

        assert_eq!(
            result,
            HashMap::from([("PATH".to_string(), "/usr/bin".to_string())])
        );
    }

    #[test]
    fn explicitly_included_rust_log_is_preserved() {
        let vars = vec![("RUST_LOG".to_string(), "codex_core=trace".to_string())];
        let policy = ShellEnvironmentPolicy {
            include_only: vec![EnvironmentVariablePattern::new_case_insensitive("RUST_LOG")],
            ..Default::default()
        };

        let result = populate_env(vars.clone(), &policy, /*thread_id*/ None);

        assert_eq!(result, vars.into_iter().collect());
    }

    #[test]
    fn explicit_rust_log_override_is_preserved() {
        let vars = vec![("RUST_LOG".to_string(), "warn".to_string())];
        let policy = ShellEnvironmentPolicy {
            r#set: HashMap::from([("RUST_LOG".to_string(), "codex_core=trace".to_string())]),
            ..Default::default()
        };

        let result = populate_env(vars, &policy, /*thread_id*/ None);

        assert_eq!(
            result,
            HashMap::from([("RUST_LOG".to_string(), "codex_core=trace".to_string())])
        );
    }
}

#[cfg(all(test, target_os = "windows"))]
mod windows_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn make_vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn core_inherit_preserves_windows_startup_vars_case_insensitively() {
        let vars = make_vars(&[
            ("Shell", "C:\\Program Files\\Git\\bin\\bash.exe"),
            ("SystemRoot", "C:\\Windows"),
            ("AppData", "C:\\Users\\codex\\AppData\\Roaming"),
            ("TmpDir", "C:\\Temp\\custom"),
            ("OPENAI_API_KEY", "secret"),
        ]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::Core,
            ignore_default_excludes: true,
            ..Default::default()
        };

        // Check a few sample vars instead of the full Windows core list.
        let result = populate_env(vars, &policy, /*thread_id*/ None);
        let expected = HashMap::from([
            (
                "Shell".to_string(),
                "C:\\Program Files\\Git\\bin\\bash.exe".to_string(),
            ),
            ("SystemRoot".to_string(), "C:\\Windows".to_string()),
            (
                "AppData".to_string(),
                "C:\\Users\\codex\\AppData\\Roaming".to_string(),
            ),
            ("TmpDir".to_string(), "C:\\Temp\\custom".to_string()),
        ]);

        assert_eq!(result, expected);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn create_env_inserts_pathext_on_windows_when_missing() {
        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::None,
            ignore_default_excludes: true,
            ..Default::default()
        };

        let result = create_env_from_vars(Vec::new(), &policy, /*thread_id*/ None);
        let expected = HashMap::from([("PATHEXT".to_string(), ".COM;.EXE;.BAT;.CMD".to_string())]);

        assert_eq!(result, expected);
    }
}

#[cfg(all(test, not(target_os = "windows")))]
mod non_windows_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn make_vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn core_inherit_preserves_non_windows_core_vars_case_insensitively() {
        let vars = make_vars(&[
            ("path", "/usr/bin"),
            ("home", "/home/codex"),
            ("TmpDir", "/tmp/custom"),
            ("OPENAI_API_KEY", "secret"),
        ]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::Core,
            ignore_default_excludes: true,
            ..Default::default()
        };

        let result = populate_env(vars, &policy, /*thread_id*/ None);
        let expected = HashMap::from([
            ("path".to_string(), "/usr/bin".to_string()),
            ("home".to_string(), "/home/codex".to_string()),
            ("TmpDir".to_string(), "/tmp/custom".to_string()),
        ]);

        assert_eq!(result, expected);
    }

    #[test]
    fn similarly_named_unix_env_vars_are_preserved() {
        let vars = make_vars(&[("rust_log", "warn"), ("Rust_Log", "debug")]);

        let result = populate_env(
            vars.clone(),
            &ShellEnvironmentPolicy::default(),
            /*thread_id*/ None,
        );

        assert_eq!(result, vars.into_iter().collect());
    }
}
