use super::*;
use pretty_assertions::assert_eq;

fn string_map<const N: usize>(entries: [(&str, &str); N]) -> HashMap<String, String> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn override_map<const N: usize>(
    entries: [(&str, Option<&str>); N],
) -> HashMap<String, Option<String>> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.map(std::string::ToString::to_string)))
        .collect()
}

fn apply_legacy_case_sensitive_overrides(
    env: &mut HashMap<String, String>,
    overrides: HashMap<String, Option<String>>,
) {
    for (key, value) in overrides {
        match value {
            Some(value) => {
                env.insert(key, value);
            }
            None => {
                env.remove(&key);
            }
        }
    }
}

#[test]
fn windows_present_override_replaces_every_case_variant() {
    let mut env = string_map([
        ("git_allow_protocol", "ext"),
        ("Git_Allow_Protocol", "file"),
        ("OTHER", "keep"),
    ]);

    apply_environment_overrides_with_semantics(
        &mut env,
        override_map([("GIT_ALLOW_PROTOCOL", Some(""))]),
        EnvironmentNameSemantics::WindowsAsciiCaseInsensitive,
    )
    .expect("Windows override should be unambiguous");

    assert_eq!(
        env,
        string_map([("GIT_ALLOW_PROTOCOL", ""), ("OTHER", "keep")])
    );
}

#[test]
fn windows_unset_removes_every_case_variant() {
    let mut env = string_map([("Path", "first"), ("PATH", "second"), ("OTHER", "keep")]);

    apply_environment_overrides_with_semantics(
        &mut env,
        override_map([("path", None)]),
        EnvironmentNameSemantics::WindowsAsciiCaseInsensitive,
    )
    .expect("Windows unset should be unambiguous");

    assert_eq!(env, string_map([("OTHER", "keep")]));
}

#[test]
fn windows_local_only_overrides_replace_both_permissive_controls() {
    let mut env = string_map([
        ("git_allow_protocol", "ext"),
        ("git_no_lazy_fetch", "0"),
        ("OTHER", "keep"),
    ]);

    apply_environment_overrides_with_semantics(
        &mut env,
        override_map([
            ("GIT_ALLOW_PROTOCOL", Some("")),
            ("GIT_NO_LAZY_FETCH", Some("1")),
        ]),
        EnvironmentNameSemantics::WindowsAsciiCaseInsensitive,
    )
    .expect("local-only overrides should be unambiguous");

    assert_eq!(
        env,
        string_map([
            ("GIT_ALLOW_PROTOCOL", ""),
            ("GIT_NO_LAZY_FETCH", "1"),
            ("OTHER", "keep"),
        ])
    );
}

#[test]
fn windows_rejects_equivalent_request_keys_before_mutation() {
    let initial = string_map([("Path", "old"), ("OTHER", "keep")]);
    let mut env = initial.clone();

    let error = apply_environment_overrides_with_semantics(
        &mut env,
        override_map([("PATH", Some("first")), ("Path", Some("second"))]),
        EnvironmentNameSemantics::WindowsAsciiCaseInsensitive,
    )
    .expect_err("equivalent Windows request keys should be rejected");

    assert_eq!(
        error,
        EnvironmentOverrideError {
            first_key: "PATH".to_string(),
            second_key: "Path".to_string(),
        }
    );
    assert_eq!(env, initial);
}

#[test]
fn case_sensitive_mode_preserves_differently_cased_names() {
    let mut env = string_map([("Path", "old"), ("PATH", "replace"), ("OTHER", "keep")]);

    apply_environment_overrides_with_semantics(
        &mut env,
        override_map([("PATH", None), ("path", Some("new"))]),
        EnvironmentNameSemantics::CaseSensitive,
    )
    .expect("case-sensitive overrides should not conflict");

    assert_eq!(
        env,
        string_map([("Path", "old"), ("path", "new"), ("OTHER", "keep")])
    );
}

#[test]
fn case_sensitive_mode_matches_legacy_merge_behavior() {
    let fixtures = [
        (
            string_map([("Path", "old"), ("PATH", "replace"), ("OTHER", "keep")]),
            override_map([("PATH", None), ("path", Some("new"))]),
        ),
        (
            string_map([("FOO", "old"), ("BAR", "keep"), ("EMPTY", "old")]),
            override_map([
                ("FOO", Some("new")),
                ("ADDED", Some("value")),
                ("EMPTY", None),
            ]),
        ),
    ];

    for (initial, overrides) in fixtures {
        let mut expected = initial.clone();
        apply_legacy_case_sensitive_overrides(&mut expected, overrides.clone());
        let mut actual = initial;

        apply_environment_overrides_with_semantics(
            &mut actual,
            overrides,
            EnvironmentNameSemantics::CaseSensitive,
        )
        .expect("case-sensitive overrides should not conflict");

        assert_eq!(actual, expected);
    }
}

#[test]
fn target_semantics_matches_platform() {
    let expected = if cfg!(windows) {
        EnvironmentNameSemantics::WindowsAsciiCaseInsensitive
    } else {
        EnvironmentNameSemantics::CaseSensitive
    };

    assert_eq!(EnvironmentNameSemantics::for_target(), expected);
}
