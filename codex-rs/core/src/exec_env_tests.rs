use super::*;
use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use maplit::hashmap;
use pretty_assertions::assert_eq;

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
    let thread_id = ThreadId::new();
    let result = populate_env(vars, &policy, Some(thread_id));

    let mut expected: HashMap<String, String> = hashmap! {
        "PATH".to_string() => "/usr/bin".to_string(),
        "HOME".to_string() => "/home/user".to_string(),
        "API_KEY".to_string() => "secret".to_string(),
        "SECRET_TOKEN".to_string() => "t".to_string(),
    };
    expected.insert(CODEX_THREAD_ID_ENV_VAR.to_string(), thread_id.to_string());

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
    let thread_id = ThreadId::new();
    let result = populate_env(vars, &policy, Some(thread_id));

    let mut expected: HashMap<String, String> = hashmap! {
        "PATH".to_string() => "/usr/bin".to_string(),
        "HOME".to_string() => "/home/user".to_string(),
    };
    expected.insert(CODEX_THREAD_ID_ENV_VAR.to_string(), thread_id.to_string());

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

    let thread_id = ThreadId::new();
    let result = populate_env(vars, &policy, Some(thread_id));

    let mut expected: HashMap<String, String> = hashmap! {
        "PATH".to_string() => "/usr/bin".to_string(),
    };
    expected.insert(CODEX_THREAD_ID_ENV_VAR.to_string(), thread_id.to_string());

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

    let thread_id = ThreadId::new();
    let result = populate_env(vars, &policy, Some(thread_id));

    let mut expected: HashMap<String, String> = hashmap! {
        "PATH".to_string() => "/usr/bin".to_string(),
        "NEW_VAR".to_string() => "42".to_string(),
    };
    expected.insert(CODEX_THREAD_ID_ENV_VAR.to_string(), thread_id.to_string());

    assert_eq!(result, expected);
}

#[test]
fn populate_env_inserts_thread_id() {
    let vars = make_vars(&[("PATH", "/usr/bin")]);
    let policy = ShellEnvironmentPolicy::default();
    let thread_id = ThreadId::new();
    let result = populate_env(vars, &policy, Some(thread_id));

    let mut expected: HashMap<String, String> = hashmap! {
        "PATH".to_string() => "/usr/bin".to_string(),
    };
    expected.insert(CODEX_THREAD_ID_ENV_VAR.to_string(), thread_id.to_string());

    assert_eq!(result, expected);
}

#[test]
fn populate_env_omits_thread_id_when_missing() {
    let vars = make_vars(&[("PATH", "/usr/bin")]);
    let policy = ShellEnvironmentPolicy::default();
    let result = populate_env(vars, &policy, /*thread_id*/ None);

    let expected: HashMap<String, String> = hashmap! {
        "PATH".to_string() => "/usr/bin".to_string(),
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

    let thread_id = ThreadId::new();
    let result = populate_env(vars.clone(), &policy, Some(thread_id));
    let mut expected: HashMap<String, String> = vars.into_iter().collect();
    expected.insert(CODEX_THREAD_ID_ENV_VAR.to_string(), thread_id.to_string());
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

    let thread_id = ThreadId::new();
    let result = populate_env(vars, &policy, Some(thread_id));
    let mut expected: HashMap<String, String> = hashmap! {
        "PATH".to_string() => "/usr/bin".to_string(),
    };
    expected.insert(CODEX_THREAD_ID_ENV_VAR.to_string(), thread_id.to_string());
    assert_eq!(result, expected);
}

#[test]
#[cfg(target_os = "windows")]
fn test_core_inherit_respects_case_insensitive_names_on_windows() {
    let vars = make_vars(&[
        ("Path", "C:\\Windows\\System32"),
        ("PathExt", ".COM;.EXE;.BAT;.CMD"),
        ("TEMP", "C:\\Temp"),
        ("FOO", "bar"),
    ]);

    let policy = ShellEnvironmentPolicy {
        inherit: ShellEnvironmentPolicyInherit::Core,
        ignore_default_excludes: true,
        ..Default::default()
    };

    let thread_id = ThreadId::new();
    let result = populate_env(vars, &policy, Some(thread_id));
    let mut expected: HashMap<String, String> = hashmap! {
        "Path".to_string() => "C:\\Windows\\System32".to_string(),
        "PathExt".to_string() => ".COM;.EXE;.BAT;.CMD".to_string(),
        "TEMP".to_string() => "C:\\Temp".to_string(),
    };
    expected.insert(CODEX_THREAD_ID_ENV_VAR.to_string(), thread_id.to_string());

    assert_eq!(result, expected);
}

#[test]
#[cfg(target_os = "windows")]
fn create_env_inserts_pathext_on_windows_when_missing() {
    let vars = make_vars(&[]);

    let policy = ShellEnvironmentPolicy {
        inherit: ShellEnvironmentPolicyInherit::None,
        ignore_default_excludes: true,
        ..Default::default()
    };

    let result = create_env_from_vars(vars, &policy, /*thread_id*/ None);

    let expected: HashMap<String, String> = hashmap! {
        "PATHEXT".to_string() => ".COM;.EXE;.BAT;.CMD".to_string(),
    };
    assert_eq!(result, expected);
}

#[test]
#[cfg(target_os = "windows")]
fn create_env_preserves_existing_pathext_case_insensitively_on_windows() {
    let vars = make_vars(&[("PathExt", ".COM;.EXE;.BAT;.CMD;.PS1")]);

    let policy = ShellEnvironmentPolicy {
        inherit: ShellEnvironmentPolicyInherit::Core,
        ignore_default_excludes: true,
        ..Default::default()
    };

    let result = create_env_from_vars(vars, &policy, /*thread_id*/ None);

    let pathext_vars = result
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case("PATHEXT"))
        .collect::<Vec<_>>();

    assert_eq!(pathext_vars.len(), 1);
    assert_eq!(pathext_vars[0].1, ".COM;.EXE;.BAT;.CMD;.PS1");
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

    let thread_id = ThreadId::new();
    let result = populate_env(vars, &policy, Some(thread_id));
    let mut expected: HashMap<String, String> = hashmap! {
        "ONLY_VAR".to_string() => "yes".to_string(),
    };
    expected.insert(CODEX_THREAD_ID_ENV_VAR.to_string(), thread_id.to_string());
    assert_eq!(result, expected);
}
