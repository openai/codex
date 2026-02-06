use crate::config::types::EnvironmentVariablePattern;
use crate::config::types::ShellEnvironmentPolicy;
use crate::config::types::ShellEnvironmentPolicyInherit;
use std::collections::HashMap;
use std::collections::HashSet;

/// Construct an environment map based on the rules in the specified policy. The
/// resulting map can be passed directly to `Command::envs()` after calling
/// `env_clear()` to ensure no unintended variables are leaked to the spawned
/// process.
///
/// The derivation follows the algorithm documented in the struct-level comment
/// for [`ShellEnvironmentPolicy`].
pub fn create_env(policy: &ShellEnvironmentPolicy) -> HashMap<String, String> {
    populate_env(std::env::vars(), policy)
}

fn populate_env<I>(vars: I, policy: &ShellEnvironmentPolicy) -> HashMap<String, String>
where
    I: IntoIterator<Item = (String, String)>,
{
    // Step 1 – determine the starting set of variables based on the
    // `inherit` strategy.
    let mut env_map: HashMap<String, String> = match policy.inherit {
        ShellEnvironmentPolicyInherit::All => vars.into_iter().collect(),
        ShellEnvironmentPolicyInherit::None => HashMap::new(),
        ShellEnvironmentPolicyInherit::Core => {
            const CORE_VARS: &[&str] = &[
                "HOME", "LOGNAME", "PATH", "SHELL", "USER", "USERNAME", "TMPDIR", "TEMP", "TMP",
            ];
            let allow: HashSet<&str> = CORE_VARS.iter().copied().collect();
            vars.into_iter()
                .filter(|(k, _)| allow.contains(k.as_str()))
                .collect()
        }
    };

    // Internal helper – does `name` match **any** pattern in `patterns`?
    let matches_any = |name: &str, patterns: &[EnvironmentVariablePattern]| -> bool {
        patterns.iter().any(|pattern| pattern.matches(name))
    };

    // Step 2 – Apply the default exclude if not disabled.
    if !policy.ignore_default_excludes {
        let default_excludes = vec![
            EnvironmentVariablePattern::new_case_insensitive("*KEY*"),
            EnvironmentVariablePattern::new_case_insensitive("*SECRET*"),
            EnvironmentVariablePattern::new_case_insensitive("*TOKEN*"),
        ];
        env_map.retain(|k, _| !matches_any(k, &default_excludes));
    }

    // Step 3 – Apply custom excludes.
    if !policy.exclude.is_empty() {
        env_map.retain(|k, _| !matches_any(k, &policy.exclude));
    }

    // Step 4 – Apply user-provided overrides.
    for (key, val) in &policy.r#set {
        env_map.insert(key.clone(), val.clone());
    }

    // Step 5 – If include_only is non-empty, keep *only* the matching vars.
    if !policy.include_only.is_empty() {
        env_map.retain(|k, _| matches_any(k, &policy.include_only));
    }

    env_map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::ShellEnvironmentPolicyInherit;
    use maplit::hashmap;

    fn make_vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_core_inherit_defaults_keep_sensitive_vars() {
        let vars = make_vars(&[
            ("PATH", "/usr/bin"),
            ("HOME", "/home/user"),
            ("API_KEY", "secret"),
            ("SECRET_TOKEN", "t"),
        ]);

        let policy = ShellEnvironmentPolicy::default(); // inherit All, default excludes ignored
        let result = populate_env(vars, &policy);

        let expected: HashMap<String, String> = hashmap! {
            "PATH".to_string() => "/usr/bin".to_string(),
            "HOME".to_string() => "/home/user".to_string(),
            "API_KEY".to_string() => "secret".to_string(),
            "SECRET_TOKEN".to_string() => "t".to_string(),
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_core_inherit_with_default_excludes_enabled() {
        let vars = make_vars(&[
            ("PATH", "/usr/bin"),
            ("HOME", "/home/user"),
            ("API_KEY", "secret"),
            ("SECRET_TOKEN", "t"),
        ]);

        let policy = ShellEnvironmentPolicy {
            ignore_default_excludes: false, // apply KEY/SECRET/TOKEN filter
            ..Default::default()
        };
        let result = populate_env(vars, &policy);

        let expected: HashMap<String, String> = hashmap! {
            "PATH".to_string() => "/usr/bin".to_string(),
            "HOME".to_string() => "/home/user".to_string(),
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_include_only() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("FOO", "bar")]);

        let policy = ShellEnvironmentPolicy {
            // skip default excludes so nothing is removed prematurely
            ignore_default_excludes: true,
            include_only: vec![EnvironmentVariablePattern::new_case_insensitive("*PATH")],
            ..Default::default()
        };

        let result = populate_env(vars, &policy);

        let expected: HashMap<String, String> = hashmap! {
            "PATH".to_string() => "/usr/bin".to_string(),
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_set_overrides() {
        let vars = make_vars(&[("PATH", "/usr/bin")]);

        let mut policy = ShellEnvironmentPolicy {
            ignore_default_excludes: true,
            ..Default::default()
        };
        policy.r#set.insert("NEW_VAR".to_string(), "42".to_string());

        let result = populate_env(vars, &policy);

        let expected: HashMap<String, String> = hashmap! {
            "PATH".to_string() => "/usr/bin".to_string(),
            "NEW_VAR".to_string() => "42".to_string(),
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_inherit_all() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("FOO", "bar")]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            ignore_default_excludes: true, // keep everything
            ..Default::default()
        };

        let result = populate_env(vars.clone(), &policy);
        let expected: HashMap<String, String> = vars.into_iter().collect();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_inherit_all_with_default_excludes() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("API_KEY", "secret")]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            ignore_default_excludes: false,
            ..Default::default()
        };

        let result = populate_env(vars, &policy);
        let expected: HashMap<String, String> = hashmap! {
            "PATH".to_string() => "/usr/bin".to_string(),
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_inherit_none() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("HOME", "/home")]);

        let mut policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::None,
            ignore_default_excludes: true,
            ..Default::default()
        };
        policy
            .r#set
            .insert("ONLY_VAR".to_string(), "yes".to_string());

        let result = populate_env(vars, &policy);
        let expected: HashMap<String, String> = hashmap! {
            "ONLY_VAR".to_string() => "yes".to_string(),
        };
        assert_eq!(result, expected);
    }
}
