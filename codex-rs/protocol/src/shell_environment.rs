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
    #[cfg(target_os = "windows")]
    normalize_windows_shell_search_environment(&mut env_map, policy);
    env_map
}

/// Canonicalize the case-insensitive variables that control Windows
/// executable lookup before the environment is used for resolution or launch.
///
/// The map is case-sensitive even on Windows, so inherited and configured
/// spellings can otherwise coexist. A configured entry wins over an inherited
/// entry, and the canonical spelling wins if the policy itself contains more
/// than one case variant.
#[cfg(any(target_os = "windows", test))]
fn normalize_windows_shell_search_environment(
    env: &mut HashMap<String, String>,
    policy: &ShellEnvironmentPolicy,
) {
    for canonical_key in ["PATH", "PATHEXT"] {
        let configured_value = windows_environment_value(&policy.r#set, canonical_key)
            .filter(|(key, value)| env.get(*key) == Some(*value))
            .map(|(_, value)| value.clone());
        let value = configured_value.or_else(|| {
            windows_environment_value(env, canonical_key).map(|(_, value)| value.clone())
        });

        env.retain(|key, _| !key.eq_ignore_ascii_case(canonical_key));
        if let Some(value) = value {
            env.insert(canonical_key.to_string(), value);
        }
    }
}

#[cfg(any(target_os = "windows", test))]
fn windows_environment_value<'a>(
    env: &'a HashMap<String, String>,
    canonical_key: &str,
) -> Option<(&'a String, &'a String)> {
    env.get_key_value(canonical_key).or_else(|| {
        env.iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case(canonical_key))
            .min_by(|(left, _), (right, _)| left.cmp(right))
    })
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

    // Step 2 - Apply the default exclude if not disabled.
    if !policy.ignore_default_excludes {
        let default_excludes = vec![
            EnvironmentVariablePattern::new_case_insensitive("*KEY*"),
            EnvironmentVariablePattern::new_case_insensitive("*SECRET*"),
            EnvironmentVariablePattern::new_case_insensitive("*TOKEN*"),
        ];
        env_map.retain(|k, _| !matches_any(k, &default_excludes));
    }

    // Step 3 - Apply custom excludes.
    if !policy.exclude.is_empty() {
        env_map.retain(|k, _| !matches_any(k, &policy.exclude));
    }

    // Step 4 - Apply user-provided overrides.
    for (key, val) in &policy.r#set {
        env_map.insert(key.clone(), val.clone());
    }

    // Step 5 - If include_only is non-empty, keep only the matching vars.
    if !policy.include_only.is_empty() {
        env_map.retain(|k, _| matches_any(k, &policy.include_only));
    }

    // Step 6 - Populate the thread ID environment variable when provided.
    if let Some(thread_id) = thread_id {
        env_map.insert(CODEX_THREAD_ID_ENV_VAR.to_string(), thread_id.to_string());
    }

    env_map
}

#[cfg(not(target_os = "windows"))]
const UNIX_CORE_ENV_VARS: &[&str] = &[
    "PATH", "SHELL", "TMPDIR", "TEMP", "TMP", "HOME", "LANG", "LC_ALL", "LC_CTYPE", "LOGNAME",
    "USER",
];

#[cfg(test)]
mod normalization_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn windows_shell_search_normalization_prefers_configured_canonical_keys() {
        let mut env = HashMap::from([
            ("Path".to_string(), r"C:\inherited".to_string()),
            ("path".to_string(), r"C:\configured-alias".to_string()),
            ("PATH".to_string(), r"C:\configured-canonical".to_string()),
            ("PathExt".to_string(), ".COM;.EXE;.CMD".to_string()),
        ]);
        let policy = ShellEnvironmentPolicy {
            r#set: HashMap::from([
                ("path".to_string(), r"C:\configured-alias".to_string()),
                ("PATH".to_string(), r"C:\configured-canonical".to_string()),
            ]),
            ..Default::default()
        };

        normalize_windows_shell_search_environment(&mut env, &policy);

        assert_eq!(
            env,
            HashMap::from([
                ("PATH".to_string(), r"C:\configured-canonical".to_string()),
                ("PATHEXT".to_string(), ".COM;.EXE;.CMD".to_string()),
            ])
        );
    }
}

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

    #[test]
    fn create_env_policy_rebuild_coalesces_windows_path_and_pathext() {
        let vars = make_vars(&[
            ("Path", r"C:\inherited-bin"),
            ("PATH", r"C:\other-inherited-bin"),
            ("PathExt", ".COM;.EXE;.BAT;.CMD"),
        ]);
        let mut policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            ignore_default_excludes: true,
            ..Default::default()
        };
        policy
            .r#set
            .insert("PATH".to_string(), r"C:\configured-bin".to_string());
        policy
            .r#set
            .insert("pathext".to_string(), ".EXE".to_string());

        let result = create_env_from_vars(vars, &policy, /*thread_id*/ None);

        assert_eq!(
            result.get("PATH").map(String::as_str),
            Some(r"C:\configured-bin")
        );
        assert_eq!(result.get("PATHEXT").map(String::as_str), Some(".EXE"));
        assert_eq!(
            result
                .keys()
                .filter(|key| key.eq_ignore_ascii_case("PATH"))
                .count(),
            1
        );
        assert_eq!(
            result
                .keys()
                .filter(|key| key.eq_ignore_ascii_case("PATHEXT"))
                .count(),
            1
        );
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
}
