use crate::types::EnvironmentVariablePattern;
use crate::types::ShellEnvironmentPolicy;
use crate::types::ShellEnvironmentPolicyInherit;
use std::collections::HashMap;
use std::collections::HashSet;

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
            let core_vars: HashSet<&str> = COMMON_CORE_VARS
                .iter()
                .copied()
                .chain(PLATFORM_CORE_VARS.iter().copied())
                .collect();
            let is_core_var = |name: &str| {
                if cfg!(target_os = "windows") {
                    core_vars
                        .iter()
                        .any(|allowed| allowed.eq_ignore_ascii_case(name))
                } else {
                    core_vars.contains(name)
                }
            };
            vars.into_iter().filter(|(k, _)| is_core_var(k)).collect()
        }
    };

    // Internal helper - does `name` match any pattern in `patterns`?
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

const COMMON_CORE_VARS: &[&str] = &["PATH", "SHELL", "TMPDIR", "TEMP", "TMP"];

#[cfg(target_os = "windows")]
const PLATFORM_CORE_VARS: &[&str] = &[
    "APPDATA",
    "COMSPEC",
    "HOMEDRIVE",
    "HOMEPATH",
    "LOCALAPPDATA",
    "PATHEXT",
    "PROGRAMDATA",
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "PROGRAMW6432",
    "SYSTEMROOT",
    "USERNAME",
    "USERPROFILE",
    "WINDIR",
];

#[cfg(unix)]
const PLATFORM_CORE_VARS: &[&str] = &["HOME", "LANG", "LC_ALL", "LC_CTYPE", "LOGNAME", "USER"];

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use crate::types::ShellEnvironmentPolicy;
    use crate::types::ShellEnvironmentPolicyInherit;
    use std::collections::HashMap;

    fn make_vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn core_inherit_keeps_windows_system_environment() {
        let vars = make_vars(&[
            ("Path", "C:\\Windows\\System32"),
            ("PATHEXT", ".COM;.EXE;.BAT;.CMD"),
            ("APPDATA", "C:\\Users\\alice\\AppData\\Roaming"),
            ("LOCALAPPDATA", "C:\\Users\\alice\\AppData\\Local"),
            ("ProgramFiles", "C:\\Program Files"),
            ("ProgramFiles(x86)", "C:\\Program Files (x86)"),
            ("ProgramW6432", "C:\\Program Files"),
            ("SystemRoot", "C:\\Windows"),
            ("ComSpec", "C:\\Windows\\System32\\cmd.exe"),
            ("USERPROFILE", "C:\\Users\\alice"),
            ("HOMEDRIVE", "C:"),
            ("HOMEPATH", "\\Users\\alice"),
            ("TEMP", "C:\\Temp"),
            ("FOO", "bar"),
        ]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::Core,
            ignore_default_excludes: true,
            ..Default::default()
        };

        let result = populate_env(vars, &policy, Some("thread-123"));

        let expected = HashMap::from([
            ("Path".to_string(), "C:\\Windows\\System32".to_string()),
            ("PATHEXT".to_string(), ".COM;.EXE;.BAT;.CMD".to_string()),
            (
                "APPDATA".to_string(),
                "C:\\Users\\alice\\AppData\\Roaming".to_string(),
            ),
            (
                "LOCALAPPDATA".to_string(),
                "C:\\Users\\alice\\AppData\\Local".to_string(),
            ),
            ("ProgramFiles".to_string(), "C:\\Program Files".to_string()),
            (
                "ProgramFiles(x86)".to_string(),
                "C:\\Program Files (x86)".to_string(),
            ),
            ("ProgramW6432".to_string(), "C:\\Program Files".to_string()),
            ("SystemRoot".to_string(), "C:\\Windows".to_string()),
            (
                "ComSpec".to_string(),
                "C:\\Windows\\System32\\cmd.exe".to_string(),
            ),
            ("USERPROFILE".to_string(), "C:\\Users\\alice".to_string()),
            ("HOMEDRIVE".to_string(), "C:".to_string()),
            ("HOMEPATH".to_string(), "\\Users\\alice".to_string()),
            ("TEMP".to_string(), "C:\\Temp".to_string()),
            ("CODEX_THREAD_ID".to_string(), "thread-123".to_string()),
        ]);

        assert_eq!(result, expected);
    }
}
